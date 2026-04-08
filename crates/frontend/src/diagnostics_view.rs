use eframe::egui;
use eframe::egui::{Color32, RichText};
use shared::NodeDiagnosticSeverity;

use crate::app::FrontendApp;

/// Renders the shared node-diagnostics detail window for the currently targeted graph/node.
pub(crate) fn render_node_diagnostics_window(ctx: &egui::Context, app: &mut FrontendApp) {
    let Some(graph_id) = app.ui.diagnostics_window_graph_id.clone() else {
        return;
    };
    let Some(node_id) = app.ui.diagnostics_window_node_id.clone() else {
        return;
    };

    let mut open = true;
    let diagnostics = app.node_diagnostic_details(&graph_id, &node_id).to_vec();
    let node_name = app
        .graphs
        .loaded_graph_document
        .as_ref()
        .filter(|document| document.metadata.id == graph_id)
        .and_then(|document| {
            document
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .map(|node| node.metadata.name.clone())
        })
        .or_else(|| {
            app.graphs
                .graph_documents
                .iter()
                .find(|graph| graph.id == graph_id)
                .map(|graph| format!("{} ({node_id})", graph.name))
        })
        .unwrap_or_else(|| node_id.clone());

    egui::Window::new(format!("Diagnostics: {node_name}"))
        .open(&mut open)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    app.clear_node_diagnostics(graph_id.clone(), node_id.clone());
                }
            });
            ui.separator();
            if diagnostics.is_empty() {
                ui.label("No diagnostics reported.");
            } else {
                for diagnostic in diagnostics {
                    ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!("{:?}", diagnostic.severity))
                                    .color(severity_color(diagnostic.severity))
                                    .strong(),
                            );
                            if let Some(code) = &diagnostic.code {
                                ui.label(
                                    RichText::new(code)
                                        .monospace()
                                        .color(Color32::from_gray(170)),
                                );
                            }
                            ui.label(format!("x{}", diagnostic.occurrences));
                        });
                        ui.label(diagnostic.message);
                    });
                }
            }
        });

    if !open {
        app.close_node_diagnostics();
    }
}

/// Returns the UI color associated with one diagnostic severity level.
pub(crate) fn severity_color(severity: NodeDiagnosticSeverity) -> Color32 {
    match severity {
        NodeDiagnosticSeverity::Info => Color32::from_rgb(90, 150, 220),
        NodeDiagnosticSeverity::Warning => Color32::from_rgb(210, 160, 60),
        NodeDiagnosticSeverity::Error => Color32::from_rgb(190, 80, 80),
    }
}
