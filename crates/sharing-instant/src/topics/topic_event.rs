use std::time::Instant;

/// A single event received from a topic channel peer.
///
/// Contains the typed payload, the sender's peer ID, and a timestamp.
///
/// # Example
///
/// ```
/// use sharing_instant::TopicEvent;
/// use std::time::Instant;
///
/// let event = TopicEvent {
///     peer_id: "peer-123".to_string(),
///     data: "hello".to_string(),
///     received_at: Instant::now(),
/// };
/// assert_eq!(event.peer_id, "peer-123");
/// ```
#[derive(Debug, Clone)]
pub struct TopicEvent<T> {
    /// The peer session ID that sent this event.
    pub peer_id: String,
    /// The typed event payload.
    pub data: T,
    /// When this event was received by this client.
    pub received_at: Instant,
}
