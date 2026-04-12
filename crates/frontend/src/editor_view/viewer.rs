use eframe::egui;
use eframe::egui::emath::TSTransform;
use eframe::egui::{Popup, PopupCloseBehavior, SetOpenCommand};
use egui_snarl::ui::{NodeLayout, PinInfo, PinPlacement, SnarlPin, SnarlStyle, SnarlViewer};
use egui_snarl::{InPin, NodeId, OutPin, Snarl};
use shared::{
    GraphDocument, MqttBrokerConfig, NodeCategory, NodeDefinition, NodeDiagnosticSeverity,
    NodeDiagnosticSummary, NodeTypeId, ValueKind, WledInstance,
};

use super::EditorSnarlNode;
use super::model::{
    build_snarl_from_document, editor_node_from_definition, find_node_definition,
    sync_document_from_snarl, visible_parameter_definitions,
};
use super::widgets::{
    draw_color_frame_preview, draw_float_plot, edit_input_value, edit_parameter_value,
    ensure_parameter_defaults, format_input_value, max_input_label_width, show_runtime_value,
};

struct GraphSnarlViewer {
    initial_transform: Option<TSTransform>,
    current_transform: Option<TSTransform>,
    wled_instances: Vec<WledInstance>,
    mqtt_broker_configs: Vec<MqttBrokerConfig>,
    available_node_definitions: Vec<NodeDefinition>,
    plot_history: std::collections::HashMap<String, Vec<f32>>,
    diagnostic_summaries: std::collections::HashMap<String, NodeDiagnosticSummary>,
    opened_diagnostics_node_id: Option<String>,
    requested_image_upload: Option<(String, String)>,
    node_menu_search: String,
    requested_graph_menu_pos: Option<egui::Pos2>,
}

const MIN_NODE_WIDTH: f32 = 220.0;

impl SnarlViewer<EditorSnarlNode> for GraphSnarlViewer {
    /// Returns the node layout used for all editor nodes.
    fn node_layout(
        &mut self,
        _default: NodeLayout,
        _node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        _snarl: &Snarl<EditorSnarlNode>,
    ) -> NodeLayout {
        NodeLayout::sandwich()
    }

    /// Returns the node title shown in the canvas header.
    ///
    /// The title includes the shared display name and the node type ID to make debugging easier
    /// while the editor still uses machine-stable node types internally.
    fn title(&mut self, node: &EditorSnarlNode) -> String {
        format!("{} ({})", node.title, node.node_type_id)
    }

    /// Returns the number of output pins exposed by a node.
    fn outputs(&mut self, node: &EditorSnarlNode) -> usize {
        node.outputs.len()
    }

    /// Returns the number of input pins exposed by a node.
    fn inputs(&mut self, node: &EditorSnarlNode) -> usize {
        node.inputs.len()
    }

