use eframe::egui;
use eframe::egui::{Color32, RichText};
use shared::{GraphRuntimeMode, InputValue, NodeDiagnosticSeverity, NodeParameter, ValueKind};

use crate::app::FrontendApp;

/// Converts persisted graph documents into the editor's node-canvas representation.
mod model;
/// Owns the snarl canvas widget and node-graph interaction behavior.
mod viewer;
/// Renders parameter editors and runtime preview widgets used inside editor nodes.
mod widgets;

#[derive(Clone)]
/// Editor-side representation of a single input port on a rendered node card.
struct EditorInputPort {
    name: String,
    display_name: String,
    value_kind: ValueKind,
    value: InputValue,
}

#[derive(Clone)]
/// Editor-side representation of a single output port on a rendered node card.
struct EditorOutputPort {
    name: String,
    display_name: String,
    value_kind: ValueKind,
    runtime_value: Option<InputValue>,
}

#[derive(Clone)]
/// Editor-side representation of a graph node as shown on the snarl canvas.
struct EditorSnarlNode {
    graph_node_id: String,
    title: String,
    node_type_id: String,
    inputs: Vec<EditorInputPort>,
    outputs: Vec<EditorOutputPort>,
    parameters: Vec<NodeParameter>,
    runtime_values: Vec<(String, InputValue)>,
}

