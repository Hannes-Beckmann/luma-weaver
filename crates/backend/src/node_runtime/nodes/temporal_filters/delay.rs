use std::collections::{HashMap, VecDeque};

use anyhow::Result;
use shared::{
    ColorFrame, FloatTensor, InputValue, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity,
    RgbaColor,
};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeInputs, RuntimeNode, TypedNodeEvaluation,
};

pub(crate) struct DelayNode {
    ticks: usize,
    history: VecDeque<InputValue>,
    initial_type: DelayInitialType,
}

impl Default for DelayNode {
    fn default() -> Self {
        Self {
            ticks: 1,
            history: VecDeque::new(),
            initial_type: DelayInitialType::Float,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(DelayNode {
    ticks: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 1usize,
    initial_type: String => |value| DelayInitialType::from_id(&value), default DelayInitialType::Float,
    ..Self::default()
});

pub(crate) struct DelayInputs {
    value: Option<AnyInputValue>,
}

impl RuntimeInputs for DelayInputs {
    fn from_runtime_inputs(inputs: &HashMap<String, InputValue>) -> Result<Self> {
        Ok(Self {
            value: inputs.get("value").cloned().map(AnyInputValue),
        })
    }
}

pub(crate) struct DelayOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(DelayOutputs { value });

impl RuntimeNode for DelayNode {
    type Inputs = DelayInputs;
    type Outputs = DelayOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = Vec::new();
        let current = inputs.value.map(|value| value.0);
        if self.ticks == 0 {
            return Ok(TypedNodeEvaluation {
                outputs: DelayOutputs {
                    value: current.unwrap_or_else(|| {
                        initial_zero_value(self.initial_type, context.render_layout.as_ref())
                    }),
                },
                frontend_updates: Vec::new(),
                diagnostics,
            });
        }

        if self.history.is_empty() {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Info,
                code: Some("delay_not_primed".to_owned()),
                message: format!(
                    "Delay is not primed yet; outputting a zero-like value for the first {} tick(s).",
                    self.ticks
                ),
            });
        }

        let value = match current {
            Some(current) => {
                self.history.push_back(current.clone());
                if self.history.len() > self.ticks {
                    self.history
                        .pop_front()
                        .unwrap_or_else(|| zero_like(&current))
                } else {
                    zero_like(&current)
                }
            }
            None => initial_zero_value(self.initial_type, context.render_layout.as_ref()),
        };

        Ok(TypedNodeEvaluation {
            outputs: DelayOutputs { value },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

#[derive(Clone, Copy)]
enum DelayInitialType {
    Float,
    Tensor,
    ColorFrame,
}

impl DelayInitialType {
    fn from_id(value: &str) -> Self {
        match value {
            "tensor" => Self::Tensor,
            "colorframe" => Self::ColorFrame,
            _ => Self::Float,
        }
    }

    fn from_optional_id(value: Option<&str>) -> Self {
        value.map(Self::from_id).unwrap_or(Self::Float)
    }
}

pub(crate) fn seeded_initial_value_for_layout(
    initial_type_id: Option<&str>,
    render_layout: Option<&LedLayout>,
) -> InputValue {
    initial_zero_value(
        DelayInitialType::from_optional_id(initial_type_id),
        render_layout,
    )
}

fn initial_zero_value(kind: DelayInitialType, render_layout: Option<&LedLayout>) -> InputValue {
    match kind {
        DelayInitialType::Float => InputValue::Float(0.0),
        DelayInitialType::Tensor => InputValue::FloatTensor(FloatTensor {
            shape: initial_shape(render_layout),
            values: vec![0.0; initial_element_count(render_layout)],
        }),
        DelayInitialType::ColorFrame => InputValue::ColorFrame(ColorFrame {
            layout: initial_layout(render_layout),
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                initial_element_count(render_layout)
            ],
        }),
    }
}

fn zero_like(value: &InputValue) -> InputValue {
    match value {
        InputValue::Float(_) => InputValue::Float(0.0),
        InputValue::String(_) => InputValue::String(String::new()),
        InputValue::FloatTensor(tensor) => InputValue::FloatTensor(FloatTensor {
            shape: tensor.shape.clone(),
            values: vec![0.0; tensor.values.len()],
        }),
        InputValue::Color(_) => InputValue::Color(RgbaColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }),
        InputValue::LedLayout(layout) => InputValue::LedLayout(LedLayout {
            id: layout.id.clone(),
            pixel_count: layout.pixel_count,
            width: layout.width,
            height: layout.height,
        }),
        InputValue::ColorFrame(frame) => InputValue::ColorFrame(ColorFrame {
            layout: frame.layout.clone(),
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                frame.pixels.len()
            ],
        }),
    }
}

fn initial_layout(layout: Option<&LedLayout>) -> LedLayout {
    layout.cloned().unwrap_or(LedLayout {
        id: "delay.initial".to_owned(),
        pixel_count: 0,
        width: None,
        height: None,
    })
}

fn initial_shape(layout: Option<&LedLayout>) -> Vec<usize> {
    match layout {
        Some(layout) => match (layout.width, layout.height) {
            (Some(width), Some(height)) => vec![height, width],
            _ => vec![layout.pixel_count],
        },
        None => vec![0],
    }
}

fn initial_element_count(layout: Option<&LedLayout>) -> usize {
    match layout {
        Some(layout) => layout.pixel_count,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use shared::{FloatTensor, InputValue, LedLayout, RgbaColor};

    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    use super::{DelayInitialType, DelayInputs, DelayNode, initial_zero_value};

    fn evaluation_context(render_layout: Option<LedLayout>) -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout,
        }
    }

    #[test]
    fn first_frame_output_is_zero_like_input() {
        let mut node = DelayNode::default();
        let layout = LedLayout {
            id: "test".to_owned(),
            pixel_count: 2,
            width: None,
            height: None,
        };
        let input = InputValue::ColorFrame(shared::ColorFrame {
            layout: layout.clone(),
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.5,
                    b: 0.25,
                    a: 1.0,
                };
                2
            ],
        });

        let evaluation = RuntimeNode::evaluate(
            &mut node,
            &evaluation_context(None),
            DelayInputs {
                value: Some(AnyInputValue(input)),
            },
        )
        .expect("evaluate delay");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::ColorFrame(shared::ColorFrame {
                layout,
                pixels: vec![
                    RgbaColor {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    };
                    2
                ],
            })
        );
    }

    #[test]
    fn disconnected_delay_uses_selected_tensor_initial_type() {
        let context = evaluation_context(Some(LedLayout {
            id: "panel".to_owned(),
            pixel_count: 6,
            width: Some(3),
            height: Some(2),
        }));

        assert_eq!(
            initial_zero_value(DelayInitialType::Tensor, context.render_layout.as_ref()),
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2, 3],
                values: vec![0.0; 6],
            })
        );
    }

    #[test]
    fn disconnected_delay_uses_selected_frame_initial_type() {
        let context = evaluation_context(Some(LedLayout {
            id: "strip".to_owned(),
            pixel_count: 4,
            width: None,
            height: None,
        }));

        assert_eq!(
            initial_zero_value(DelayInitialType::ColorFrame, context.render_layout.as_ref()),
            InputValue::ColorFrame(shared::ColorFrame {
                layout: LedLayout {
                    id: "strip".to_owned(),
                    pixel_count: 4,
                    width: None,
                    height: None,
                },
                pixels: vec![
                    RgbaColor {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    };
                    4
                ],
            })
        );
    }
}
