use std::collections::HashMap;

use egui_snarl::{NodeId, Snarl};
use shared::{
    GraphDocument, GraphEdge, GraphNode, InputValue, NodeDefinition, NodeInputValue, NodeMetadata,
    NodeParameter, NodeParameterDefinition, NodeTypeId, ValueKind,
};

use super::{EditorInputPort, EditorOutputPort, EditorSnarlNode};

/// Builds an editor snarl graph from a persisted graph document.
///
/// Node positions, collapsed state, wires, schema-driven defaults, and live runtime values are
/// all projected into the editor model used by `egui_snarl`.
pub(super) fn build_snarl_from_document(
    document: &GraphDocument,
    available_node_definitions: &[NodeDefinition],
    runtime_node_values: &HashMap<String, HashMap<String, InputValue>>,
) -> Snarl<EditorSnarlNode> {
    let mut snarl = Snarl::new();
    let mut node_id_by_graph_id = HashMap::<String, NodeId>::new();

    for node in &document.nodes {
        let runtime_values = runtime_node_values.get(&node.id);
        let editor_node =
            editor_node_from_graph_node(node, available_node_definitions, runtime_values);
        let node_position = &node.viewport.position;
        let node_id = if node.viewport.collapsed {
            snarl.insert_node_collapsed(egui::pos2(node_position.x, node_position.y), editor_node)
        } else {
            snarl.insert_node(egui::pos2(node_position.x, node_position.y), editor_node)
        };
        node_id_by_graph_id.insert(node.id.clone(), node_id);
    }

    for edge in &document.edges {
        let Some(from_node_id) = node_id_by_graph_id.get(&edge.from_node_id).copied() else {
            continue;
        };
        let Some(to_node_id) = node_id_by_graph_id.get(&edge.to_node_id).copied() else {
            continue;
        };

        let Some(from_node) = snarl.get_node(from_node_id) else {
            continue;
        };
        let Some(to_node) = snarl.get_node(to_node_id) else {
            continue;
        };

        let Some(from_output_index) = output_port_index(&from_node.outputs, &edge.from_output_name)
        else {
            continue;
        };
        let Some(to_input_index) = input_port_index(&to_node.inputs, &edge.to_input_name) else {
            continue;
        };

        snarl.connect(
            egui_snarl::OutPinId {
                node: from_node_id,
                output: from_output_index,
            },
            egui_snarl::InPinId {
                node: to_node_id,
                input: to_input_index,
            },
        );
    }

    snarl
}

/// Synchronizes the persisted graph document from the current editor snarl state.
///
/// This updates node positions, collapsed state, titles, input values, parameters, and edges so
/// the document can be autosaved or sent back to the backend.
pub(super) fn sync_document_from_snarl(
    document: &mut GraphDocument,
    snarl: &Snarl<EditorSnarlNode>,
) {
    let mut existing_nodes = HashMap::<String, GraphNode>::new();
    for node in document.nodes.drain(..) {
        existing_nodes.insert(node.id.clone(), node);
    }

    let mut synced_nodes = Vec::new();
    for (node_id, pos, editor_node) in snarl.nodes_pos_ids() {
        let collapsed = snarl
            .get_node_info(node_id)
            .map(|node_info| !node_info.open)
            .unwrap_or(false);
        if let Some(mut node) = existing_nodes.remove(&editor_node.graph_node_id) {
            node.viewport.position.x = pos.x;
            node.viewport.position.y = pos.y;
            node.viewport.collapsed = collapsed;
            node.metadata.name = editor_node.title.clone();
            node.input_values = editor_node
                .inputs
                .iter()
                .map(|input| NodeInputValue {
                    name: input.name.clone(),
                    value: input.value.clone(),
                })
                .collect();
            node.parameters = editor_node.parameters.clone();
            synced_nodes.push(node);
            continue;
        }

        let node_type = NodeTypeId::new(editor_node.node_type_id.clone());
        let input_values = editor_node
            .inputs
            .iter()
            .map(|input| NodeInputValue {
                name: input.name.clone(),
                value: input.value.clone(),
            })
            .collect();

        synced_nodes.push(GraphNode {
            id: editor_node.graph_node_id.clone(),
            metadata: NodeMetadata {
                name: editor_node.title.clone(),
            },
            node_type,
            viewport: shared::NodeViewport {
                position: shared::NodePosition { x: pos.x, y: pos.y },
                collapsed,
            },
            input_values,
            parameters: editor_node.parameters.clone(),
        });
    }
    document.nodes = synced_nodes;

    let mut edges = Vec::<GraphEdge>::new();
    for (out_pin, in_pin) in snarl.wires() {
        let Some(from_node) = snarl.get_node(out_pin.node) else {
            continue;
        };
        let Some(to_node) = snarl.get_node(in_pin.node) else {
            continue;
        };

        let Some(from_output) = from_node.outputs.get(out_pin.output) else {
            continue;
        };
        let Some(to_input) = to_node.inputs.get(in_pin.input) else {
            continue;
        };

        edges.push(GraphEdge {
            from_node_id: from_node.graph_node_id.clone(),
            from_output_name: from_output.name.clone(),
            to_node_id: to_node.graph_node_id.clone(),
            to_input_name: to_input.name.clone(),
        });
    }
    edges.sort_by(|a, b| {
        (
            &a.from_node_id,
            &a.from_output_name,
            &a.to_node_id,
            &a.to_input_name,
        )
            .cmp(&(
                &b.from_node_id,
                &b.from_output_name,
                &b.to_node_id,
                &b.to_input_name,
            ))
    });
    document.edges = edges;
}

