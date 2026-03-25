use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;

/// Marker trait for types that can be used as presence data.
///
/// Blanket-implemented for all types that are serializable, cloneable,
/// and thread-safe. Developers just derive the usual traits.
///
/// # Example
///
/// ```
/// use sharing_instant::PresenceData;
///
/// #[derive(Clone, serde::Serialize, serde::Deserialize)]
/// struct Cursor { x: f64, y: f64 }
///
/// // Cursor automatically implements PresenceData
/// fn takes_presence<P: PresenceData>(_data: &P) {}
/// takes_presence(&Cursor { x: 0.0, y: 0.0 });
/// ```
pub trait PresenceData: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {}

// Blanket implementation: any type meeting the bounds is PresenceData.
impl<T> PresenceData for T where T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {}

/// Current presence state for a room.
///
/// Contains this client's presence data plus all peer presence data,
/// keyed by peer session ID.
///
/// # Example
///
/// ```
/// use sharing_instant::PresenceState;
///
/// let state = PresenceState::<serde_json::Value>::default();
/// assert!(state.user.is_none());
/// assert!(state.peers.is_empty());
/// assert!(state.is_loading);
/// ```
#[derive(Debug, Clone)]
pub struct PresenceState<P: PresenceData> {
    /// This client's current presence data, if set.
    pub user: Option<P>,
    /// Presence data from all other peers in the room, keyed by peer ID.
    pub peers: HashMap<String, P>,
    /// Whether the initial presence sync is still loading.
    pub is_loading: bool,
    /// The most recent error, if any.
    pub error: Option<String>,
}

impl<P: PresenceData> Default for PresenceState<P> {
    fn default() -> Self {
        Self {
            user: None,
            peers: HashMap::new(),
            is_loading: true,
            error: None,
        }
    }
}

impl<P: PresenceData> PresenceState<P> {
    /// Total number of peers (excluding this client).
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Whether any peers are present (excluding this client).
    pub fn has_peers(&self) -> bool {
        !self.peers.is_empty()
    }

    /// All peer IDs in the room.
    pub fn peer_ids(&self) -> Vec<&String> {
        self.peers.keys().collect()
    }
}
