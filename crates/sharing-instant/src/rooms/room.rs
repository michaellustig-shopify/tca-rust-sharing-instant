use crate::error::{Result, SharingInstantError};
use crate::mutation_callbacks::MutationCallbacks;
use crate::operation_state::OperationState;
use crate::rooms::presence::{PresenceData, PresenceState};
use instant_client::Reactor;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

/// A real-time room with typed presence.
///
/// Wraps the Reactor's room primitives (join/leave/set_presence) with
/// type-safe presence data. Automatically leaves the room on Drop (RAII).
///
/// # Example
///
/// ```ignore
/// #[derive(Clone, Serialize, Deserialize)]
/// struct Cursor { x: f64, y: f64 }
///
/// let room = engine.room::<Cursor>("editor", "doc-123")?;
/// room.set_presence(&Cursor { x: 100.0, y: 200.0 })?;
///
/// for (peer_id, cursor) in &room.presence().peers {
///     println!("{peer_id}: ({}, {})", cursor.x, cursor.y);
/// }
/// ```
pub struct Room<P: PresenceData = serde_json::Value> {
    reactor: Arc<Reactor>,
    handle: tokio::runtime::Handle,
    room_type: String,
    room_id: String,
    state: Arc<RwLock<PresenceState<P>>>,
    state_sender: watch::Sender<PresenceState<P>>,
    state_receiver: watch::Receiver<PresenceState<P>>,
    joined: Arc<RwLock<bool>>,
    presence_op_sender: watch::Sender<OperationState<()>>,
    presence_op_receiver: watch::Receiver<OperationState<()>>,
}

impl<P: PresenceData> Room<P> {
    /// Join a room and start receiving presence updates.
    pub fn join(
        reactor: Arc<Reactor>,
        handle: tokio::runtime::Handle,
        room_type: impl Into<String>,
        room_id: impl Into<String>,
    ) -> Result<Self> {
        let room_type = room_type.into();
        let room_id = room_id.into();
        let initial_state = PresenceState::default();
        let (state_sender, state_receiver) = watch::channel(initial_state.clone());
        let state = Arc::new(RwLock::new(initial_state));
        let joined = Arc::new(RwLock::new(false));
        let (presence_op_sender, presence_op_receiver) = watch::channel(OperationState::Idle);

        let room = Self {
            reactor: reactor.clone(),
            handle: handle.clone(),
            room_type: room_type.clone(),
            room_id: room_id.clone(),
            state,
            state_sender,
            state_receiver,
            joined,
            presence_op_sender,
            presence_op_receiver,
        };

        // Join the room via reactor and spawn the presence listener.
        let rt = room_type.clone();
        let ri = room_id.clone();
        let state_ref = room.state.clone();
        let sender_ref = room.state_sender.clone();
        let joined_ref = room.joined.clone();

        handle.spawn(async move {
            // join_room returns watch::Receiver<Option<serde_json::Value>> directly.
            let mut rx = reactor.join_room(&rt, &ri).await;
            *joined_ref.write() = true;

            while rx.changed().await.is_ok() {
                let raw = rx.borrow().clone();
                let mut new_state = PresenceState::<P> {
                    user: None,
                    peers: std::collections::HashMap::new(),
                    is_loading: false,
                    error: None,
                };

                // Parse raw presence: Option<{ peer_id: { data } }>
                if let Some(val) = raw {
                    if let Some(obj) = val.as_object() {
                        for (peer_id, data) in obj {
                            if let Ok(parsed) = serde_json::from_value::<P>(data.clone()) {
                                new_state.peers.insert(peer_id.clone(), parsed);
                            }
                        }
                    }
                }

                *state_ref.write() = new_state.clone();
                if sender_ref.send(new_state).is_err() {
                    break;
                }
            }
        });

        Ok(room)
    }

    /// Publish this client's presence data.
    ///
    /// Note: `Reactor::set_presence()` returns `()` (no error path today).
    /// OperationState will always transition to Success. If the reactor adds
    /// error returns later, the Failure path will be wired automatically.
    pub fn set_presence(&self, data: &P) -> Result<()> {
        let json = serde_json::to_value(data)
            .map_err(|e| SharingInstantError::SerializationError(e.to_string()))?;

        // Update local state immediately (optimistic).
        {
            let mut state = self.state.write();
            state.user = Some(data.clone());
            let _ = self.state_sender.send(state.clone());
        }

        let _ = self.presence_op_sender.send(OperationState::in_flight());

        let reactor = self.reactor.clone();
        let rt = self.room_type.clone();
        let ri = self.room_id.clone();
        let op_tx = self.presence_op_sender.clone();

        self.handle.spawn(async move {
            reactor.set_presence(&rt, &ri, json).await;
            let _ = op_tx.send(OperationState::success(()));
        });

        Ok(())
    }

    /// Publish this client's presence data with mutation callbacks.
    ///
    /// Fires `on_mutate` immediately, then `on_success` after the reactor
    /// call completes. (`on_error` is reserved for when the reactor gains
    /// an error return path.)
    pub fn set_presence_with_callbacks(
        &self,
        data: &P,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        let json = serde_json::to_value(data)
            .map_err(|e| SharingInstantError::SerializationError(e.to_string()))?;

        // Update local state immediately (optimistic).
        {
            let mut state = self.state.write();
            state.user = Some(data.clone());
            let _ = self.state_sender.send(state.clone());
        }

        let _ = self.presence_op_sender.send(OperationState::in_flight());

        let reactor = self.reactor.clone();
        let rt = self.room_type.clone();
        let ri = self.room_id.clone();
        let op_tx = self.presence_op_sender.clone();

        self.handle.spawn(async move {
            if let Some(f) = callbacks.on_mutate {
                f();
            }
            reactor.set_presence(&rt, &ri, json).await;
            let _ = op_tx.send(OperationState::success(()));
            if let Some(f) = callbacks.on_success {
                f(());
            }
            if let Some(f) = callbacks.on_settled {
                f();
            }
        });

        Ok(())
    }

    /// Get a reactive watch receiver for the presence operation state.
    pub fn presence_operation_state(&self) -> watch::Receiver<OperationState<()>> {
        self.presence_op_receiver.clone()
    }

    /// Get the current presence state (snapshot).
    pub fn presence(&self) -> PresenceState<P> {
        self.state.read().clone()
    }

    /// Get a reactive watch receiver for presence changes.
    pub fn watch_presence(&self) -> watch::Receiver<PresenceState<P>> {
        self.state_receiver.clone()
    }

    /// The room type (e.g., "editor", "chat").
    pub fn room_type(&self) -> &str {
        &self.room_type
    }

    /// The room ID (e.g., "doc-123").
    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    /// Whether this room has been successfully joined.
    pub fn is_joined(&self) -> bool {
        *self.joined.read()
    }

    /// Explicitly leave the room (also happens on Drop).
    pub fn leave(&self) {
        if *self.joined.read() {
            let reactor = self.reactor.clone();
            let rt = self.room_type.clone();
            let ri = self.room_id.clone();
            let joined = self.joined.clone();

            self.handle.spawn(async move {
                reactor.leave_room(&rt, &ri).await;
                *joined.write() = false;
            });
        }
    }
}

impl<P: PresenceData> Drop for Room<P> {
    fn drop(&mut self) {
        self.leave();
    }
}
