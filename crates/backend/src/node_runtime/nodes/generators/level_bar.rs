use anyhow::Result;
use shared::{
    ColorFrame, ColorGradient, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

pub(crate) struct LevelBarNode {
    gradient: ColorGradient,
}

impl Default for LevelBarNode {
    fn default() -> Self {
        Self {
            gradient: default_gradient(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(LevelBarNode {
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
    ..Self::default()
});

pub(crate) struct LevelBarInputs {
    loudness: f32,
}

crate::node_runtime::impl_runtime_inputs!(LevelBarInputs {
    loudness = 0.0,
});

pub(crate) struct LevelBarOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(LevelBarOutputs { frame });

impl RuntimeNode for LevelBarNode {
    type Inputs = LevelBarInputs;
    type Outputs = LevelBarOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation {
                outputs: LevelBarOutputs {
                    frame: ColorFrame {
                        layout: LedLayout {
                            id: "level_bar:unbound".to_owned(),
                            pixel_count: 0,
                            width: None,
                            height: None,
                        },
                        pixels: Vec::new(),
                    },
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Warning,
                    code: Some("level_bar_missing_render_layout".to_owned()),
                    message: "Level Bar has no render layout, so it cannot render a frame yet."
                        .to_owned(),
                }],
            });
        };

        let loudness = inputs.loudness.clamp(0.0, 1.0);
        let mut diagnostics = Vec::new();
        if (loudness - inputs.loudness).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("level_bar_loudness_clamped".to_owned()),
                message: format!(
                    "Level Bar loudness {} is out of range; using {} instead.",
                    inputs.loudness, loudness
                ),
            });
        }

        let frame = render_level_bar_frame(&layout, loudness, &self.gradient);

        Ok(TypedNodeEvaluation {
            outputs: LevelBarOutputs { frame },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

fn render_level_bar_frame(
    layout: &LedLayout,
    loudness: f32,
    gradient: &ColorGradient,
) -> ColorFrame {
    let (width, height) = layout_dims(layout);
    let mut pixels = Vec::with_capacity(layout.pixel_count);

    let raw_extent = loudness * width as f32 * 1.2;
    let full_columns = raw_extent.floor() as usize;
    let remainder = (raw_extent - full_columns as f32).clamp(0.0, 1.0);
    let partial_column = if full_columns < width {
        Some(full_columns)
    } else {
        None
    };
    let last_full_column = full_columns.min(width.saturating_sub(1));

    for y in 0..height {
        for x in 0..width {
            let position = if width <= 1 {
                0.0
            } else {
                x as f32 / (width - 1) as f32
            };
            let gradient_color = sample_gradient_hsv(gradient, position);
            let brightness = if x < last_full_column {
                1.0
            } else if x == 0 && loudness > 0.0 && width == 1 {
                1.0
            } else if partial_column == Some(x) {
                remainder
            } else if full_columns >= width && width > 0 {
                1.0
            } else {
                0.0
            };
            let color = RgbaColor {
                r: (gradient_color.r * brightness).clamp(0.0, 1.0),
                g: (gradient_color.g * brightness).clamp(0.0, 1.0),
                b: (gradient_color.b * brightness).clamp(0.0, 1.0),
                a: (gradient_color.a * brightness).clamp(0.0, 1.0),
            };

            pixels.push(color);
            if pixels.len() == layout.pixel_count {
                return ColorFrame {
                    layout: layout.clone(),
                    pixels,
                };
            }
        }
        if y + 1 == height && pixels.len() < layout.pixel_count {
            pixels.extend(std::iter::repeat_n(
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                },
                layout.pixel_count - pixels.len(),
            ));
        }
    }

    ColorFrame {
        layout: layout.clone(),
        pixels,
    }
}

fn layout_dims(layout: &LedLayout) -> (usize, usize) {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => (layout.pixel_count.max(1), 1),
    }
}

fn default_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            stop(0.0, 1.0, 0.0, 0.0),
            stop(0.2, 1.0, 0.5, 0.0),
            stop(0.4, 1.0, 1.0, 0.0),
            stop(0.6, 0.0, 1.0, 0.0),
            stop(0.8, 0.0, 0.4, 1.0),
            stop(1.0, 0.7, 0.0, 1.0),
        ],
    }
}

fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}