    /// Renders the node header, including the title field, diagnostics badge, and delete button.
    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorSnarlNode>,
    ) {
        let total_width = (ui.available_width() - 2.0).max(MIN_NODE_WIDTH);
        let button_width = 40.0;
        let badge_width = if snarl[node].graph_node_id.as_str().is_empty() {
            0.0
        } else if self
            .diagnostic_summaries
            .contains_key(snarl[node].graph_node_id.as_str())
        {
            80.0
        } else {
            0.0
        };
        let spacing = ui.spacing().item_spacing.x;
        let title_width = (total_width
            - button_width
            - badge_width
            - (if badge_width > 0.0 { spacing } else { 0.0 })
            - spacing)
            .max(80.0);

        ui.add_sized(
            [title_width, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut snarl[node].title),
        );

        if let Some(summary) = self
            .diagnostic_summaries
            .get(snarl[node].graph_node_id.as_str())
        {
            let (label, color) = match summary.highest_severity {
                NodeDiagnosticSeverity::Info => ("I", egui::Color32::from_rgb(90, 150, 220)),
                NodeDiagnosticSeverity::Warning => ("W", egui::Color32::from_rgb(210, 160, 60)),
                NodeDiagnosticSeverity::Error => ("E", egui::Color32::from_rgb(190, 80, 80)),
            };
            if ui
                .add_sized(
                    [badge_width, ui.spacing().interact_size.y],
                    egui::Button::new(
                        egui::RichText::new(format!("{label}{}", summary.active_count))
                            .color(color)
                            .strong(),
                    ),
                )
                .clicked()
            {
                self.opened_diagnostics_node_id = Some(snarl[node].graph_node_id.clone());
            }
        }

        let response = ui
            .add_sized(
                [button_width, ui.spacing().interact_size.y],
                egui::Button::new("X").fill(egui::Color32::from_rgb(224, 108, 117)),
            )
            .clicked();

        if response {
            snarl.remove_node(node);
        }
    }

    /// Renders a node input pin and its inline editor when the pin is not connected.
    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorSnarlNode>,
    ) -> impl SnarlPin + 'static {
        let label_width = {
            let node = &snarl[pin.id.node];
            max_input_label_width(ui, node)
        };
        let is_connected = !pin.remotes.is_empty();
        let node = &mut snarl[pin.id.node];
        let Some(port) = node.inputs.get_mut(pin.id.input) else {
            ui.label("?");
            return PinInfo::default();
        };

        let response = ui.label(&port.display_name);
        let spacing = label_width - response.rect.width();
        ui.add_space(spacing);

        if !is_connected {
            edit_input_value(ui, port);
        }
        pin_info_for_kind(port.value_kind)
    }

    /// Connects two pins when their shared node definitions allow the connection.
    ///
    /// The destination input's connection policy is enforced before the edge is created.
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<EditorSnarlNode>) {
        let Some(from_node) = snarl.get_node(from.id.node) else {
            return;
        };
        let Some(to_node) = snarl.get_node(to.id.node) else {
            return;
        };
        let Some(from_port) = from_node.outputs.get(from.id.output) else {
            return;
        };
        let Some(to_port) = to_node.inputs.get(to.id.input) else {
            return;
        };

        let Some(from_definition) =
            find_node_definition(&self.available_node_definitions, &from_node.node_type_id)
        else {
            return;
        };
        let Some(to_definition) =
            find_node_definition(&self.available_node_definitions, &to_node.node_type_id)
        else {
            return;
        };

        let Some(from_port_definition) = from_definition.output_port(&from_port.name) else {
            return;
        };
        let Some(to_port_definition) = to_definition.input_port(&to_port.name) else {
            return;
        };
        if !to_definition.can_connect_ports(from_port_definition, to_port_definition) {
            return;
        }

        let max_input_connections = to_definition.connection.max_input_connections;
        if max_input_connections == 0 {
            return;
        }
        if max_input_connections == 1 {
            for &remote in &to.remotes {
                snarl.disconnect(remote, to.id);
            }
        } else if to.remotes.len() >= max_input_connections {
            return;
        }

        snarl.connect(from.id, to.id);
    }

    /// Returns whether the node should render a body section beneath its pins.
    fn has_body(&mut self, node: &EditorSnarlNode) -> bool {
        let has_parameters =
            find_node_definition(&self.available_node_definitions, &node.node_type_id)
                .map(|definition| !definition.parameters.is_empty())
                .unwrap_or(false);
        has_parameters || !node.runtime_values.is_empty()
    }

    /// Renders the node body, including parameters, runtime values, and plot previews.
    fn show_body(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorSnarlNode>,
    ) {
        let editor_node = &mut snarl[node];
        let Some(definition) =
            find_node_definition(&self.available_node_definitions, &editor_node.node_type_id)
        else {
            return;
        };
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ensure_parameter_defaults(
                &mut editor_node.parameters,
                &editor_node.node_type_id,
                &self.available_node_definitions,
            );
            let visible_parameters =
                visible_parameter_definitions(definition, &editor_node.parameters);
            if !definition.inputs.is_empty()
                && (!visible_parameters.is_empty()
                    || !editor_node.runtime_values.is_empty()
                    || !definition.outputs.is_empty())
            {
                ui.separator();
            }
            egui::Grid::new(ui.id().with("parameter_grid"))
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    for parameter_definition in &visible_parameters {
                        ui.label(&parameter_definition.display_name);
                        let requested_upload = edit_parameter_value(
                            ui,
                            &mut editor_node.parameters,
                            &parameter_definition.name,
                            &parameter_definition.ui_hint,
                            parameter_definition.default_value.to_json_value(),
                            &self.wled_instances,
                            &self.mqtt_broker_configs,
                        );
                        if requested_upload {
                            self.requested_image_upload = Some((
                                editor_node.graph_node_id.clone(),
                                parameter_definition.name.clone(),
                            ));
                        }
                        ui.end_row();
                    }
                });
            if !visible_parameters.is_empty()
                && (!editor_node.runtime_values.is_empty() || !definition.outputs.is_empty())
            {
                ui.separator();
            }
            for (_name, value) in &editor_node.runtime_values {
                match value {
                    shared::InputValue::ColorFrame(frame) => {
                        draw_color_frame_preview(ui, frame);
                    }
                    _ => {
                        show_runtime_value(ui, value);
                    }
                }
            }
            if editor_node.node_type_id == NodeTypeId::PLOT {
                if let Some(samples) = self.plot_history.get(&editor_node.graph_node_id) {
                    if !samples.is_empty() {
                        if !editor_node.runtime_values.is_empty() || !editor_node.outputs.is_empty()
                        {
                            ui.separator();
                        }
                        draw_float_plot(ui, samples);
                    }
                }
            }
            if !editor_node.runtime_values.is_empty() && !editor_node.outputs.is_empty() {
                ui.separator();
            }
        });
    }

    /// Renders a node output pin and its latest runtime value when available.
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorSnarlNode>,
    ) -> impl SnarlPin + 'static {
        let node = &snarl[pin.id.node];
        let Some(port) = node.outputs.get(pin.id.output) else {
            ui.label("?");
            return PinInfo::default();
        };

        if let Some(value) = &port.runtime_value {
            ui.label(format!(
                "{} = {}",
                port.display_name,
                format_input_value(value)
            ));
        } else {
            ui.label(&port.display_name);
        }
        pin_info_for_kind(port.value_kind)
    }

    /// Returns whether the graph canvas should expose a context menu at `pos`.
    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<EditorSnarlNode>) -> bool {
        true
    }

    /// Requests that the graph context menu be opened at the clicked canvas position.
    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _snarl: &mut Snarl<EditorSnarlNode>,
    ) {
        self.requested_graph_menu_pos = Some(pos);
        ui.close();
    }

    /// Tracks the current canvas transform reported by `egui_snarl`.
    ///
    /// When an initial transform is queued, it is applied once before the current transform is
    /// recorded for persistence back into the graph document viewport.
    fn current_transform(
        &mut self,
        to_global: &mut TSTransform,
        _snarl: &mut Snarl<EditorSnarlNode>,
    ) {
        if let Some(initial_transform) = self.initial_transform.take() {
            *to_global = initial_transform;
        }
        self.current_transform = Some(*to_global);
    }
}

