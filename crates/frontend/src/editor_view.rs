use eframe::egui;
use eframe::egui::{Color32, RichText};
use shared::{GraphRuntimeMode, InputValue, NodeParameter, RgbaColor, SinkPreviewFrame, ValueKind};

use crate::app::FrontendApp;

/// Converts persisted graph documents into the editor's node-canvas representation.
mod model;
/// Owns the snarl canvas widget and node-graph interaction behavior.
mod viewer;
/// Renders parameter editors and runtime preview widgets used inside editor nodes.
mod widgets;

#[derive(Clone)]
/// Editor-side representation of a single input port on a rendered node card.
pub(crate) struct EditorInputPort {
    name: String,
    display_name: String,
    value_kind: ValueKind,
    value: InputValue,
}

#[derive(Clone)]
/// Editor-side representation of a single output port on a rendered node card.
pub(crate) struct EditorOutputPort {
    name: String,
    display_name: String,
    value_kind: ValueKind,
    runtime_value: Option<InputValue>,
}

#[derive(Clone)]
/// Editor-side representation of a graph node as shown on the snarl canvas.
pub(crate) struct EditorSnarlNode {
    graph_node_id: String,
    title: String,
    node_type_id: String,
    inputs: Vec<EditorInputPort>,
    outputs: Vec<EditorOutputPort>,
    parameters: Vec<NodeParameter>,
    runtime_values: Vec<(String, InputValue)>,
}

pub(crate) use self::model::{
    build_snarl_from_document, clipboard_fragment_from_document,
    paste_clipboard_fragment_into_document, patch_snarl_from_document,
    refresh_snarl_runtime_values,
};

#[cfg(test)]
pub(crate) fn snarl_node_titles(snarl: &egui_snarl::Snarl<EditorSnarlNode>) -> Vec<String> {
    snarl.nodes().map(|node| node.title.clone()).collect()
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

                        let copy_response = ui.add_enabled(
                            !app.ui.selected_graph_node_ids.is_empty(),
                            secondary_action_button("Copy"),
                        );
                        if copy_response.clicked() {
                            app.copy_selected_nodes_to_clipboard();
                        }

                        if ui.add(secondary_action_button("Paste")).clicked() {
                            app.paste_nodes_from_clipboard();
                        }

                        if ui.add(secondary_action_button("Export")).clicked() {
                            app.request_graph_export(graph.id.clone());
                        }

                        if ui.add(secondary_action_button("3D Preview")).clicked() {
                            app.ui.sink_preview_window_open = true;
                            app.ui.sink_preview_selected_view_key = "all".to_owned();
                            app.ui.sink_preview_display_scope_node_id = None;
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
            let diagnostic_summaries = app
                .graph_diagnostic_summaries(&graph.id)
                .cloned()
                .unwrap_or_default();
            let available_node_definitions = app.graphs.available_node_definitions.clone();
            let node_menu_search = app.ui.node_menu_search.clone();
            let node_menu_graph_position = app
                .ui
                .node_menu_graph_position
                .map(|(x, y)| egui::pos2(x, y));
            let mut requested_image_upload = None;
            let mut requested_layout_upload = None;
            let mut requested_preview_node_id = None;
            app.ensure_live_snarl_for_active_graph();
            if let Some((document, snarl)) = app.active_graph_document_and_snarl_mut() {
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
                refresh_snarl_runtime_values(
                    snarl,
                    &available_node_definitions,
                    &runtime_node_values,
                );
                let (
                    initialized,
                    opened_diagnostics_node_id,
                    canvas_hovered,
                    selected_graph_node_ids,
                    editor_pointer_graph_position,
                    node_menu_search,
                    node_menu_graph_position,
                    image_upload_request,
                    layout_upload_request,
                    preview_node_request,
                ) = viewer::show_snarl_canvas(
                    ui,
                    snarl,
                    document,
                    &available_node_definitions,
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
                app.ui.selected_graph_node_ids = selected_graph_node_ids;
                app.ui.editor_pointer_graph_position =
                    editor_pointer_graph_position.map(|pos| (pos.x, pos.y));
                app.ui.node_menu_search = node_menu_search;
                app.ui.node_menu_graph_position =
                    node_menu_graph_position.map(|pos| (pos.x, pos.y));
                if let Some(node_id) = opened_diagnostics_node_id {
                    app.open_node_diagnostics(node_id);
                }
                requested_image_upload = image_upload_request;
                requested_layout_upload = layout_upload_request;
                requested_preview_node_id = preview_node_request;
            } else {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_height(100.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new("Loading graph document...").strong());
                        ui.label("Waiting for the backend to send the latest saved version.");
                    });
                });
                app.ui.selected_graph_node_ids.clear();
                app.ui.editor_pointer_graph_position = None;
            }
            if initialized_viewport_for_graph {
                app.graphs.snarl_viewport_initialized_graph_id = Some(graph.id.clone());
            }
            if let Some((node_id, parameter_name)) = requested_image_upload {
                app.begin_image_asset_upload(node_id, parameter_name);
            }
            if let Some((node_id, parameter_name)) = requested_layout_upload {
                app.begin_layout_asset_upload(node_id, parameter_name);
            }
            if let Some(node_id) = requested_preview_node_id {
                app.ui.sink_preview_window_open = true;
                app.ui.sink_preview_display_scope_node_id = Some(node_id.clone());
                app.ui.sink_preview_selected_view_key = "all".to_owned();
            }

            crate::diagnostics_view::render_node_diagnostics_window(ui.ctx(), app);
            render_sink_preview_window(ui.ctx(), app, &graph.id);

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

