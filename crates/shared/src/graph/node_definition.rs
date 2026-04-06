//! Shared schema types for node definitions, ports, parameters, and runtime-update metadata.
//!
//! These types describe what a node looks like to persistence, validation, the frontend editor,
//! and the backend runtime registry.

use super::{
    ColorGradient, ColorGradientStop, FloatTensor, InputValue, NodeParameter, RgbaColor, ValueKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Identifies a node type in persisted graphs, shared schema definitions, and runtime lookup tables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeTypeId(String);

impl NodeTypeId {
    pub const FLOAT_CONSTANT: &'static str = "inputs.float_constant";
    pub const COLOR_CONSTANT: &'static str = "inputs.color_constant";
    pub const DISPLAY: &'static str = "outputs.display";
    pub const PLOT: &'static str = "outputs.plot";
    pub const DELAY: &'static str = "temporal_filters.delay";
    pub const WLED_TARGET: &'static str = "outputs.wled_target";
    pub const WLED_SINK: &'static str = "inputs.wled_sink";
    pub const AUDIO_FFT_RECEIVER: &'static str = "inputs.audio_fft_receiver";
    pub const HA_MQTT_NUMBER: &'static str = "inputs.ha_mqtt_number";
    pub const SIGNAL_GENERATOR: &'static str = "inputs.signal_generator";
    pub const ADD_FLOAT: &'static str = "math.add_float";
    pub const SUBTRACT_FLOAT: &'static str = "math.subtract_float";
    pub const DIVIDE_FLOAT: &'static str = "math.divide_float";
    pub const ABS_FLOAT: &'static str = "math.abs_float";
    pub const MIN_MAX_FLOAT: &'static str = "math.min_max_float";
    pub const CLAMP_FLOAT: &'static str = "math.clamp_float";
    pub const POWER_FLOAT: &'static str = "math.power_float";
    pub const ROOT_FLOAT: &'static str = "math.root_float";
    pub const EXPONENTIAL_FLOAT: &'static str = "math.exponential_float";
    pub const LOG_FLOAT: &'static str = "math.log_float";
    pub const MAP_RANGE_FLOAT: &'static str = "math.map_range_float";
    pub const ROUND_FLOAT: &'static str = "math.round_float";
    pub const MULTIPLY_FLOAT: &'static str = "math.multiply_float";
    pub const SCALE_TENSOR: &'static str = "math.scale_tensor";
    pub const SCALE_COLOR: &'static str = "frame_operations.scale";
    pub const MULTIPLY_COLOR: &'static str = "frame_operations.multiply";
    pub const TINT_FRAME: &'static str = "frame_operations.tint_frame";
    pub const MASK_FRAME: &'static str = "frame_operations.mask_frame";
    pub const MIX_COLOR: &'static str = "frame_operations.mix";
    pub const ALPHA_OVER: &'static str = "frame_operations.alpha_over";
    pub const FADE: &'static str = "temporal_filters.fade";
    pub const MOVING_AVERAGE: &'static str = "temporal_filters.moving_average";
    pub const MOVING_MEDIAN: &'static str = "temporal_filters.moving_median";
    pub const BOX_BLUR: &'static str = "spatial_filters.box_blur";
    pub const GAUSSIAN_BLUR: &'static str = "spatial_filters.gaussian_blur";
    pub const MEDIAN_FILTER: &'static str = "spatial_filters.median_filter";
    pub const LAPLACIAN_FILTER: &'static str = "spatial_filters.laplacian_filter";
    pub const SPECTRUM_ANALYZER: &'static str = "generators.spectrum_analyzer";
    pub const SOLID_FRAME: &'static str = "generators.solid_frame";
    pub const RAINBOW_SWEEP: &'static str = "generators.rainbow_sweep";
    pub const CIRCLE_SWEEP: &'static str = "generators.circle_sweep";
    pub const LEVEL_BAR: &'static str = "generators.level_bar";
    pub const TWINKLE_STARS: &'static str = "generators.twinkle_stars";
    pub const PLASMA: &'static str = "generators.plasma";
    pub const BOUNCING_BALLS: &'static str = "generators.bouncing_balls";
    pub const FRAME_BRIGHTNESS: &'static str = "frame_operations.frame_brightness";
    pub const WLED_DUMMY_DISPLAY: &'static str = "debug.wled_dummy_display";

    /// Returns the stable string identifier used on the wire and in persisted graphs.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Builds a node type identifier from its stable string form.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl Serialize for NodeTypeId {
    /// Serializes the node type identifier as its stable string form.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for NodeTypeId {
    /// Deserializes a node type identifier from its stable string form.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self(value))
    }
}

