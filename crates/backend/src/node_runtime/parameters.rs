use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;

/// Describes whether a typed parameter was missing, invalid, or successfully decoded.
pub(crate) enum ParameterStatus<T> {
    Missing,
    Invalid,
    Present(T),
}

/// Attempts to decode a typed parameter value from a raw JSON parameter map.
///
/// Missing keys and failed deserializations are distinguished so callers can emit more precise
/// construction diagnostics or apply different fallback rules.
pub(crate) fn parameter_status<T>(
    parameters: &HashMap<String, JsonValue>,
    name: &str,
) -> ParameterStatus<T>
where
    T: DeserializeOwned,
{
    let Some(value) = parameters.get(name) else {
        return ParameterStatus::Missing;
    };
    match serde_json::from_value::<T>(value.clone()) {
        Ok(value) => ParameterStatus::Present(value),
        Err(_) => ParameterStatus::Invalid,
    }
}
