use anyhow::Result;
use shared::{ColorFrame, ColorGradient, FloatTensor, LedLayout, RgbaColor};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

pub(crate) struct SpectrumAnalyzerNode {
    gradient: ColorGradient,
    background: RgbaColor,
    gain: f32,
    bar_gap: f32,
    decay: f32,
    displayed_levels: Vec<f32>,
    last_elapsed_seconds: Option<f64>,
}

impl Default for SpectrumAnalyzerNode {
    /// Builds the default smoothing state and color styling for the spectrum analyzer node.
    fn default() -> Self {
        Self {
            gradient: default_gradient(),
            background: RgbaColor {
                r: 0.02,
                g: 0.02,
                b: 0.03,
                a: 1.0,
            },
            gain: 1.0,
            bar_gap: 0.15,
            decay: 8.0,
            displayed_levels: Vec::new(),
            last_elapsed_seconds: None,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(SpectrumAnalyzerNode {
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
    background: RgbaColor = RgbaColor {
        r: 0.02,
        g: 0.02,
        b: 0.03,
        a: 1.0,
    },
    gain: f64 => |value| crate::node_runtime::max_f64_to_f32(value, 0.0), default 1.0f32,
    bar_gap: f64 => |value| crate::node_runtime::clamp_f64_to_f32(value, 0.0, 0.95), default 0.15f32,
    decay: f64 => |value| crate::node_runtime::max_f64_to_f32(value, 0.0), default 8.0f32,
    ..Self::default()
});

pub(crate) struct SpectrumAnalyzerInputs {
    spectrum: FloatTensor,
}

crate::node_runtime::impl_runtime_inputs!(SpectrumAnalyzerInputs {
    spectrum = FloatTensor {
        shape: vec![16],
        values: vec![0.0; 16],
    },
});

pub(crate) struct SpectrumAnalyzerOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(SpectrumAnalyzerOutputs { frame });

impl RuntimeNode for SpectrumAnalyzerNode {
    type Inputs = SpectrumAnalyzerInputs;
    type Outputs = SpectrumAnalyzerOutputs;

    /// Applies decay smoothing to the input spectrum and renders it into the active layout.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let layout = context.render_layout.clone().unwrap_or_else(|| LedLayout {
            id: "spectrum_analyzer:unbound".to_owned(),
            pixel_count: inputs.spectrum.values.len().max(1),
            width: Some(inputs.spectrum.values.len().max(1)),
            height: Some(1),
        });

        let levels = self.display_levels(&inputs.spectrum.values, context.elapsed_seconds);
        let frame = render_spectrum_frame(
            &layout,
            &levels,
            &self.gradient,
            self.background,
            self.bar_gap,
        );

        Ok(TypedNodeEvaluation::from_outputs(SpectrumAnalyzerOutputs {
            frame,
        }))
    }
}

impl SpectrumAnalyzerNode {
    /// Applies gain and temporal decay so displayed bar heights change smoothly over time.
    fn display_levels(&mut self, spectrum: &[f32], elapsed_seconds: f64) -> Vec<f32> {
        let dt = match self.last_elapsed_seconds {
            Some(previous) if elapsed_seconds >= previous => (elapsed_seconds - previous) as f32,
            _ => 0.0,
        };
        self.last_elapsed_seconds = Some(elapsed_seconds);

        if self.displayed_levels.len() != spectrum.len() {
            self.displayed_levels = vec![0.0; spectrum.len()];
        }

        let decay_multiplier = (-(self.decay.max(0.0) * dt)).exp();
        for (displayed, incoming) in self.displayed_levels.iter_mut().zip(spectrum.iter()) {
            let incoming = (incoming * self.gain).clamp(0.0, 1.0);
            if incoming >= *displayed {
                *displayed = incoming;
            } else {
                *displayed = (*displayed * decay_multiplier).max(incoming);
            }
        }

        self.displayed_levels.clone()
    }
}

/// Renders the normalized spectrum into a one- or two-dimensional bar visualization.
fn render_spectrum_frame(
    layout: &LedLayout,
    spectrum: &[f32],
    gradient: &ColorGradient,
    background: RgbaColor,
    bar_gap: f32,
) -> ColorFrame {
    let (width, height) = layout_dims(layout);
    let band_count = spectrum.len().max(1);
    let mut pixels = Vec::with_capacity(layout.pixel_count);

    for y in 0..height {
        for x in 0..width {
            let band_index = (x * band_count / width.max(1)).min(band_count.saturating_sub(1));
            let level = spectrum
                .get(band_index)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let band_t = if band_count <= 1 {
                0.0
            } else {
                band_index as f32 / (band_count - 1) as f32
            };
            let local_x = fractional_position_within_band(x, width, band_count);
            let in_bar = local_x >= bar_gap * 0.5 && local_x <= 1.0 - bar_gap * 0.5;

            let pixel = if !in_bar {
                background
            } else if height <= 1 {
                scale_rgb(sample_gradient_hsv(gradient, band_t), level)
            } else {
                let row_from_bottom = height - 1 - y;
                let brightness = fractional_fill(level, row_from_bottom, height);
                blend_rgb(
                    background,
                    sample_gradient_hsv(gradient, band_t),
                    brightness,
                )
            };

            pixels.push(pixel);
            if pixels.len() == layout.pixel_count {
                return ColorFrame {
                    layout: layout.clone(),
                    pixels,
                };
            }
        }
    }

    if pixels.len() < layout.pixel_count {
        pixels.extend(std::iter::repeat_n(
            background,
            layout.pixel_count - pixels.len(),
        ));
    }

    ColorFrame {
        layout: layout.clone(),
        pixels,
    }
}

