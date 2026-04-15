use super::{
    ColorGradientStop, EnumOption, FloatTensor, InputKindRef, InputValue, NodeCategory,
    NodeConnectionDefinition, NodeDefinition, NodeInputDefinition, NodeOutputDefinition,
    NodeParameter, NodeParameterDefinition, NodeRuntimeUpdateDefinition,
    NodeRuntimeValueDefinition, NodeSchema, NodeTypeId, OutputInference, ParameterDefaultValue,
    ParameterUiHint, ParameterVisibilityCondition, RgbaColor, ValueKind, infer_numeric_output_kind,
    infer_preferred_kind, input_kind, parameter_string,
};
use serde_json::json;
use std::sync::LazyLock;

/// Returns the canonical opaque-white default input value used by the node catalog.
fn white_input() -> InputValue {
    InputValue::Color(RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    })
}

/// Returns the canonical fully transparent color used by the node catalog.
fn transparent_input() -> InputValue {
    InputValue::Color(RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    })
}

/// Returns the canonical single-element zero tensor used by the node catalog.
fn default_tensor_input() -> InputValue {
    InputValue::FloatTensor(FloatTensor {
        shape: vec![1],
        values: vec![0.0],
    })
}

/// Converts a snake_case field name into the title-cased label used by node schema definitions.
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

/// Enumerates the waveform options exposed by the signal generator node.
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

static IMAGE_FIT_MODES: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "stretch".to_owned(),
            label: "Stretch".to_owned(),
        },
        EnumOption {
            value: "contain".to_owned(),
            label: "Contain".to_owned(),
        },
        EnumOption {
            value: "cover".to_owned(),
            label: "Cover".to_owned(),
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

static FRAME_CHANNEL_OPTIONS: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "r".to_owned(),
            label: "Red".to_owned(),
        },
        EnumOption {
            value: "g".to_owned(),
            label: "Green".to_owned(),
        },
        EnumOption {
            value: "b".to_owned(),
            label: "Blue".to_owned(),
        },
        EnumOption {
            value: "a".to_owned(),
            label: "Alpha".to_owned(),
        },
    ]
});

static FRAME_SET_CHANNEL_OPTIONS: LazyLock<Vec<EnumOption>> = LazyLock::new(|| {
    vec![
        EnumOption {
            value: "r".to_owned(),
            label: "Red".to_owned(),
        },
        EnumOption {
            value: "g".to_owned(),
            label: "Green".to_owned(),
        },
        EnumOption {
            value: "b".to_owned(),
            label: "Blue".to_owned(),
        },
        EnumOption {
            value: "a".to_owned(),
            label: "Alpha".to_owned(),
        },
        EnumOption {
            value: "h".to_owned(),
            label: "Hue".to_owned(),
        },
        EnumOption {
            value: "s".to_owned(),
            label: "Saturation".to_owned(),
        },
        EnumOption {
            value: "v".to_owned(),
            label: "Value".to_owned(),
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

static FLOAT_CONSTANT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::FLOAT_CONSTANT.to_owned(),
    display_name: "Float Constant".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: false,
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

static COLOR_CONSTANT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::COLOR_CONSTANT.to_owned(),
    display_name: "Color Constant".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: false,
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

static IMAGE_SOURCE_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::IMAGE_SOURCE.to_owned(),
    display_name: "Image Source".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: true,
    inputs: vec![],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "asset_id",
            title_case_name("asset_id"),
            ParameterDefaultValue::String(String::new()),
            ParameterUiHint::ImageAssetUpload,
        ),
        NodeParameterDefinition::new(
            "fit_mode",
            title_case_name("fit_mode"),
            ParameterDefaultValue::String("contain".to_owned()),
            ParameterUiHint::EnumSelect {
                options: IMAGE_FIT_MODES.clone(),
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static DISPLAY_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::DISPLAY.to_owned(),
    display_name: "Display".to_owned(),
    category: NodeCategory::Outputs,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![
            ValueKind::FloatTensor,
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
                ValueKind::FloatTensor,
                ValueKind::Color,
                ValueKind::ColorFrame,
                ValueKind::LedLayout,
            ],
        }],
    }),
});

static PLOT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::PLOT.to_owned(),
    display_name: "Plot".to_owned(),
    category: NodeCategory::Outputs,
    needs_io: false,
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