/// Renders the add-node context menu and inserts a selected node at `pos`.
///
/// When a search term is active the menu is flattened into a single result list; otherwise nodes
/// remain grouped by category.
fn render_graph_menu_contents(
    pos: egui::Pos2,
    node_menu_search: &mut String,
    available_node_definitions: &[NodeDefinition],
    ui: &mut egui::Ui,
    snarl: &mut Snarl<EditorSnarlNode>,
) {
    ui.set_min_width(260.0);
    ui.label("Add Node");
    ui.add_space(4.0);
    ui.add_sized(
        [ui.available_width(), ui.spacing().interact_size.y],
        egui::TextEdit::singleline(node_menu_search).hint_text("Search nodes..."),
    );
    ui.separator();

    let categories = node_menu_categories(available_node_definitions, node_menu_search);
    let searching = !node_menu_search.trim().is_empty();
    let no_matches = categories.is_empty();
    let mut selected_definition_id: Option<String> = None;
    if searching {
        for category in &categories {
            for definition in &category.definitions {
                let label = format!("{} ({})", definition.display_name, category.label);
                if ui.button(label).clicked() {
                    selected_definition_id = Some(definition.id.clone());
                    ui.close();
                }
            }
        }
    } else {
        for category in categories {
            ui.menu_button(category.label, |ui| {
                ui.set_min_width(220.0);
                for definition in &category.definitions {
                    if ui.button(&definition.display_name).clicked() {
                        selected_definition_id = Some(definition.id.clone());
                        ui.close();
                    }
                }
            });
        }
    }
    if selected_definition_id.is_none() && no_matches {
        ui.label(
            egui::RichText::new("No matching nodes")
                .italics()
                .color(egui::Color32::from_gray(140)),
        );
    }
    if let Some(definition_id) = selected_definition_id {
        let Some(definition) = find_node_definition(available_node_definitions, &definition_id)
        else {
            return;
        };
        let graph_node_id = next_context_menu_node_id(snarl, &definition.id);
        snarl.insert_node(
            pos,
            editor_node_from_definition(
                graph_node_id,
                definition.display_name.clone(),
                definition.id.clone(),
                available_node_definitions,
            ),
        );
        node_menu_search.clear();
        ui.close();
    }
}

