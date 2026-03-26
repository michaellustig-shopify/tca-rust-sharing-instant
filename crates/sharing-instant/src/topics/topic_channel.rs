use crate::error::{Result, SharingInstantError};
use crate::mutation_callbacks::MutationCallbacks;
use crate::operation_state::OperationState;
use crate::topics::topic_event::TopicEvent;
use instant_client::Reactor;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;

/// Maximum number of recent events to keep in the ring buffer.
const MAX_EVENTS: usize = 50;

/// Handle returned by `publish()` / `publish_with_callbacks()` that tracks
/// the in-flight state of a broadcast operation.
pub struct PublishHandle {
    state_receiver: watch::Receiver<OperationState<()>>,
}

impl PublishHandle {
    /// Get the current operation state (snapshot).
    pub fn state(&self) -> OperationState<()> {
        self.state_receiver.borrow().clone()
    }

    /// Get a reactive watch receiver for state changes.
    pub fn watch(&self) -> watch::Receiver<OperationState<()>> {
        self.state_receiver.clone()
    }

    /// Whether the broadcast is currently in flight.
    pub fn is_loading(&self) -> bool {
        self.state_receiver.borrow().is_loading()
    }

    /// Whether the broadcast succeeded.
    pub fn is_success(&self) -> bool {
        self.state_receiver.borrow().is_success()
    }

    /// The error message, if the broadcast failed.
    pub fn error(&self) -> Option<String> {
        self.state_receiver.borrow().error().map(|s| s.to_string())
    }
}

/// A typed, fire-and-forget topic channel for real-time peer events.
///
/// Wraps the Reactor's topic primitives (subscribe_topic/broadcast) with
/// type-safe event payloads. Maintains a ring buffer of recent events.
///
/// # Example
///
/// ```ignore
/// #[derive(Clone, Serialize, Deserialize)]
/// struct Emoji { name: String, angle: f64 }
///
/// let emojis = engine.topic::<Emoji>("game", "room-1", "emoji")?;
/// emojis.publish(&Emoji { name: "fire".into(), angle: 45.0 })?;
///
/// for event in emojis.events() {
///     println!("{} sent {}", event.peer_id, event.data.name);
/// }
/// ```
pub struct TopicChannel<T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> {
    reactor: Arc<Reactor>,
    handle: tokio::runtime::Handle,
    room_type: String,
    room_id: String,
    topic: String,
    events: Arc<RwLock<VecDeque<TopicEvent<T>>>>,
    events_sender: watch::Sender<Vec<TopicEvent<T>>>,
    events_receiver: watch::Receiver<Vec<TopicEvent<T>>>,
}