impl Default for NodeTypeId {
    /// Returns the default node type used by fallback constructors and tests.
    fn default() -> Self {
        Self(Self::FLOAT_CONSTANT.to_owned())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
/// Groups node definitions into the categories shown in the editor and dashboard.
pub enum NodeCategory {
    Inputs,
    Generators,
    Math,
    FrameOperations,
    TemporalFilters,
    SpatialFilters,
    Outputs,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes the shared schema for a single node type.
///
/// A `NodeDefinition` is the canonical shared description of a node's ports, parameters,
/// connection rules, and editor-visible runtime updates.
pub struct NodeDefinition {
    pub id: String,
    pub display_name: String,
    pub category: NodeCategory,
    pub inputs: Vec<NodeInputDefinition>,
    pub outputs: Vec<NodeOutputDefinition>,
    pub parameters: Vec<NodeParameterDefinition>,
    pub connection: NodeConnectionDefinition,
    pub runtime_updates: Option<NodeRuntimeUpdateDefinition>,
}

impl NodeDefinition {
    /// Returns the named input port definition, if present.
    pub fn input_port(&self, name: &str) -> Option<&NodeInputDefinition> {
        self.inputs.iter().find(|input| input.name == name)
    }

    /// Returns the named output port definition, if present.
    pub fn output_port(&self, name: &str) -> Option<&NodeOutputDefinition> {
        self.outputs.iter().find(|output| output.name == name)
    }

    /// Returns the named parameter definition, if present.
    pub fn parameter(&self, name: &str) -> Option<&NodeParameterDefinition> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name == name)
    }

    /// Returns whether an output port can legally connect to an input port under this node's rules.
    pub fn can_connect_ports(
        &self,
        from_port: &NodeOutputDefinition,
        to_port: &NodeInputDefinition,
    ) -> bool {
        if self.connection.require_value_kind_match && !to_port.accepts_kind(from_port.value_kind) {
            return false;
        }
        true
    }

    /// Returns the named runtime-update value definition, if runtime updates are declared.
    pub fn runtime_update_value(&self, name: &str) -> Option<&NodeRuntimeValueDefinition> {
        self.runtime_updates
            .as_ref()?
            .values
            .iter()
            .find(|value| value.name == name)
    }
}

/// Returns the canonical opaque-white default color used by schema helpers.
fn white_input() -> InputValue {
    InputValue::Color(RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    })
}

/// Returns the canonical fully transparent default color used by schema helpers.
fn transparent_input() -> InputValue {
    InputValue::Color(RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    })
}

/// Returns the canonical single-sample zero tensor used by schema helpers.
fn default_tensor_input() -> InputValue {
    InputValue::FloatTensor(FloatTensor {
        shape: vec![1],
        values: vec![0.0],
    })
}