/// Builds a new editor node from a shared node definition.
///
/// This is used when inserting a node from the add-node menu, so all inputs, outputs, and
/// parameters start with their schema-defined defaults.
pub(super) fn editor_node_from_definition(
    graph_node_id: String,
    title: String,
    node_type_id: String,
    available_node_definitions: &[NodeDefinition],
) -> EditorSnarlNode {
    let Some(definition) = find_node_definition(available_node_definitions, &node_type_id) else {
        return EditorSnarlNode {
            graph_node_id,
            title,
            node_type_id,
            inputs: Vec::new(),
            outputs: Vec::new(),
            parameters: Vec::new(),
            runtime_values: Vec::new(),
        };
    };

    let inputs = definition
        .inputs
        .iter()
        .map(|input| EditorInputPort {
            name: input.name.clone(),
            display_name: input.display_name.clone(),
            value_kind: input.value_kind,
            value: default_input_value_for_node_input(
                &node_type_id,
                &input.name,
                input.value_kind,
                available_node_definitions,
            ),
        })
        .collect();
    let outputs = definition
        .outputs
        .iter()
        .map(|output| EditorOutputPort {
            name: output.name.clone(),
            display_name: output.display_name.clone(),
            value_kind: output.value_kind,
            runtime_value: None,
        })
        .collect();

    EditorSnarlNode {
        graph_node_id,
        title,
        node_type_id,
        inputs,
        outputs,
        parameters: parameters_with_defaults(&[], &definition.id, available_node_definitions),
        runtime_values: Vec::new(),
    }
}

