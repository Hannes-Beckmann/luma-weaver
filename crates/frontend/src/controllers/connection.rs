use eframe::egui;
use tracing::warn;

use crate::app::FrontendApp;

impl FrontendApp {
    pub(crate) fn maintain_connection(&mut self, ctx: &egui::Context) {
        if self.connection.is_connected() {
            return;
        }

        let now_secs = Self::now_secs(ctx);
        if now_secs < self.connection.next_reconnect_at_secs {
            return;
        }

        match crate::transport::FrontendTransport::connect(ctx) {
            Ok((transport, ws_status)) => self.handle_connected(ws_status, transport, ctx.clone()),
            Err(ws_status) => {
                self.handle_disconnected(now_secs, ws_status);
                self.ui.status = "WebSocket disconnected, retrying".to_owned();
            }
        }
    }

    /// Pumps the active transport for disconnect signals and demo-runtime ticks.
    pub(crate) fn pump_connection(&mut self, ctx: &egui::Context) {
        let Some(transport) = self.connection.transport.as_mut() else {
            return;
        };

        if let Some(reason) = transport.poll(Self::now_secs(ctx)) {
            warn!(%reason, "frontend websocket disconnected");
            self.handle_connection_loss(ctx, format!("Disconnected: {reason}"));
        }
    }

    /// Applies a transport-level connection loss using the current frontend time for backoff.
    pub(crate) fn handle_connection_loss(&mut self, ctx: &egui::Context, ws_status: String) {
        self.handle_disconnected(Self::now_secs(ctx), ws_status);
    }
}
