use anyhow::Result;
use shared::{
    ColorFrame, ColorGradient, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

pub(crate) struct TwinkleStarsNode {
    phase: f32,
    last_elapsed_seconds: Option<f64>,
    gradient: ColorGradient,
}

impl Default for TwinkleStarsNode {
    /// Builds the default phase state and palette for the twinkle-stars effect.
    fn default() -> Self {
        Self {
            phase: 0.0,
            last_elapsed_seconds: None,
            gradient: default_gradient(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(TwinkleStarsNode {
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
    ..Self::default()
});

pub(crate) struct TwinkleStarsInputs {
    speed: f32,
    density: f32,
    min_brightness: f32,
    max_brightness: f32,
}

crate::node_runtime::impl_runtime_inputs!(TwinkleStarsInputs {
    speed = 1.0,
    density = 0.2,
    min_brightness = 0.03,
    max_brightness = 1.0,
});

pub(crate) struct TwinkleStarsOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(TwinkleStarsOutputs { frame });

impl RuntimeNode for TwinkleStarsNode {
    type Inputs = TwinkleStarsInputs;
    type Outputs = TwinkleStarsOutputs;

    /// Advances the twinkle phase and renders a procedurally gated star field.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation {
                outputs: TwinkleStarsOutputs {
                    frame: ColorFrame {
                        layout: LedLayout {
                            id: "twinkle_stars:unbound".to_owned(),
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
                    code: Some("twinkle_stars_missing_render_layout".to_owned()),
                    message: "Twinkle Stars has no render layout, so it cannot render a frame yet."
                        .to_owned(),
                }],
            });
        };
        let mut diagnostics = Vec::new();
        let speed = inputs.speed.max(0.0);
        let density = inputs.density.clamp(0.0, 1.0);
        let mut min_brightness = inputs.min_brightness.clamp(0.0, 1.0);
        let mut max_brightness = inputs.max_brightness.clamp(0.0, 1.0);
        if (speed - inputs.speed).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("twinkle_stars_speed_clamped".to_owned()),
                message: format!(
                    "Speed {} is too small; using {} instead.",
                    inputs.speed, speed
                ),
            });
        }
        if (density - inputs.density).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("twinkle_stars_density_clamped".to_owned()),
                message: format!(
                    "Density {} is out of range; using {} instead.",
                    inputs.density, density
                ),
            });
        }
        if (min_brightness - inputs.min_brightness).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("twinkle_stars_min_brightness_clamped".to_owned()),
                message: format!(
                    "Min brightness {} is out of range; using {} instead.",
                    inputs.min_brightness, min_brightness
                ),
            });
        }
        if (max_brightness - inputs.max_brightness).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("twinkle_stars_max_brightness_clamped".to_owned()),
                message: format!(
                    "Max brightness {} is out of range; using {} instead.",
                    inputs.max_brightness, max_brightness
                ),
            });
        }
        if max_brightness < min_brightness {
            std::mem::swap(&mut max_brightness, &mut min_brightness);
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("twinkle_stars_brightness_swapped".to_owned()),
                message: "Min brightness was greater than max brightness; swapped them.".to_owned(),
            });
        }
        let t = self.advance_phase(context.elapsed_seconds, speed);

        let mut pixels = Vec::with_capacity(layout.pixel_count);
        for i in 0..layout.pixel_count {
            let seed = hash_u32(i as u32 ^ 0x9E37_79B9);
            let gate = hash_to_unit(seed);
            let phase = hash_to_unit(hash_u32(seed ^ 0xA511_E9B3));
            let sparkle = ((t * std::f32::consts::TAU + phase * std::f32::consts::TAU).sin() * 0.5
                + 0.5)
                .powf(3.0);

            let bright = if gate <= density {
                lerp(min_brightness, max_brightness, sparkle)
            } else {
                min_brightness
            };

            let base_color = sample_gradient_hsv(&self.gradient, phase);
            pixels.push(RgbaColor {
                r: base_color.r * bright,
                g: base_color.g * bright,
                b: base_color.b * bright,
                a: base_color.a,
            });
        }

        Ok(TypedNodeEvaluation {
            outputs: TwinkleStarsOutputs {
                frame: ColorFrame { layout, pixels },
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

/// Builds the default cool-white star palette.
fn default_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            stop(0.0, 0.65, 0.75, 1.0),
            stop(0.5, 1.0, 0.95, 0.85),
            stop(1.0, 1.0, 1.0, 1.0),
        ],
    }
}

/// Creates one opaque gradient stop for the twinkle palette.
fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}

impl TwinkleStarsNode {
    /// Integrates the shared animation phase from elapsed time and the configured speed.
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

/// Hashes an integer seed into a deterministic pseudo-random bit pattern.
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

/// Converts a hashed integer into a normalized unit-interval float.
fn hash_to_unit(x: u32) -> f32 {
    (x as f32) / (u32::MAX as f32)
}

/// Linearly interpolates between two brightness values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::TwinkleStarsNode;

    /// Tests that the twinkle phase accumulates elapsed time scaled by speed.
    #[test]
    fn phase_integrates_speed_over_time() {
        let mut node = TwinkleStarsNode::default();

        assert_eq!(node.advance_phase(0.0, 0.25), 0.0);
        assert!((node.advance_phase(1.0, 0.25) - 0.25).abs() < 1e-6);
        assert!((node.advance_phase(2.0, 1.0) - 1.25).abs() < 1e-6);
        assert!((node.advance_phase(3.0, 1.0) - 2.25).abs() < 1e-6);
    }
}