static DELAY_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::DELAY.to_owned(),
    display_name: "Delay".to_owned(),
    category: NodeCategory::TemporalFilters,
    needs_io: false,
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
    parameters: vec![
        NodeParameterDefinition::new(
            "ticks",
            title_case_name("ticks"),
            ParameterDefaultValue::Integer(1),
            ParameterUiHint::IntegerDrag {
                speed: 1.0,
                min: 1,
                max: 10_000,
            },
        ),
        NodeParameterDefinition::new(
            "initial_type",
            title_case_name("initial_type"),
            ParameterDefaultValue::String("float".to_owned()),
            ParameterUiHint::EnumSelect {
                options: vec![
                    EnumOption {
                        value: "float".to_owned(),
                        label: "Float".to_owned(),
                    },
                    EnumOption {
                        value: "tensor".to_owned(),
                        label: "Tensor".to_owned(),
                    },
                    EnumOption {
                        value: "colorframe".to_owned(),
                        label: "ColorFrame".to_owned(),
                    },
                ],
            },
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static DIFFERENTIATE_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::DIFFERENTIATE.to_owned(),
    display_name: "Differentiate".to_owned(),
    category: NodeCategory::TemporalFilters,
    needs_io: false,
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

static WLED_TARGET_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::WLED_TARGET.to_owned(),
    display_name: "Wled Target".to_owned(),
    category: NodeCategory::Outputs,
    needs_io: true,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![ValueKind::Color],
            default_value: None,
        },
        NodeInputDefinition {
            name: "disable".to_owned(),
            display_name: title_case_name("disable"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
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

static WLED_SINK_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::WLED_SINK.to_owned(),
    display_name: "Wled Sink".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: true,
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

static AUDIO_FFT_RECEIVER_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::AUDIO_FFT_RECEIVER.to_owned(),
    display_name: "Audio FFT Receiver".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: true,
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

static HA_MQTT_NUMBER_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::HA_MQTT_NUMBER.to_owned(),
    display_name: "Home Assistant MQTT Number".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: true,
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
            ParameterDefaultValue::String("luma_weaver_number".to_owned()),
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

static BINARY_SELECT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::BINARY_SELECT.to_owned(),
    display_name: "Binary Select".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "selector".to_owned(),
            display_name: title_case_name("selector"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![],
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

static ADD_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::ADD.to_owned(),
    display_name: "Add".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "sum".to_owned(),
        display_name: title_case_name("sum"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![
            ValueKind::Float,
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

static SUBTRACT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SUBTRACT.to_owned(),
    display_name: "Subtract".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "difference".to_owned(),
        display_name: title_case_name("difference"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![
            ValueKind::Float,
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

static SIGNAL_GENERATOR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SIGNAL_GENERATOR.to_owned(),
    display_name: "Signal Generator".to_owned(),
    category: NodeCategory::Inputs,
    needs_io: false,
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

static DIVIDE_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::DIVIDE.to_owned(),
    display_name: "Divide".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "quotient".to_owned(),
        display_name: title_case_name("quotient"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MIN_MAX_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MIN_MAX.to_owned(),
    display_name: "Min/Max".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
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

static MULTIPLY_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MULTIPLY.to_owned(),
    display_name: "Multiply".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "a".to_owned(),
            display_name: title_case_name("a"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "b".to_owned(),
            display_name: title_case_name("b"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "product".to_owned(),
        display_name: title_case_name("product"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![
            ValueKind::Float,
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

static ABS_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::ABS.to_owned(),
    display_name: "Abs".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static CLAMP_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::CLAMP.to_owned(),
    display_name: "Clamp".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "min".to_owned(),
            display_name: title_case_name("min"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "max".to_owned(),
            display_name: title_case_name("max"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![
            ValueKind::Float,
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

static POWER_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::POWER.to_owned(),
    display_name: "Power".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "base".to_owned(),
            display_name: title_case_name("base"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "exponent".to_owned(),
            display_name: title_case_name("exponent"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ROOT_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::ROOT.to_owned(),
    display_name: "Root".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "degree".to_owned(),
            display_name: title_case_name("degree"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(2.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static EXPONENTIAL_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::EXPONENTIAL.to_owned(),
    display_name: "Exponential".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static LOG_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::LOG.to_owned(),
    display_name: "Log".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "base".to_owned(),
            display_name: title_case_name("base"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(std::f32::consts::E)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MAP_RANGE_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MAP_RANGE.to_owned(),
    display_name: "Map Range".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "source_min".to_owned(),
            display_name: title_case_name("source_min"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "source_max".to_owned(),
            display_name: title_case_name("source_max"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
        NodeInputDefinition {
            name: "target_min".to_owned(),
            display_name: title_case_name("target_min"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "target_max".to_owned(),
            display_name: title_case_name("target_max"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![ValueKind::FloatTensor],
            default_value: Some(InputValue::Float(1.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
    }],
    parameters: vec![],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static ROUND_FLOAT_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::ROUND.to_owned(),
    display_name: "Round".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
        default_value: Some(InputValue::Float(0.0)),
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
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

static SCALE_TENSOR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SCALE_TENSOR.to_owned(),
    display_name: "Scale Tensor".to_owned(),
    category: NodeCategory::Math,
    needs_io: false,
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

static SCALE_COLOR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SCALE_COLOR.to_owned(),
    display_name: "Scale Color".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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

static MULTIPLY_COLOR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MULTIPLY_COLOR.to_owned(),
    display_name: "Multiply Color".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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

static TINT_FRAME_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::TINT_FRAME.to_owned(),
    display_name: "Tint Frame".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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

static EXTRACT_CHANNELS_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::EXTRACT_CHANNELS.to_owned(),
    display_name: "Extract Channels".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
        default_value: None,
    }],
    outputs: vec![NodeOutputDefinition {
        name: "tensor".to_owned(),
        display_name: title_case_name("tensor"),
        value_kind: ValueKind::FloatTensor,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "channel",
        title_case_name("channel"),
        ParameterDefaultValue::String("r".to_owned()),
        ParameterUiHint::EnumSelect {
            options: FRAME_CHANNEL_OPTIONS.clone(),
        },
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MASK_FRAME_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MASK_FRAME.to_owned(),
    display_name: "Mask Frame".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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
            accepted_kinds: vec![ValueKind::FloatTensor],
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

static SET_CHANNEL_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SET_CHANNEL.to_owned(),
    display_name: "Set Channel".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "frame".to_owned(),
            display_name: title_case_name("frame"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![],
            default_value: None,
        },
        NodeInputDefinition {
            name: "tensor".to_owned(),
            display_name: title_case_name("tensor"),
            value_kind: ValueKind::FloatTensor,
            accepted_kinds: vec![],
            default_value: Some(default_tensor_input()),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![],
    }],
    parameters: vec![NodeParameterDefinition::new(
        "channel",
        title_case_name("channel"),
        ParameterDefaultValue::String("r".to_owned()),
        ParameterUiHint::EnumSelect {
            options: FRAME_SET_CHANNEL_OPTIONS.clone(),
        },
    )],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static COLORIZE_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::COLORIZE.to_owned(),
    display_name: "Colorize".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Float,
        accepted_kinds: vec![ValueKind::FloatTensor],
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

static MIX_COLOR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MIX_COLOR.to_owned(),
    display_name: "Mix Color".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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

static ALPHA_OVER_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::ALPHA_OVER.to_owned(),
    display_name: "Alpha Over".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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

static FADE_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::FADE.to_owned(),
    display_name: "Fade".to_owned(),
    category: NodeCategory::TemporalFilters,
    needs_io: false,
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

static INTEGRATE_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::INTEGRATE.to_owned(),
    display_name: "Integrate".to_owned(),
    category: NodeCategory::TemporalFilters,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "rate".to_owned(),
            display_name: title_case_name("rate"),
            value_kind: ValueKind::Any,
            accepted_kinds: vec![
                ValueKind::Float,
                ValueKind::FloatTensor,
                ValueKind::ColorFrame,
            ],
            default_value: Some(InputValue::Float(0.0)),
        },
        NodeInputDefinition {
            name: "reset".to_owned(),
            display_name: title_case_name("reset"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![
            ValueKind::Float,
            ValueKind::FloatTensor,
            ValueKind::ColorFrame,
        ],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "initial_value",
            title_case_name("initial_value"),
            ParameterDefaultValue::Float(0.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        ),
        NodeParameterDefinition::new(
            "clamp_output",
            title_case_name("clamp_output"),
            ParameterDefaultValue::Bool(false),
            ParameterUiHint::Checkbox,
        ),
        NodeParameterDefinition::new(
            "min",
            title_case_name("min"),
            ParameterDefaultValue::Float(-1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        )
        .visible_when(ParameterVisibilityCondition::Equals {
            parameter: "clamp_output".to_owned(),
            value: json!(true),
        }),
        NodeParameterDefinition::new(
            "max",
            title_case_name("max"),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: -10_000.0,
                max: 10_000.0,
            },
        )
        .visible_when(ParameterVisibilityCondition::Equals {
            parameter: "clamp_output".to_owned(),
            value: json!(true),
        }),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static MOVING_AVERAGE_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MOVING_AVERAGE.to_owned(),
    display_name: "Moving Average".to_owned(),
    category: NodeCategory::TemporalFilters,
    needs_io: false,
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

static MOVING_MEDIAN_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MOVING_MEDIAN.to_owned(),
    display_name: "Moving Median".to_owned(),
    category: NodeCategory::TemporalFilters,
    needs_io: false,
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

static BOX_BLUR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::BOX_BLUR.to_owned(),
    display_name: "Box Blur".to_owned(),
    category: NodeCategory::SpatialFilters,
    needs_io: false,
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

static GAUSSIAN_BLUR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::GAUSSIAN_BLUR.to_owned(),
    display_name: "Gaussian Blur".to_owned(),
    category: NodeCategory::SpatialFilters,
    needs_io: false,
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

static MEDIAN_FILTER_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::MEDIAN_FILTER.to_owned(),
    display_name: "Median Filter".to_owned(),
    category: NodeCategory::SpatialFilters,
    needs_io: false,
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

static LAPLACIAN_FILTER_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::LAPLACIAN_FILTER.to_owned(),
    display_name: "Laplacian Filter".to_owned(),
    category: NodeCategory::SpatialFilters,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "frame".to_owned(),
        display_name: title_case_name("frame"),
        value_kind: ValueKind::ColorFrame,
        accepted_kinds: vec![ValueKind::FloatTensor],
        default_value: None,
    }],
    outputs: vec![NodeOutputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![ValueKind::ColorFrame, ValueKind::FloatTensor],
    }],
    parameters: vec![
        NodeParameterDefinition::new(
            "strength",
            title_case_name("strength"),
            ParameterDefaultValue::Float(1.0),
            ParameterUiHint::DragFloat {
                speed: 0.01,
                min: 0.0,
                max: 8.0,
            },
        ),
        NodeParameterDefinition::new(
            "absolute_value",
            title_case_name("absolute_value"),
            ParameterDefaultValue::Bool(true),
            ParameterUiHint::Checkbox,
        ),
        NodeParameterDefinition::new(
            "filter_alpha",
            title_case_name("filter_alpha"),
            ParameterDefaultValue::Bool(false),
            ParameterUiHint::Checkbox,
        ),
    ],
    connection: NodeConnectionDefinition {
        max_input_connections: 1,
        require_value_kind_match: true,
    },
    runtime_updates: None,
});

static SPECTRUM_ANALYZER_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SPECTRUM_ANALYZER.to_owned(),
    display_name: "Spectrum Analyzer".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static SOLID_FRAME_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::SOLID_FRAME.to_owned(),
    display_name: "Solid Frame".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static RAINBOW_SWEEP_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::RAINBOW_SWEEP.to_owned(),
    display_name: "Linear Sweep".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static CIRCLE_SWEEP_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::CIRCLE_SWEEP.to_owned(),
    display_name: "Circle Sweep".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static LEVEL_BAR_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::LEVEL_BAR.to_owned(),
    display_name: "Level Bar".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static TWINKLE_STARS_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::TWINKLE_STARS.to_owned(),
    display_name: "Twinkle Stars".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static PLASMA_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::PLASMA.to_owned(),
    display_name: "Plasma".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static BOUNCING_BALLS_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::BOUNCING_BALLS.to_owned(),
    display_name: "Bouncing Balls".to_owned(),
    category: NodeCategory::Generators,
    needs_io: false,
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

static WLED_DUMMY_DISPLAY_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::WLED_DUMMY_DISPLAY.to_owned(),
    display_name: "Wled Dummy Display".to_owned(),
    category: NodeCategory::Debug,
    needs_io: false,
    inputs: vec![
        NodeInputDefinition {
            name: "value".to_owned(),
            display_name: title_case_name("value"),
            value_kind: ValueKind::ColorFrame,
            accepted_kinds: vec![ValueKind::Color],
            default_value: None,
        },
        NodeInputDefinition {
            name: "disable".to_owned(),
            display_name: title_case_name("disable"),
            value_kind: ValueKind::Float,
            accepted_kinds: vec![],
            default_value: Some(InputValue::Float(0.0)),
        },
    ],
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

static TYPE_DEBUG_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::TYPE_DEBUG.to_owned(),
    display_name: "Type Debug".to_owned(),
    category: NodeCategory::Debug,
    needs_io: false,
    inputs: vec![NodeInputDefinition {
        name: "value".to_owned(),
        display_name: title_case_name("value"),
        value_kind: ValueKind::Any,
        accepted_kinds: vec![],
        default_value: None,
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
            name: "type".to_owned(),
            display_name: title_case_name("type"),
            value_kind: ValueKind::String,
            accepted_kinds: vec![],
        }],
    }),
});

static FRAME_BRIGHTNESS_NODE_TYPE: LazyLock<NodeSchema> = LazyLock::new(|| NodeSchema {
    id: NodeTypeId::FRAME_BRIGHTNESS.to_owned(),
    display_name: "Frame Brightness".to_owned(),
    category: NodeCategory::FrameOperations,
    needs_io: false,
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

macro_rules! default_node_definition {
    ($instance:ident, $type_name:ident, $schema:ident) => {
        struct $type_name;

        impl NodeDefinition for $type_name {
            fn schema(&self) -> &'static NodeSchema {
                &$schema
            }
        }

        static $instance: $type_name = $type_name;
    };
}

macro_rules! numeric_node_definition {
    ($instance:ident, $type_name:ident, $schema:ident) => {
        struct $type_name;

        impl NodeDefinition for $type_name {
            fn schema(&self) -> &'static NodeSchema {
                &$schema
            }

            fn infer_output(
                &self,
                output_name: &str,
                input_kinds: &[InputKindRef<'_>],
                _parameters: &[NodeParameter],
            ) -> OutputInference {
                let Some(output) = self.schema().output_port(output_name) else {
                    return OutputInference::Unavailable;
                };
                OutputInference::Resolved(infer_numeric_output_kind(
                    self.schema().id.as_str(),
                    output,
                    input_kinds,
                ))
            }
        }

        static $instance: $type_name = $type_name;
    };
}

default_node_definition!(
    FLOAT_CONSTANT_NODE_DEFINITION,
    FloatConstantNodeDefinition,
    FLOAT_CONSTANT_NODE_TYPE
);
default_node_definition!(
    COLOR_CONSTANT_NODE_DEFINITION,
    ColorConstantNodeDefinition,
    COLOR_CONSTANT_NODE_TYPE
);
default_node_definition!(
    IMAGE_SOURCE_NODE_DEFINITION,
    ImageSourceNodeDefinition,
    IMAGE_SOURCE_NODE_TYPE
);
default_node_definition!(PLOT_NODE_DEFINITION, PlotNodeDefinition, PLOT_NODE_TYPE);
default_node_definition!(
    DIFFERENTIATE_NODE_DEFINITION,
    DifferentiateNodeDefinition,
    DIFFERENTIATE_NODE_TYPE
);
default_node_definition!(
    WLED_TARGET_NODE_DEFINITION,
    WledTargetNodeDefinition,
    WLED_TARGET_NODE_TYPE
);
default_node_definition!(
    WLED_SINK_NODE_DEFINITION,
    WledSinkNodeDefinition,
    WLED_SINK_NODE_TYPE
);
default_node_definition!(
    AUDIO_FFT_RECEIVER_NODE_DEFINITION,
    AudioFftReceiverNodeDefinition,
    AUDIO_FFT_RECEIVER_NODE_TYPE
);
default_node_definition!(
    HA_MQTT_NUMBER_NODE_DEFINITION,
    HaMqttNumberNodeDefinition,
    HA_MQTT_NUMBER_NODE_TYPE
);
default_node_definition!(
    SIGNAL_GENERATOR_NODE_DEFINITION,
    SignalGeneratorNodeDefinition,
    SIGNAL_GENERATOR_NODE_TYPE
);
default_node_definition!(
    SCALE_TENSOR_NODE_DEFINITION,
    ScaleTensorNodeDefinition,
    SCALE_TENSOR_NODE_TYPE
);
default_node_definition!(
    SCALE_COLOR_NODE_DEFINITION,
    ScaleColorNodeDefinition,
    SCALE_COLOR_NODE_TYPE
);
default_node_definition!(
    MULTIPLY_COLOR_NODE_DEFINITION,
    MultiplyColorNodeDefinition,
    MULTIPLY_COLOR_NODE_TYPE
);
default_node_definition!(
    TINT_FRAME_NODE_DEFINITION,
    TintFrameNodeDefinition,
    TINT_FRAME_NODE_TYPE
);
default_node_definition!(
    EXTRACT_CHANNELS_NODE_DEFINITION,
    ExtractChannelsNodeDefinition,
    EXTRACT_CHANNELS_NODE_TYPE
);
default_node_definition!(
    MASK_FRAME_NODE_DEFINITION,
    MaskFrameNodeDefinition,
    MASK_FRAME_NODE_TYPE
);
default_node_definition!(
    SET_CHANNEL_NODE_DEFINITION,
    SetChannelNodeDefinition,
    SET_CHANNEL_NODE_TYPE
);
default_node_definition!(
    COLORIZE_NODE_DEFINITION,
    ColorizeNodeDefinition,
    COLORIZE_NODE_TYPE
);
default_node_definition!(
    MIX_COLOR_NODE_DEFINITION,
    MixColorNodeDefinition,
    MIX_COLOR_NODE_TYPE
);
default_node_definition!(
    ALPHA_OVER_NODE_DEFINITION,
    AlphaOverNodeDefinition,
    ALPHA_OVER_NODE_TYPE
);
default_node_definition!(
    BOX_BLUR_NODE_DEFINITION,
    BoxBlurNodeDefinition,
    BOX_BLUR_NODE_TYPE
);
default_node_definition!(
    GAUSSIAN_BLUR_NODE_DEFINITION,
    GaussianBlurNodeDefinition,
    GAUSSIAN_BLUR_NODE_TYPE
);
default_node_definition!(
    MEDIAN_FILTER_NODE_DEFINITION,
    MedianFilterNodeDefinition,
    MEDIAN_FILTER_NODE_TYPE
);
default_node_definition!(
    SPECTRUM_ANALYZER_NODE_DEFINITION,
    SpectrumAnalyzerNodeDefinition,
    SPECTRUM_ANALYZER_NODE_TYPE
);
default_node_definition!(
    SOLID_FRAME_NODE_DEFINITION,
    SolidFrameNodeDefinition,
    SOLID_FRAME_NODE_TYPE
);
default_node_definition!(
    RAINBOW_SWEEP_NODE_DEFINITION,
    RainbowSweepNodeDefinition,
    RAINBOW_SWEEP_NODE_TYPE
);
default_node_definition!(
    CIRCLE_SWEEP_NODE_DEFINITION,
    CircleSweepNodeDefinition,
    CIRCLE_SWEEP_NODE_TYPE
);
default_node_definition!(
    LEVEL_BAR_NODE_DEFINITION,
    LevelBarNodeDefinition,
    LEVEL_BAR_NODE_TYPE
);
default_node_definition!(
    TWINKLE_STARS_NODE_DEFINITION,
    TwinkleStarsNodeDefinition,
    TWINKLE_STARS_NODE_TYPE
);
default_node_definition!(
    PLASMA_NODE_DEFINITION,
    PlasmaNodeDefinition,
    PLASMA_NODE_TYPE
);
default_node_definition!(
    BOUNCING_BALLS_NODE_DEFINITION,
    BouncingBallsNodeDefinition,
    BOUNCING_BALLS_NODE_TYPE
);
default_node_definition!(
    WLED_DUMMY_DISPLAY_NODE_DEFINITION,
    WledDummyDisplayNodeDefinition,
    WLED_DUMMY_DISPLAY_NODE_TYPE
);
default_node_definition!(
    TYPE_DEBUG_NODE_DEFINITION,
    TypeDebugNodeDefinition,
    TYPE_DEBUG_NODE_TYPE
);
default_node_definition!(
    FRAME_BRIGHTNESS_NODE_DEFINITION,
    FrameBrightnessNodeDefinition,
    FRAME_BRIGHTNESS_NODE_TYPE
);

numeric_node_definition!(ADD_NODE_DEFINITION, AddNodeDefinition, ADD_NODE_TYPE);
numeric_node_definition!(
    SUBTRACT_NODE_DEFINITION,
    SubtractNodeDefinition,
    SUBTRACT_NODE_TYPE
);
numeric_node_definition!(
    DIVIDE_NODE_DEFINITION,
    DivideNodeDefinition,
    DIVIDE_FLOAT_NODE_TYPE
);
numeric_node_definition!(
    MIN_MAX_NODE_DEFINITION,
    MinMaxNodeDefinition,
    MIN_MAX_FLOAT_NODE_TYPE
);
numeric_node_definition!(
    MULTIPLY_NODE_DEFINITION,
    MultiplyNodeDefinition,
    MULTIPLY_NODE_TYPE
);
numeric_node_definition!(ABS_NODE_DEFINITION, AbsNodeDefinition, ABS_FLOAT_NODE_TYPE);
numeric_node_definition!(CLAMP_NODE_DEFINITION, ClampNodeDefinition, CLAMP_NODE_TYPE);
numeric_node_definition!(
    POWER_NODE_DEFINITION,
    PowerNodeDefinition,
    POWER_FLOAT_NODE_TYPE
);
numeric_node_definition!(
    ROOT_NODE_DEFINITION,
    RootNodeDefinition,
    ROOT_FLOAT_NODE_TYPE
);
numeric_node_definition!(
    EXPONENTIAL_NODE_DEFINITION,
    ExponentialNodeDefinition,
    EXPONENTIAL_FLOAT_NODE_TYPE
);
numeric_node_definition!(LOG_NODE_DEFINITION, LogNodeDefinition, LOG_FLOAT_NODE_TYPE);
numeric_node_definition!(
    MAP_RANGE_NODE_DEFINITION,
    MapRangeNodeDefinition,
    MAP_RANGE_FLOAT_NODE_TYPE
);
numeric_node_definition!(
    ROUND_NODE_DEFINITION,
    RoundNodeDefinition,
    ROUND_FLOAT_NODE_TYPE
);

struct BinarySelectNodeDefinition;

impl NodeDefinition for BinarySelectNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &BINARY_SELECT_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        _parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };
        let a_kind = input_kind(input_kinds, "a");
        let b_kind = input_kind(input_kinds, "b");

        if let (Some(a_kind), Some(b_kind)) = (a_kind, b_kind) {
            if a_kind != b_kind {
                return OutputInference::Invalid {
                    message: format!(
                        "binary select inputs 'a' and 'b' must resolve to the same kind, found {:?} and {:?}",
                        a_kind, b_kind
                    ),
                };
            }
        }

        OutputInference::Resolved(infer_preferred_kind(output, &[a_kind, b_kind]))
    }
}

static BINARY_SELECT_NODE_DEFINITION: BinarySelectNodeDefinition = BinarySelectNodeDefinition;

struct DelayNodeDefinition;

impl NodeDefinition for DelayNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &DELAY_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        _input_kinds: &[InputKindRef<'_>],
        parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };

        match output_name {
            "value" => OutputInference::Resolved(
                match parameter_string(parameters, "initial_type").as_deref() {
                    Some("tensor") => ValueKind::FloatTensor,
                    Some("colorframe") => ValueKind::ColorFrame,
                    _ => ValueKind::Float,
                },
            ),
            _ => OutputInference::Resolved(output.value_kind),
        }
    }
}

static DELAY_NODE_DEFINITION: DelayNodeDefinition = DelayNodeDefinition;

struct FadeNodeDefinition;

impl NodeDefinition for FadeNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &FADE_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        _parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };

        match input_kind(input_kinds, "value") {
            Some(ValueKind::Color) | Some(ValueKind::ColorFrame) => OutputInference::Resolved(
                infer_preferred_kind(output, &[input_kind(input_kinds, "value")]),
            ),
            Some(kind) => OutputInference::Invalid {
                message: format!(
                    "fade input 'value' must resolve to Color or ColorFrame, found {:?}",
                    kind
                ),
            },
            None => OutputInference::Resolved(output.value_kind),
        }
    }
}

static FADE_NODE_DEFINITION: FadeNodeDefinition = FadeNodeDefinition;

struct IntegrateNodeDefinition;

impl NodeDefinition for IntegrateNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &INTEGRATE_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        _parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };

        match input_kind(input_kinds, "rate") {
            Some(ValueKind::Float) | Some(ValueKind::FloatTensor) | Some(ValueKind::ColorFrame) => {
                OutputInference::Resolved(infer_preferred_kind(
                    output,
                    &[input_kind(input_kinds, "rate")],
                ))
            }
            Some(kind) => OutputInference::Invalid {
                message: format!(
                    "integrate input 'rate' must resolve to Float, FloatTensor, or ColorFrame, found {:?}",
                    kind
                ),
            },
            None => OutputInference::Resolved(output.value_kind),
        }
    }
}

static INTEGRATE_NODE_DEFINITION: IntegrateNodeDefinition = IntegrateNodeDefinition;

struct MovingAverageNodeDefinition;

impl NodeDefinition for MovingAverageNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &MOVING_AVERAGE_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        _parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };

        match input_kind(input_kinds, "value") {
            Some(ValueKind::Float)
            | Some(ValueKind::Color)
            | Some(ValueKind::FloatTensor)
            | Some(ValueKind::ColorFrame) => OutputInference::Resolved(infer_preferred_kind(
                output,
                &[input_kind(input_kinds, "value")],
            )),
            Some(kind) => OutputInference::Invalid {
                message: format!(
                    "moving average input 'value' must resolve to Float, Color, FloatTensor, or ColorFrame, found {:?}",
                    kind
                ),
            },
            None => OutputInference::Resolved(output.value_kind),
        }
    }
}

static MOVING_AVERAGE_NODE_DEFINITION: MovingAverageNodeDefinition = MovingAverageNodeDefinition;

struct MovingMedianNodeDefinition;

impl NodeDefinition for MovingMedianNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &MOVING_MEDIAN_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        _parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };

        match input_kind(input_kinds, "value") {
            Some(ValueKind::Float) | Some(ValueKind::FloatTensor) => OutputInference::Resolved(
                infer_preferred_kind(output, &[input_kind(input_kinds, "value")]),
            ),
            Some(kind) => OutputInference::Invalid {
                message: format!(
                    "moving median input 'value' must resolve to Float or FloatTensor, found {:?}",
                    kind
                ),
            },
            None => OutputInference::Resolved(output.value_kind),
        }
    }
}

static MOVING_MEDIAN_NODE_DEFINITION: MovingMedianNodeDefinition = MovingMedianNodeDefinition;

struct LaplacianFilterNodeDefinition;

impl NodeDefinition for LaplacianFilterNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &LAPLACIAN_FILTER_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        _parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };

        match output_name {
            "value" => OutputInference::Resolved(infer_preferred_kind(
                output,
                &[input_kind(input_kinds, "frame")],
            )),
            _ => OutputInference::Resolved(output.value_kind),
        }
    }
}

