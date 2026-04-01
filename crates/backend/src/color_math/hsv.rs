use palette::{FromColor, Hsv, RgbHue, Srgb};
use shared::{ColorGradient, ColorGradientStop, RgbaColor};

/// Linearly interpolates two RGBA colors in RGB space.
///
/// `factor` is clamped to the unit interval before interpolation.
pub(crate) fn mix_rgba(background: RgbaColor, foreground: RgbaColor, factor: f32) -> RgbaColor {
    let factor = factor.clamp(0.0, 1.0);

    RgbaColor {
        r: background.r + (foreground.r - background.r) * factor,
        g: background.g + (foreground.g - background.g) * factor,
        b: background.b + (foreground.b - background.b) * factor,
        a: background.a + (foreground.a - background.a) * factor,
    }
}

/// Interpolates two RGBA colors in HSV space while following the shortest hue rotation.
///
/// Alpha is still blended linearly, but hue, saturation, and value are mixed in HSV space so
/// transitions between saturated colors avoid the muddy midpoint typical of RGB interpolation.
pub(crate) fn mix_rgba_hsv(background: RgbaColor, foreground: RgbaColor, factor: f32) -> RgbaColor {
    let factor = factor.clamp(0.0, 1.0);
    let background_hsv = rgb_to_hsv(background);
    let foreground_hsv = rgb_to_hsv(foreground);

    let background_hue = background_hsv.hue.into_degrees();
    let foreground_hue = foreground_hsv.hue.into_degrees();
    let hue_delta = shortest_hue_delta(background_hue, foreground_hue);
    let mixed_hue = RgbHue::from_degrees(background_hue + hue_delta * factor);

    let mixed_hsv = Hsv::new(
        mixed_hue,
        background_hsv.saturation
            + (foreground_hsv.saturation - background_hsv.saturation) * factor,
        background_hsv.value + (foreground_hsv.value - background_hsv.value) * factor,
    );
    let mixed_rgb: Srgb<f32> = Srgb::from_color(mixed_hsv);

    RgbaColor {
        r: mixed_rgb.red.clamp(0.0, 1.0),
        g: mixed_rgb.green.clamp(0.0, 1.0),
        b: mixed_rgb.blue.clamp(0.0, 1.0),
        a: background.a + (foreground.a - background.a) * factor,
    }
}

/// Samples a gradient by interpolating between its stops in HSV space.
///
/// Empty gradients fall back to opaque white, positions are clamped to `[0, 1]`, and stops are
/// sorted by position before sampling.
pub(crate) fn sample_gradient_hsv(gradient: &ColorGradient, position: f32) -> RgbaColor {
    if gradient.stops.is_empty() {
        return RgbaColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
    }

    let position = position.clamp(0.0, 1.0);
    let mut stops = gradient.stops.iter().copied().collect::<Vec<_>>();
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));

    if position <= stops[0].position {
        return stops[0].color;
    }

    for window in stops.windows(2) {
        let left = window[0];
        let right = window[1];
        if position <= right.position {
            let span = (right.position - left.position).max(f32::EPSILON);
            let factor = (position - left.position) / span;
            return mix_rgba_hsv(left.color, right.color, factor);
        }
    }

    stops
        .last()
        .copied()
        .unwrap_or(ColorGradientStop {
            position: 1.0,
            color: RgbaColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        })
        .color
}

/// Converts an RGBA color into HSV, ignoring the alpha channel.
fn rgb_to_hsv(color: RgbaColor) -> Hsv {
    Hsv::from_color(Srgb::new(color.r, color.g, color.b))
}

/// Returns the signed shortest hue delta in degrees between two hue angles.
fn shortest_hue_delta(from_degrees: f32, to_degrees: f32) -> f32 {
    let delta = (to_degrees - from_degrees).rem_euclid(360.0);
    if delta > 180.0 { delta - 360.0 } else { delta }
}

#[cfg(test)]
mod tests {
    use shared::{ColorGradient, ColorGradientStop, RgbaColor};

    use super::{mix_rgba_hsv, sample_gradient_hsv};

    /// Tests that HSV interpolation between red and green passes through yellow.
    #[test]
    fn hsv_mix_between_red_and_green_goes_through_yellow() {
        let red = RgbaColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let green = RgbaColor {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };

        let mixed = mix_rgba_hsv(red, green, 0.5);

        assert!(mixed.r > 0.95);
        assert!(mixed.g > 0.95);
        assert!(mixed.b < 0.1);
        assert!((mixed.a - 1.0).abs() < 1e-6);
    }

    /// Tests that gradient sampling reuses HSV interpolation between neighboring stops.
    #[test]
    fn gradient_sampling_uses_hsv_hue_interpolation() {
        let gradient = ColorGradient {
            stops: vec![
                ColorGradientStop {
                    position: 0.0,
                    color: RgbaColor {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                },
                ColorGradientStop {
                    position: 1.0,
                    color: RgbaColor {
                        r: 0.0,
                        g: 1.0,
                        b: 0.0,
                        a: 1.0,
                    },
                },
            ],
        };

        let mixed = sample_gradient_hsv(&gradient, 0.5);
        assert!(mixed.r > 0.95);
        assert!(mixed.g > 0.95);
        assert!(mixed.b < 0.1);
    }
}