/// Renders the graph editor view, including header controls, the node canvas, and diagnostics UI.
pub(crate) fn render(ui: &mut egui::Ui, app: &mut FrontendApp) {
    app.ui.editor_canvas_hovered = false;
    let selected_graph = app.selected_graph().cloned();
    match selected_graph {
        Some(graph) => {
            let runtime_mode = app.graph_runtime_mode(&graph.id);
            let mut focus_clicked = false;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if app.ui.rename_graph_id.as_deref() != Some(graph.id.as_str())
                            || app.ui.rename_graph_name.is_empty()
                        {
                            app.ui.rename_graph_id = Some(graph.id.clone());
                            app.ui.rename_graph_name = graph.name.clone();
                        }
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&graph.name).strong().size(24.0));
                            if ui.add(secondary_action_button("Edit")).clicked() {
                                app.ui.rename_graph_dialog_open = true;
                                app.ui.rename_graph_id = Some(graph.id.clone());
                                app.ui.rename_graph_name = graph.name.clone();
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Runtime").color(Color32::from_gray(150)));
                            ui.label(runtime_status_text(runtime_mode));
                            if let Some(document) = app.active_graph_document_mut() {
                                ui.separator();
                                ui.label(RichText::new("Tick rate").color(Color32::from_gray(150)));
                                ui.add(
                                    egui::DragValue::new(
                                        &mut document.metadata.execution_frequency_hz,
                                    )
                                    .speed(1.0)
                                    .range(1..=1_000),
                                );
                                ui.label(RichText::new("Hz").color(Color32::from_gray(150)));
                            }
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if runtime_action_button(
                            ui,
                            "Stop",
                            runtime_mode.is_some(),
                            Color32::from_rgb(190, 92, 92),
                        )
                        .clicked()
                        {
                            app.stop_graph(graph.id.clone());
                        }
                        if runtime_action_button(
                            ui,
                            "Step",
                            runtime_mode != Some(GraphRuntimeMode::Running),
                            Color32::from_rgb(196, 147, 60),
                        )
                        .clicked()
                        {
                            app.step_graph(graph.id.clone(), app.ui.step_tick_count.max(1));
                        }
                        if runtime_action_button(
                            ui,
                            "Pause",
                            runtime_mode == Some(GraphRuntimeMode::Running),
                            Color32::from_rgb(196, 147, 60),
                        )
                        .clicked()
                        {
                            app.pause_graph(graph.id.clone());
                        }
                        if runtime_action_button(
                            ui,
                            "Run",
                            runtime_mode != Some(GraphRuntimeMode::Running),
                            Color32::from_rgb(62, 140, 96),
                        )
                        .clicked()
                        {
                            app.start_graph(graph.id.clone());
                        }

                        ui.add(
                            egui::DragValue::new(&mut app.ui.step_tick_count)
                                .speed(1.0)
                                .range(1..=10_000),
                        );
                        ui.label(RichText::new("Step ticks").color(Color32::from_gray(150)));

                        let undo_response = ui.add_enabled(
                            app.can_undo_graph_edit(),
                            secondary_action_button("Undo"),
                        );
                        if undo_response.clicked() {
                            app.undo_graph_edit();
                        }

                        let redo_response = ui.add_enabled(
                            app.can_redo_graph_edit(),
                            secondary_action_button("Redo"),
                        );
                        if redo_response.clicked() {
                            app.redo_graph_edit();
                        }

                        if ui.add(secondary_action_button("Export")).clicked() {
                            app.request_graph_export(graph.id.clone());
                        }

                        if ui.add(secondary_action_button("Focus")).clicked() {
                            focus_clicked = true;
                        }
                        if ui.add(secondary_action_button("Reload")).clicked() {
                            app.reload_selected_graph();
                        }
                        if ui.add(secondary_action_button("Back")).clicked() {
                            app.return_to_dashboard();
                        }
                    });
                });
            });
            ui.add_space(8.0);

            app.ensure_selected_graph_document_requested();

            let apply_document_viewport = app.graphs.snarl_viewport_initialized_graph_id.as_deref()
                != Some(graph.id.as_str());
            let mut initialized_viewport_for_graph = false;
            let runtime_node_values = app.graphs.runtime_node_values.clone();
            let plot_history = app.graphs.plot_history.clone();
            let wled_instances = app.graphs.wled_instances.clone();
            let mqtt_broker_configs = app.graphs.mqtt_broker_configs.clone();
            let diagnostic_summaries = app.graphs.node_diagnostic_summaries.clone();
            let available_node_definitions = app.graphs.available_node_definitions.clone();
            let node_menu_search = app.ui.node_menu_search.clone();
            let node_menu_graph_position = app
                .ui
                .node_menu_graph_position
                .map(|(x, y)| egui::pos2(x, y));
            if let Some(document) = app.active_graph_document_mut() {
                if available_node_definitions.is_empty() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_min_height(100.0);
                        ui.vertical_centered(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new("Loading node definitions...").strong());
                            ui.label("Waiting for the backend node registry.");
                        });
                    });
                    return;
                }
                let (
                    initialized,
                    opened_diagnostics_node_id,
                    canvas_hovered,
                    node_menu_search,
                    node_menu_graph_position,
                ) = viewer::show_snarl_canvas(
                    ui,
                    document,
                    &available_node_definitions,
                    &runtime_node_values,
                    &plot_history,
                    &diagnostic_summaries,
                    &wled_instances,
                    &mqtt_broker_configs,
                    &node_menu_search,
                    node_menu_graph_position,
                    apply_document_viewport,
                    focus_clicked,
                );
                initialized_viewport_for_graph = initialized;
                app.ui.editor_canvas_hovered = canvas_hovered;
                app.ui.node_menu_search = node_menu_search;
                app.ui.node_menu_graph_position =
                    node_menu_graph_position.map(|pos| (pos.x, pos.y));
                if let Some(node_id) = opened_diagnostics_node_id {
                    app.open_node_diagnostics(node_id);
                }
            } else {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_height(100.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new("Loading graph document...").strong());
                        ui.label("Waiting for the backend to send the latest saved version.");
                    });
                });
            }
            if initialized_viewport_for_graph {
                app.graphs.snarl_viewport_initialized_graph_id = Some(graph.id.clone());
            }

            if let Some(node_id) = app.ui.diagnostics_window_node_id.clone() {
                let mut open = true;
                let graph_id = graph.id.clone();
                let diagnostics = app
                    .graphs
                    .node_diagnostic_details
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_default();
                let node_name = app
                    .graphs
                    .loaded_graph_document
                    .as_ref()
                    .and_then(|document| {
                        document
                            .nodes
                            .iter()
                            .find(|node| node.id == node_id)
                            .map(|node| node.metadata.name.clone())
                    })
                    .unwrap_or_else(|| node_id.clone());
                egui::Window::new(format!("Diagnostics: {node_name}"))
                    .open(&mut open)
                    .resizable(true)
                    .show(ui.ctx(), |ui| {
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
                                let color = match diagnostic.severity {
                                    NodeDiagnosticSeverity::Info => Color32::from_rgb(90, 150, 220),
                                    NodeDiagnosticSeverity::Warning => {
                                        Color32::from_rgb(210, 160, 60)
                                    }
                                    NodeDiagnosticSeverity::Error => Color32::from_rgb(190, 80, 80),
                                };
                                ui.group(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:?}", diagnostic.severity))
                                                .color(color)
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

            if app.ui.rename_graph_dialog_open {
                let mut open = app.ui.rename_graph_dialog_open;
                egui::Window::new("Rename Graph Document")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ui.ctx(), |ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut app.ui.rename_graph_name);

                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                let Some(graph_id) = app.ui.rename_graph_id.clone() else {
                                    return;
                                };
                                let name = app.ui.rename_graph_name.trim().to_owned();
                                if name.is_empty() {
                                    app.ui.status =
                                        "Graph document name must not be empty".to_owned();
                                } else {
                                    app.update_graph_name(graph_id, name);
                                    app.ui.rename_graph_dialog_open = false;
                                }
                            }

                            if ui.button("Close").clicked() {
                                app.ui.rename_graph_dialog_open = false;
                            }
                        });
                    });
                app.ui.rename_graph_dialog_open = open && app.ui.rename_graph_dialog_open;
                if !app.ui.rename_graph_dialog_open {
                    app.ui.rename_graph_id = Some(graph.id.clone());
                    app.ui.rename_graph_name = graph.name.clone();
                }
            }
        }
        None => {
            ui.label("Selected graph no longer exists.");
            if ui.button("Return to Dashboard").clicked() {
                app.return_to_dashboard();
            }
        }
    }
}

/// Renders one of the colored runtime control buttons used in the editor header.
fn runtime_action_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    color: Color32,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).strong())
            .fill(color.gamma_multiply(0.9))
            .stroke(egui::Stroke::new(1.0, color))
            .min_size(egui::vec2(60.0, 28.0)),
    )
}

/// Builds a neutral secondary action button for editor header actions.
fn secondary_action_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).color(Color32::from_gray(215)))
        .fill(Color32::from_gray(44))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(78)))
        .min_size(egui::vec2(58.0, 28.0))
}

/// Returns the styled runtime-status label shown beside the graph title.
fn runtime_status_text(runtime_mode: Option<GraphRuntimeMode>) -> RichText {
    match runtime_mode {
        Some(GraphRuntimeMode::Running) => RichText::new("Running")
            .color(Color32::from_rgb(62, 140, 96))
            .strong(),
        Some(GraphRuntimeMode::Paused) => RichText::new("Paused")
            .color(Color32::from_rgb(196, 147, 60))
            .strong(),
        None => RichText::new("Stopped")
            .color(Color32::from_gray(130))
            .strong(),
    }
}
