use std::collections::{HashMap, VecDeque};

use shared::{LedLayout, NodeTypeId, RenderLayoutKind, Vec3};

use crate::services::runtime::types::{CompiledIncomingEdge, CompiledNode, RenderContext};
use crate::spatial_layout::{
    MatrixStripMode, SpatialTransform, spatial_layout_dimensions, spatial_layout_pixel_count,
    spatial_points_for_mode,
};

/// Plans the render contexts that each compiled node should evaluate in.
///
/// Contexts are first propagated backward from sink nodes and then forward-filled into downstream
/// observer branches so sibling viewers inherit the same render context as the render-producing
/// branch they inspect.
pub(crate) fn plan_render_contexts(
    nodes: &[CompiledNode],
    incoming_edges_by_node: &[Vec<CompiledIncomingEdge>],
    layout_assets: &HashMap<String, Vec<Vec3>>,
) -> Vec<Vec<RenderContext>> {
    tracing::debug!(node_count = nodes.len(), "planning render contexts");

    let mut by_node = backpropagate_from_sinks(nodes, incoming_edges_by_node, layout_assets);
    forward_fill_from_inputs(incoming_edges_by_node, &mut by_node);

    for (node_index, planned_contexts) in by_node.iter().enumerate() {
        tracing::debug!(
            node_id = %nodes[node_index].id,
            node_type = %nodes[node_index].node_type.as_str(),
            context_ids = ?planned_contexts.iter().map(|ctx| ctx.id.as_str()).collect::<Vec<_>>(),
            "planned node render contexts"
        );
    }

    by_node
}

/// Propagates render contexts backward from sink nodes through their upstream dependencies.
///
/// Each sink contributes one context, and a node may accumulate multiple contexts when it feeds
/// multiple sinks.
fn backpropagate_from_sinks(
    nodes: &[CompiledNode],
    incoming_edges_by_node: &[Vec<CompiledIncomingEdge>],
    layout_assets: &HashMap<String, Vec<Vec3>>,
) -> Vec<Vec<RenderContext>> {
    let mut by_node = vec![Vec::<RenderContext>::new(); nodes.len()];
    let mut queue = VecDeque::<(usize, RenderContext)>::new();

    for (node_index, node) in nodes.iter().enumerate() {
        if let Some(context) = sink_context_for_node(node, layout_assets) {
            queue.push_back((node_index, context));
        }
    }

    while let Some((node_index, context)) = queue.pop_front() {
        if contains_context(&by_node[node_index], &context.id) {
            continue;
        }
        tracing::trace!(
            node_id = %nodes[node_index].id,
            node_type = %nodes[node_index].node_type.as_str(),
            context_id = %context.id,
            layout_pixels = context.layout.pixel_count,
            layout_width = ?context.layout.width,
            layout_height = ?context.layout.height,
            "assigned render context to node during sink backpropagation"
        );
        by_node[node_index].push(context.clone());
        for incoming in &incoming_edges_by_node[node_index] {
            if !incoming.participates_in_render_context {
                continue;
            }
            queue.push_back((
                incoming.from_node_index,
                context_for_upstream_node(&nodes[node_index], &context),
            ));
        }
    }

    for contexts in &mut by_node {
        sort_and_dedup_contexts(contexts);
    }

    by_node
}

/// Propagates render contexts forward into nodes that were not reached by sink backpropagation.
///
/// This allows observer-style nodes to inherit the contexts of their upstream producers even when
/// they do not themselves lead to a sink.
fn forward_fill_from_inputs(
    incoming_edges_by_node: &[Vec<CompiledIncomingEdge>],
    by_node: &mut [Vec<RenderContext>],
) {
    let outgoing_edges_by_node = outgoing_edges_by_node(incoming_edges_by_node, by_node.len());
    let topo_order = topological_order(&outgoing_edges_by_node);

    for node_index in topo_order {
        if !by_node[node_index].is_empty() {
            continue;
        }

        let mut inherited = Vec::new();
        for incoming in &incoming_edges_by_node[node_index] {
            inherited.extend(by_node[incoming.from_node_index].iter().cloned());
        }
        sort_and_dedup_contexts(&mut inherited);
        if !inherited.is_empty() {
            tracing::trace!(
                node_index,
                context_ids = ?inherited.iter().map(|ctx| ctx.id.as_str()).collect::<Vec<_>>(),
                "forward-filled render contexts from upstream nodes"
            );
            by_node[node_index] = inherited;
        }
    }
}

