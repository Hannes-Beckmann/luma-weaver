use anyhow::Result;
use shared::{
    ColorFrame, ColorGradient, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

pub(crate) struct PlasmaNode {
    phase: f32,
    last_elapsed_seconds: Option<f64>,
    gradient: ColorGradient,
}

impl Default for PlasmaNode {
    /// Builds the default phase state and palette for the plasma effect.
    fn default() -> Self {
        Self {
            phase: 0.0,
            last_elapsed_seconds: None,
            gradient: default_gradient(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(PlasmaNode {
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
    ..Self::default()
});

pub(crate) struct PlasmaInputs {
    speed: f32,
    freq_x: f32,
    freq_y: f32,
    freq_t: f32,
    contrast: f32,
}

crate::node_runtime::impl_runtime_inputs!(PlasmaInputs {
    speed = 1.0,
    freq_x = 3.0,
    freq_y = 4.0,
    freq_t = 1.0,
    contrast = 1.0,
});

pub(crate) struct PlasmaOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(PlasmaOutputs { frame });

impl RuntimeNode for PlasmaNode {
    type Inputs = PlasmaInputs;
    type Outputs = PlasmaOutputs;

    /// Advances the plasma phase and renders the current procedural field into the active layout.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation {
                outputs: PlasmaOutputs {
                    frame: ColorFrame {
                        layout: LedLayout {
                            id: "plasma:unbound".to_owned(),
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
                    code: Some("plasma_missing_render_layout".to_owned()),
                    message: "Plasma has no render layout, so it cannot render a frame yet."
                        .to_owned(),
                }],
            });
        };
        let mut diagnostics = Vec::new();
        let speed = inputs.speed;
        let freq_x = inputs.freq_x.max(0.1);
        let freq_y = inputs.freq_y.max(0.1);
        let freq_t = inputs.freq_t.max(0.1);
        let contrast = inputs.contrast.clamp(0.0, 4.0);
        if (freq_x - inputs.freq_x).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("plasma_freq_x_clamped".to_owned()),
                message: format!(
                    "freq_x {} is too small; using {} instead.",
                    inputs.freq_x, freq_x
                ),
            });
        }
        if (freq_y - inputs.freq_y).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("plasma_freq_y_clamped".to_owned()),
                message: format!(
                    "freq_y {} is too small; using {} instead.",
                    inputs.freq_y, freq_y
                ),
            });
        }
        if (freq_t - inputs.freq_t).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("plasma_freq_t_clamped".to_owned()),
                message: format!(
                    "freq_t {} is too small; using {} instead.",
                    inputs.freq_t, freq_t
                ),
            });
        }
        if (contrast - inputs.contrast).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("plasma_contrast_clamped".to_owned()),
                message: format!(
                    "Contrast {} is out of range; using {} instead.",
                    inputs.contrast, contrast
                ),
            });
        }
        let t = self.advance_phase(context.elapsed_seconds, speed);

        let mut pixels = Vec::with_capacity(layout.pixel_count);
        for i in 0..layout.pixel_count {
            let (x, y) = normalized_xy(i, &layout);
            let p1 = (std::f32::consts::TAU * (x * freq_x + t * freq_t)).sin();
            let p2 = (std::f32::consts::TAU * (y * freq_y - t * freq_t * 0.7)).sin();
            let p3 = (std::f32::consts::TAU * ((x + y) * 0.5 * freq_x + t * freq_t * 0.33)).sin();
            let mut v = ((p1 + p2 + p3) / 3.0) * 0.5 + 0.5;
            v = ((v - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
            let gradient_position = fract01(v * 0.8 + t * 0.08);
            let mut color = sample_gradient_hsv(&self.gradient, gradient_position);
            color.a *= v;
            pixels.push(color);
        }

        Ok(TypedNodeEvaluation {
            outputs: PlasmaOutputs {
                frame: ColorFrame { layout, pixels },
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

/// Builds the default palette used by the plasma effect.
fn default_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            stop(0.0, 0.02, 0.0, 0.1),
            stop(0.3, 0.3, 0.0, 0.6),
            stop(0.55, 0.0, 0.7, 0.9),
            stop(0.8, 1.0, 0.5, 0.0),
            stop(1.0, 1.0, 0.95, 0.4),
        ],
    }
}

/// Creates one opaque gradient stop for the plasma palette.
fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}

impl PlasmaNode {
    /// Integrates the animated phase from wall-clock time and the configured speed.
    fn advance_phase(&mut self, elapsed_seconds: f64, speed: f32) -> f32 {
        let dt = match self.last_elapsed_seconds {
            Some(last_elapsed_seconds) if elapsed_seconds >= last_elapsed_seconds => {
                (elapsed_seconds - last_elapsed_seconds) as f32
            }
            _ => 0.0,
        };
        self.last_elapsed_seconds = Some(elapsed_seconds);
        self.phase += speed * dt;
        self.phase
    }
}

/// Maps a layout pixel index to normalized coordinates used by the plasma field.
fn normalized_xy(index: usize, layout: &LedLayout) -> (f32, f32) {
    if let (Some(width), Some(height)) = (layout.width, layout.height) {
        if width > 1 && height > 1 {
            let x = (index % width) as f32 / (width - 1) as f32;
            let y = (index / width).min(height - 1) as f32 / (height - 1) as f32;
            return (x, y);
        }
    }

    if layout.pixel_count <= 1 {
        (0.0, 0.0)
    } else {
        (index as f32 / (layout.pixel_count - 1) as f32, 0.0)
    }
}

/// Wraps a float into the `[0, 1)` interval.
fn fract01(v: f32) -> f32 {
    let f = v.fract();
    if f < 0.0 { f + 1.0 } else { f }
}

#[cfg(test)]
mod tests {
    use super::PlasmaNode;

    /// Tests that the plasma phase accumulates elapsed time scaled by speed.
    #[test]
    fn phase_integrates_speed_over_time() {
        let mut node = PlasmaNode::default();

        assert_eq!(node.advance_phase(0.0, 0.25), 0.0);
        assert!((node.advance_phase(1.0, 0.25) - 0.25).abs() < 1e-6);
        assert!((node.advance_phase(2.0, 1.0) - 1.25).abs() < 1e-6);
        assert!((node.advance_phase(3.0, 1.0) - 2.25).abs() < 1e-6);
    }
}
