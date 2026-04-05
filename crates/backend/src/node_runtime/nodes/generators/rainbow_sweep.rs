use anyhow::Result;
use shared::{
    ColorFrame, ColorGradient, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

pub(crate) struct RainbowSweepNode {
    cached_layout: Option<CachedLayout>,
    cached_angle_degrees: f32,
    wave_basis: Vec<f32>,
    phase: f32,
    last_elapsed_seconds: Option<f64>,
    gradient: ColorGradient,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CachedLayout {
    pixel_count: usize,
    width: Option<usize>,
    height: Option<usize>,
}

impl Default for RainbowSweepNode {
    /// Builds the default cached state and palette for the directional rainbow sweep.
    fn default() -> Self {
        Self {
            cached_layout: None,
            cached_angle_degrees: f32::NAN,
            wave_basis: Vec::new(),
            phase: 0.0,
            last_elapsed_seconds: None,
            gradient: default_gradient(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(RainbowSweepNode {
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
    ..Self::default()
});

pub(crate) struct RainbowSweepInputs {
    speed: f32,
    scale: f32,
    angle_degrees: f32,
}

crate::node_runtime::impl_runtime_inputs!(RainbowSweepInputs {
    speed = 0.25,
    scale = 1.0,
    angle_degrees = 0.0,
});

pub(crate) struct RainbowSweepOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(RainbowSweepOutputs { frame });

impl RuntimeNode for RainbowSweepNode {
    type Inputs = RainbowSweepInputs;
    type Outputs = RainbowSweepOutputs;

    /// Advances the sweep phase and renders a directional gradient wave into the active layout.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation {
                outputs: RainbowSweepOutputs {
                    frame: ColorFrame {
                        layout: LedLayout {
                            id: "rainbow_sweep:unbound".to_owned(),
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
                    code: Some("rainbow_sweep_missing_render_layout".to_owned()),
                    message: "Rainbow Sweep has no render layout, so it cannot render a frame yet."
                        .to_owned(),
                }],
            });
        };

        let mut diagnostics = Vec::new();
        let scale = inputs.scale.max(0.01);
        if (scale - inputs.scale).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("rainbow_sweep_scale_clamped".to_owned()),
                message: format!(
                    "Scale {} is too small; using {} instead.",
                    inputs.scale, scale
                ),
            });
        }
        let angle = inputs.angle_degrees.to_radians();
        let dir_x = angle.cos();
        let dir_y = angle.sin();
        let phase = self.advance_phase(context.elapsed_seconds, inputs.speed);
        self.ensure_wave_basis(&layout, inputs.angle_degrees, dir_x, dir_y);

        let mut pixels = Vec::with_capacity(layout.pixel_count);
        for i in 0..layout.pixel_count {
            let gradient_position = fract01(phase + scale * self.wave_basis[i]);
            pixels.push(sample_gradient_hsv(&self.gradient, gradient_position));
        }

        Ok(TypedNodeEvaluation {
            outputs: RainbowSweepOutputs {
                frame: ColorFrame { layout, pixels },
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl RainbowSweepNode {
    /// Integrates the animated sweep phase from elapsed time and the configured speed.
    fn advance_phase(&mut self, elapsed_seconds: f64, speed: f32) -> f32 {
        let dt = match self.last_elapsed_seconds {
            Some(last_elapsed_seconds) if elapsed_seconds >= last_elapsed_seconds => {
                (elapsed_seconds - last_elapsed_seconds) as f32
            }
            _ => 0.0,
        };
        self.last_elapsed_seconds = Some(elapsed_seconds);
        self.phase = fract01(self.phase + speed * dt);
        self.phase
    }

    /// Recomputes the cached directional basis when the layout or sweep angle changes.
    fn ensure_wave_basis(
        &mut self,
        layout: &LedLayout,
        angle_degrees: f32,
        dir_x: f32,
        dir_y: f32,
    ) {
        let cached_layout = CachedLayout {
            pixel_count: layout.pixel_count,
            width: layout.width,
            height: layout.height,
        };
        if self.cached_layout == Some(cached_layout) && self.cached_angle_degrees == angle_degrees {
            return;
        }

        self.wave_basis.clear();
        self.wave_basis.reserve(layout.pixel_count);
        for i in 0..layout.pixel_count {
            let (x, y) = normalized_xy(i, layout);
            self.wave_basis.push(x * dir_x + y * dir_y);
        }
        self.cached_layout = Some(cached_layout);
        self.cached_angle_degrees = angle_degrees;
    }
}

/// Builds the default rainbow palette used by the directional sweep.
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

/// Creates one opaque gradient stop for the rainbow-sweep palette.
fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}

/// Maps a layout pixel index to normalized coordinates used by the sweep basis.
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
    use super::RainbowSweepNode;

    /// Tests that the rainbow-sweep phase accumulates elapsed time scaled by speed.
    #[test]
    fn phase_integrates_speed_over_time() {
        let mut node = RainbowSweepNode::default();

        assert_eq!(node.advance_phase(0.0, 0.25), 0.0);
        assert!((node.advance_phase(1.0, 0.25) - 0.25).abs() < 1e-6);
        assert!((node.advance_phase(2.0, 1.0) - 0.25).abs() < 1e-6);
        assert!((node.advance_phase(3.0, 1.0) - 0.25).abs() < 1e-6);
    }
}
