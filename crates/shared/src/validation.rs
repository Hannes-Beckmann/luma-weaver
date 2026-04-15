use std::collections::{HashMap, HashSet};

use crate::graph::{GraphDocument, GraphNode, ValueKind, node_definition};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Describes one validation problem found while checking a persisted graph document.
pub struct GraphValidationIssue {
    pub code: GraphValidationIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Classifies the kind of validation problem found in a graph document.
pub enum GraphValidationIssueCode {
    DuplicateNodeId,
    UnknownNodeType,
    DuplicateNodeInputName,
    UnknownNodeInputName,
    NodeInputTypeMismatch,
    UnknownEdgeSourceNode,
    UnknownEdgeTargetNode,
    UnknownEdgeSourceOutput,
    UnknownEdgeTargetInput,
    EdgeTypeMismatch,
}

/// Validates a graph document against the shared node catalog.
///
/// Validation checks node identity, declared input values, edge endpoints, and type compatibility.
/// All discovered issues are returned at once so callers can show a complete error list.
pub fn validate_graph_document(document: &GraphDocument) -> Vec<GraphValidationIssue> {
    let mut issues = Vec::new();
    let mut node_ids = HashSet::new();
    let mut nodes_by_id = HashMap::new();

    for (node_index, node) in document.nodes.iter().enumerate() {
        let node_path = format!("nodes[{node_index}]");
        if !node_ids.insert(node.id.clone()) {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::DuplicateNodeId,
                path: format!("{node_path}.id"),
                message: format!("duplicate node id '{}'", node.id),
            });
        }
        nodes_by_id.insert(node.id.clone(), node.clone());

