use std::collections::{HashMap, HashSet};

use crate::graph::{GraphDocument, builtin_node_definition};

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

/// Validates a graph document against the shared built-in node catalog.
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
        nodes_by_id.insert(node.id.clone(), node);

        let Some(definition) = builtin_node_definition(node.node_type.as_str()) else {
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

        let Some(from_node) = nodes_by_id.get(&edge.from_node_id).copied() else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownEdgeSourceNode,
                path: format!("{edge_path}.from_node_id"),
                message: format!("source node '{}' does not exist", edge.from_node_id),
            });
            continue;
        };
        let Some(to_node) = nodes_by_id.get(&edge.to_node_id).copied() else {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::UnknownEdgeTargetNode,
                path: format!("{edge_path}.to_node_id"),
                message: format!("target node '{}' does not exist", edge.to_node_id),
            });
            continue;
        };

        let Some(from_definition) = builtin_node_definition(from_node.node_type.as_str()) else {
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
        let Some(to_definition) = builtin_node_definition(to_node.node_type.as_str()) else {
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

        if !to_definition.can_connect_ports(from_output, to_input) {
            issues.push(GraphValidationIssue {
                code: GraphValidationIssueCode::EdgeTypeMismatch,
                path: edge_path,
                message: format!(
                    "edge type mismatch: output '{}' is {:?} but input '{}' expects {:?}",
                    edge.from_output_name,
                    from_output.value_kind,
                    edge.to_input_name,
                    to_input.value_kind
                ),
            });
        }
    }

    issues
}