/// Renders the live spatial sink preview window for the selected graph.
fn render_sink_preview_window(ctx: &egui::Context, app: &mut FrontendApp, graph_id: &str) {
    if !app.ui.sink_preview_window_open {
        return;
    }

    let mut open = app.ui.sink_preview_window_open;
    egui::Window::new("3D Preview")
        .default_size(egui::vec2(760.0, 560.0))
        .open(&mut open)
        .show(ctx, |ui| {
            let preview_views = collect_preview_views(
                app,
                graph_id,
                app.ui.sink_preview_display_scope_node_id.as_deref(),
            );
            if preview_views.is_empty() {
                app.ui.sink_preview_selected_view_key = "all".to_owned();
            } else if !preview_views
                .iter()
                .any(|view| view.key == app.ui.sink_preview_selected_view_key)
            {
                app.ui.sink_preview_selected_view_key = "all".to_owned();
            }

            ui.horizontal(|ui| {
                ui.label(RichText::new("View").color(Color32::from_gray(150)));
                egui::ComboBox::from_id_salt("preview_view_select")
                    .selected_text(
                        preview_views
                            .iter()
                            .find(|view| view.key == app.ui.sink_preview_selected_view_key)
                            .map(|view| view.label.as_str())
                            .unwrap_or("All together"),
                    )
                    .show_ui(ui, |ui| {
                        for view in &preview_views {
                            if ui
                                .selectable_label(
                                    app.ui.sink_preview_selected_view_key == view.key,
                                    &view.label,
                                )
                                .clicked()
                            {
                                app.ui.sink_preview_selected_view_key = view.key.clone();
                            }
                        }
                    });

                ui.label(RichText::new("LED Size").color(Color32::from_gray(150)));
                ui.add(
                    egui::DragValue::new(&mut app.ui.sink_preview_led_size)
                        .speed(0.1)
                        .range(1.0..=24.0),
                );
                ui.checkbox(&mut app.ui.sink_preview_show_axes, "Show Axes");
                if ui.add(secondary_action_button("Reset View")).clicked() {
                    app.ui.sink_preview_yaw = 0.5;
                    app.ui.sink_preview_pitch = -0.35;
                    app.ui.sink_preview_zoom = 1.0;
                    app.ui.sink_preview_pan_x = 0.0;
                    app.ui.sink_preview_pan_y = 0.0;
                }
            });

            let selected_frames = preview_views
                .iter()
                .find(|view| view.key == app.ui.sink_preview_selected_view_key)
                .map(|view| view.frames.as_slice())
                .unwrap_or(&[]);
            let mut camera = PreviewCamera {
                yaw: app.ui.sink_preview_yaw,
                pitch: app.ui.sink_preview_pitch,
                zoom: app.ui.sink_preview_zoom,
                pan: egui::vec2(app.ui.sink_preview_pan_x, app.ui.sink_preview_pan_y),
                led_size: app.ui.sink_preview_led_size,
            };
            draw_sink_preview_scene(
                ui,
                selected_frames,
                &mut camera,
                app.ui.sink_preview_show_axes,
            );
            app.ui.sink_preview_yaw = camera.yaw;
            app.ui.sink_preview_pitch = camera.pitch;
            app.ui.sink_preview_zoom = camera.zoom;
            app.ui.sink_preview_pan_x = camera.pan.x;
            app.ui.sink_preview_pan_y = camera.pan.y;
            app.ui.sink_preview_led_size = camera.led_size;
        });
    app.ui.sink_preview_window_open = open;
}

#[derive(Clone)]
struct PreviewView {
    key: String,
    label: String,
    frames: Vec<SinkPreviewFrame>,
}

