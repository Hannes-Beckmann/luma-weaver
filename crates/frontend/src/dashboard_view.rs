use eframe::egui;
use eframe::egui::{Color32, RichText};
use shared::{ClientMessage, GraphImportCollisionPolicy, GraphRuntimeMode, MqttBrokerConfig};

/// Renders a single-line text edit with a fixed width for settings-style forms.
fn sized_text_edit_singleline(ui: &mut egui::Ui, text: &mut String, width: f32) -> egui::Response {
    ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::TextEdit::singleline(text),
    )
}

use crate::app::FrontendApp;

/// Renders the dashboard view for listing, creating, importing, exporting, and controlling graphs.
pub(crate) fn render(ctx: &egui::Context, ui: &mut egui::Ui, app: &mut FrontendApp) {
    let graph_count = app.graphs.graph_documents.len();
    let running_count = app
        .graphs
        .graph_documents
        .iter()
        .filter(|graph| app.graph_runtime_mode(&graph.id) == Some(GraphRuntimeMode::Running))
        .count();
    let paused_count = app
        .graphs
        .graph_documents
        .iter()
        .filter(|graph| app.graph_runtime_mode(&graph.id) == Some(GraphRuntimeMode::Paused))
        .count();

    ui.vertical(|ui| {
        ui.label(RichText::new("Graph Documents").heading().strong());
        ui.label(
            RichText::new("Open a graph to edit it, or run playback directly from the dashboard.")
                .color(Color32::from_gray(170)),
        );
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            summary_badge(
                ui,
                format!("{graph_count} graphs"),
                Color32::from_rgb(87, 127, 173),
            );
            summary_badge(
                ui,
                format!("{running_count} running"),
                Color32::from_rgb(62, 140, 96),
            );
            summary_badge(
                ui,
                format!("{paused_count} paused"),
                Color32::from_rgb(196, 147, 60),
            );
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add(secondary_action_button("Add Graph Document"))
                .clicked()
            {
                app.ui.create_graph_dialog_open = true;
                app.ui.new_graph_name.clear();
            }
            if ui.add(secondary_action_button("Import Graph")).clicked() {
                app.begin_graph_import();
            }
            if ui.add(secondary_action_button("MQTT Brokers")).clicked() {
                app.ui.mqtt_broker_draft = app.graphs.mqtt_broker_configs.clone();
                app.ui.mqtt_broker_dialog_open = true;
            }
            if ui
                .add(secondary_action_button("Refresh Metadata"))
                .clicked()
            {
                app.send(ClientMessage::GetGraphMetadata);
            }
        });
    });
    ui.add_space(10.0);

    if app.graphs.graph_documents.is_empty() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_height(120.0);
            ui.vertical_centered(|ui| {
                ui.add_space(18.0);
                ui.label(RichText::new("No graph documents yet").heading());
                ui.label("Create one to start building an animation graph.");
                ui.add_space(6.0);
                if ui
                    .add(secondary_action_button("Create Graph Document"))
                    .clicked()
                {
                    app.ui.create_graph_dialog_open = true;
                    app.ui.new_graph_name.clear();
                }
            });
        });
    } else {
        let graphs = app.graphs.graph_documents.clone();
        let mut open_graph: Option<String> = None;
        let mut delete_graph: Option<String> = None;
        let mut rename_graph: Option<(String, String)> = None;

        for graph in &graphs {
            let graph_id = graph.id.clone();
            let mode = app.graph_runtime_mode(&graph.id);
            let mut execution_frequency_hz = graph.execution_frequency_hz.max(1);
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&graph.name).strong().size(18.0));
                                if ui.add(secondary_action_button("Rename")).clicked() {
                                    rename_graph = Some((graph_id.clone(), graph.name.clone()));
                                }
                            });
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    danger_action_button("Remove")
                                        .fill(Color32::from_rgba_unmultiplied(190, 92, 92, 24)),
                                )
                                .clicked()
                            {
                                delete_graph = Some(graph_id.clone());
                            }

                            ui.add_space(8.0);
                            if runtime_action_button(
                                ui,
                                "Stop",
                                mode.is_some(),
                                Color32::from_rgb(190, 92, 92),
                            )
                            .clicked()
                            {
                                app.stop_graph(graph_id.clone());
                            }
                            if runtime_action_button(
                                ui,
                                "Pause",
                                mode == Some(GraphRuntimeMode::Running),
                                Color32::from_rgb(196, 147, 60),
                            )
                            .clicked()
                            {
                                app.pause_graph(graph_id.clone());
                            }
                            if runtime_action_button(
                                ui,
                                "Run",
                                mode != Some(GraphRuntimeMode::Running),
                                Color32::from_rgb(62, 140, 96),
                            )
                            .clicked()
                            {
                                app.start_graph(graph_id.clone());
                            }

                            if ui
                                .add_sized([96.0, 28.0], primary_action_button("Open"))
                                .clicked()
                            {
                                open_graph = Some(graph_id.clone());
                            }
                            if ui.add(secondary_action_button("Export")).clicked() {
                                app.request_graph_export(graph_id.clone());
                            }
                        });
                    });

                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        runtime_badge(ui, mode);
                        ui.label(RichText::new("Tick rate").color(Color32::from_gray(150)));
                        let response = ui.add(
                            egui::DragValue::new(&mut execution_frequency_hz)
                                .speed(1.0)
                                .range(1..=1_000),
                        );
                        ui.label(RichText::new("Hz").color(Color32::from_gray(150)));
                        if response.changed() {
                            app.update_graph_execution_frequency(
                                graph_id.clone(),
                                execution_frequency_hz,
                            );
                        }
                    });
                });
            ui.add_space(8.0);
        }

        if let Some(id) = open_graph {
            app.open_graph(id);
        }

        if let Some(id) = delete_graph {
            app.send(ClientMessage::DeleteGraphDocument { id });
        }

        if let Some((id, name)) = rename_graph {
            app.ui.rename_graph_dialog_open = true;
            app.ui.rename_graph_id = Some(id);
            app.ui.rename_graph_name = name;
        }
    }

    if app.ui.create_graph_dialog_open {
        let mut open = app.ui.create_graph_dialog_open;
        egui::Window::new("Create Graph Document")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut app.ui.new_graph_name);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let name = app.ui.new_graph_name.trim().to_owned();
                        if name.is_empty() {
                            app.ui.status = "Graph document name must not be empty".to_owned();
                        } else {
                            app.send(ClientMessage::CreateGraphDocument { name });
                            app.ui.new_graph_name.clear();
                            app.ui.create_graph_dialog_open = false;
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        app.ui.create_graph_dialog_open = false;
                    }
                });
            });
        app.ui.create_graph_dialog_open = open && app.ui.create_graph_dialog_open;
    }

    if app.ui.mqtt_broker_dialog_open {
        let mut open = app.ui.mqtt_broker_dialog_open;
        const TEXT_EDIT_WIDTH: f32 = 150.0;
        egui::Window::new("MQTT Brokers")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    "Configure reusable brokers for Home Assistant MQTT nodes. Unchecked brokers stay stored, but are hidden from Home Assistant node selectors.",
                );
                ui.add_space(8.0);

                for (index, broker) in app.ui.mqtt_broker_draft.iter_mut().enumerate() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Broker {}", index + 1)).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Remove").clicked() {
                                        broker.id.push('\u{0}');
                                    }
                                },
                            );
                        });
                        egui::Grid::new(format!("mqtt_broker_grid_{index}"))
                            .num_columns(4)
                            .spacing([10.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("Id");
                                sized_text_edit_singleline(ui, &mut broker.id, TEXT_EDIT_WIDTH);

                                ui.label("Name");
                                sized_text_edit_singleline(
                                    ui,
                                    &mut broker.display_name,
                                    TEXT_EDIT_WIDTH,
                                );
                                ui.end_row();

                                ui.label("Host");
                                sized_text_edit_singleline(ui, &mut broker.host, TEXT_EDIT_WIDTH);
                                ui.label("Port");
                                ui.add(egui::DragValue::new(&mut broker.port).range(1..=65_535));
                                ui.end_row();

                                ui.label("Username");
                                sized_text_edit_singleline(
                                    ui,
                                    &mut broker.username,
                                    TEXT_EDIT_WIDTH,
                                );
                                ui.label("Password");
                                ui.add_sized(
                                    [TEXT_EDIT_WIDTH, ui.spacing().interact_size.y],
                                    egui::TextEdit::singleline(&mut broker.password).password(true),
                                );
                                ui.end_row();
                                ui.label("Discovery Prefix");
                                sized_text_edit_singleline(
                                    ui,
                                    &mut broker.discovery_prefix,
                                    TEXT_EDIT_WIDTH,
                                );
                                ui.end_row();

                                ui.label("Home Assistant");
                                ui.checkbox(
                                    &mut broker.is_home_assistant,
                                    "Use for Home Assistant MQTT nodes",
                                );
                                ui.end_row();
                            });
                    });
                    ui.add_space(6.0);
                }

                app.ui
                    .mqtt_broker_draft
                    .retain(|broker| !broker.id.contains('\u{0}'));

                if ui.button("Add Broker").clicked() {
                    app.ui.mqtt_broker_draft.push(MqttBrokerConfig {
                        id: format!("broker_{}", app.ui.mqtt_broker_draft.len() + 1),
                        display_name: format!("Broker {}", app.ui.mqtt_broker_draft.len() + 1),
                        host: "127.0.0.1".to_owned(),
                        port: 1883,
                        username: String::new(),
                        password: String::new(),
                        discovery_prefix: "homeassistant".to_owned(),
                        is_home_assistant: true,
                    });
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        app.save_mqtt_broker_configs(app.ui.mqtt_broker_draft.clone());
                        app.ui.mqtt_broker_dialog_open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        app.ui.mqtt_broker_dialog_open = false;
                    }
                });
            });
        app.ui.mqtt_broker_dialog_open = open && app.ui.mqtt_broker_dialog_open;
    }

    if app.ui.rename_graph_dialog_open {
        let mut open = app.ui.rename_graph_dialog_open;
        egui::Window::new("Rename Graph Document")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut app.ui.rename_graph_name);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let Some(graph_id) = app.ui.rename_graph_id.clone() else {
                            return;
                        };
                        let name = app.ui.rename_graph_name.trim().to_owned();
                        if name.is_empty() {
                            app.ui.status = "Graph document name must not be empty".to_owned();
                        } else {
                            app.update_graph_name(graph_id, name);
                            app.ui.rename_graph_dialog_open = false;
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        app.ui.rename_graph_dialog_open = false;
                    }
                });
            });
        app.ui.rename_graph_dialog_open = open && app.ui.rename_graph_dialog_open;
        if !app.ui.rename_graph_dialog_open {
            app.ui.rename_graph_id = None;
            app.ui.rename_graph_name.clear();
        }
    }

    if let Some(file) = app.ui.pending_import_graph_file.clone() {
        let mut open = true;
        egui::Window::new("Import Graph Collision")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "A graph with id '{}' already exists.",
                    file.document.metadata.id
                ));
                ui.label(format!(
                    "Imported graph name: {}",
                    file.document.metadata.name
                ));
                ui.add_space(8.0);
                ui.label("Choose how to import this graph:");
                ui.add_space(8.0);

                if ui.button("Import Copy").clicked() {
                    app.import_pending_graph_file(GraphImportCollisionPolicy::ImportCopy);
                }
                if ui.button("Overwrite Existing").clicked() {
                    app.import_pending_graph_file(GraphImportCollisionPolicy::OverwriteExisting);
                }
                if ui.button("Cancel").clicked() {
                    app.ui.pending_import_graph_file = None;
                    app.ui.status = "Cancelled graph import".to_owned();
                }
            });

        if !open {
            app.ui.pending_import_graph_file = None;
        }
    }
}