/// Builds an editor node from a persisted graph node.
///
/// Known node types use the shared node definition to restore ports, defaults, and runtime values.
/// Unknown node types fall back to the persisted input values so the graph can still be edited.
fn editor_node_from_graph_node(
    node: &GraphNode,
    available_node_definitions: &[NodeDefinition],
    runtime_values: Option<&HashMap<String, InputValue>>,
) -> EditorSnarlNode {
    if let Some(definition) =
        find_node_definition(available_node_definitions, node.node_type.as_str())
    {
        let inputs = definition
            .inputs
            .iter()
            .map(|input| EditorInputPort {
                name: input.name.clone(),
                display_name: input.display_name.clone(),
                value_kind: input.value_kind,
                value: graph_node_input_value_or_default(
                    node,
                    &input.name,
                    input.value_kind,
                    available_node_definitions,
                ),
            })
            .collect();
        let outputs = definition
            .outputs
            .iter()
            .map(|output| EditorOutputPort {
                name: output.name.clone(),
                display_name: output.display_name.clone(),
                value_kind: output.value_kind,
                runtime_value: runtime_values
                    .and_then(|values| values.get(&output.name))
                    .cloned(),
            })
            .collect();

        return EditorSnarlNode {
            graph_node_id: node.id.clone(),
            title: node.metadata.name.clone(),
            node_type_id: node.node_type.as_str().to_owned(),
            inputs,
            outputs,
            parameters: parameters_with_defaults(
                &node.parameters,
                node.node_type.as_str(),
                available_node_definitions,
            ),
            runtime_values: runtime_values
                .map(|values| {
                    values
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        };
    }

    let inputs = node
        .input_values
        .iter()
        .map(|input| EditorInputPort {
            name: input.name.clone(),
            display_name: input.name.clone(),
            value_kind: input.value.value_kind(),
            value: input.value.clone(),
        })
        .collect();

    EditorSnarlNode {
        graph_node_id: node.id.clone(),
        title: node.metadata.name.clone(),
        node_type_id: node.node_type.as_str().to_owned(),
        inputs,
        outputs: Vec::new(),
        parameters: parameters_with_defaults(
            &node.parameters,
            node.node_type.as_str(),
            available_node_definitions,
        ),
        runtime_values: Vec::new(),
    }
}

/// Returns the generic default input value for a shared value kind.
pub(super) fn default_input_value(kind: ValueKind) -> InputValue {
    match kind {
        ValueKind::Any => InputValue::Float(0.0),
        ValueKind::Float => InputValue::Float(0.0),
        ValueKind::FloatTensor => InputValue::FloatTensor(shared::FloatTensor {
            shape: vec![1],
            values: vec![0.0],
        }),
        ValueKind::Color => InputValue::Color(shared::RgbaColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }),
        ValueKind::LedLayout => InputValue::LedLayout(shared::LedLayout {
            id: "default".to_owned(),
            pixel_count: 60,
            width: None,
            height: None,
        }),
        ValueKind::ColorFrame => InputValue::ColorFrame(shared::ColorFrame {
            layout: shared::LedLayout {
                id: "default".to_owned(),
                pixel_count: 0,
                width: None,
                height: None,
            },
            pixels: Vec::new(),
        }),
    }
}

/// Coerces an input value to the expected port kind.
///
/// Values that already match the requested kind are preserved. Mismatched values are replaced with
/// the generic default for that kind.
pub(super) fn coerce_input_value_kind(value: InputValue, kind: ValueKind) -> InputValue {
    match (kind, value) {
        (ValueKind::Any, value) => value,
        (ValueKind::Float, InputValue::Float(v)) => InputValue::Float(v),
        (ValueKind::FloatTensor, InputValue::FloatTensor(v)) => InputValue::FloatTensor(v),
        (ValueKind::Color, InputValue::Color(v)) => InputValue::Color(v),
        (ValueKind::LedLayout, InputValue::LedLayout(v)) => InputValue::LedLayout(v),
        (ValueKind::ColorFrame, InputValue::ColorFrame(v)) => InputValue::ColorFrame(v),
        (expected_kind, _) => default_input_value(expected_kind),
    }
}

/// Returns node parameters with any missing schema defaults filled in.
pub(super) fn parameters_with_defaults(
    parameters: &[NodeParameter],
    node_type_id: &str,
    available_node_definitions: &[NodeDefinition],
) -> Vec<NodeParameter> {
    let mut merged = parameters.to_vec();
    let Some(definition) = find_node_definition(available_node_definitions, node_type_id) else {
        return merged;
    };
    for default_parameter in &definition.parameters {
        if merged
            .iter()
            .all(|parameter| parameter.name != default_parameter.name)
        {
            merged.push(NodeParameter {
                name: default_parameter.name.clone(),
                value: default_parameter.default_value.to_json_value(),
            });
        }
    }
    merged
}

/// Returns the subset of parameters that should currently be shown in the editor.
pub(super) fn visible_parameter_definitions<'a>(
    definition: &'a NodeDefinition,
    parameters: &[NodeParameter],
) -> Vec<&'a NodeParameterDefinition> {
    definition
        .parameters
        .iter()
        .filter(|parameter_definition| parameter_definition.is_visible(parameters))
        .collect()
}

/// Returns the index of the named input port.
fn input_port_index(ports: &[EditorInputPort], name: &str) -> Option<usize> {
    ports.iter().position(|port| port.name == name)
}

/// Returns the index of the named output port.
fn output_port_index(ports: &[EditorOutputPort], name: &str) -> Option<usize> {
    ports.iter().position(|port| port.name == name)
}