/// Renders the graph canvas, synchronizes it with the document model, and returns UI side effects.
///
/// The returned tuple carries viewport-application state, diagnostics-panel requests, hover state,
/// node-menu search text, the next graph-space position for the add-node menu, and any requested
/// image upload action.
pub(super) fn show_snarl_canvas(
    ui: &mut egui::Ui,
    document: &mut GraphDocument,
    available_node_definitions: &[NodeDefinition],
    runtime_node_values: &std::collections::HashMap<
        String,
        std::collections::HashMap<String, shared::InputValue>,
    >,
    plot_history: &std::collections::HashMap<String, std::collections::VecDeque<f32>>,
    diagnostic_summaries: &std::collections::HashMap<String, NodeDiagnosticSummary>,
    wled_instances: &[WledInstance],
    mqtt_broker_configs: &[MqttBrokerConfig],
    node_menu_search: &str,
    node_menu_graph_position: Option<egui::Pos2>,
    apply_document_viewport: bool,
    focus_requested: bool,
) -> (
    bool,
    Option<String>,
    bool,
    String,
    Option<egui::Pos2>,
    Option<(String, String)>,
) {
    let canvas_size = ui.available_size();
    if focus_requested {
        center_viewport_on_nodes(document, canvas_size);
    }

    let mut snarl =
        build_snarl_from_document(document, available_node_definitions, runtime_node_values);
    let should_apply_transform = apply_document_viewport || focus_requested;
    let initial_transform = if should_apply_transform {
        Some(TSTransform::new(
            egui::vec2(document.viewport.pan.x, document.viewport.pan.y),
            document.viewport.zoom.max(0.0001),
        ))
    } else {
        None
    };
    let mut viewer = GraphSnarlViewer {
        initial_transform,
        current_transform: None,
        wled_instances: wled_instances.to_vec(),
        mqtt_broker_configs: mqtt_broker_configs.to_vec(),
        available_node_definitions: available_node_definitions.to_vec(),
        plot_history: plot_history
            .iter()
            .map(|(node_id, samples)| (node_id.clone(), samples.iter().copied().collect()))
            .collect(),
        diagnostic_summaries: diagnostic_summaries.clone(),
        opened_diagnostics_node_id: None,
        requested_image_upload: None,
        node_menu_search: node_menu_search.to_owned(),
        requested_graph_menu_pos: None,
    };
    let style = SnarlStyle {
        collapsible: Some(true),
        crisp_magnified_text: Some(true),
        pin_placement: Some(PinPlacement::Edge),
        header_drag_space: Some([0.0, 0.0].into()),
        ..SnarlStyle::new()
    };

    let canvas_response = ui.allocate_ui_with_layout(
        canvas_size,
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            snarl.show(
                &mut viewer,
                &style,
                format!("graph_snarl_{}", document.metadata.id),
                ui,
            );
        },
    );
    let mut next_node_menu_graph_position = node_menu_graph_position;
    let open_context_menu = viewer.requested_graph_menu_pos.is_some();
    if let Some(menu_pos) = viewer.requested_graph_menu_pos {
        next_node_menu_graph_position = Some(menu_pos);
    }

    Popup::context_menu(&canvas_response.response)
        .open_memory(open_context_menu.then_some(SetOpenCommand::Bool(true)))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            if let Some(menu_pos) = next_node_menu_graph_position {
                render_graph_menu_contents(
                    menu_pos,
                    &mut viewer.node_menu_search,
                    &viewer.available_node_definitions,
                    ui,
                    &mut snarl,
                );
            }
        });
    if !Popup::context_menu(&canvas_response.response).is_open() {
        next_node_menu_graph_position = None;
    }

    sync_document_from_snarl(document, &snarl);
    if let Some(transform) = viewer.current_transform {
        document.viewport.zoom = transform.scaling;
        document.viewport.pan.x = transform.translation.x;
        document.viewport.pan.y = transform.translation.y;
    }
    let canvas_hovered = ui.rect_contains_pointer(canvas_response.response.rect);
    (
        should_apply_transform,
        viewer.opened_diagnostics_node_id,
        canvas_hovered,
        viewer.node_menu_search,
        next_node_menu_graph_position,
        viewer.requested_image_upload,
    )
}