static LAPLACIAN_FILTER_NODE_DEFINITION: LaplacianFilterNodeDefinition =
    LaplacianFilterNodeDefinition;

struct DisplayNodeDefinition;

impl NodeDefinition for DisplayNodeDefinition {
    fn schema(&self) -> &'static NodeSchema {
        &DISPLAY_NODE_TYPE
    }

    fn infer_output(
        &self,
        output_name: &str,
        input_kinds: &[InputKindRef<'_>],
        parameters: &[NodeParameter],
    ) -> OutputInference {
        let Some(output) = self.schema().output_port(output_name) else {
            return OutputInference::Unavailable;
        };
        let mode = parameter_string(parameters, "mode");

        match output_name {
            "value" => {
                if mode.as_deref() == Some("plot") {
                    OutputInference::Resolved(ValueKind::Float)
                } else {
                    OutputInference::Resolved(infer_preferred_kind(
                        output,
                        &[input_kind(input_kinds, "value")],
                    ))
                }
            }
            _ => OutputInference::Resolved(output.value_kind),
        }
    }
}

static DISPLAY_NODE_DEFINITION: DisplayNodeDefinition = DisplayNodeDefinition;

/// Returns the full shared schema catalog for all node types.
///
/// Each call clones the lazily initialized static node definitions into a fresh vector so callers
/// can freely sort or modify their local copy.
pub fn node_definitions() -> Vec<NodeSchema> {
    vec![
        FLOAT_CONSTANT_NODE_DEFINITION.schema().clone(),
        COLOR_CONSTANT_NODE_DEFINITION.schema().clone(),
        IMAGE_SOURCE_NODE_DEFINITION.schema().clone(),
        DISPLAY_NODE_DEFINITION.schema().clone(),
        PLOT_NODE_DEFINITION.schema().clone(),
        DELAY_NODE_DEFINITION.schema().clone(),
        DIFFERENTIATE_NODE_DEFINITION.schema().clone(),
        WLED_TARGET_NODE_DEFINITION.schema().clone(),
        WLED_SINK_NODE_DEFINITION.schema().clone(),
        AUDIO_FFT_RECEIVER_NODE_DEFINITION.schema().clone(),
        HA_MQTT_NUMBER_NODE_DEFINITION.schema().clone(),
        BINARY_SELECT_NODE_DEFINITION.schema().clone(),
        ADD_NODE_DEFINITION.schema().clone(),
        SUBTRACT_NODE_DEFINITION.schema().clone(),
        SIGNAL_GENERATOR_NODE_DEFINITION.schema().clone(),
        DIVIDE_NODE_DEFINITION.schema().clone(),
        MIN_MAX_NODE_DEFINITION.schema().clone(),
        MULTIPLY_NODE_DEFINITION.schema().clone(),
        ABS_NODE_DEFINITION.schema().clone(),
        CLAMP_NODE_DEFINITION.schema().clone(),
        POWER_NODE_DEFINITION.schema().clone(),
        ROOT_NODE_DEFINITION.schema().clone(),
        EXPONENTIAL_NODE_DEFINITION.schema().clone(),
        LOG_NODE_DEFINITION.schema().clone(),
        MAP_RANGE_NODE_DEFINITION.schema().clone(),
        ROUND_NODE_DEFINITION.schema().clone(),
        SCALE_TENSOR_NODE_DEFINITION.schema().clone(),
        SCALE_COLOR_NODE_DEFINITION.schema().clone(),
        MULTIPLY_COLOR_NODE_DEFINITION.schema().clone(),
        TINT_FRAME_NODE_DEFINITION.schema().clone(),
        EXTRACT_CHANNELS_NODE_DEFINITION.schema().clone(),
        SET_CHANNEL_NODE_DEFINITION.schema().clone(),
        COLORIZE_NODE_DEFINITION.schema().clone(),
        MASK_FRAME_NODE_DEFINITION.schema().clone(),
        MIX_COLOR_NODE_DEFINITION.schema().clone(),
        ALPHA_OVER_NODE_DEFINITION.schema().clone(),
        FADE_NODE_DEFINITION.schema().clone(),
        INTEGRATE_NODE_DEFINITION.schema().clone(),
        MOVING_AVERAGE_NODE_DEFINITION.schema().clone(),
        MOVING_MEDIAN_NODE_DEFINITION.schema().clone(),
        BOX_BLUR_NODE_DEFINITION.schema().clone(),
        GAUSSIAN_BLUR_NODE_DEFINITION.schema().clone(),
        MEDIAN_FILTER_NODE_DEFINITION.schema().clone(),
        LAPLACIAN_FILTER_NODE_DEFINITION.schema().clone(),
        SPECTRUM_ANALYZER_NODE_DEFINITION.schema().clone(),
        SOLID_FRAME_NODE_DEFINITION.schema().clone(),
        RAINBOW_SWEEP_NODE_DEFINITION.schema().clone(),
        CIRCLE_SWEEP_NODE_DEFINITION.schema().clone(),
        LEVEL_BAR_NODE_DEFINITION.schema().clone(),
        TWINKLE_STARS_NODE_DEFINITION.schema().clone(),
        PLASMA_NODE_DEFINITION.schema().clone(),
        BOUNCING_BALLS_NODE_DEFINITION.schema().clone(),
        WLED_DUMMY_DISPLAY_NODE_DEFINITION.schema().clone(),
        TYPE_DEBUG_NODE_DEFINITION.schema().clone(),
        FRAME_BRIGHTNESS_NODE_DEFINITION.schema().clone(),
    ]
}

