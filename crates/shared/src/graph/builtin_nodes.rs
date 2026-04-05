use super::{
    ColorGradientStop, EnumOption, FloatTensor, InputValue, NodeCategory, NodeConnectionDefinition,
    NodeDefinition, NodeInputDefinition, NodeOutputDefinition, NodeParameterDefinition,
    NodeRuntimeUpdateDefinition, NodeRuntimeValueDefinition, NodeTypeId, ParameterDefaultValue,
    ParameterUiHint, ParameterVisibilityCondition, RgbaColor, ValueKind,
};
use serde_json::json;
use std::sync::LazyLock;

/// Returns the canonical opaque-white default input value used by the built-in catalog.
fn white_input() -> InputValue {
    InputValue::Color(RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    })
}

/// Returns the canonical fully transparent color used by the built-in catalog.
fn transparent_input() -> InputValue {
    InputValue::Color(RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    })
}

/// Returns the canonical single-element zero tensor used by the built-in catalog.
fn default_tensor_input() -> InputValue {
    InputValue::FloatTensor(FloatTensor {
        shape: vec![1],
        values: vec![0.0],
    })
}

/// Converts a snake_case field name into the title-cased label used by built-in schema definitions.
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

/// Enumerates the waveform options exposed by the built-in signal generator node.
static SIGNAL_GENERATOR_WAVEFORMS: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "sinus".to_owned(),
            label: "Sine".to_owned(),
        },
        EnumOption {
            value: "triangle".to_owned(),
            label: "Triangle".to_owned(),
        },
        EnumOption {
            value: "sawtooth".to_owned(),
            label: "Sawtooth".to_owned(),
        },
        EnumOption {
            value: "rectangle".to_owned(),
            label: "Rectangle".to_owned(),
        },
    ]
});

/// Enumerates the supported receive modes for the audio FFT receiver node.
static AUDIO_FFT_RECEIVE_MODES: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "udp_multicast".to_owned(),
            label: "UDP Multicast".to_owned(),
        },
        EnumOption {
            value: "udp_unicast".to_owned(),
            label: "UDP Unicast".to_owned(),
        },
        EnumOption {
            value: "wled_sound_sync".to_owned(),
            label: "WLED Sound Sync".to_owned(),
        },
    ]
});

static MIN_MAX_FLOAT_MODES: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "min".to_owned(),
            label: "Min".to_owned(),
        },
        EnumOption {
            value: "max".to_owned(),
            label: "Max".to_owned(),
        },
    ]
});

static ROUND_FLOAT_MODES: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "floor".to_owned(),
            label: "Floor".to_owned(),
        },
        EnumOption {
            value: "round".to_owned(),
            label: "Round".to_owned(),
        },
        EnumOption {
            value: "ceil".to_owned(),
            label: "Ceil".to_owned(),
        },
    ]
});

static FLOAT_CONSTANT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::FLOAT_CONSTANT.to_owned(),
    display_name: "Float Constant".to_owned(),
    category: NodeCategory::Inputs,
    inputs: vec![],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "value",
        title_case_name("value"),
        ParameterDefaultValue::Float(0.0),
        ParameterUiHint::DragFloat {
            speed: 0.01,
            min: -10_000.0,
            max: 10_000.0,
        },
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static COLOR_CONSTANT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::COLOR_CONSTANT.to_owned(),
    display_name: "Color Constant".to_owned(),
    category: NodeCategory::Inputs,
    inputs: vec![],
    outputs: vec![NodeOutputDefinition {
        name: "color".to_owned(),
        display_name: title_case_name("color"),
        value_kind: ValueKind::Color,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "color",
        title_case_name("color"),
        ParameterDefaultValue::Color(RgbaColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }),
        ParameterUiHint::ColorPicker,
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static DISPLAY_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::DISPLAY.to_owned(),
    display_name: "Display".to_owned(),
    category: NodeCategory::Outputs,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![
            ValueKind::Color,
            ValueKind::ColorFrame,
            ValueKind::LedLayout,
        ],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: Some(NodeRuntimeUpdateDefinition {
        auto_subscribe_in_editor: true,
        values: vec![NodeRuntimeValueDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![
                ValueKind::Color,
                ValueKind::ColorFrame,
                ValueKind::LedLayout,
            ],
        }],
    }),
});