/// Renders a colored summary badge used in the dashboard overview row.
fn summary_badge(ui: &mut egui::Ui, label: String, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.label(RichText::new(label).color(color).strong());
        });
}

/// Renders the current runtime mode badge for a graph card.
fn runtime_badge(ui: &mut egui::Ui, mode: Option<GraphRuntimeMode>) {
    let (label, color) = match mode {
        Some(GraphRuntimeMode::Running) => ("Running", Color32::from_rgb(62, 140, 96)),
        Some(GraphRuntimeMode::Paused) => ("Paused", Color32::from_rgb(196, 147, 60)),
        None => ("Stopped", Color32::from_gray(130)),
    };

    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(label).color(color).strong());
        });
}

/// Renders one of the colored runtime control buttons used on dashboard graph cards.
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

/// Builds a neutral secondary action button for dashboard toolbar and card actions.
fn secondary_action_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).color(Color32::from_gray(215)))
        .fill(Color32::from_gray(44))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(78)))
        .min_size(egui::vec2(58.0, 28.0))
}

/// Builds the neutral primary action button used for opening a graph from the dashboard.
fn primary_action_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).strong())
        .fill(Color32::from_gray(62))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(94)))
        .min_size(egui::vec2(72.0, 28.0))
}

/// Builds the destructive action button used for graph removal.
fn danger_action_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).color(Color32::from_rgb(214, 108, 108)))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(132, 70, 70)))
        .min_size(egui::vec2(58.0, 28.0))
}