/// Centers the stored viewport on the current set of nodes.
///
/// This keeps the existing zoom level and only updates the pan offset needed to place the node
/// bounding box near the center of the visible canvas.
fn center_viewport_on_nodes(document: &mut GraphDocument, canvas_size: egui::Vec2) {
    let mut iter = document.nodes.iter();
    let Some(first) = iter.next() else {
        return;
    };

    let mut min_x = first.viewport.position.x;
    let mut max_x = first.viewport.position.x;
    let mut min_y = first.viewport.position.y;
    let mut max_y = first.viewport.position.y;

    for node in iter {
        let x = node.viewport.position.x;
        let y = node.viewport.position.y;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let zoom = document.viewport.zoom.max(0.0001);
    document.viewport.pan.x = canvas_size.x * 0.5 - center_x * zoom;
    document.viewport.pan.y = canvas_size.y * 0.5 - center_y * zoom;
}

/// Returns the pin styling used for a shared value kind.
fn pin_info_for_kind(kind: ValueKind) -> PinInfo {
    match kind {
        ValueKind::Any => PinInfo::circle().with_fill(egui::Color32::from_rgb(171, 178, 191)),
        ValueKind::Float => PinInfo::circle().with_fill(egui::Color32::from_rgb(97, 175, 239)),
        ValueKind::FloatTensor => {
            PinInfo::circle().with_fill(egui::Color32::from_rgb(86, 182, 194))
        }
        ValueKind::Color => PinInfo::circle().with_fill(egui::Color32::from_rgb(198, 120, 221)),
        ValueKind::LedLayout => PinInfo::circle().with_fill(egui::Color32::from_rgb(224, 108, 117)),
        ValueKind::ColorFrame => {
            PinInfo::circle().with_fill(egui::Color32::from_rgb(152, 195, 121))
        }
    }
}

/// Returns the next graph-node ID for a node created from the context menu.
///
/// IDs are derived from the node type and incremented to avoid collisions with existing nodes in
/// the current snarl graph.
fn next_context_menu_node_id(snarl: &Snarl<EditorSnarlNode>, node_type_id: &str) -> String {
    let prefix = node_type_id.replace('.', "_");
    let mut next_index = 1usize;
    for node in snarl.nodes() {
        if let Some(suffix) = node.graph_node_id.strip_prefix(&format!("{prefix}_")) {
            if let Ok(parsed) = suffix.parse::<usize>() {
                next_index = next_index.max(parsed + 1);
            }
        }
    }
    format!("{prefix}_{next_index}")
}

struct NodeMenuCategory {
    label: &'static str,
    definitions: Vec<NodeDefinition>,
}

/// Groups node definitions into menu categories and filters them by the current search term.
fn node_menu_categories(
    available_node_definitions: &[NodeDefinition],
    search: &str,
) -> Vec<NodeMenuCategory> {
    let normalized_search = search.trim().to_lowercase();
    let mut inputs = Vec::new();
    let mut generators = Vec::new();
    let mut math = Vec::new();
    let mut frame_operations = Vec::new();
    let mut temporal_filters = Vec::new();
    let mut spatial_filters = Vec::new();
    let mut outputs = Vec::new();
    let mut debug = Vec::new();

    for definition in available_node_definitions {
        if !normalized_search.is_empty() {
            let haystacks = [
                definition.display_name.to_lowercase(),
                definition.id.to_lowercase(),
            ];
            if !haystacks
                .iter()
                .any(|candidate| candidate.contains(normalized_search.as_str()))
            {
                continue;
            }
        }
        match definition.category {
            NodeCategory::Inputs => inputs.push(definition.clone()),
            NodeCategory::Generators => generators.push(definition.clone()),
            NodeCategory::Math => math.push(definition.clone()),
            NodeCategory::FrameOperations => frame_operations.push(definition.clone()),
            NodeCategory::TemporalFilters => temporal_filters.push(definition.clone()),
            NodeCategory::SpatialFilters => spatial_filters.push(definition.clone()),
            NodeCategory::Outputs => outputs.push(definition.clone()),
            NodeCategory::Debug => debug.push(definition.clone()),
        }
    }

    let mut categories = Vec::new();
    push_node_menu_category(&mut categories, "Inputs", inputs);
    push_node_menu_category(&mut categories, "Generators", generators);
    push_node_menu_category(&mut categories, "Math", math);
    push_node_menu_category(&mut categories, "Frame Operations", frame_operations);
    push_node_menu_category(&mut categories, "Temporal Filters", temporal_filters);
    push_node_menu_category(&mut categories, "Spatial Filters", spatial_filters);
    push_node_menu_category(&mut categories, "Outputs", outputs);
    push_node_menu_category(&mut categories, "Debug", debug);
    categories
}

/// Appends a node menu category after sorting its definitions by display name.
fn push_node_menu_category(
    categories: &mut Vec<NodeMenuCategory>,
    label: &'static str,
    mut definitions: Vec<NodeDefinition>,
) {
    if definitions.is_empty() {
        return;
    }
    definitions.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    categories.push(NodeMenuCategory { label, definitions });
}

#[cfg(test)]
mod tests {
    use super::node_menu_categories;
    use shared::{
        NodeCategory, NodeConnectionDefinition, NodeDefinition, NodeInputDefinition,
        NodeOutputDefinition, NodeTypeId, ValueKind,
    };

    fn test_node(id: &str, category: NodeCategory, display_name: &str) -> NodeDefinition {
        NodeDefinition {
            id: id.to_owned(),
            display_name: display_name.to_owned(),
            category,
            needs_io: false,
            inputs: vec![NodeInputDefinition {
                name: "value".to_owned(),
                display_name: "Value".to_owned(),
                value_kind: ValueKind::Float,
                accepted_kinds: vec![],
                default_value: None,
            }],
            outputs: vec![NodeOutputDefinition {
                name: "value".to_owned(),
                display_name: "Value".to_owned(),
                value_kind: ValueKind::Float,
                accepted_kinds: vec![],
            }],
            parameters: vec![],
            connection: NodeConnectionDefinition {
                max_input_connections: 1,
                require_value_kind_match: true,
            },
            runtime_updates: None,
        }
    }

    #[test]
    fn node_menu_categories_group_nodes_by_category() {
        let definitions = vec![
            test_node(
                "inputs.float_constant",
                NodeCategory::Inputs,
                "Float Constant",
            ),
            test_node(
                "inputs.audio_fft_receiver",
                NodeCategory::Inputs,
                "Audio FFT Receiver",
            ),
            test_node("generators.plasma", NodeCategory::Generators, "Plasma"),
        ];

        let categories = node_menu_categories(&definitions, "");
        assert_eq!(categories.len(), 2);
        assert_eq!(categories[0].label, "Inputs");
        assert_eq!(categories[0].definitions.len(), 2);
        assert_eq!(
            categories[0].definitions[0].id,
            NodeTypeId::AUDIO_FFT_RECEIVER
        );
        assert_eq!(categories[0].definitions[1].id, NodeTypeId::FLOAT_CONSTANT);
        assert_eq!(categories[1].label, "Generators");
        assert_eq!(categories[1].definitions.len(), 1);
        assert_eq!(categories[1].definitions[0].id, "generators.plasma");
    }
}
