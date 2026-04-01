use anyhow::{Result, bail};
use shared::{ColorFrame, FloatTensor, InputValue, LedLayout};

use crate::color_math::mix_rgba;

/// Infers the broadcast shape shared by a set of runtime input values.
///
/// Scalars do not constrain the shape. All non-scalar shapes must match exactly or the operation
/// is rejected as a shape mismatch.
pub(crate) fn infer_broadcast_shape(values: &[&InputValue]) -> Result<Vec<usize>> {
    let shapes = values
        .iter()
        .map(|value| value_shape(value))
        .collect::<Result<Vec<_>>>()?;
    tracing::trace!(shapes = ?shapes, "inferring broadcast shape");

    let mut target = Vec::<usize>::new();
    for shape in &shapes {
        if shape.is_empty() {
            continue;
        }
        if target.is_empty() {
            target = shape.clone();
            continue;
        }
        if target != *shape {
            bail!("shape mismatch: {:?} vs {:?}", target, shape);
        }
    }

    if target.is_empty() {
        tracing::trace!(target_shape = ?vec![1usize], "broadcast shape inferred");
        Ok(vec![1])
    } else {
        tracing::trace!(target_shape = ?target, "broadcast shape inferred");
        Ok(target)
    }
}

/// Converts a color or color-frame input into a color frame with the requested shape.
///
/// Scalar colors are expanded across the full target layout. Existing frames must already match
/// the requested shape.
pub(crate) fn coerce_color_frame(
    value: &InputValue,
    target_shape: &[usize],
    fallback_layout_id: &str,
) -> Result<ColorFrame> {
    tracing::trace!(target_shape = ?target_shape, fallback_layout_id, "coercing color frame");
    match value {
        InputValue::Color(color) => {
            let layout = layout_from_shape(target_shape, fallback_layout_id);
            Ok(ColorFrame {
                pixels: vec![*color; layout.pixel_count],
                layout,
            })
        }
        InputValue::ColorFrame(frame) => {
            let shape = value_shape(value)?;
            if shape != target_shape {
                bail!(
                    "color frame shape mismatch: {:?} vs {:?}",
                    shape,
                    target_shape
                );
            }
            Ok(frame.clone())
        }
        _ => bail!("expected color or color frame"),
    }
}

/// Converts a float or float-tensor input into a tensor with the requested shape.
///
/// Scalar floats are expanded across the target shape. Existing tensors are normalized to their
/// declared element count and must then match the target shape exactly.
pub(crate) fn coerce_float_tensor(
    value: &InputValue,
    target_shape: &[usize],
) -> Result<FloatTensor> {
    tracing::trace!(target_shape = ?target_shape, "coercing float tensor");
    match value {
        InputValue::Float(value) => Ok(FloatTensor {
            shape: target_shape.to_vec(),
            values: vec![*value; shape_element_count(target_shape)],
        }),
        InputValue::FloatTensor(tensor) => {
            let normalized = normalize_float_tensor(tensor);
            if normalized.shape != target_shape {
                bail!(
                    "float tensor shape mismatch: {:?} vs {:?}",
                    normalized.shape,
                    target_shape
                );
            }
            Ok(normalized)
        }
        _ => bail!("expected float or float tensor"),
    }
}

/// Builds a fallback LED layout from a logical tensor or frame shape.
pub(crate) fn layout_from_shape(shape: &[usize], id: &str) -> LedLayout {
    match shape {
        [] => LedLayout {
            id: id.to_owned(),
            pixel_count: 1,
            width: None,
            height: None,
        },
        [len] => LedLayout {
            id: id.to_owned(),
            pixel_count: *len,
            width: None,
            height: None,
        },
        [height, width] => LedLayout {
            id: id.to_owned(),
            pixel_count: height * width,
            width: Some(*width),
            height: Some(*height),
        },
        _ => LedLayout {
            id: id.to_owned(),
            pixel_count: shape_element_count(shape),
            width: None,
            height: None,
        },
    }
}

/// Returns the logical shape implied by a runtime value.
fn value_shape(value: &InputValue) -> Result<Vec<usize>> {
    match value {
        InputValue::Float(_) | InputValue::Color(_) => Ok(Vec::new()),
        InputValue::FloatTensor(tensor) => Ok(normalize_float_tensor(tensor).shape),
        InputValue::ColorFrame(frame) => Ok(shape_from_layout(&frame.layout)),
        InputValue::LedLayout(layout) => Ok(shape_from_layout(layout)),
    }
}

/// Returns the logical shape described by a LED layout.
fn shape_from_layout(layout: &LedLayout) -> Vec<usize> {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) => vec![height, width],
        _ if layout.pixel_count > 0 => vec![layout.pixel_count],
        _ => vec![0],
    }
}

/// Returns the total element count for a logical shape.
fn shape_element_count(shape: &[usize]) -> usize {
    if shape.is_empty() {
        1
    } else {
        shape.iter().copied().product()
    }
}

/// Resizes a tensor's value storage to exactly match the number of elements implied by its shape.
fn normalize_float_tensor(tensor: &FloatTensor) -> FloatTensor {
    let mut normalized = tensor.clone();
    let expected_len = shape_element_count(&normalized.shape);
    if normalized.values.len() > expected_len {
        normalized.values.truncate(expected_len);
    } else if normalized.values.len() < expected_len {
        normalized.values.resize(expected_len, 0.0);
    }
    normalized
}

/// Blends two equally sized color frames using a per-pixel float factor tensor.
///
/// Each output pixel is computed by mixing the background and foreground colors with the matching
/// factor value from the tensor.
pub(crate) fn mix_color_frames(
    foreground: &ColorFrame,
    background: &ColorFrame,
    factor: &FloatTensor,
) -> Result<ColorFrame> {
    tracing::trace!(
        foreground_pixels = foreground.pixels.len(),
        background_pixels = background.pixels.len(),
        factor_len = factor.values.len(),
        "mixing color frames"
    );
    if foreground.pixels.len() != background.pixels.len()
        || foreground.pixels.len() != factor.values.len()
    {
        bail!("mix inputs differ in element count");
    }

    let pixels = foreground
        .pixels
        .iter()
        .zip(&background.pixels)
        .zip(&factor.values)
        .map(|((foreground, background), factor)| mix_rgba(*background, *foreground, *factor))
        .collect();

    Ok(ColorFrame {
        layout: foreground.layout.clone(),
        pixels,
    })
}