        let Some(definition) = node_definition(node.node_type.as_str()) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownNodeType,
                path: format!("{node_path}.node_type"),
                message: format!("unknown node type '{}'", node.node_type.as_str()),
            });
            continue;
        };

        let mut input_names = HashSet::new();
        for (input_index, input) in node.input_values.iter().enumerate() {
            let input_path = format!("{node_path}.input_values[{input_index}]");
            if !input_names.insert(input.name.clone()) {
                issues.push(GraphValidationIssue {
                    code: GraphValidationIssueCode::DuplicateNodeInputName,
                    path: format!("{input_path}.name"),
                    message: format!("duplicate input value name '{}'", input.name),
                });
            }

            let Some(port) = definition.input_port(&input.name) else {
                issues.push(GraphValidationIssue {
                    code: GraphValidationIssueCode::UnknownNodeInputName,
                    path: format!("{input_path}.name"),
                    message: format!(
                        "input '{}' is not declared by node type '{}'",
                        input.name, definition.id
                    ),
                });
                continue;
            };

            if !port.accepts_kind(input.value.value_kind()) {
                issues.push(GraphValidationIssue {
                    code: GraphValidationIssueCode::NodeInputTypeMismatch,
                    path: format!("{input_path}.value"),
                    message: format!(
                        "input '{}' expected {:?} (or compatible kinds) but found {:?}",
                        input.name,
                        port.value_kind,
                        input.value.value_kind()
                    ),
                });
            }
        }
    }

    for (edge_index, edge) in document.edges.iter().enumerate() {
        let edge_path = format!("edges[{edge_index}]");

        let Some(from_node) = nodes_by_id.get(&edge.from_node_id) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownEdgeSourceNode,
                path: format!("{edge_path}.from_node_id"),
                message: format!("source node '{}' does not exist", edge.from_node_id),
            });
            continue;
        };
        let Some(to_node) = nodes_by_id.get(&edge.to_node_id) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownEdgeTargetNode,
                path: format!("{edge_path}.to_node_id"),
                message: format!("target node '{}' does not exist", edge.to_node_id),
            });
            continue;
        };

        let Some(from_definition) = node_definition(from_node.node_type.as_str()) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownNodeType,
                path: format!("{edge_path}.from_node_id"),
                message: format!(
                    "source node '{}' has unknown node type '{}'",
                    from_node.id,
                    from_node.node_type.as_str()
                ),
            });
            continue;
        };
        let Some(to_definition) = node_definition(to_node.node_type.as_str()) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownNodeType,
                path: format!("{edge_path}.to_node_id"),
                message: format!(
                    "target node '{}' has unknown node type '{}'",
                    to_node.id,
                    to_node.node_type.as_str()
                ),
            });
            continue;
        };

        let Some(from_output) = from_definition.output_port(&edge.from_output_name) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownEdgeSourceOutput,
                path: format!("{edge_path}.from_output_name"),
                message: format!(
                    "output '{}' is not declared by node type '{}'",
                    edge.from_output_name, from_definition.id
                ),
            });
            continue;
        };
        let Some(to_input) = to_definition.input_port(&edge.to_input_name) else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownEdgeTargetInput,
                path: format!("{edge_path}.to_input_name"),
                message: format!(
                    "input '{}' is not declared by node type '{}'",
                    edge.to_input_name, to_definition.id
                ),
            });
            continue;
        };

        let inferred_output = infer_graph_output(
            document,
            &nodes_by_id,
            &edge.from_node_id,
            &edge.from_output_name,
        );
        let resolved_output_kind = inferred_output.kind().unwrap_or(from_output.value_kind);

        if !to_input.accepts_kind(resolved_output_kind)
            || !from_output.accepts_kind(resolved_output_kind)
            || inferred_output.message().is_some()
        {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::EdgeTypeMismatch,
                path: edge_path,
                message: inferred_output
                    .message()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        format!(
                            "edge type mismatch: output '{}' is {:?} but input '{}' expects {:?}",
                            edge.from_output_name,
                            resolved_output_kind,
                            edge.to_input_name,
                            to_input.value_kind
                        )
                    }),
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphEdge, GraphMetadata, NodeMetadata, NodeParameter, NodeViewport};
    use crate::{GraphDocument, GraphNode, InputValue, NodeInputValue, NodePosition, NodeTypeId};

    fn viewport() -> NodeViewport {
        NodeViewport {
            position: NodePosition { x: 0.0, y: 0.0 },
            collapsed: false,
        }
    }

    fn node(id: &str, node_type: &str) -> GraphNode {
        GraphNode {
            id: id.to_owned(),
            metadata: NodeMetadata {
                name: id.to_owned(),
            },
            node_type: crate::NodeTypeId::new(node_type),
            viewport: viewport(),
            input_values: Vec::new(),
            parameters: Vec::<NodeParameter>::new(),
        }
    }

    #[test]
    fn validation_accepts_tensor_inferred_min_max_to_laplacian_edge() {
        let mut extract = node("extract", NodeTypeId::EXTRACT_CHANNELS);
        extract.input_values.push(NodeInputValue {
            name: "frame".to_owned(),
            value: InputValue::ColorFrame(crate::ColorFrame {
                layout: crate::LedLayout {
                    id: "panel".to_owned(),
                    pixel_count: 1,
                    width: Some(1),
                    height: Some(1),
                },
                pixels: vec![crate::RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }],
            }),
        });
        let document = GraphDocument {
            metadata: GraphMetadata {
                id: "graph".to_owned(),
                name: "Graph".to_owned(),
                execution_frequency_hz: 60,
            },
            viewport: crate::GraphViewport::default(),
            nodes: vec![
                extract,
                node("min_max", NodeTypeId::MIN_MAX),
                node("laplacian", NodeTypeId::LAPLACIAN_FILTER),
            ],
            edges: vec![
                GraphEdge {
                    from_node_id: "extract".to_owned(),
                    from_output_name: "tensor".to_owned(),
                    to_node_id: "min_max".to_owned(),
                    to_input_name: "a".to_owned(),
                },
                GraphEdge {
                    from_node_id: "min_max".to_owned(),
                    from_output_name: "value".to_owned(),
                    to_node_id: "laplacian".to_owned(),
                    to_input_name: "frame".to_owned(),
                },
            ],
        };

        let issues = validate_graph_document(&document);
        assert!(
            !issues
                .iter()
                .any(|issue| issue.code == GraphValidationIssueCode::EdgeTypeMismatch)
        );
    }

    #[test]
    fn validation_rejects_float_min_max_to_laplacian_edge() {
        let document = GraphDocument {
            metadata: GraphMetadata {
                id: "graph".to_owned(),
                name: "Graph".to_owned(),
                execution_frequency_hz: 60,
            },
            viewport: crate::GraphViewport::default(),
            nodes: vec![
                node("float", NodeTypeId::FLOAT_CONSTANT),
                node("min_max", NodeTypeId::MIN_MAX),
                node("laplacian", NodeTypeId::LAPLACIAN_FILTER),
            ],
            edges: vec![
                GraphEdge {
                    from_node_id: "float".to_owned(),
                    from_output_name: "value".to_owned(),
                    to_node_id: "min_max".to_owned(),
                    to_input_name: "a".to_owned(),
                },
                GraphEdge {
                    from_node_id: "min_max".to_owned(),
                    from_output_name: "value".to_owned(),
                    to_node_id: "laplacian".to_owned(),
                    to_input_name: "frame".to_owned(),
                },
            ],
        };

        let issues = validate_graph_document(&document);
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == GraphValidationIssueCode::EdgeTypeMismatch)
        );
    }

    #[test]
    fn validation_rejects_binary_select_with_mismatched_branch_kinds() {
        let mut extract = node("extract", NodeTypeId::EXTRACT_CHANNELS);
        extract.input_values.push(NodeInputValue {
            name: "frame".to_owned(),
            value: InputValue::ColorFrame(crate::ColorFrame {
                layout: crate::LedLayout {
                    id: "panel".to_owned(),
                    pixel_count: 1,
                    width: Some(1),
                    height: Some(1),
                },
                pixels: vec![crate::RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }],
            }),
        });
        let document = GraphDocument {
            metadata: GraphMetadata {
                id: "graph".to_owned(),
                name: "Graph".to_owned(),
                execution_frequency_hz: 60,
            },
            viewport: crate::GraphViewport::default(),
            nodes: vec![
                extract,
                node("float", NodeTypeId::FLOAT_CONSTANT),
                node("select", NodeTypeId::BINARY_SELECT),
                node("display", NodeTypeId::DISPLAY),
            ],
            edges: vec![
                GraphEdge {
                    from_node_id: "extract".to_owned(),
                    from_output_name: "tensor".to_owned(),
                    to_node_id: "select".to_owned(),
                    to_input_name: "a".to_owned(),
                },
                GraphEdge {
                    from_node_id: "float".to_owned(),
                    from_output_name: "value".to_owned(),
                    to_node_id: "select".to_owned(),
                    to_input_name: "b".to_owned(),
                },
                GraphEdge {
                    from_node_id: "select".to_owned(),
                    from_output_name: "value".to_owned(),
                    to_node_id: "display".to_owned(),
                    to_input_name: "value".to_owned(),
                },
            ],
        };

        let issues = validate_graph_document(&document);
        assert!(issues.iter().any(|issue| {
            issue.code == GraphValidationIssueCode::EdgeTypeMismatch
                && issue.message.contains("must resolve to the same kind")
        }));
    }
}