fn collect_preview_views(
    app: &FrontendApp,
    graph_id: &str,
    scoped_node_id: Option<&str>,
) -> Vec<PreviewView> {
    if let Some(node_id) = scoped_node_id {
        return collect_scoped_node_preview_views(app, node_id);
    }

    let sink_frames = app
        .graphs
        .sink_preview_frames_by_graph
        .get(graph_id)
        .cloned()
        .unwrap_or_default();
    let display_frames = collect_display_preview_frames(app);

    let mut views = Vec::new();
    let mut all_frames = Vec::new();
    all_frames.extend(sink_frames.clone());
    all_frames.extend(display_frames.iter().flat_map(|view| view.frames.clone()));
    views.push(PreviewView {
        key: "all".to_owned(),
        label: "All together".to_owned(),
        frames: all_frames,
    });

    for frame in &sink_frames {
        views.push(PreviewView {
            key: format!("sink:{}", frame.sink_node_id),
            label: format!("Sink: {}", frame.sink_node_name),
            frames: vec![frame.clone()],
        });
    }

    views.extend(display_frames);
    views
}

fn collect_scoped_node_preview_views(app: &FrontendApp, node_id: &str) -> Vec<PreviewView> {
    let Some(document) = app.graphs.loaded_graph_document.as_ref() else {
        return Vec::new();
    };
    let Some(node) = document.nodes.iter().find(|node| node.id == node_id) else {
        return Vec::new();
    };
    let Some(values) = app.graphs.runtime_node_values.get(node_id) else {
        return Vec::new();
    };

    let mut scoped_views = Vec::new();
    let mut layout_index = 1usize;
    for (name, value) in values {
        let Some(frame) =
            preview_frame_from_input_value(value, node.id.as_str(), node.metadata.name.as_str())
        else {
            continue;
        };
        let label = if name == "source_frame" {
            format!(
                "{} / Mapped Frame ({})",
                node.metadata.name,
                preview_frame_size_label(&frame)
            )
        } else {
            let label = format!(
                "{} / Layout {} ({})",
                node.metadata.name,
                layout_index,
                preview_frame_size_label(&frame)
            );
            layout_index += 1;
            label
        };
        scoped_views.push(PreviewView {
            key: format!("node_layout:{}:{}", node.id, name),
            label,
            frames: vec![frame],
        });
    }

    let combined_frames = values
        .iter()
        .filter_map(|(_name, value)| {
            preview_frame_from_input_value(value, node.id.as_str(), node.metadata.name.as_str())
        })
        .collect::<Vec<_>>();
    if !combined_frames.is_empty() {
        scoped_views.insert(
            0,
            PreviewView {
                key: "all".to_owned(),
                label: "All together".to_owned(),
                frames: combined_frames,
            },
        );
    }

    scoped_views
}

