use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use shared::{GraphDocument, GraphNode, NodeTypeId};

use crate::node_runtime::NodeRegistry;
use crate::services::runtime::planner::plan_render_contexts;
use crate::services::runtime::types::{CompiledGraph, CompiledIncomingEdge, CompiledNode};

/// Compiles a persisted graph document into the runtime representation executed by the engine.
///
/// Compilation resolves node implementations, seeds input and parameter defaults, validates edge
/// compatibility, computes execution order, and derives render-context planning data.
pub(crate) fn compile_graph_document(
    document: GraphDocument,
    node_registry: Arc<NodeRegistry>,
) -> anyhow::Result<CompiledGraph> {
    tracing::debug!(
        graph_id = %document.metadata.id,
        graph_name = %document.metadata.name,
        node_count = document.nodes.len(),
        edge_count = document.edges.len(),
        execution_frequency_hz = document.metadata.execution_frequency_hz,
        "compiling graph document"
    );
    let mut node_index_by_id = HashMap::<String, usize>::new();
    let mut nodes = Vec::new();

    for (index, node) in document.nodes.into_iter().enumerate() {
        node_index_by_id.insert(node.id.clone(), index);
        nodes.push(compile_node(node, node_registry.as_ref())?);
    }

    let mut incoming_edges_by_node = vec![Vec::new(); nodes.len()];
    let mut adjacency = vec![Vec::new(); nodes.len()];
    let mut in_degree = vec![0usize; nodes.len()];

    for edge in &document.edges {
        let Some(&from_node_index) = node_index_by_id.get(&edge.from_node_id) else {
            continue;
        };
        let Some(&to_node_index) = node_index_by_id.get(&edge.to_node_id) else {
            continue;
        };

        let from_node = &nodes[from_node_index];
        let to_node = &nodes[to_node_index];
        let Some(from_definition) = node_registry.definition(from_node.node_type.as_str()) else {
            continue;
        };
        let Some(to_definition) = node_registry.definition(to_node.node_type.as_str()) else {
            continue;
        };

        let Some(from_port) = from_definition.output_port(&edge.from_output_name) else {
            continue;
        };
        let Some(to_port) = to_definition.input_port(&edge.to_input_name) else {
            continue;
        };
        if !to_definition.can_connect_ports(from_port, to_port) {
            continue;
        }

        incoming_edges_by_node[to_node_index].push(CompiledIncomingEdge {
            from_node_index,
            from_output_name: edge.from_output_name.clone(),
            to_input_name: edge.to_input_name.clone(),
            use_previous_tick: from_node.node_type.as_str() == NodeTypeId::DELAY,
        });
        if from_node.node_type.as_str() != NodeTypeId::DELAY {
            adjacency[from_node_index].push(to_node_index);
            in_degree[to_node_index] += 1;
        }
    }

    let topological_order = topological_order(&adjacency, &in_degree)?;
    let render_contexts_by_node = plan_render_contexts(&nodes, &incoming_edges_by_node);

    tracing::debug!(
        graph_id = %document.metadata.id,
        compiled_node_count = nodes.len(),
        topological_order = ?topological_order,
        "compiled graph document"
    );

    Ok(CompiledGraph {
        execution_frequency_hz: document.metadata.execution_frequency_hz,
        node_registry,
        nodes,
        incoming_edges_by_node,
        topological_order,
        render_contexts_by_node,
    })
}