impl<T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> TopicChannel<T> {
    /// Subscribe to a topic and start receiving peer events.
    pub fn subscribe(
        reactor: Arc<Reactor>,
        handle: tokio::runtime::Handle,
        room_type: impl Into<String>,
        room_id: impl Into<String>,
        topic: impl Into<String>,
    ) -> Result<Self> {
        let room_type = room_type.into();
        let room_id = room_id.into();
        let topic = topic.into();
        let events = Arc::new(RwLock::new(VecDeque::with_capacity(MAX_EVENTS)));
        let (events_sender, events_receiver) = watch::channel(Vec::new());

        let channel = Self {
            reactor: reactor.clone(),
            handle: handle.clone(),
            room_type: room_type.clone(),
            room_id: room_id.clone(),
            topic: topic.clone(),
            events,
            events_sender,
            events_receiver,
        };

        // Spawn the listener task.
        let events_ref = channel.events.clone();
        let sender_ref = channel.events_sender.clone();

        handle.spawn(async move {
            // The JS client calls joinRoom() inside subscribeTopic().
            // The server requires a join-room before it will route broadcasts.
            let _presence_rx = reactor.join_room(&room_type, &room_id).await;

            // subscribe_topic returns mpsc::UnboundedReceiver<Value> directly.
            let mut rx = reactor.subscribe_topic(&room_type, &room_id, &topic).await;

            while let Some(msg) = rx.recv().await {
                // msg is a serde_json::Value — could be the raw data or { peer_id, data }.
                let peer_id = msg
                    .get("peer_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let data_val = msg.get("data").cloned().unwrap_or(msg.clone());

                if let Ok(data) = serde_json::from_value::<T>(data_val) {
                    let event = TopicEvent {
                        peer_id,
                        data,
                        received_at: Instant::now(),
                    };

                    let mut buf = events_ref.write();
                    if buf.len() >= MAX_EVENTS {
                        buf.pop_front();
                    }
                    buf.push_back(event);

                    let snapshot: Vec<TopicEvent<T>> = buf.iter().cloned().collect();
                    if sender_ref.send(snapshot).is_err() {
                        break;
                    }
                }
            }
        });

        Ok(channel)
    }

    /// Broadcast an event to all peers on this topic.
    ///
    /// Returns a `PublishHandle` that tracks the in-flight state of the broadcast.
    pub fn publish(&self, data: &T) -> Result<PublishHandle> {
        let json = serde_json::to_value(data)
            .map_err(|e| SharingInstantError::SerializationError(e.to_string()))?;

        let (state_tx, state_rx) = watch::channel(OperationState::in_flight());

        let reactor = self.reactor.clone();
        let rt = self.room_type.clone();
        let ri = self.room_id.clone();
        let t = self.topic.clone();

        self.handle.spawn(async move {
            match reactor.broadcast(&rt, &ri, &t, json).await {
                Ok(()) => {
                    let _ = state_tx.send(OperationState::success(()));
                }
                Err(e) => {
                    let _ = state_tx.send(OperationState::failure(e.to_string()));
                }
            }
        });

        Ok(PublishHandle {
            state_receiver: state_rx,
        })
    }

    /// Broadcast with mutation callbacks.
    ///
    /// Returns a `PublishHandle` that tracks the in-flight state of the broadcast.
    pub fn publish_with_callbacks(
        &self,
        data: &T,
        callbacks: MutationCallbacks<()>,
    ) -> Result<PublishHandle> {
        let json = serde_json::to_value(data)
            .map_err(|e| SharingInstantError::SerializationError(e.to_string()))?;

        let (state_tx, state_rx) = watch::channel(OperationState::in_flight());

        let reactor = self.reactor.clone();
        let rt = self.room_type.clone();
        let ri = self.room_id.clone();
        let t = self.topic.clone();

        self.handle.spawn(async move {
            if let Some(f) = callbacks.on_mutate {
                f();
            }
            match reactor.broadcast(&rt, &ri, &t, json).await {
                Ok(()) => {
                    let _ = state_tx.send(OperationState::success(()));
                    if let Some(f) = callbacks.on_success {
                        f(());
                    }
                }
                Err(e) => {
                    let _ = state_tx.send(OperationState::failure(e.to_string()));
                    if let Some(f) = callbacks.on_error {
                        f(SharingInstantError::TopicError(e.to_string()));
                    }
                }
            }
            if let Some(f) = callbacks.on_settled {
                f();
            }
        });

        Ok(PublishHandle {
            state_receiver: state_rx,
        })
    }

    /// Get a snapshot of recent events (up to 50).
    pub fn events(&self) -> Vec<TopicEvent<T>> {
        self.events.read().iter().cloned().collect()
    }

    /// Get the most recent event, if any.
    pub fn latest_event(&self) -> Option<TopicEvent<T>> {
        self.events.read().back().cloned()
    }

    /// Get a reactive watch receiver for events.
    pub fn watch(&self) -> watch::Receiver<Vec<TopicEvent<T>>> {
        self.events_receiver.clone()
    }

    /// The topic name.
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// The room type.
    pub fn room_type(&self) -> &str {
        &self.room_type
    }

    /// The room ID.
    pub fn room_id(&self) -> &str {
        &self.room_id
    }
}
