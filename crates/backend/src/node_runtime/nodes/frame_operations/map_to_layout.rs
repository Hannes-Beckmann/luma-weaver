use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;
use shared::{
    ColorFrame, InputValue, LedLayout, LedLayoutRole, NodeDiagnostic, NodeDiagnosticSeverity,
};

use crate::node_runtime::{
    NodeConstruction, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    RuntimeOutputs, TypedNodeEvaluation,
};
use crate::spatial_layout::{SpatialPlacement, matrix_points};

#[derive(Default)]
pub(crate) struct MapToLayoutNode {
    width: usize,
    height: usize,
    use_spatial: bool,
    placement: SpatialPlacement,
}

#[derive(Default)]
struct MapToLayoutParameters {
    width: usize,
    height: usize,
    use_spatial: bool,
}

crate::node_runtime::impl_runtime_parameters!(MapToLayoutParameters {
    width: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 8usize,
    height: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 8usize,
    use_spatial: bool = false,
});

impl RuntimeNodeFromParameters for MapToLayoutNode {
    fn from_parameters(parameters: &HashMap<String, JsonValue>) -> NodeConstruction<Self> {
        let crate::node_runtime::NodeConstruction {
            node: config,
            diagnostics,
        } = MapToLayoutParameters::from_parameters(parameters);

        NodeConstruction {
            node: Self {
                width: config.width,
                height: config.height,
                use_spatial: config.use_spatial,
                placement: SpatialPlacement::from_parameters(parameters),
            },
            diagnostics,
        }
    }
}

pub(crate) struct MapToLayoutInputs {
    frame: Option<ColorFrame>,
}

crate::node_runtime::impl_runtime_inputs!(MapToLayoutInputs {
    frame = None,
});

pub(crate) struct MapToLayoutOutputs {
    frame: Option<InputValue>,
}

impl RuntimeOutputs for MapToLayoutOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<HashMap<String, InputValue>> {
        let mut outputs = HashMap::new();
        if let Some(frame) = self.frame {
            outputs.insert("frame".to_owned(), frame);
        }
        Ok(outputs)
    }
}

impl RuntimeNode for MapToLayoutNode {
    type Inputs = MapToLayoutInputs;
    type Outputs = MapToLayoutOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(mut frame) = inputs.frame else {
            return Ok(TypedNodeEvaluation::from_outputs(MapToLayoutOutputs {
                frame: None,
            }));
        };

        let layout = mapped_layout(
            context.render_layout.as_ref(),
            self.width,
            self.height,
            self.use_spatial,
            self.placement,
        );
        if frame.pixels.len() != layout.pixel_count {
            return Ok(TypedNodeEvaluation {
                outputs: MapToLayoutOutputs { frame: None },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Warning,
                    code: Some("map_to_layout_pixel_count_mismatch".to_owned()),
                    message: format!(
                        "Map To Layout expected {} pixels from the render context, but received {}.",
                        layout.pixel_count,
                        frame.pixels.len()
                    ),
                }],
            });
        }

        frame.layout = layout;
        Ok(TypedNodeEvaluation::from_outputs(MapToLayoutOutputs {
            frame: Some(InputValue::MappedFrame(frame)),
        }))
    }
}

fn mapped_layout(
    render_layout: Option<&LedLayout>,
    width: usize,
    height: usize,
    use_spatial: bool,
    placement: SpatialPlacement,
) -> LedLayout {
    match render_layout {
        Some(layout) => LedLayout {
            id: format!("mapped:{}", layout.id),
            role: LedLayoutRole::Source,
            pixel_count: layout.pixel_count,
            width: layout.width,
            height: layout.height,
            points_3d: layout.points_3d.clone(),
        },
        None => LedLayout {
            id: format!("mapped:{}x{}", width, height),
            role: LedLayoutRole::Source,
            pixel_count: width * height,
            width: Some(width),
            height: Some(height),
            points_3d: use_spatial.then(|| matrix_points(width, height, placement)),
        },
    }
}

#[cfg(test)]
mod tests {
    use shared::{InputValue, LedLayoutRole, RgbaColor};

    use super::{MapToLayoutInputs, MapToLayoutNode, mapped_layout};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};
    use crate::spatial_layout::SpatialPlacement;

    fn render_target_layout(width: usize, height: usize) -> shared::LedLayout {
        shared::LedLayout {
            id: format!("render:{width}x{height}"),
            role: LedLayoutRole::RenderTarget,
            pixel_count: width * height,
            width: Some(width),
            height: Some(height),
            points_3d: None,
        }
    }

    fn test_frame(width: usize, height: usize) -> shared::ColorFrame {
        shared::ColorFrame {
            layout: render_target_layout(width, height),
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.25,
                    b: 0.5,
                    a: 1.0,
                };
                width * height
            ],
        }
    }

    #[test]
    fn converts_render_target_frame_to_mapped_frame() {
        let mut node = MapToLayoutNode {
            width: 8,
            height: 8,
            use_spatial: false,
            placement: SpatialPlacement::default(),
        };

        let evaluation = node
            .evaluate(
                &NodeEvaluationContext {
                    graph_id: "graph".to_owned(),
                    graph_name: "Graph".to_owned(),
                    elapsed_seconds: 0.0,
                    render_layout: Some(render_target_layout(8, 8)),
                },
                MapToLayoutInputs {
                    frame: Some(test_frame(8, 8)),
                },
            )
            .expect("evaluate map to layout");

        let mapped = match evaluation.outputs.frame {
            Some(InputValue::MappedFrame(frame)) => frame,
            other => panic!("expected mapped frame output, got {other:?}"),
        };

        assert_eq!(mapped.layout.role, LedLayoutRole::Source);
        assert_eq!(mapped.layout.width, Some(8));
        assert_eq!(mapped.layout.height, Some(8));
        assert_eq!(mapped.pixels.len(), 64);
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn falls_back_to_parameter_layout_when_context_is_missing() {
        let layout = mapped_layout(None, 4, 2, false, SpatialPlacement::default());

        assert_eq!(layout.role, LedLayoutRole::Source);
        assert_eq!(layout.pixel_count, 8);
        assert_eq!(layout.width, Some(4));
        assert_eq!(layout.height, Some(2));
    }

    #[test]
    fn emits_warning_when_input_pixel_count_does_not_match_render_context() {
        let mut node = MapToLayoutNode {
            width: 8,
            height: 8,
            use_spatial: false,
            placement: SpatialPlacement::default(),
        };

        let evaluation = node
            .evaluate(
                &NodeEvaluationContext {
                    graph_id: "graph".to_owned(),
                    graph_name: "Graph".to_owned(),
                    elapsed_seconds: 0.0,
                    render_layout: Some(render_target_layout(8, 8)),
                },
                MapToLayoutInputs {
                    frame: Some(test_frame(2, 2)),
                },
            )
            .expect("evaluate mismatch");

        assert!(evaluation.outputs.frame.is_none());
        assert_eq!(evaluation.diagnostics.len(), 1);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("map_to_layout_pixel_count_mismatch")
        );
    }
}