/// Builds the outgoing-edge adjacency list for each compiled node.
fn outgoing_edges_by_node(
    incoming_edges_by_node: &[Vec<CompiledIncomingEdge>],
    node_count: usize,
) -> Vec<Vec<usize>> {
    let mut outgoing = vec![Vec::new(); node_count];
    for (to_node_index, incoming_edges) in incoming_edges_by_node.iter().enumerate() {
        for incoming in incoming_edges {
            outgoing[incoming.from_node_index].push(to_node_index);
        }
    }
    outgoing
}

/// Returns a topological node order for the outgoing-edge graph.
///
/// When the graph is cyclic, this falls back to the natural node order.
fn topological_order(outgoing_edges_by_node: &[Vec<usize>]) -> Vec<usize> {
    let mut in_degree = vec![0usize; outgoing_edges_by_node.len()];
    for children in outgoing_edges_by_node {
        for &child in children {
            in_degree[child] += 1;
        }
    }

    let mut queue = VecDeque::new();
    for (index, degree) in in_degree.iter().enumerate() {
        if *degree == 0 {
            queue.push_back(index);
        }
    }

    let mut order = Vec::new();
    while let Some(node_index) = queue.pop_front() {
        order.push(node_index);
        for &child in &outgoing_edges_by_node[node_index] {
            in_degree[child] -= 1;
            if in_degree[child] == 0 {
                queue.push_back(child);
            }
        }
    }

    if order.len() == outgoing_edges_by_node.len() {
        order
    } else {
        (0..outgoing_edges_by_node.len()).collect()
    }
}

/// Returns whether `contexts` already contains `context_id`.
fn contains_context(contexts: &[RenderContext], context_id: &str) -> bool {
    contexts.iter().any(|known| known.id == context_id)
}

fn context_for_upstream_node(node: &CompiledNode, context: &RenderContext) -> RenderContext {
    if node.node_type.as_str() != NodeTypeId::TRANSFORM {
        return context.clone();
    }

    let transform = SpatialTransform::from_parameters(&node.parameters);
    let mut derived = context.clone();
    derived.id = format!("{}|transform:{}", context.id, node.id);
    derived.layout.id = format!("{}|transform:{}", context.layout.id, node.id);
    if let Some(points) = derived.layout.points_3d.as_mut() {
        for point in points.iter_mut() {
            *point = transform.inverse_transform_point(*point);
        }
    }
    derived
}

/// Sorts render contexts by ID and removes duplicate entries.
fn sort_and_dedup_contexts(contexts: &mut Vec<RenderContext>) {
    contexts.sort_by(|a, b| a.id.cmp(&b.id));
    contexts.dedup_by(|a, b| a.id == b.id);
}