/// Returns the effective raster dimensions used for the analyzer layout.
fn layout_dims(layout: &LedLayout) -> (usize, usize) {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => (layout.pixel_count.max(1), 1),
    }
}

/// Returns the horizontal position of a pixel within its assigned spectrum band.
fn fractional_position_within_band(x: usize, width: usize, band_count: usize) -> f32 {
    let start = x * band_count / width.max(1);
    let band_width = width as f32 / band_count.max(1) as f32;
    let band_start = start as f32 * band_width;
    (((x as f32 + 0.5) - band_start) / band_width).clamp(0.0, 1.0)
}

/// Returns how much of a vertical cell should be filled for a normalized bar level.
fn fractional_fill(level: f32, row_from_bottom: usize, height: usize) -> f32 {
    let filled_height = level.clamp(0.0, 1.0) * height as f32;
    (filled_height - row_from_bottom as f32).clamp(0.0, 1.0)
}

/// Scales an RGB color toward black while preserving alpha.
fn scale_rgb(color: RgbaColor, factor: f32) -> RgbaColor {
    let factor = factor.clamp(0.0, 1.0);
    RgbaColor {
        r: (color.r * factor).clamp(0.0, 1.0),
        g: (color.g * factor).clamp(0.0, 1.0),
        b: (color.b * factor).clamp(0.0, 1.0),
        a: color.a,
    }
}

/// Blends two colors linearly in RGB space.
fn blend_rgb(background: RgbaColor, foreground: RgbaColor, factor: f32) -> RgbaColor {
    let factor = factor.clamp(0.0, 1.0);
    RgbaColor {
        r: background.r + (foreground.r - background.r) * factor,
        g: background.g + (foreground.g - background.g) * factor,
        b: background.b + (foreground.b - background.b) * factor,
        a: background.a + (foreground.a - background.a) * factor,
    }
}

/// Builds the default low-to-high energy gradient for analyzer bars.
fn default_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            stop(0.0, 0.0, 0.55, 1.0),
            stop(0.45, 0.0, 1.0, 0.45),
            stop(0.75, 1.0, 0.85, 0.1),
            stop(1.0, 1.0, 0.2, 0.15),
        ],
    }
}

/// Creates one opaque gradient stop.
fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}

#[cfg(test)]
mod tests {
    use shared::LedLayout;

    use super::{SpectrumAnalyzerNode, default_gradient, fractional_fill, render_spectrum_frame};

    /// Tests that one-dimensional layouts map band strength directly to pixel brightness.
    #[test]
    fn one_dimensional_layout_maps_band_strength_to_brightness() {
        let layout = LedLayout {
            id: "strip".to_owned(),
            pixel_count: 4,
            width: None,
            height: None,
        };
        let frame = render_spectrum_frame(
            &layout,
            &[0.0, 0.5, 1.0, 0.25],
            &default_gradient(),
            shared::RgbaColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            0.0,
        );

        assert_eq!(frame.pixels.len(), 4);
        assert_eq!(frame.pixels[0].r, 0.0);
        assert!(frame.pixels[2].r >= frame.pixels[1].r);
    }

    /// Tests that two-dimensional layouts use fractional fill for partially lit rows.
    #[test]
    fn two_dimensional_layout_uses_fractional_fill() {
        assert_eq!(fractional_fill(0.0, 0, 4), 0.0);
        assert!((fractional_fill(0.375, 1, 4) - 0.5).abs() < 0.0001);
    }

    /// Tests that analyzer decay holds peaks briefly before releasing them over time.
    #[test]
    fn decay_holds_and_releases_levels_over_time() {
        let mut node = SpectrumAnalyzerNode::default();

        let first = node.display_levels(&[1.0, 0.5], 0.0);
        assert_eq!(first, vec![1.0, 0.5]);

        let second = node.display_levels(&[0.0, 0.0], 0.1);
        assert!(second[0] < 1.0);
        assert!(second[0] > 0.0);
    }
}
