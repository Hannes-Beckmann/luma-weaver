/// Autosave scheduling and graph-document canonicalization.
pub(crate) mod autosave;
/// WebSocket connection management and reconnect behavior.
pub(crate) mod connection;
/// Server-message handling and application of backend state to the frontend model.
pub(crate) mod messages;
/// Event-subscription and runtime-stream reconciliation.
pub(crate) mod subscriptions;