fn collect_display_preview_frames(app: &FrontendApp) -> Vec<PreviewView> {
    let Some(document) = app.graphs.loaded_graph_document.as_ref() else {
        return Vec::new();
    };

    document
        .nodes
        .iter()
        .filter(|node| node.node_type.as_str() == shared::NodeTypeId::DISPLAY)
        .flat_map(|node| {
            app.graphs
                .runtime_node_values
                .get(&node.id)
                .into_iter()
                .flat_map(move |values| {
                    let mut layout_index = 1usize;
                    values.iter().filter_map(move |(name, value)| {
                        let frame = preview_frame_from_input_value(
                            value,
                            node.id.as_str(),
                            node.metadata.name.as_str(),
                        )?;
                        let label = if name == "source_frame" {
                            format!(
                                "Display: {} / Mapped Frame ({})",
                                node.metadata.name,
                                preview_frame_size_label(&frame)
                            )
                        } else {
                            let label = format!(
                                "Display: {} / Layout {} ({})",
                                node.metadata.name,
                                layout_index,
                                preview_frame_size_label(&frame)
                            );
                            layout_index += 1;
                            label
                        };
                        Some(PreviewView {
                            key: format!("display_layout:{}:{}", node.id, name),
                            label,
                            frames: vec![frame],
                        })
                    })
                })
                .chain({
                    let combined_frames = app
                        .graphs
                        .runtime_node_values
                        .get(&node.id)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(|(_name, value)| {
                                    preview_frame_from_input_value(
                                        value,
                                        node.id.as_str(),
                                        node.metadata.name.as_str(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    (!combined_frames.is_empty())
                        .then(|| PreviewView {
                            key: format!("display_node:{}", node.id),
                            label: format!("Display: {} (all layouts)", node.metadata.name),
                            frames: combined_frames,
                        })
                        .into_iter()
                })
        })
        .collect()
}

fn preview_frame_from_input_value(
    value: &InputValue,
    node_id: &str,
    node_name: &str,
) -> Option<SinkPreviewFrame> {
    match value {
        InputValue::ColorFrame(frame) | InputValue::MappedFrame(frame)
            if frame.layout.points_3d.is_some() =>
        {
            Some(SinkPreviewFrame {
                sink_node_id: node_id.to_owned(),
                sink_node_name: node_name.to_owned(),
                layout: frame.layout.clone(),
                pixels: frame.pixels.clone(),
            })
        }
        _ => None,
    }
}

fn preview_frame_size_label(frame: &SinkPreviewFrame) -> String {
    let width = frame
        .layout
        .width
        .unwrap_or(frame.layout.pixel_count.max(1));
    let height = frame
        .layout
        .height
        .unwrap_or_else(|| if frame.layout.width.is_some() { 1 } else { 1 });
    format!("{}x{}", width, height)
}

#[derive(Clone, Copy)]
struct PreviewCamera {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    led_size: f32,
}

fn draw_sink_preview_scene(
    ui: &mut egui::Ui,
    frames: &[SinkPreviewFrame],
    camera: &mut PreviewCamera,
    show_axes: bool,
) {
    let available = ui.available_size();
    let desired_size = egui::vec2(available.x.max(320.0), available.y.max(320.0));
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 20, 23));
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, Color32::from_gray(58)),
        egui::StrokeKind::Inside,
    );

    let pointer_delta = ui.input(|input| input.pointer.delta());
    if response.dragged_by(egui::PointerButton::Primary) {
        camera.yaw += pointer_delta.x * 0.01;
        camera.pitch = (camera.pitch + pointer_delta.y * 0.01).clamp(-3.124, 3.124);
        ui.ctx().request_repaint();
    }
    if response.dragged_by(egui::PointerButton::Secondary)
        || response.dragged_by(egui::PointerButton::Middle)
    {
        camera.pan += pointer_delta;
        ui.ctx().request_repaint();
    }
    if response.hovered() {
        let scroll_y = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll_y != 0.0 {
            camera.zoom = (camera.zoom * (scroll_y * 0.0015).exp()).clamp(0.1, 20.0);
            ui.ctx().request_repaint();
        }
    }

    let points = collect_spatial_preview_points(frames);
    if points.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No spatial preview frames available.",
            egui::FontId::proportional(15.0),
            Color32::from_gray(155),
        );
        return;
    }

    let bounds = world_bounds(&points);
    let target = bounds.center();
    let radius = bounds.radius(&points).max(0.001);
    let scale = rect.width().min(rect.height()) * 0.42 / radius * camera.zoom.max(0.1);
    let mut sorted_samples = project_preview_points(&points, target, camera);
    sorted_samples.sort_by(|a, b| a.z.total_cmp(&b.z));
    for sample in sorted_samples {
        let pos = egui::pos2(
            rect.center().x + camera.pan.x + sample.x * scale,
            rect.center().y + camera.pan.y - sample.y * scale,
        );
        let depth = (0.65 + sample.z / (radius * 2.0).max(0.001) * 0.35).clamp(0.4, 1.0);
        let point_radius = (camera.led_size * depth * camera.zoom.sqrt()).clamp(1.0, 32.0);
        painter.circle_filled(pos, point_radius, sample.color);
    }

    if show_axes {
        draw_preview_axes(&painter, rect, camera, target, scale);
    }
}

#[derive(Clone)]
struct PreviewPoint {
    x: f32,
    y: f32,
    z: f32,
    color: Color32,
}

#[derive(Clone)]
struct PreviewSample {
    x: f32,
    y: f32,
    z: f32,
    color: Color32,
}

#[derive(Clone, Copy)]
struct PreviewBounds {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    min_z: f32,
    max_z: f32,
}

impl PreviewBounds {
    fn center(self) -> (f32, f32, f32) {
        (
            (self.min_x + self.max_x) * 0.5,
            (self.min_y + self.max_y) * 0.5,
            (self.min_z + self.max_z) * 0.5,
        )
    }

    fn radius(self, points: &[PreviewPoint]) -> f32 {
        let center = self.center();
        points
            .iter()
            .map(|point| {
                let dx = point.x - center.0;
                let dy = point.y - center.1;
                let dz = point.z - center.2;
                (dx * dx + dy * dy + dz * dz).sqrt()
            })
            .fold(0.0, f32::max)
    }
}

