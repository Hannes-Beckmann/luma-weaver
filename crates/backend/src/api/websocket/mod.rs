/// WebSocket request handling, outbound message helpers, and route dispatch for the backend API.
pub(crate) mod handler;
/// Outbound WebSocket serialization and specialized runtime-update transport helpers.
pub(crate) mod outbound;
/// Client-message parsing and domain-specific WebSocket routing.
pub(crate) mod routing;

/// Entry point used by the HTTP layer to upgrade and serve WebSocket connections.
pub(crate) use handler::websocket_handler;