static PLOT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::PLOT.to_owned(),
    display_name: "Plot".to_owned(),
    category: NodeCategory::Outputs,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: Some(NodeRuntimeUpdateDefinition {
        auto_subscribe_in_editor: true,
        values: vec![NodeRuntimeValueDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
        }],
    }),
});

static DELAY_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::DELAY.to_owned(),
    display_name: "Delay".to_owned(),
    category: NodeCategory::TemporalFilters,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "ticks",
        title_case_name("ticks"),
        ParameterDefaultValue::Integer(1),
        ParameterUiHint::IntegerDrag {
            speed: 1.0,
            min: 1,
            max: 10_000,
        },
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static WLED_TARGET_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::WLED_TARGET.to_owned(),
    display_name: "Wled Target".to_owned(),
    category: NodeCategory::Outputs,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![ValueKind::Color],
        default_value: None,
    }],
    outputs: vec![],
    parameters: vec![
        NodeParameterDefinition::new(
            "target",
            title_case_name("target"),
            ParameterDefaultValue::String("".to_owned()),
            ParameterUiHint::WledInstanceOrHost,
        ),
        NodeParameterDefinition::new(
            "led_count",
            title_case_name("led_count"),
            ParameterDefaultValue::Integer(60),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 8192,
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static WLED_SINK_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::WLED_SINK.to_owned(),
    display_name: "Wled Sink".to_owned(),
    category: NodeCategory::Inputs,
    inputs: vec![],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "protocol",
            title_case_name("protocol"),
            ParameterDefaultValue::String("ddp".to_owned()),
            ParameterUiHint::EnumSelect {
                options: vec![
                    EnumOption {
                        value: "ddp".to_owned(),
                        label: "DDP".to_owned(),
                    },
                    EnumOption {
                        value: "udp_raw".to_owned(),
                        label: "UDP Raw".to_owned(),
                    },
                ],
            },
        ),
        NodeParameterDefinition::new(
            "port",
            title_case_name("port"),
            ParameterDefaultValue::Integer(4048),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 65535,
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static AUDIO_FFT_RECEIVER_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::AUDIO_FFT_RECEIVER.to_owned(),
    display_name: "Audio FFT Receiver".to_owned(),
    category: NodeCategory::Inputs,
    inputs: vec![],
    outputs: vec![
        NodeOutputDefinition {
            name: "spectrum".to_owned(),
            display_name: title_case_name("spectrum"),
            value_kind: ValueKind::FloatTensor,
            accepted_kinds: vec![],
        },
        NodeOutputDefinition {
            name: "spectral_peak".to_owned(),
            display_name: title_case_name("spectral_peak"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
        },
        NodeOutputDefinition {
            name: "overall_loudness".to_owned(),
            display_name: title_case_name("overall_loudness"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
        },
    ],
    parameters: vec![
        NodeParameterDefinition::new(
            "receive_mode",
            title_case_name("receive_mode"),
            ParameterDefaultValue::String("udp_multicast".to_owned()),
            ParameterUiHint::EnumSelect {
                options: AUDIO_FFT_RECEIVE_MODES.clone(),
            },
        ),
        NodeParameterDefinition::new(
            "port",
            title_case_name("port"),
            ParameterDefaultValue::Integer(11988),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 65535,
            },
        ),
        NodeParameterDefinition::new(
            "multicast_group",
            title_case_name("multicast_group"),
            ParameterDefaultValue::String("239.0.0.1".to_owned()),
            ParameterUiHint::TextSingleLine,
        )
        .visible_when(ParameterVisibilityCondition::Any {
            conditions: vec![
                ParameterVisibilityCondition::Equals {
                    parameter: "receive_mode".to_owned(),
                    value: json!("udp_multicast"),
                },
                ParameterVisibilityCondition::Equals {
                    parameter: "receive_mode".to_owned(),
                    value: json!("wled_sound_sync"),
                },
            ],
        }),
        NodeParameterDefinition::new(
            "bind_host",
            title_case_name("bind_host"),
            ParameterDefaultValue::String("0.0.0.0".to_owned()),
            ParameterUiHint::TextSingleLine,
        )
        .visible_when(ParameterVisibilityCondition::Equals {
            parameter: "receive_mode".to_owned(),
            value: json!("udp_unicast"),
        }),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static HA_MQTT_NUMBER_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::HA_MQTT_NUMBER.to_owned(),
    display_name: "Home Assistant MQTT Number".to_owned(),
    category: NodeCategory::Inputs,
    inputs: vec![],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "broker_id",
            title_case_name("broker_id"),
            ParameterDefaultValue::String(String::new()),
            ParameterUiHint::MqttBrokerSelect,
        ),
        NodeParameterDefinition::new(
            "entity_id",
            title_case_name("entity_id"),
            ParameterDefaultValue::String("animation_builder_number".to_owned()),
            ParameterUiHint::TextSingleLine,
        ),
        NodeParameterDefinition::new(
            "display_name",
            title_case_name("display_name"),
            ParameterDefaultValue::String("Luma Weaver Number".to_owned()),
            ParameterUiHint::TextSingleLine,
        ),
        NodeParameterDefinition::new(
            "default_value",
            title_case_name("default_value"),
            ParameterDefaultValue::Float(0.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "min",
            title_case_name("min"),
            ParameterDefaultValue::Float(0.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "max",
            title_case_name("max"),
            ParameterDefaultValue::Float(100.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "step",
            title_case_name("step"),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: 0.0001,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "retain",
            title_case_name("retain"),
            ParameterDefaultValue::Bool(true),
            ParameterUiHint::Checkbox,
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ADD_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::ADD_FLOAT.to_owned(),
    display_name: "Add Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "sum".to_owned(),
        display_name: title_case_name("sum"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SUBTRACT_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::SUBTRACT_FLOAT.to_owned(),
    display_name: "Subtract Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "difference".to_owned(),
        display_name: title_case_name("difference"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SIGNAL_GENERATOR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::SIGNAL_GENERATOR.to_owned(),
    display_name: "Signal Generator".to_owned(),
    category: NodeCategory::Inputs,
    inputs: vec![],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "waveform",
            title_case_name("waveform"),
            ParameterDefaultValue::String("sinus".to_owned()),
            ParameterUiHint::EnumSelect {
                options: SIGNAL_GENERATOR_WAVEFORMS.clone(),
            },
        ),
        NodeParameterDefinition::new(
            "frequency",
            title_case_name("frequency"),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "amplitude",
            title_case_name("amplitude"),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "phase",
            title_case_name("phase"),
            ParameterDefaultValue::Float(0.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static DIVIDE_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::DIVIDE_FLOAT.to_owned(),
    display_name: "Divide Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "quotient".to_owned(),
        display_name: title_case_name("quotient"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MIN_MAX_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MIN_MAX_FLOAT.to_owned(),
    display_name: "Min/Max Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "mode",
        title_case_name("mode"),
        ParameterDefaultValue::String("min".to_owned()),
        ParameterUiHint::EnumSelect {
            options: MIN_MAX_FLOAT_MODES.clone(),
        },
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MULTIPLY_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MULTIPLY_FLOAT.to_owned(),
    display_name: "Multiply Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "product".to_owned(),
        display_name: title_case_name("product"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ABS_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::ABS_FLOAT.to_owned(),
    display_name: "Abs Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static CLAMP_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::CLAMP_FLOAT.to_owned(),
    display_name: "Clamp Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "min".to_owned(),
            display_name: title_case_name("min"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "max".to_owned(),
            display_name: title_case_name("max"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static POWER_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::POWER_FLOAT.to_owned(),
    display_name: "Power Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "base".to_owned(),
            display_name: title_case_name("base"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "exponent".to_owned(),
            display_name: title_case_name("exponent"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ROOT_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::ROOT_FLOAT.to_owned(),
    display_name: "Root Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "degree".to_owned(),
            display_name: title_case_name("degree"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(2.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static EXPONENTIAL_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::EXPONENTIAL_FLOAT.to_owned(),
    display_name: "Exponential Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static LOG_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::LOG_FLOAT.to_owned(),
    display_name: "Log Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "base".to_owned(),
            display_name: title_case_name("base"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(std::f32::consts::E)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MAP_RANGE_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MAP_RANGE_FLOAT.to_owned(),
    display_name: "Map Range Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "source_min".to_owned(),
            display_name: title_case_name("source_min"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "source_max".to_owned(),
            display_name: title_case_name("source_max"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "target_min".to_owned(),
            display_name: title_case_name("target_min"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "target_max".to_owned(),
            display_name: title_case_name("target_max"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ROUND_FLOAT_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::ROUND_FLOAT.to_owned(),
    display_name: "Round Float".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "mode",
        title_case_name("mode"),
        ParameterDefaultValue::String("round".to_owned()),
        ParameterUiHint::EnumSelect {
            options: ROUND_FLOAT_MODES.clone(),
        },
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SCALE_TENSOR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::SCALE_TENSOR.to_owned(),
    display_name: "Scale Tensor".to_owned(),
    category: NodeCategory::Math,
    inputs: vec![
        NodeInputDefinition {
            name: "tensor".to_owned(),
            display_name: title_case_name("tensor"),
            value_kind: ValueKind::FloatTensor,
            accepted_kinds: vec![],
            default_value: Some(default_tensor_input()),
        },
        NodeInputDefinition {
            name: "factor".to_owned(),
            display_name: title_case_name("factor"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "tensor".to_owned(),
        display_name: title_case_name("tensor"),
        value_kind: ValueKind::FloatTensor,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SCALE_COLOR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::SCALE_COLOR.to_owned(),
    display_name: "Scale Color".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "color".to_owned(),
            display_name: title_case_name("color"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![],
            default_value: Some(white_input()),
        },
        NodeInputDefinition {
            name: "factor".to_owned(),
            display_name: title_case_name("factor"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "color".to_owned(),
        display_name: title_case_name("color"),
        value_kind: ValueKind::Color,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MULTIPLY_COLOR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MULTIPLY_COLOR.to_owned(),
    display_name: "Multiply Color".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![],
            default_value: Some(white_input()),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![],
            default_value: Some(white_input()),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "color".to_owned(),
        display_name: title_case_name("color"),
        value_kind: ValueKind::Color,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static TINT_FRAME_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::TINT_FRAME.to_owned(),
    display_name: "Tint Frame".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "tint".to_owned(),
            display_name: title_case_name("tint"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![],
            default_value: Some(white_input()),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MASK_FRAME_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MASK_FRAME.to_owned(),
    display_name: "Mask Frame".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "mask".to_owned(),
            display_name: title_case_name("mask"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MIX_COLOR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MIX_COLOR.to_owned(),
    display_name: "Mix Color".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "foreground".to_owned(),
            display_name: title_case_name("foreground"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![ValueKind::ColorFrame],
            default_value: Some(white_input()),
        },
        NodeInputDefinition {
            name: "background".to_owned(),
            display_name: title_case_name("background"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![ValueKind::ColorFrame],
            default_value: Some(white_input()),
        },
        NodeInputDefinition {
            name: "factor".to_owned(),
            display_name: title_case_name("factor"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "color".to_owned(),
        display_name: title_case_name("color"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ALPHA_OVER_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::ALPHA_OVER.to_owned(),
    display_name: "Alpha Over".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "foreground".to_owned(),
            display_name: title_case_name("foreground"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![ValueKind::ColorFrame],
            default_value: Some(transparent_input()),
        },
        NodeInputDefinition {
            name: "background".to_owned(),
            display_name: title_case_name("background"),
            value_kind: ValueKind::Color,
            accepted_kinds: vec![ValueKind::ColorFrame],
            default_value: Some(transparent_input()),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "color".to_owned(),
        display_name: title_case_name("color"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static FADE_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::FADE.to_owned(),
    display_name: "Fade".to_owned(),
    category: NodeCategory::TemporalFilters,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![],
            default_value: Some(white_input()),
        },
        NodeInputDefinition {
            name: "decay".to_owned(),
            display_name: title_case_name("decay"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MOVING_AVERAGE_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MOVING_AVERAGE.to_owned(),
    display_name: "Moving Average".to_owned(),
    category: NodeCategory::TemporalFilters,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::Color,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "window_size".to_owned(),
            display_name: title_case_name("window_size"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(4.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![
            ValueKind::Float,
            ValueKind::Color,
            ValueKind::FloatTensor,
            ValueKind::ColorFrame,
        ],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MOVING_MEDIAN_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MOVING_MEDIAN.to_owned(),
    display_name: "Moving Median".to_owned(),
    category: NodeCategory::TemporalFilters,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![ValueKind::Float, ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "window_size".to_owned(),
            display_name: title_case_name("window_size"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(4.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![ValueKind::Float, ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static BOX_BLUR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::BOX_BLUR.to_owned(),
    display_name: "Box Blur".to_owned(),
    category: NodeCategory::SpatialFilters,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "radius".to_owned(),
            display_name: title_case_name("radius"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static GAUSSIAN_BLUR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::GAUSSIAN_BLUR.to_owned(),
    display_name: "Gaussian Blur".to_owned(),
    category: NodeCategory::SpatialFilters,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "radius".to_owned(),
            display_name: title_case_name("radius"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(2.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MEDIAN_FILTER_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::MEDIAN_FILTER.to_owned(),
    display_name: "Median Filter".to_owned(),
    category: NodeCategory::SpatialFilters,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "radius".to_owned(),
            display_name: title_case_name("radius"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SPECTRUM_ANALYZER_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::SPECTRUM_ANALYZER.to_owned(),
    display_name: "Spectrum Analyzer".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![NodeInputDefinition {
        name: "spectrum".to_owned(),
        display_name: title_case_name("spectrum"),
        value_kind: ValueKind::FloatTensor,
        accepted_kinds: vec![],
        default_value: Some(InputValue::FloatTensor(FloatTensor {
            shape: vec![16],
            values: vec![0.0; 16],
        })),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "gradient",
            title_case_name("gradient"),
            ParameterDefaultValue::Gradient(DEFAULT_RAINBOW_GRADIENT_STOPS.to_vec()),
            ParameterUiHint::ColorGradient,
        ),
        NodeParameterDefinition::new(
            "background",
            title_case_name("background"),
            ParameterDefaultValue::Color(RgbaColor {
                r: 0.02,
                g: 0.02,
                b: 0.03,
                a: 1.0,
            }),
            ParameterUiHint::ColorPicker,
        ),
        NodeParameterDefinition::new(
            "gain",
            title_case_name("gain"),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: 0.0,
                max: 8.0,
            },
        ),
        NodeParameterDefinition::new(
            "bar_gap",
            title_case_name("bar_gap"),
            ParameterDefaultValue::Float(0.15),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: 0.0,
                max: 0.95,
            },
        ),
        NodeParameterDefinition::new(
            "decay",
            title_case_name("decay"),
            ParameterDefaultValue::Float(8.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: 0.0,
                max: 32.0,
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SOLID_FRAME_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::SOLID_FRAME.to_owned(),
    display_name: "Solid Frame".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![NodeInputDefinition {
        name: "color".to_owned(),
        display_name: title_case_name("color"),
        value_kind: ValueKind::Color,
        accepted_kinds: vec![],
        default_value: Some(white_input()),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static RAINBOW_SWEEP_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::RAINBOW_SWEEP.to_owned(),
    display_name: "Linear Sweep".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![
        NodeInputDefinition {
            name: "speed".to_owned(),
            display_name: title_case_name("speed"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.25)),
        },
        NodeInputDefinition {
            name: "scale".to_owned(),
            display_name: title_case_name("scale"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "angle_degrees".to_owned(),
            display_name: title_case_name("angle_degrees"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "gradient",
        title_case_name("gradient"),
        ParameterDefaultValue::Gradient(DEFAULT_RAINBOW_GRADIENT_STOPS.to_vec()),
        ParameterUiHint::ColorGradient,
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static CIRCLE_SWEEP_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::CIRCLE_SWEEP.to_owned(),
    display_name: "Circle Sweep".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![
        NodeInputDefinition {
            name: "speed".to_owned(),
            display_name: title_case_name("speed"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.25)),
        },
        NodeInputDefinition {
            name: "scale".to_owned(),
            display_name: title_case_name("scale"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "aspect".to_owned(),
            display_name: title_case_name("aspect"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "gradient",
        title_case_name("gradient"),
        ParameterDefaultValue::Gradient(DEFAULT_RAINBOW_GRADIENT_STOPS.to_vec()),
        ParameterUiHint::ColorGradient,
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static LEVEL_BAR_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::LEVEL_BAR.to_owned(),
    display_name: "Level Bar".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![NodeInputDefinition {
        name: "loudness".to_owned(),
        display_name: title_case_name("loudness"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "gradient",
        title_case_name("gradient"),
        ParameterDefaultValue::Gradient(DEFAULT_RAINBOW_GRADIENT_STOPS.to_vec()),
        ParameterUiHint::ColorGradient,
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static TWINKLE_STARS_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::TWINKLE_STARS.to_owned(),
    display_name: "Twinkle Stars".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![
        NodeInputDefinition {
            name: "speed".to_owned(),
            display_name: title_case_name("speed"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "density".to_owned(),
            display_name: title_case_name("density"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.2)),
        },
        NodeInputDefinition {
            name: "min_brightness".to_owned(),
            display_name: title_case_name("min_brightness"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.03)),
        },
        NodeInputDefinition {
            name: "max_brightness".to_owned(),
            display_name: title_case_name("max_brightness"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "gradient",
        title_case_name("gradient"),
        ParameterDefaultValue::Gradient(DEFAULT_TWINKLE_GRADIENT_STOPS.to_vec()),
        ParameterUiHint::ColorGradient,
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static PLASMA_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::PLASMA.to_owned(),
    display_name: "Plasma".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![
        NodeInputDefinition {
            name: "speed".to_owned(),
            display_name: title_case_name("speed"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "freq_x".to_owned(),
            display_name: title_case_name("freq_x"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(3.0)),
        },
        NodeInputDefinition {
            name: "freq_y".to_owned(),
            display_name: title_case_name("freq_y"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(4.0)),
        },
        NodeInputDefinition {
            name: "freq_t".to_owned(),
            display_name: title_case_name("freq_t"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "contrast".to_owned(),
            display_name: title_case_name("contrast"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "gradient",
        title_case_name("gradient"),
        ParameterDefaultValue::Gradient(DEFAULT_PLASMA_GRADIENT_STOPS.to_vec()),
        ParameterUiHint::ColorGradient,
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static BOUNCING_BALLS_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::BOUNCING_BALLS.to_owned(),
    display_name: "Bouncing Balls".to_owned(),
    category: NodeCategory::Generators,
    inputs: vec![
        NodeInputDefinition {
            name: "speed".to_owned(),
            display_name: title_case_name("speed"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.3)),
        },
        NodeInputDefinition {
            name: "radius".to_owned(),
            display_name: title_case_name("radius"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.5)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "circle_count",
            title_case_name("circle_count"),
            ParameterDefaultValue::Integer(6),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 64,
            },
        ),
        NodeParameterDefinition::new(
            "radius_variance",
            title_case_name("radius_variance"),
            ParameterDefaultValue::Float(0.35),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: 0.0,
                max: 1.0,
            },
        ),
        NodeParameterDefinition::new(
            "gradient",
            title_case_name("gradient"),
            ParameterDefaultValue::Gradient(DEFAULT_BOUNCING_BALLS_GRADIENT_STOPS.to_vec()),
            ParameterUiHint::ColorGradient,
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static WLED_DUMMY_DISPLAY_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::WLED_DUMMY_DISPLAY.to_owned(),
    display_name: "Wled Dummy Display".to_owned(),
    category: NodeCategory::Debug,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![ValueKind::Color],
        default_value: None,
    }],
    outputs: vec![],
    parameters: vec![
        NodeParameterDefinition::new(
            "width",
            title_case_name("width"),
            ParameterDefaultValue::Integer(8),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 256,
            },
        ),
        NodeParameterDefinition::new(
            "height",
            title_case_name("height"),
            ParameterDefaultValue::Integer(8),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 256,
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: Some(NodeRuntimeUpdateDefinition {
        auto_subscribe_in_editor: true,
        values: vec![NodeRuntimeValueDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![ValueKind::Color],
        }],
    }),
});

static FRAME_BRIGHTNESS_NODE_TYPE: LazyLock<NodeDefinition> = LazyLock::new(|| NodeDefinition {
    id: NodeTypeId::FRAME_BRIGHTNESS.to_owned(),
    display_name: "Frame Brightness".to_owned(),
    category: NodeCategory::FrameOperations,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "factor".to_owned(),
            display_name: title_case_name("factor"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

/// Returns the full shared schema catalog for all built-in node types.
///
/// Each call clones the lazily initialized static node definitions into a fresh vector so callers
/// can freely sort or modify their local copy.
pub fn builtin_node_definitions() -> Vec<NodeDefinition> {
    vec![
        (*FLOAT_CONSTANT_NODE_TYPE).clone(),
        (*COLOR_CONSTANT_NODE_TYPE).clone(),
        (*DISPLAY_NODE_TYPE).clone(),
        (*PLOT_NODE_TYPE).clone(),
        (*DELAY_NODE_TYPE).clone(),
        (*WLED_TARGET_NODE_TYPE).clone(),
        (*WLED_SINK_NODE_TYPE).clone(),
        (*AUDIO_FFT_RECEIVER_NODE_TYPE).clone(),
        (*HA_MQTT_NUMBER_NODE_TYPE).clone(),
        (*ADD_FLOAT_NODE_TYPE).clone(),
        (*SUBTRACT_FLOAT_NODE_TYPE).clone(),
        (*SIGNAL_GENERATOR_NODE_TYPE).clone(),
        (*DIVIDE_FLOAT_NODE_TYPE).clone(),
        (*MIN_MAX_FLOAT_NODE_TYPE).clone(),
        (*MULTIPLY_FLOAT_NODE_TYPE).clone(),
        (*ABS_FLOAT_NODE_TYPE).clone(),
        (*CLAMP_FLOAT_NODE_TYPE).clone(),
        (*POWER_FLOAT_NODE_TYPE).clone(),
        (*ROOT_FLOAT_NODE_TYPE).clone(),
        (*EXPONENTIAL_FLOAT_NODE_TYPE).clone(),
        (*LOG_FLOAT_NODE_TYPE).clone(),
        (*MAP_RANGE_FLOAT_NODE_TYPE).clone(),
        (*ROUND_FLOAT_NODE_TYPE).clone(),
        (*SCALE_TENSOR_NODE_TYPE).clone(),
        (*SCALE_COLOR_NODE_TYPE).clone(),
        (*MULTIPLY_COLOR_NODE_TYPE).clone(),
        (*TINT_FRAME_NODE_TYPE).clone(),
        (*MASK_FRAME_NODE_TYPE).clone(),
        (*MIX_COLOR_NODE_TYPE).clone(),
        (*ALPHA_OVER_NODE_TYPE).clone(),
        (*FADE_NODE_TYPE).clone(),
        (*MOVING_AVERAGE_NODE_TYPE).clone(),
        (*MOVING_MEDIAN_NODE_TYPE).clone(),
        (*BOX_BLUR_NODE_TYPE).clone(),
        (*GAUSSIAN_BLUR_NODE_TYPE).clone(),
        (*MEDIAN_FILTER_NODE_TYPE).clone(),
        (*SPECTRUM_ANALYZER_NODE_TYPE).clone(),
        (*SOLID_FRAME_NODE_TYPE).clone(),
        (*RAINBOW_SWEEP_NODE_TYPE).clone(),
        (*CIRCLE_SWEEP_NODE_TYPE).clone(),
        (*LEVEL_BAR_NODE_TYPE).clone(),
        (*TWINKLE_STARS_NODE_TYPE).clone(),
        (*PLASMA_NODE_TYPE).clone(),
        (*BOUNCING_BALLS_NODE_TYPE).clone(),
        (*WLED_DUMMY_DISPLAY_NODE_TYPE).clone(),
        (*FRAME_BRIGHTNESS_NODE_TYPE).clone(),
    ]
}

/// Returns the built-in node definition for the requested node type identifier, if it exists.
pub fn builtin_node_definition(node_type_id: &str) -> Option<NodeDefinition> {
    builtin_node_definitions()
        .into_iter()
        .find(|definition| definition.id == node_type_id)
}

/// Default gradient used by the linear and circular sweep animations.
const DEFAULT_RAINBOW_GRADIENT_STOPS: &[ColorGradientStop] = &[
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
        position: 0.2,
        color: RgbaColor {
            r: 1.0,
            g: 0.5,
            b: 0.0,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.4,
        color: RgbaColor {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.6,
        color: RgbaColor {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.8,
        color: RgbaColor {
            r: 0.0,
            g: 0.4,
            b: 1.0,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 1.0,
        color: RgbaColor {
            r: 0.7,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
    },
];

/// Default gradient used by the plasma animation.
const DEFAULT_PLASMA_GRADIENT_STOPS: &[ColorGradientStop] = &[
    ColorGradientStop {
        position: 0.0,
        color: RgbaColor {
            r: 0.02,
            g: 0.0,
            b: 0.1,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.3,
        color: RgbaColor {
            r: 0.3,
            g: 0.0,
            b: 0.6,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.55,
        color: RgbaColor {
            r: 0.0,
            g: 0.7,
            b: 0.9,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.8,
        color: RgbaColor {
            r: 1.0,
            g: 0.5,
            b: 0.0,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 1.0,
        color: RgbaColor {
            r: 1.0,
            g: 0.95,
            b: 0.4,
            a: 1.0,
        },
    },
];

/// Default gradient used by the twinkle animation.
const DEFAULT_TWINKLE_GRADIENT_STOPS: &[ColorGradientStop] = &[
    ColorGradientStop {
        position: 0.0,
        color: RgbaColor {
            r: 0.65,
            g: 0.75,
            b: 1.0,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.5,
        color: RgbaColor {
            r: 1.0,
            g: 0.95,
            b: 0.85,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 1.0,
        color: RgbaColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    },
];

/// Default gradient used by the bouncing balls animation.
const DEFAULT_BOUNCING_BALLS_GRADIENT_STOPS: &[ColorGradientStop] = &[
    ColorGradientStop {
        position: 0.0,
        color: RgbaColor {
            r: 0.09,
            g: 0.64,
            b: 0.98,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 0.5,
        color: RgbaColor {
            r: 0.98,
            g: 0.73,
            b: 0.16,
            a: 1.0,
        },
    },
    ColorGradientStop {
        position: 1.0,
        color: RgbaColor {
            r: 0.96,
            g: 0.24,
            b: 0.43,
            a: 1.0,
        },
    },
];
