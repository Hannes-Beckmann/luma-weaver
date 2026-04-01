use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value as JsonValue};
use shared::{FloatTensor, InputValue, LedLayout, RgbaColor};

/// Deserializes a typed value from a map of named runtime inputs.
///
/// The generic input map is first converted into JSON-like values and then deserialized using
/// serde so ordinary Rust structs can be used as node input types.
pub(crate) fn deserialize_inputs<T>(inputs: &HashMap<String, InputValue>) -> Result<T>
where
    T: DeserializeOwned,
{
    let input_summary = summarize_inputs(inputs);
    let object = inputs
        .iter()
        .map(|(name, value)| (name.clone(), input_value_to_json(value)))
        .collect::<Map<String, JsonValue>>();
    let value = JsonValue::Object(object.clone());
    match serde_json::from_value(value) {
        Ok(parsed) => Ok(parsed),
        Err(error) if object.is_empty() => serde_json::from_value(JsonValue::Null)
            .with_context(|| format!("deserialize node inputs ({input_summary})"))
            .or_else(|_| {
                Err(error).with_context(|| format!("deserialize node inputs ({input_summary})"))
            }),
        Err(error) => {
            Err(error).with_context(|| format!("deserialize node inputs ({input_summary})"))
        }
    }
}

/// Serializes a typed output value into a map of named runtime values.
pub(crate) fn serialize_outputs<T>(outputs: T) -> Result<HashMap<String, InputValue>>
where
    T: Serialize,
{
    let value = serde_json::to_value(outputs).context("serialize node outputs")?;
    let object = match value {
        JsonValue::Object(object) => object,
        JsonValue::Null => Map::new(),
        _ => anyhow::bail!("node outputs must serialize to a JSON object"),
    };

    object
        .into_iter()
        .map(|(name, value)| Ok((name, json_to_input_value(value)?)))
        .collect()
}

/// Converts a single runtime input value into its JSON representation for serde-based decoding.
fn input_value_to_json(value: &InputValue) -> JsonValue {
    match value {
        InputValue::Float(value) => JsonValue::from(*value as f64),
        InputValue::FloatTensor(value) => {
            serde_json::to_value(value).expect("float tensor must serialize")
        }
        InputValue::Color(value) => serde_json::to_value(value).expect("color must serialize"),
        InputValue::LedLayout(value) => serde_json::to_value(value).expect("layout must serialize"),
        InputValue::ColorFrame(value) => serde_json::to_value(value).expect("frame must serialize"),
    }
}

/// Converts a JSON value into a runtime `InputValue`.
fn json_to_input_value(value: JsonValue) -> Result<InputValue> {
    if value.is_null() {
        anyhow::bail!("cannot convert null into InputValue");
    }

    if let Ok(value) = serde_json::from_value::<f32>(value.clone()) {
        return Ok(InputValue::Float(value));
    }
    if let Ok(value) = serde_json::from_value::<shared::ColorFrame>(value.clone()) {
        return Ok(InputValue::ColorFrame(value));
    }
    if let Ok(value) = serde_json::from_value::<LedLayout>(value.clone()) {
        return Ok(InputValue::LedLayout(value));
    }
    if let Ok(value) = serde_json::from_value::<RgbaColor>(value.clone()) {
        return Ok(InputValue::Color(value));
    }
    if let Ok(value) = serde_json::from_value::<FloatTensor>(value.clone()) {
        return Ok(InputValue::FloatTensor(value));
    }

    anyhow::bail!("unsupported serialized output value: {value}")
}

#[derive(Debug, Clone)]
/// Wrapper that preserves an arbitrary runtime input value without attempting typed conversion.
pub(crate) struct AnyInputValue(pub(crate) InputValue);

impl Serialize for AnyInputValue {
    /// Serializes the wrapped generic runtime value through the shared JSON conversion path.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        input_value_to_json(&self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AnyInputValue {
    /// Deserializes a wrapped generic runtime value through the shared JSON conversion path.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        json_to_input_value(value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// Builds a compact human-readable summary of the current input map for error context.
fn summarize_inputs(inputs: &HashMap<String, InputValue>) -> String {
    if inputs.is_empty() {
        return "none".to_owned();
    }

    let mut entries = inputs
        .iter()
        .map(|(name, value)| format!("{name}={}", summarize_input_value(value)))
        .collect::<Vec<_>>();
    entries.sort();
    entries.join(", ")
}

/// Summarizes one runtime input value without including large payload contents.
fn summarize_input_value(value: &InputValue) -> String {
    match value {
        InputValue::Float(_) => "Float".to_owned(),
        InputValue::FloatTensor(tensor) => format!("FloatTensor{:?}", tensor.shape),
        InputValue::Color(_) => "Color".to_owned(),
        InputValue::LedLayout(layout) => format!("LedLayout({})", layout.pixel_count),
        InputValue::ColorFrame(frame) => format!("ColorFrame({})", frame.layout.pixel_count),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use shared::{ColorFrame, InputValue, LedLayout, RgbaColor};

    use super::{input_value_to_json, json_to_input_value, summarize_inputs};

    /// Tests that color-frame values survive the shared JSON conversion path unchanged.
    #[test]
    fn color_frame_json_round_trips_as_color_frame() {
        let value = InputValue::ColorFrame(ColorFrame {
            layout: LedLayout {
                id: "frame".to_owned(),
                pixel_count: 0,
                width: None,
                height: None,
            },
            pixels: Vec::new(),
        });

        let parsed = json_to_input_value(input_value_to_json(&value)).expect("parse input value");
        assert_eq!(parsed, value);
    }

    /// Tests that color values survive the shared JSON conversion path unchanged.
    #[test]
    fn color_json_round_trips_as_color() {
        let value = InputValue::Color(RgbaColor {
            r: 1.0,
            g: 0.5,
            b: 0.25,
            a: 1.0,
        });

        let parsed = json_to_input_value(input_value_to_json(&value)).expect("parse input value");
        assert_eq!(parsed, value);
    }

    /// Tests that input summaries keep frame diagnostics concise instead of dumping full payloads.
    #[test]
    fn summarize_inputs_omits_large_payloads() {
        let mut inputs = HashMap::new();
        inputs.insert(
            "frame".to_owned(),
            InputValue::ColorFrame(ColorFrame {
                layout: LedLayout {
                    id: "frame".to_owned(),
                    pixel_count: 64,
                    width: Some(8),
                    height: Some(8),
                },
                pixels: vec![
                    RgbaColor {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    64
                ],
            }),
        );
        inputs.insert("factor".to_owned(), InputValue::Float(0.5));

        assert_eq!(
            summarize_inputs(&inputs),
            "factor=Float, frame=ColorFrame(64)"
        );
    }
}