pub fn infer_graph_output_kind(
    document: &GraphDocument,
    nodes_by_id: &HashMap<String, GraphNode>,
    node_id: &str,
    output_name: &str,
) -> Option<ValueKind> {
    infer_graph_output(document, nodes_by_id, node_id, output_name).kind()
}

pub fn infer_graph_output(
    document: &GraphDocument,
    nodes_by_id: &HashMap<String, GraphNode>,
    node_id: &str,
    output_name: &str,
) -> crate::OutputInference {
    let mut visiting = HashSet::new();
    infer_graph_output_inner(document, nodes_by_id, node_id, output_name, &mut visiting)
}

fn infer_graph_output_inner(
    document: &GraphDocument,
    nodes_by_id: &HashMap<String, GraphNode>,
    node_id: &str,
    output_name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> crate::OutputInference {
    let visit_key = (node_id.to_owned(), output_name.to_owned());
    if !visiting.insert(visit_key.clone()) {
        let Some(node) = nodes_by_id.get(node_id) else {
            return crate::OutputInference::Unavailable;
        };
        let Some(definition) = node_definition(node.node_type.as_str()) else {
            return crate::OutputInference::Unavailable;
        };
        return definition
            .output_port(output_name)
            .map(|output| crate::OutputInference::Resolved(output.value_kind))
            .unwrap_or(crate::OutputInference::Unavailable);
    }

    let result = infer_graph_output_uncached(document, nodes_by_id, node_id, output_name, visiting);
    visiting.remove(&visit_key);
    result
}

fn infer_graph_output_uncached(
    document: &GraphDocument,
    nodes_by_id: &HashMap<String, GraphNode>,
    node_id: &str,
    output_name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> crate::OutputInference {
    let Some(node) = nodes_by_id.get(node_id) else {
        return crate::OutputInference::Unavailable;
    };
    let Some(definition) = node_definition(node.node_type.as_str()) else {
        return crate::OutputInference::Unavailable;
    };

    let input_kinds = definition
        .inputs
        .iter()
        .map(|input| {
            let connected_kind =
                infer_connected_input_kind(document, nodes_by_id, node_id, &input.name, visiting)
                    .kind();
            let inline_kind = node
                .input_values
                .iter()
                .find(|value| value.name == input.name)
                .map(|value| value.value.value_kind())
                .or_else(|| input.default_value.as_ref().map(|value| value.value_kind()))
                .unwrap_or(input.value_kind);
            (input.name.as_str(), connected_kind.unwrap_or(inline_kind))
        })
        .collect::<Vec<_>>();

    definition.infer_output(output_name, &input_kinds, &node.parameters)
}

fn infer_connected_input_kind(
    document: &GraphDocument,
    nodes_by_id: &HashMap<String, GraphNode>,
    node_id: &str,
    input_name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> crate::OutputInference {
    let edge = document
        .edges
        .iter()
        .find(|edge| edge.to_node_id == node_id && edge.to_input_name == input_name);
    let Some(edge) = edge else {
        return crate::OutputInference::Unavailable;
    };

    infer_graph_output_inner(
        document,
        nodes_by_id,
        &edge.from_node_id,
        &edge.from_output_name,
        visiting,
    )
}