pub fn node_definition_impl(node_type_id: &str) -> Option<&'static dyn NodeDefinition> {
    match node_type_id {
        NodeTypeId::FLOAT_CONSTANT => Some(&FLOAT_CONSTANT_NODE_DEFINITION),
        NodeTypeId::COLOR_CONSTANT => Some(&COLOR_CONSTANT_NODE_DEFINITION),
        NodeTypeId::IMAGE_SOURCE => Some(&IMAGE_SOURCE_NODE_DEFINITION),
        NodeTypeId::DISPLAY => Some(&DISPLAY_NODE_DEFINITION),
        NodeTypeId::PLOT => Some(&PLOT_NODE_DEFINITION),
        NodeTypeId::DELAY => Some(&DELAY_NODE_DEFINITION),
        NodeTypeId::DIFFERENTIATE => Some(&DIFFERENTIATE_NODE_DEFINITION),
        NodeTypeId::WLED_TARGET => Some(&WLED_TARGET_NODE_DEFINITION),
        NodeTypeId::WLED_SINK => Some(&WLED_SINK_NODE_DEFINITION),
        NodeTypeId::AUDIO_FFT_RECEIVER => Some(&AUDIO_FFT_RECEIVER_NODE_DEFINITION),
        NodeTypeId::HA_MQTT_NUMBER => Some(&HA_MQTT_NUMBER_NODE_DEFINITION),
        NodeTypeId::BINARY_SELECT => Some(&BINARY_SELECT_NODE_DEFINITION),
        NodeTypeId::ADD => Some(&ADD_NODE_DEFINITION),
        NodeTypeId::SUBTRACT => Some(&SUBTRACT_NODE_DEFINITION),
        NodeTypeId::SIGNAL_GENERATOR => Some(&SIGNAL_GENERATOR_NODE_DEFINITION),
        NodeTypeId::DIVIDE => Some(&DIVIDE_NODE_DEFINITION),
        NodeTypeId::MIN_MAX => Some(&MIN_MAX_NODE_DEFINITION),
        NodeTypeId::MULTIPLY => Some(&MULTIPLY_NODE_DEFINITION),
        NodeTypeId::ABS => Some(&ABS_NODE_DEFINITION),
        NodeTypeId::CLAMP => Some(&CLAMP_NODE_DEFINITION),
        NodeTypeId::POWER => Some(&POWER_NODE_DEFINITION),
        NodeTypeId::ROOT => Some(&ROOT_NODE_DEFINITION),
        NodeTypeId::EXPONENTIAL => Some(&EXPONENTIAL_NODE_DEFINITION),
        NodeTypeId::LOG => Some(&LOG_NODE_DEFINITION),
        NodeTypeId::MAP_RANGE => Some(&MAP_RANGE_NODE_DEFINITION),
        NodeTypeId::ROUND => Some(&ROUND_NODE_DEFINITION),
        NodeTypeId::SCALE_TENSOR => Some(&SCALE_TENSOR_NODE_DEFINITION),
        NodeTypeId::SCALE_COLOR => Some(&SCALE_COLOR_NODE_DEFINITION),
        NodeTypeId::MULTIPLY_COLOR => Some(&MULTIPLY_COLOR_NODE_DEFINITION),
        NodeTypeId::TINT_FRAME => Some(&TINT_FRAME_NODE_DEFINITION),
        NodeTypeId::EXTRACT_CHANNELS => Some(&EXTRACT_CHANNELS_NODE_DEFINITION),
        NodeTypeId::SET_CHANNEL => Some(&SET_CHANNEL_NODE_DEFINITION),
        NodeTypeId::COLORIZE => Some(&COLORIZE_NODE_DEFINITION),
        NodeTypeId::MASK_FRAME => Some(&MASK_FRAME_NODE_DEFINITION),
        NodeTypeId::MIX_COLOR => Some(&MIX_COLOR_NODE_DEFINITION),
        NodeTypeId::ALPHA_OVER => Some(&ALPHA_OVER_NODE_DEFINITION),
        NodeTypeId::FADE => Some(&FADE_NODE_DEFINITION),
        NodeTypeId::INTEGRATE => Some(&INTEGRATE_NODE_DEFINITION),
        NodeTypeId::MOVING_AVERAGE => Some(&MOVING_AVERAGE_NODE_DEFINITION),
        NodeTypeId::MOVING_MEDIAN => Some(&MOVING_MEDIAN_NODE_DEFINITION),
        NodeTypeId::BOX_BLUR => Some(&BOX_BLUR_NODE_DEFINITION),
        NodeTypeId::GAUSSIAN_BLUR => Some(&GAUSSIAN_BLUR_NODE_DEFINITION),
        NodeTypeId::MEDIAN_FILTER => Some(&MEDIAN_FILTER_NODE_DEFINITION),
        NodeTypeId::LAPLACIAN_FILTER => Some(&LAPLACIAN_FILTER_NODE_DEFINITION),
        NodeTypeId::SPECTRUM_ANALYZER => Some(&SPECTRUM_ANALYZER_NODE_DEFINITION),
        NodeTypeId::SOLID_FRAME => Some(&SOLID_FRAME_NODE_DEFINITION),
        NodeTypeId::RAINBOW_SWEEP => Some(&RAINBOW_SWEEP_NODE_DEFINITION),
        NodeTypeId::CIRCLE_SWEEP => Some(&CIRCLE_SWEEP_NODE_DEFINITION),
        NodeTypeId::LEVEL_BAR => Some(&LEVEL_BAR_NODE_DEFINITION),
        NodeTypeId::TWINKLE_STARS => Some(&TWINKLE_STARS_NODE_DEFINITION),
        NodeTypeId::PLASMA => Some(&PLASMA_NODE_DEFINITION),
        NodeTypeId::BOUNCING_BALLS => Some(&BOUNCING_BALLS_NODE_DEFINITION),
        NodeTypeId::WLED_DUMMY_DISPLAY => Some(&WLED_DUMMY_DISPLAY_NODE_DEFINITION),
        NodeTypeId::TYPE_DEBUG => Some(&TYPE_DEBUG_NODE_DEFINITION),
        NodeTypeId::FRAME_BRIGHTNESS => Some(&FRAME_BRIGHTNESS_NODE_DEFINITION),
        _ => None,
    }
}

/// Returns the node definition for the requested node type identifier, if it exists.
pub fn node_definition(node_type_id: &str) -> Option<NodeSchema> {
    node_definition_impl(node_type_id).map(|definition| definition.schema().clone())
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