/// Converts a snake_case internal field name into a simple title-cased display label.
fn title_case_name(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().collect::<String>();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes one input port in the shared node schema.
///
/// Inputs may optionally declare a disconnected default value and a set of additional accepted
/// kinds beyond their primary `value_kind`.
pub struct NodeInputDefinition {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub value_kind: ValueKind,
    pub accepted_kinds: Vec<ValueKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<InputValue>,
}

impl NodeInputDefinition {
    /// Returns whether this input accepts the given value kind.
    pub fn accepts_kind(&self, kind: ValueKind) -> bool {
        self.value_kind == ValueKind::Any
            || kind == ValueKind::Any
            || self.value_kind == kind
            || self.accepted_kinds.contains(&kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes one output port in the shared node schema.
///
/// Outputs do not carry default values, but may advertise compatible kinds for flexible
/// downstream connections and UI previews.
pub struct NodeOutputDefinition {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub value_kind: ValueKind,
    pub accepted_kinds: Vec<ValueKind>,
}

impl NodeOutputDefinition {
    /// Returns whether this output can be treated as the given value kind.
    pub fn accepts_kind(&self, kind: ValueKind) -> bool {
        self.value_kind == ValueKind::Any
            || kind == ValueKind::Any
            || self.value_kind == kind
            || self.accepted_kinds.contains(&kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes one named runtime-update value emitted outside the normal graph edge system.
///
/// These values are intended for frontend inspection and diagnostics-style views rather than
/// normal graph wiring.
pub struct NodeRuntimeValueDefinition {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub value_kind: ValueKind,
    pub accepted_kinds: Vec<ValueKind>,
}

impl NodeRuntimeValueDefinition {
    /// Returns whether this runtime-update value can be treated as the given value kind.
    pub fn accepts_kind(&self, kind: ValueKind) -> bool {
        self.value_kind == ValueKind::Any
            || kind == ValueKind::Any
            || self.value_kind == kind
            || self.accepted_kinds.contains(&kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes one editable node parameter in the shared schema.
///
/// Parameter values are persisted as JSON on graph nodes and interpreted according to the default
/// value and UI hint defined here.
pub struct NodeParameterDefinition {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    pub default_value: ParameterDefaultValue,
    pub ui_hint: ParameterUiHint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<ParameterVisibilityCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes when a parameter should be rendered in the editor.
///
/// Conditions are intentionally small but composable so mode-based node UIs can be expressed in
/// shared schema without frontend-specific hardcoding.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParameterVisibilityCondition {
    Equals {
        parameter: String,
        value: JsonValue,
    },
    Any {
        conditions: Vec<ParameterVisibilityCondition>,
    },
    All {
        conditions: Vec<ParameterVisibilityCondition>,
    },
    Not {
        condition: Box<ParameterVisibilityCondition>,
    },
}

impl ParameterVisibilityCondition {
    /// Returns whether this visibility condition matches the current parameter values.
    pub fn matches(&self, parameters: &[NodeParameter]) -> bool {
        match self {
            Self::Equals { parameter, value } => parameters
                .iter()
                .find(|candidate| candidate.name == *parameter)
                .is_some_and(|candidate| candidate.value == *value),
            Self::Any { conditions } => conditions
                .iter()
                .any(|condition| condition.matches(parameters)),
            Self::All { conditions } => conditions
                .iter()
                .all(|condition| condition.matches(parameters)),
            Self::Not { condition } => !condition.matches(parameters),
        }
    }
}

impl NodeParameterDefinition {
    pub fn new(
        name: impl Into<String>,
        display_name: impl Into<String>,
        default_value: ParameterDefaultValue,
        ui_hint: ParameterUiHint,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            default_value,
            ui_hint,
            visible_when: None,
        }
    }

    pub fn visible_when(mut self, condition: ParameterVisibilityCondition) -> Self {
        self.visible_when = Some(condition);
        self
    }

    /// Returns whether the parameter should be shown for the current parameter values.
    pub fn is_visible(&self, parameters: &[NodeParameter]) -> bool {
        match &self.visible_when {
            None => true,
            Some(condition) => condition.matches(parameters),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Describes one selectable enum-style option for a parameter UI.
pub struct EnumOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Encodes the default value for a node parameter in a shared, serializable form.
pub enum ParameterDefaultValue {
    Float(f64),
    Color(RgbaColor),
    Gradient(Vec<ColorGradientStop>),
    Bool(bool),
    Integer(i64),
    String(String),
}

impl ParameterDefaultValue {
    /// Converts the default value into the JSON representation used in persisted graph parameters.
    pub fn to_json_value(&self) -> JsonValue {
        match self {
            Self::Float(value) => JsonValue::from(*value),
            Self::Color(value) => serde_json::to_value(value).unwrap_or(JsonValue::Null),
            Self::Gradient(stops) => serde_json::to_value(ColorGradient {
                stops: stops.clone(),
            })
            .unwrap_or(JsonValue::Null),
            Self::Bool(value) => JsonValue::from(*value),
            Self::Integer(value) => JsonValue::from(*value),
            Self::String(value) => JsonValue::from(value.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Selects the editor UI widget used to edit a parameter.
pub enum ParameterUiHint {
    DragFloat { speed: f64, min: f64, max: f64 },
    ColorPicker,
    ColorGradient,
    Checkbox,
    TextSingleLine,
    EnumSelect { options: Vec<EnumOption> },
    IntegerDrag { speed: f64, min: i64, max: i64 },
    WledInstanceOrHost,
    MqttBrokerSelect,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parameters_are_visible_without_an_explicit_condition() {
        let definition = NodeParameterDefinition::new(
            "custom_value",
            String::new(),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.1,
                min: 0.0,
                max: 10.0,
            },
        );

        assert!(definition.is_visible(&[]));
    }

    #[test]
    fn parameters_follow_boolean_visibility_conditions() {
        let definition = NodeParameterDefinition::new(
            "custom_value",
            String::new(),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.1,
                min: 0.0,
                max: 10.0,
            },
        )
        .visible_when(ParameterVisibilityCondition::Equals {
            parameter: "use_custom_value".to_owned(),
            value: JsonValue::from(true),
        });
        let enabled_parameters = vec![NodeParameter {
            name: "use_custom_value".to_owned(),
            value: JsonValue::from(true),
        }];
        let disabled_parameters = vec![NodeParameter {
            name: "use_custom_value".to_owned(),
            value: JsonValue::from(false),
        }];

        assert!(definition.is_visible(&enabled_parameters));
        assert!(!definition.is_visible(&disabled_parameters));
        assert!(!definition.is_visible(&[]));
    }

    #[test]
    fn parameters_follow_string_visibility_conditions() {
        let definition = NodeParameterDefinition::new(
            "advanced_value",
            String::new(),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.1,
                min: 0.0,
                max: 10.0,
            },
        )
        .visible_when(ParameterVisibilityCondition::Equals {
            parameter: "mode".to_owned(),
            value: json!("advanced"),
        });
        let parameters = vec![NodeParameter {
            name: "mode".to_owned(),
            value: json!("advanced"),
        }];

        assert!(definition.is_visible(&parameters));
    }

    #[test]
    fn parameters_follow_any_visibility_conditions() {
        let definition = NodeParameterDefinition::new(
            "multicast_group",
            String::new(),
            ParameterDefaultValue::String("239.0.0.1".to_owned()),
            ParameterUiHint::TextSingleLine,
        )
        .visible_when(ParameterVisibilityCondition::Any {
            conditions: vec![
                ParameterVisibilityCondition::Equals {
                    parameter: "mode".to_owned(),
                    value: json!("udp_multicast"),
                },
                ParameterVisibilityCondition::Equals {
                    parameter: "mode".to_owned(),
                    value: json!("wled_sound_sync"),
                },
            ],
        });

        assert!(definition.is_visible(&[NodeParameter {
            name: "mode".to_owned(),
            value: json!("udp_multicast"),
        }]));
        assert!(definition.is_visible(&[NodeParameter {
            name: "mode".to_owned(),
            value: json!("wled_sound_sync"),
        }]));
        assert!(!definition.is_visible(&[NodeParameter {
            name: "mode".to_owned(),
            value: json!("udp_unicast"),
        }]));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Describes global connection rules for a node's ports.
pub struct NodeConnectionDefinition {
    pub max_input_connections: usize,
    pub require_value_kind_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Declares frontend-visible runtime values emitted by a node outside normal graph outputs.
pub struct NodeRuntimeUpdateDefinition {
    pub auto_subscribe_in_editor: bool,
    pub values: Vec<NodeRuntimeValueDefinition>,
}