/// Compiles a single persisted graph node into its runtime form.
///
/// This resolves schema defaults, validates that a runtime evaluator exists for the chosen node
/// type and parameters, and captures any construction-time diagnostics emitted by the node.
fn compile_node(node: GraphNode, node_registry: &NodeRegistry) -> anyhow::Result<CompiledNode> {
    tracing::trace!(
        node_id = %node.id,
        node_type = %node.node_type.as_str(),
        input_value_count = node.input_values.len(),
        parameter_count = node.parameters.len(),
        "compiling node"
    );
    let mut parameters = HashMap::new();
    for parameter in node.parameters {
        parameters.insert(parameter.name, parameter.value);
    }

    let Some(definition) = node_registry.definition(node.node_type.as_str()) else {
        return Err(anyhow::anyhow!(
            "Unknown node type {}",
            node.node_type.as_str()
        ));
    };

    let mut input_defaults = HashMap::new();
    for input_definition in &definition.inputs {
        if let Some(value) = input_definition.default_value.clone() {
            input_defaults.insert(input_definition.name.clone(), value);
        }
    }
    for input in node.input_values {
        input_defaults.insert(input.name, input.value);
    }

    for parameter_definition in &definition.parameters {
        parameters
            .entry(parameter_definition.name.clone())
            .or_insert_with(|| parameter_definition.default_value.to_json_value());
    }

    let allowed_runtime_update_names = definition
        .runtime_updates
        .as_ref()
        .map(|updates| {
            updates
                .values
                .iter()
                .map(|value| value.name.clone())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if node_registry
        .evaluator_for(node.node_type.as_str(), &parameters)
        .is_none()
    {
        return Err(anyhow::anyhow!(
            "Node {} has unsupported runtime type {}",
            node.id,
            node.node_type.as_str()
        ));
    }
    let construction_diagnostics = node_registry
        .construction_diagnostics_for(node.node_type.as_str(), &parameters)
        .unwrap_or_default();

    Ok(CompiledNode {
        id: node.id,
        node_type: node.node_type,
        input_defaults,
        parameters,
        construction_diagnostics,
        allowed_runtime_update_names,
    })
}

/// Computes a topological execution order for the compiled graph.
///
/// Delay edges are excluded from the dependency graph earlier in compilation, so remaining cycles
/// are considered invalid and result in an error here.
fn topological_order(adjacency: &[Vec<usize>], in_degree: &[usize]) -> anyhow::Result<Vec<usize>> {
    let mut in_degree = in_degree.to_vec();
    let mut queue = VecDeque::new();
    for (index, degree) in in_degree.iter().enumerate() {
        if *degree == 0 {
            queue.push_back(index);
        }
    }

    let mut order = Vec::new();
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for &next in &adjacency[node] {
            in_degree[next] -= 1;
            if in_degree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    anyhow::ensure!(
        order.len() == adjacency.len(),
        "Graph contains at least one cycle"
    );
    Ok(order)
}

#[cfg(test)]
mod tests {
    use crate::node_runtime::build_builtin_node_registry;
    use crate::services::runtime::compiler::compile_graph_document;

    /// Tests that parameter-normalization diagnostics are preserved on compiled nodes.
    #[test]
    fn construction_diagnostics_are_collected_for_parameter_adjustments() {
        let document: shared::GraphDocument = serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "construction-diagnostics",
                "name": "construction diagnostics",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "bouncing_balls_1",
                    "metadata": { "name": "bouncing_balls_1" },
                    "node_type": shared::NodeTypeId::BOUNCING_BALLS,
                    "parameters": [
                        {
                            "name": "circle_count",
                            "value": 200
                        }
                    ]
                }
            ],
            "edges": []
        }))
        .expect("parse graph");

        let node_registry = build_builtin_node_registry().expect("build builtin node registry");
        let compiled = compile_graph_document(document, node_registry).expect("compile graph");
        let diagnostics = &compiled.nodes[0].construction_diagnostics;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("parameter_clamped.circle_count")
        );
        assert_eq!(
            diagnostics[0].severity,
            shared::NodeDiagnosticSeverity::Warning
        );
        assert!(
            diagnostics[0]
                .message
                .contains("Parameter value 200 was clamped to 64.")
        );
    }

    /// Tests that compilation reports an explicit error when no runtime implementation exists.
    #[test]
    fn unknown_node_types_fail_cleanly() {
        let document: shared::GraphDocument = serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "unknown-node",
                "name": "unknown node",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "custom_1",
                    "metadata": { "name": "custom_1" },
                    "node_type": "custom.missing"
                }
            ],
            "edges": []
        }))
        .expect("parse unknown node graph");

        let node_registry = build_builtin_node_registry().expect("build builtin node registry");
        let error = match compile_graph_document(document, node_registry) {
            Ok(_) => panic!("unknown node type should fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("Unknown node type"));
    }
}