/// Returns a persisted node input value or the schema default when no value is stored.
fn graph_node_input_value_or_default(
    node: &GraphNode,
    input_name: &str,
    kind: ValueKind,
    available_node_definitions: &[NodeDefinition],
) -> InputValue {
    let value = node
        .input_values
        .iter()
        .find(|input| input.name == input_name)
        .map(|input| input.value.clone())
        .unwrap_or_else(|| {
            default_input_value_for_node_input(
                node.node_type.as_str(),
                input_name,
                kind,
                available_node_definitions,
            )
        });
    coerce_input_value_kind(value, kind)
}

/// Returns the default value for a specific node input definition.
///
/// When the shared schema does not provide an explicit default, the generic value-kind default is
/// used instead.
fn default_input_value_for_node_input(
    node_type_id: &str,
    input_name: &str,
    kind: ValueKind,
    available_node_definitions: &[NodeDefinition],
) -> InputValue {
    find_node_definition(available_node_definitions, node_type_id)
        .and_then(|definition| definition.input_port(input_name))
        .and_then(|input| input.default_value.clone())
        .unwrap_or_else(|| default_input_value(kind))
}

/// Finds the shared node definition for `node_type_id`.
pub(super) fn find_node_definition<'a>(
    available_node_definitions: &'a [NodeDefinition],
    node_type_id: &str,
) -> Option<&'a NodeDefinition> {
    available_node_definitions
        .iter()
        .find(|definition| definition.id == node_type_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shared::{
        NodeCategory, NodeConnectionDefinition, ParameterDefaultValue, ParameterUiHint,
        ParameterVisibilityCondition,
    };

    fn sample_definition() -> NodeDefinition {
        NodeDefinition {
            id: "test.visibility".to_owned(),
            display_name: "Test Visibility".to_owned(),
            category: NodeCategory::Debug,
            inputs: vec![],
            outputs: vec![],
            parameters: vec![
                NodeParameterDefinition {
                    name: "mode".to_owned(),
                    display_name: "Mode".to_owned(),
                    default_value: ParameterDefaultValue::String("basic".to_owned()),
                    ui_hint: ParameterUiHint::TextSingleLine,
                    visible_when: None,
                },
                NodeParameterDefinition {
                    name: "advanced_value".to_owned(),
                    display_name: "Advanced Value".to_owned(),
                    default_value: ParameterDefaultValue::Float(1.5),
                    ui_hint: ParameterUiHint::DragFloat {
                        speed: 0.1,
                        min: 0.0,
                        max: 10.0,
                    },
                    visible_when: Some(ParameterVisibilityCondition {
                        parameter: "mode".to_owned(),
                        equals: json!("advanced"),
                    }),
                },
            ],
            connection: NodeConnectionDefinition {
                max_input_connections: 1,
                require_value_kind_match: true,
            },
            runtime_updates: None,
        }
    }

    #[test]
    fn visible_parameter_definitions_hides_conditionally_hidden_controls() {
        let definition = sample_definition();
        let parameters = parameters_with_defaults(&[], &definition.id, &[definition.clone()]);

        let visible = visible_parameter_definitions(&definition, &parameters);
        assert_eq!(
            visible
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            vec!["mode"]
        );
    }

    #[test]
    fn visible_parameter_definitions_shows_controls_when_the_condition_matches() {
        let definition = sample_definition();
        let mut parameters = parameters_with_defaults(&[], &definition.id, &[definition.clone()]);
        let mode = parameters
            .iter_mut()
            .find(|parameter| parameter.name == "mode")
            .unwrap();
        mode.value = json!("advanced");

        let visible = visible_parameter_definitions(&definition, &parameters);
        assert_eq!(
            visible
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            vec!["mode", "advanced_value"]
        );
    }

    #[test]
    fn parameters_with_defaults_still_preserves_hidden_values() {
        let definition = sample_definition();
        let definition_id = definition.id.clone();
        let parameters = parameters_with_defaults(
            &[NodeParameter {
                name: "advanced_value".to_owned(),
                value: json!(9.0),
            }],
            &definition_id,
            &[definition],
        );

        assert!(
            parameters
                .iter()
                .any(|parameter| parameter.name == "advanced_value")
        );
    }
}
