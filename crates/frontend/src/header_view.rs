use eframe::egui;
use eframe::egui::{Align, Color32, Layout, RichText};

use crate::app::FrontendApp;

/// Renders the top application header shared by the dashboard and graph editor.
pub(crate) fn render(ctx: &egui::Context, app: &FrontendApp) {
    egui::TopBottomPanel::top("app_header").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading("Luma Weaver");
                let context_line = match app.current_graph_name() {
                    Some(graph_name) => format!("Editing {graph_name}"),
                    None => "Graph dashboard".to_owned(),
                };
                ui.label(RichText::new(context_line).color(Color32::from_gray(170)));
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (connection_fill, connection_label) = match app.websocket_status_label() {
                    "Connected" => (Color32::from_rgb(62, 140, 96), "Connected"),
                    "Demo" => (Color32::from_rgb(66, 120, 185), "Demo"),
                    "Connecting" => (Color32::from_rgb(196, 147, 60), "Connecting"),
                    _ => (Color32::from_rgb(148, 78, 63), "Offline"),
                };
                status_badge(ui, connection_label, connection_fill);
                ui.add_space(6.0);
                if app.is_demo_mode() {
                    status_badge(ui, "Local Runtime", Color32::from_rgb(66, 120, 185));
                    ui.add_space(6.0);
                }
                status_badge(
                    ui,
                    app.graph_save_status_label(),
                    match app.graph_save_status_label() {
                        "All changes saved" => Color32::from_rgb(62, 140, 96),
                        "Saving" => Color32::from_rgb(196, 147, 60),
                        "Unsaved changes" => Color32::from_rgb(196, 147, 60),
                        _ => Color32::from_gray(90),
                    },
                );
            });
        });
        ui.add_space(2.0);
    });
}

/// Renders a colored status badge used for save and connection state in the header.
fn status_badge(ui: &mut egui::Ui, label: &str, fill: Color32) {
    egui::Frame::new()
        .fill(fill.gamma_multiply(0.22))
        .stroke(egui::Stroke::new(1.0, fill))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(label).color(fill).strong());
        });
}
