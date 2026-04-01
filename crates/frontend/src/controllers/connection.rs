use eframe::egui;
#[cfg(target_arch = "wasm32")]
use tracing::warn;

use crate::app::FrontendApp;

impl FrontendApp {
    #[cfg(target_arch = "wasm32")]
    /// Opens or retries the frontend WebSocket connection when no live transport is present.
    ///
    /// Reconnect attempts are delayed by the backoff stored in `ConnectionState`.
    pub(crate) fn maintain_connection(&mut self, ctx: &egui::Context) {
        if self.connection.sender.is_some() || self.connection.incoming.is_some() {
            return;
        }

        let now_secs = Self::now_secs(ctx);
        if now_secs < self.connection.next_reconnect_at_secs {
            return;
        }

        let (sender, incoming, events, ws_status) = crate::websocket_client::connect_websocket(ctx);

        if sender.is_some() && incoming.is_some() {
            self.handle_connected(
                ws_status,
                sender.expect("sender exists when connection succeeds"),
                incoming.expect("incoming exists when connection succeeds"),
                events.expect("events exists when connection succeeds"),
            );
        } else {
            self.handle_disconnected(now_secs, "WebSocket disconnected, retrying".to_owned());
            self.ui.status = "WebSocket disconnected, retrying".to_owned();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// No-op connection maintenance for non-wasm builds.
    pub(crate) fn maintain_connection(&mut self, _ctx: &egui::Context) {}

    #[cfg(target_arch = "wasm32")]
    /// Drains transport-level WebSocket events such as disconnect notifications.
    pub(crate) fn drain_websocket_events(&mut self, ctx: &egui::Context) {
        let Some(events) = &mut self.connection.events else {
            return;
        };

        let mut disconnected = None;
        while let Ok(event) = events.try_recv() {
            match event {
                crate::websocket_client::WebSocketEvent::Disconnected { reason } => {
                    disconnected = Some(reason);
                }
            }
        }

        if let Some(reason) = disconnected {
            warn!(%reason, "frontend websocket disconnected");
            self.handle_connection_loss(ctx, format!("Disconnected: {reason}"));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// No-op WebSocket event draining for non-wasm builds.
    pub(crate) fn drain_websocket_events(&mut self, _ctx: &egui::Context) {}

    /// Applies a transport-level connection loss using the current frontend time for backoff.
    pub(crate) fn handle_connection_loss(&mut self, ctx: &egui::Context, ws_status: String) {
        self.handle_disconnected(Self::now_secs(ctx), ws_status);
    }
}
