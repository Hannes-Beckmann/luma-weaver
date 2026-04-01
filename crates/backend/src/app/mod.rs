/// Backend application-state construction and shared process-wide state.
pub(crate) mod startup;
/// Shared backend state passed into HTTP and WebSocket handlers.
pub(crate) mod state;