fn collect_spatial_preview_points(frames: &[SinkPreviewFrame]) -> Vec<PreviewPoint> {
    let mut points = Vec::new();
    for frame in frames {
        let Some(layout_points) = frame.layout.points_3d.as_ref() else {
            continue;
        };
        if layout_points.len() != frame.layout.pixel_count {
            continue;
        }
        for (index, point) in layout_points.iter().enumerate() {
            let color = frame.pixels.get(index).copied().unwrap_or_else(black_color);
            points.push(PreviewPoint {
                x: point.x,
                y: point.y,
                z: point.z,
                color: rgba_to_color32(color),
            });
        }
    }
    points
}

fn world_bounds(points: &[PreviewPoint]) -> PreviewBounds {
    points.iter().fold(
        PreviewBounds {
            min_x: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            min_y: f32::INFINITY,
            max_y: f32::NEG_INFINITY,
            min_z: f32::INFINITY,
            max_z: f32::NEG_INFINITY,
        },
        |mut bounds, point| {
            bounds.min_x = bounds.min_x.min(point.x);
            bounds.max_x = bounds.max_x.max(point.x);
            bounds.min_y = bounds.min_y.min(point.y);
            bounds.max_y = bounds.max_y.max(point.y);
            bounds.min_z = bounds.min_z.min(point.z);
            bounds.max_z = bounds.max_z.max(point.z);
            bounds
        },
    )
}

fn project_preview_points(
    points: &[PreviewPoint],
    target: (f32, f32, f32),
    camera: &PreviewCamera,
) -> Vec<PreviewSample> {
    let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
    let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();

    points
        .iter()
        .map(|point| {
            let x = point.x - target.0;
            let y = point.y - target.1;
            let z = point.z - target.2;
            let yawed_x = x * cos_yaw - y * sin_yaw;
            let yawed_y = x * sin_yaw + y * cos_yaw;
            let pitched_y = yawed_y * cos_pitch - z * sin_pitch;
            let pitched_z = yawed_y * sin_pitch + z * cos_pitch;
            PreviewSample {
                x: yawed_x,
                y: pitched_y,
                z: pitched_z,
                color: point.color,
            }
        })
        .collect()
}

fn project_preview_vector(x: f32, y: f32, z: f32, camera: &PreviewCamera) -> (f32, f32, f32) {
    let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
    let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();
    let yawed_x = x * cos_yaw - y * sin_yaw;
    let yawed_y = x * sin_yaw + y * cos_yaw;
    let pitched_y = yawed_y * cos_pitch - z * sin_pitch;
    let pitched_z = yawed_y * sin_pitch + z * cos_pitch;
    (yawed_x, pitched_y, pitched_z)
}

fn draw_preview_axes(
    painter: &egui::Painter,
    rect: egui::Rect,
    camera: &PreviewCamera,
    target: (f32, f32, f32),
    scale: f32,
) {
    let (origin_x, origin_y, _origin_z) =
        project_preview_vector(-target.0, -target.1, -target.2, camera);
    let origin = egui::pos2(
        rect.center().x + camera.pan.x + origin_x * scale,
        rect.center().y + camera.pan.y - origin_y * scale,
    );
    let axis_scale = (scale * 0.18).clamp(18.0, 48.0);
    let stroke_width = 2.0;
    let font = egui::FontId::proportional(12.0);
    let axes = [
        (
            "X",
            Color32::from_rgb(220, 80, 80),
            project_preview_vector(1.0, 0.0, 0.0, camera),
        ),
        (
            "Y",
            Color32::from_rgb(90, 210, 120),
            project_preview_vector(0.0, 1.0, 0.0, camera),
        ),
        (
            "Z",
            Color32::from_rgb(90, 150, 230),
            project_preview_vector(0.0, 0.0, 1.0, camera),
        ),
    ];

    painter.circle_filled(origin, 3.0, Color32::from_gray(180));
    for (label, color, (x, y, _z)) in axes {
        let tip = egui::pos2(origin.x + x * axis_scale, origin.y - y * axis_scale);
        painter.line_segment(
            [origin, tip],
            egui::Stroke::new(stroke_width, color.gamma_multiply(0.95)),
        );
        painter.circle_filled(tip, 2.5, color);
        painter.text(
            egui::pos2(tip.x + 6.0, tip.y),
            egui::Align2::LEFT_CENTER,
            label,
            font.clone(),
            color,
        );
    }
}

fn rgba_to_color32(color: RgbaColor) -> Color32 {
    Color32::from_rgba_premultiplied(
        unit_to_u8(color.r),
        unit_to_u8(color.g),
        unit_to_u8(color.b),
        unit_to_u8(color.a),
    )
}

fn unit_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn black_color() -> RgbaColor {
    RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
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
