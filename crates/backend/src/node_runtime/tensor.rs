use anyhow::{Result, bail};
use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

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

/// Returns the shared tensor target shape when any input is tensor-valued.
///
/// Scalar-only inputs return `None`, while mixed scalar/tensor inputs return the inferred tensor
/// shape used for broadcasting.
pub(crate) fn infer_float_tensor_target_shape(
    values: &[&InputValue],
) -> Result<Option<Vec<usize>>> {
    if values
        .iter()
        .any(|value| matches!(value, InputValue::FloatTensor(_)))
    {
        Ok(Some(infer_broadcast_shape(values)?))
    } else {
        Ok(None)
    }
}

/// Builds a zero fallback that matches either scalar or tensor numeric outputs.
pub(crate) fn zero_like_float_output(shape: Option<&[usize]>) -> InputValue {
    match shape {
        Some(shape) => InputValue::FloatTensor(FloatTensor {
            shape: shape.to_vec(),
            values: vec![0.0; shape_element_count(shape)],
        }),
        None => InputValue::Float(0.0),
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

/// Converts a float, tensor, or frame input into a color frame with the requested shape.
///
/// Float and tensor inputs are broadcast across RGBA channels. Existing frames must already match
/// the requested shape.
pub(crate) fn coerce_numeric_color_frame(
    value: &InputValue,
    target_shape: &[usize],
    fallback_layout_id: &str,
) -> Result<ColorFrame> {
    tracing::trace!(
        target_shape = ?target_shape,
        fallback_layout_id,
        "coercing numeric color frame"
    );
    match value {
        InputValue::Float(value) => {
            let layout = layout_from_shape(target_shape, fallback_layout_id);
            Ok(ColorFrame {
                pixels: vec![
                    RgbaColor {
                        r: *value,
                        g: *value,
                        b: *value,
                        a: *value,
                    };
                    layout.pixel_count
                ],
                layout,
            })
        }
        InputValue::FloatTensor(_) => {
            let tensor = coerce_float_tensor(value, target_shape)?;
            let layout = layout_from_shape(target_shape, fallback_layout_id);
            Ok(ColorFrame {
                pixels: tensor
                    .values
                    .into_iter()
                    .map(|value| RgbaColor {
                        r: value,
                        g: value,
                        b: value,
                        a: value,
                    })
                    .collect(),
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
        _ => bail!("expected float, float tensor, or color frame"),
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

/// Applies a numeric binary operation across scalar, tensor, and frame inputs.
///
/// Scalars broadcast into tensors, and floats or tensors broadcast into frames channel-wise.
pub(crate) fn apply_binary_numeric_op<F>(
    left: &InputValue,
    right: &InputValue,
    fallback_layout_id: &str,
    op: F,
) -> Result<InputValue>
where
    F: Fn(f32, f32) -> f32 + Copy,
{
    if matches!(left, InputValue::ColorFrame(_)) || matches!(right, InputValue::ColorFrame(_)) {
        let target_shape = infer_broadcast_shape(&[left, right])?;
        let left = coerce_numeric_color_frame(left, &target_shape, fallback_layout_id)?;
        let right = coerce_numeric_color_frame(right, &target_shape, fallback_layout_id)?;

        let pixels = left
            .pixels
            .iter()
            .zip(&right.pixels)
            .map(|(left, right)| RgbaColor {
                r: op(left.r, right.r),
                g: op(left.g, right.g),
                b: op(left.b, right.b),
                a: op(left.a, right.a),
            })
            .collect();

        return Ok(InputValue::ColorFrame(ColorFrame {
            layout: left.layout,
            pixels,
        }));
    }

    if matches!(left, InputValue::FloatTensor(_)) || matches!(right, InputValue::FloatTensor(_)) {
        let target_shape = infer_broadcast_shape(&[left, right])?;
        let left = coerce_float_tensor(left, &target_shape)?;
        let right = coerce_float_tensor(right, &target_shape)?;

        return Ok(InputValue::FloatTensor(FloatTensor {
            shape: target_shape,
            values: left
                .values
                .iter()
                .zip(&right.values)
                .map(|(left, right)| op(*left, *right))
                .collect(),
        }));
    }

    match (left, right) {
        (InputValue::Float(left), InputValue::Float(right)) => {
            Ok(InputValue::Float(op(*left, *right)))
        }
        _ => bail!(
            "expected Float, FloatTensor, or ColorFrame inputs, got {:?} and {:?}",
            left.value_kind(),
            right.value_kind()
        ),
    }
}

/// Applies a unary operation across either a scalar float or a float tensor.
pub(crate) fn apply_unary_float_tensor_op<F>(value: &InputValue, op: F) -> Result<InputValue>
where
    F: Fn(f32) -> f32 + Copy,
{
    match value {
        InputValue::Float(value) => Ok(InputValue::Float(op(*value))),
        InputValue::FloatTensor(tensor) => {
            let tensor = normalize_float_tensor(tensor);
            Ok(InputValue::FloatTensor(FloatTensor {
                shape: tensor.shape,
                values: tensor.values.into_iter().map(op).collect(),
            }))
        }
        _ => bail!(
            "expected Float or FloatTensor input, got {:?}",
            value.value_kind()
        ),
    }
}

/// Applies a binary operation across scalar floats and float tensors with scalar broadcasting.
pub(crate) fn apply_binary_float_tensor_op<F>(
    left: &InputValue,
    right: &InputValue,
    op: F,
) -> Result<InputValue>
where
    F: Fn(f32, f32) -> f32 + Copy,
{
    if let Some(target_shape) = infer_float_tensor_target_shape(&[left, right])? {
        let left = coerce_float_tensor(left, &target_shape)?;
        let right = coerce_float_tensor(right, &target_shape)?;
        Ok(InputValue::FloatTensor(FloatTensor {
            shape: target_shape,
            values: left
                .values
                .iter()
                .zip(&right.values)
                .map(|(left, right)| op(*left, *right))
                .collect(),
        }))
    } else {
        match (left, right) {
            (InputValue::Float(left), InputValue::Float(right)) => {
                Ok(InputValue::Float(op(*left, *right)))
            }
            _ => bail!(
                "expected Float or FloatTensor inputs, got {:?} and {:?}",
                left.value_kind(),
                right.value_kind()
            ),
        }
    }
}

/// Clamps a numeric scalar, tensor, or frame value between scalar or tensor bounds.
pub(crate) fn clamp_numeric_value(
    value: &InputValue,
    min: &InputValue,
    max: &InputValue,
    fallback_layout_id: &str,
) -> Result<InputValue> {
    if matches!(value, InputValue::ColorFrame(_)) {
        let target_shape = infer_broadcast_shape(&[value, min, max])?;
        let value = coerce_numeric_color_frame(value, &target_shape, fallback_layout_id)?;
        let min = coerce_float_tensor(min, &target_shape)?;
        let max = coerce_float_tensor(max, &target_shape)?;

        let pixels = value
            .pixels
            .iter()
            .zip(min.values.iter().zip(&max.values))
            .map(|(pixel, (min, max))| RgbaColor {
                r: pixel.r.clamp(*min, *max),
                g: pixel.g.clamp(*min, *max),
                b: pixel.b.clamp(*min, *max),
                a: pixel.a.clamp(*min, *max),
            })
            .collect();

        return Ok(InputValue::ColorFrame(ColorFrame {
            layout: value.layout,
            pixels,
        }));
    }

    if matches!(value, InputValue::FloatTensor(_))
        || matches!(min, InputValue::FloatTensor(_))
        || matches!(max, InputValue::FloatTensor(_))
    {
        let target_shape = infer_broadcast_shape(&[value, min, max])?;
        let value = coerce_float_tensor(value, &target_shape)?;
        let min = coerce_float_tensor(min, &target_shape)?;
        let max = coerce_float_tensor(max, &target_shape)?;

        return Ok(InputValue::FloatTensor(FloatTensor {
            shape: target_shape,
            values: value
                .values
                .iter()
                .zip(min.values.iter().zip(&max.values))
                .map(|(value, (min, max))| value.clamp(*min, *max))
                .collect(),
        }));
    }

    match (value, min, max) {
        (InputValue::Float(value), InputValue::Float(min), InputValue::Float(max)) => {
            Ok(InputValue::Float(value.clamp(*min, *max)))
        }
        _ => {
            bail!("expected Float, FloatTensor, or ColorFrame value with Float/FloatTensor bounds")
        }
    }
}

/// Returns whether all scalar channels contained in a runtime value are finite.
pub(crate) fn input_value_is_finite(value: &InputValue) -> bool {
    match value {
        InputValue::Float(value) => value.is_finite(),
        InputValue::String(_) => true,
        InputValue::FloatTensor(tensor) => tensor.values.iter().all(|value| value.is_finite()),
        InputValue::Color(color) => {
            color.r.is_finite() && color.g.is_finite() && color.b.is_finite() && color.a.is_finite()
        }
        InputValue::LedLayout(_) => true,
        InputValue::ColorFrame(frame) => frame.pixels.iter().all(|pixel| {
            pixel.r.is_finite() && pixel.g.is_finite() && pixel.b.is_finite() && pixel.a.is_finite()
        }),
    }
}

/// Returns the logical shape implied by a runtime value.
fn value_shape(value: &InputValue) -> Result<Vec<usize>> {
    match value {
        InputValue::Float(_) | InputValue::String(_) | InputValue::Color(_) => Ok(Vec::new()),
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