/// Returns the sink-owned render context for a compiled node, if it is a sink node.
///
/// Sink contexts encode the LED layout that upstream render nodes should produce for.
fn sink_context_for_node(
    node: &CompiledNode,
    layout_assets: &HashMap<String, Vec<Vec3>>,
) -> Option<RenderContext> {
    match node.node_type.as_str() {
        NodeTypeId::WLED_TARGET => {
            let target = node
                .parameters
                .get("target")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim();
            let led_count = node
                .parameters
                .get("led_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(60)
                .max(1) as usize;
            let use_spatial = bool_parameter(node, "use_spatial");
            let id = if target.is_empty() {
                format!("sink:wled:{}", node.id)
            } else {
                format!("sink:wled:{target}")
            };
            Some(RenderContext {
                id: id.clone(),
                layout: LedLayout {
                    id,
                    role: ::shared::LedLayoutRole::RenderTarget,
                    pixel_count: led_count,
                    width: Some(led_count),
                    height: Some(1),
                    points_3d: use_spatial.then(|| {
                        spatial_points_for_mode(
                            layout_assets,
                            &node.parameters,
                            "",
                            led_count,
                            MatrixStripMode::Strip,
                        )
                    }),
                },
                kind: if use_spatial {
                    RenderLayoutKind::Spatial3d
                } else {
                    RenderLayoutKind::Index1d
                },
            })
        }
        NodeTypeId::WLED_DUMMY_DISPLAY | NodeTypeId::MAP_TO_LAYOUT => {
            let width = node
                .parameters
                .get("width")
                .and_then(|value| value.as_u64())
                .unwrap_or(8)
                .max(1) as usize;
            let height = node
                .parameters
                .get("height")
                .and_then(|value| value.as_u64())
                .unwrap_or(8)
                .max(1) as usize;
            let use_spatial = bool_parameter(node, "use_spatial");
            let pixel_count =
                spatial_layout_pixel_count(layout_assets, &node.parameters, "", width, height, use_spatial);
            let (layout_width, layout_height) =
                spatial_layout_dimensions(layout_assets, &node.parameters, "", width, height, use_spatial);
            let id = if node.node_type.as_str() == NodeTypeId::MAP_TO_LAYOUT {
                format!("sink:map_to_layout:{}", node.id)
            } else {
                format!("sink:dummy:{}", node.id)
            };
            Some(RenderContext {
                id: id.clone(),
                layout: LedLayout {
                    id,
                    role: ::shared::LedLayoutRole::RenderTarget,
                    pixel_count,
                    width: layout_width,
                    height: layout_height,
                    points_3d: use_spatial.then(|| {
                        spatial_points_for_mode(
                            layout_assets,
                            &node.parameters,
                            "",
                            pixel_count,
                            MatrixStripMode::Auto { width, height },
                        )
                    }),
                },
                kind: if use_spatial {
                    RenderLayoutKind::Spatial3d
                } else if width > 1 && height > 1 {
                    RenderLayoutKind::Matrix2d
                } else {
                    RenderLayoutKind::Index1d
                },
            })
        }
        _ => None,
    }
}

fn bool_parameter(node: &CompiledNode, name: &str) -> bool {
    node.parameters
        .get(name)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value as JsonValue;
    use shared::{InputValue, NodeTypeId};

    use super::plan_render_contexts;
    use crate::services::runtime::types::{CompiledIncomingEdge, CompiledNode};

    /// Builds a compiled node for planner tests.
    fn compiled_node(id: &str, node_type: &str, parameters: &[(&str, JsonValue)]) -> CompiledNode {
        CompiledNode {
            id: id.to_owned(),
            display_name: id.to_owned(),
            node_type: shared::NodeTypeId::new(node_type),
            input_defaults: HashMap::<String, InputValue>::new(),
            parameters: parameters
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
            construction_diagnostics: Vec::new(),
            allowed_runtime_update_names: Default::default(),
        }
    }

    #[test]
    /// Tests that a display node inherits the render context of its upstream render node.
    fn display_inherits_context_from_upstream_render_node() {
        let nodes = vec![
            compiled_node("bouncing", NodeTypeId::BOUNCING_BALLS, &[]),
            compiled_node("display", NodeTypeId::DISPLAY, &[]),
            compiled_node(
                "dummy",
                NodeTypeId::WLED_DUMMY_DISPLAY,
                &[
                    ("width", JsonValue::from(8)),
                    ("height", JsonValue::from(8)),
                ],
            ),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert_eq!(planned[0].len(), 1);
        assert_eq!(planned[1].len(), 1);
        assert_eq!(planned[2].len(), 1);
        assert_eq!(planned[0][0].id, "sink:dummy:dummy");
        assert_eq!(planned[1][0].id, "sink:dummy:dummy");
        assert_eq!(planned[2][0].id, "sink:dummy:dummy");
    }

    #[test]
    /// Tests that separate sink branches preserve distinct render contexts.
    fn parallel_sinks_keep_distinct_contexts() {
        let nodes = vec![
            compiled_node("bouncing", NodeTypeId::BOUNCING_BALLS, &[]),
            compiled_node(
                "dummy_a",
                NodeTypeId::WLED_DUMMY_DISPLAY,
                &[
                    ("width", JsonValue::from(8)),
                    ("height", JsonValue::from(8)),
                ],
            ),
            compiled_node(
                "dummy_b",
                NodeTypeId::WLED_DUMMY_DISPLAY,
                &[
                    ("width", JsonValue::from(16)),
                    ("height", JsonValue::from(1)),
                ],
            ),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert_eq!(planned[0].len(), 2);
        assert_eq!(planned[1].len(), 1);
        assert_eq!(planned[2].len(), 1);
        assert_eq!(planned[1][0].id, "sink:dummy:dummy_a");
        assert_eq!(planned[2][0].id, "sink:dummy:dummy_b");
    }

    #[test]
    /// Tests that source-frame inputs do not pull sink render contexts upstream.
    fn fill_from_frame_source_input_does_not_backpropagate_context() {
        let nodes = vec![
            compiled_node("wled_source", NodeTypeId::WLED_SINK, &[]),
            compiled_node("fill", NodeTypeId::FILL_FROM_FRAME, &[]),
            compiled_node(
                "dummy",
                NodeTypeId::WLED_DUMMY_DISPLAY,
                &[
                    ("width", JsonValue::from(10)),
                    ("height", JsonValue::from(1)),
                    ("use_spatial", JsonValue::from(true)),
                ],
            ),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: false,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![CompiledIncomingEdge {
                from_node_index: 1,
                from_output_name: "frame".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert!(planned[0].is_empty());
        assert_eq!(planned[1].len(), 1);
        assert_eq!(planned[2].len(), 1);
        assert_eq!(planned[1][0].id, "sink:dummy:dummy");
        assert_eq!(planned[2][0].id, "sink:dummy:dummy");
    }

    #[test]
    fn map_to_layout_backpropagates_its_configured_render_context() {
        let nodes = vec![
            compiled_node("bouncing", NodeTypeId::BOUNCING_BALLS, &[]),
            compiled_node(
                "map",
                NodeTypeId::MAP_TO_LAYOUT,
                &[
                    ("width", JsonValue::from(12)),
                    ("height", JsonValue::from(1)),
                ],
            ),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert_eq!(planned[0].len(), 1);
        assert_eq!(planned[1].len(), 1);
        assert_eq!(planned[0][0].id, "sink:map_to_layout:map");
        assert_eq!(planned[1][0].layout.pixel_count, 12);
    }

    #[test]
    fn mapped_frame_output_does_not_backpropagate_context_downstream() {
        let nodes = vec![
            compiled_node("bouncing", NodeTypeId::BOUNCING_BALLS, &[]),
            compiled_node(
                "map",
                NodeTypeId::MAP_TO_LAYOUT,
                &[
                    ("width", JsonValue::from(8)),
                    ("height", JsonValue::from(8)),
                ],
            ),
            compiled_node("fill", NodeTypeId::FILL_FROM_FRAME, &[]),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![CompiledIncomingEdge {
                from_node_index: 1,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: false,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert_eq!(planned[0].len(), 1);
        assert_eq!(planned[1].len(), 1);
        assert_eq!(planned[2].len(), 1);
        assert_eq!(planned[0][0].id, "sink:map_to_layout:map");
        assert_eq!(planned[1][0].id, "sink:map_to_layout:map");
        assert_eq!(planned[2][0].id, "sink:map_to_layout:map");
    }

    #[test]
    fn transform_backpropagates_inverse_spatial_layout_and_derived_context_id() {
        let nodes = vec![
            compiled_node("solid", NodeTypeId::SOLID_FRAME, &[]),
            compiled_node(
                "transform",
                NodeTypeId::TRANSFORM,
                &[("translation_x", JsonValue::from(5.0))],
            ),
            compiled_node(
                "dummy",
                NodeTypeId::WLED_DUMMY_DISPLAY,
                &[
                    ("width", JsonValue::from(2)),
                    ("height", JsonValue::from(1)),
                    ("use_spatial", JsonValue::from(true)),
                ],
            ),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![CompiledIncomingEdge {
                from_node_index: 1,
                from_output_name: "frame".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert_eq!(planned[0].len(), 1);
        assert_eq!(planned[1].len(), 1);
        assert_eq!(planned[2].len(), 1);
        assert_eq!(planned[2][0].id, "sink:dummy:dummy");
        assert_eq!(planned[1][0].id, "sink:dummy:dummy");
        assert_eq!(planned[0][0].id, "sink:dummy:dummy|transform:transform");
        let points = planned[0][0]
            .layout
            .points_3d
            .as_ref()
            .expect("spatial points");
        assert!((points[0].x + 5.0).abs() < 1e-5);
        assert!((points[1].x + 4.0).abs() < 1e-5);
    }

    #[test]
    fn reconverging_transform_branches_keep_distinct_context_ids_upstream() {
        let nodes = vec![
            compiled_node("solid", NodeTypeId::SOLID_FRAME, &[]),
            compiled_node(
                "transform_left",
                NodeTypeId::TRANSFORM,
                &[("translation_x", JsonValue::from(-2.0))],
            ),
            compiled_node(
                "transform_right",
                NodeTypeId::TRANSFORM,
                &[("translation_x", JsonValue::from(2.0))],
            ),
            compiled_node("alpha", NodeTypeId::ALPHA_OVER, &[]),
            compiled_node(
                "dummy",
                NodeTypeId::WLED_DUMMY_DISPLAY,
                &[
                    ("width", JsonValue::from(2)),
                    ("height", JsonValue::from(1)),
                    ("use_spatial", JsonValue::from(true)),
                ],
            ),
        ];
        let incoming = vec![
            Vec::new(),
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![CompiledIncomingEdge {
                from_node_index: 0,
                from_output_name: "frame".to_owned(),
                to_input_name: "frame".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
            vec![
                CompiledIncomingEdge {
                    from_node_index: 1,
                    from_output_name: "frame".to_owned(),
                    to_input_name: "foreground".to_owned(),
                    participates_in_render_context: true,
                    source_context_suffix: None,
                    use_previous_tick: false,
                },
                CompiledIncomingEdge {
                    from_node_index: 2,
                    from_output_name: "frame".to_owned(),
                    to_input_name: "background".to_owned(),
                    participates_in_render_context: true,
                    source_context_suffix: None,
                    use_previous_tick: false,
                },
            ],
            vec![CompiledIncomingEdge {
                from_node_index: 3,
                from_output_name: "color".to_owned(),
                to_input_name: "value".to_owned(),
                participates_in_render_context: true,
                source_context_suffix: None,
                use_previous_tick: false,
            }],
        ];

        let planned = plan_render_contexts(&nodes, &incoming, &HashMap::new());

        assert_eq!(planned[0].len(), 2);
        assert_eq!(
            planned[0]
                .iter()
                .map(|context| context.id.clone())
                .collect::<Vec<_>>(),
            vec![
                "sink:dummy:dummy|transform:transform_left".to_owned(),
                "sink:dummy:dummy|transform:transform_right".to_owned(),
            ]
        );
    }
}
