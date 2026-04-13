use std::collections::HashMap;

use anyhow::{Result, bail};
use serde_json::Value as JsonValue;
use shared::{
    ColorFrame, FloatTensor, InputValue, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::node_runtime::{
    AnyInputValue, NodeConstruction, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct IntegrateNode {
    initial_value: f32,
    clamp_output: bool,
    min: f32,
    max: f32,
    accumulated: Option<InputValue>,
    initialized: bool,
    last_elapsed_seconds: Option<f64>,
}

#[derive(Default)]
struct IntegrateParameters {
    initial_value: f32,
    clamp_output: bool,
    min: f32,
    max: f32,
}

crate::node_runtime::impl_runtime_parameters!(IntegrateParameters {
    initial_value: f64 => |value| value as f32, default 0.0f32,
    clamp_output: bool = false,
    min: f64 => |value| value as f32, default -1.0f32,
    max: f64 => |value| value as f32, default 1.0f32,
});

impl IntegrateNode {
    fn from_config(config: IntegrateParameters) -> (Self, Vec<NodeDiagnostic>) {
        let mut diagnostics = Vec::new();
        let (min, max) = if config.clamp_output && config.min > config.max {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("integrate_bounds_swapped".to_owned()),
                message: "Integrate min exceeded max, so the bounds were swapped.".to_owned(),
            });
            (config.max, config.min)
        } else {
            (config.min, config.max)
        };

        let node = Self {
            initial_value: config.initial_value,
            clamp_output: config.clamp_output,
            min,
            max,
            accumulated: None,
            initialized: false,
            last_elapsed_seconds: None,
        };
        (node, diagnostics)
    }

    fn clamp_scalar(&self, value: f32) -> f32 {
        if self.clamp_output {
            value.clamp(self.min, self.max)
        } else {
            value
        }
    }

    fn reset_state(&mut self, elapsed_seconds: f64, rate: &InputValue) {
        self.accumulated = Some(self.initial_state(rate));
        self.initialized = true;
        self.last_elapsed_seconds = Some(elapsed_seconds);
    }

    fn initial_state(&self, rate: &InputValue) -> InputValue {
        match rate {
            InputValue::Float(_) => InputValue::Float(self.clamp_scalar(self.initial_value)),
            InputValue::FloatTensor(tensor) => InputValue::FloatTensor(FloatTensor {
                shape: tensor.shape.clone(),
                values: vec![self.clamp_scalar(self.initial_value); tensor.values.len()],
            }),
            InputValue::ColorFrame(frame) => InputValue::ColorFrame(ColorFrame {
                layout: frame.layout.clone(),
                pixels: vec![
                    RgbaColor {
                        r: self.clamp_scalar(self.initial_value),
                        g: self.clamp_scalar(self.initial_value),
                        b: self.clamp_scalar(self.initial_value),
                        a: self.clamp_scalar(self.initial_value),
                    };
                    frame.pixels.len()
                ],
            }),
            other => panic!("unsupported integrate input kind: {:?}", other.value_kind()),
        }
    }

    fn integrate_step(
        &self,
        accumulated: &InputValue,
        rate: &InputValue,
        dt: f32,
    ) -> Result<InputValue> {
        match (accumulated, rate) {
            (InputValue::Float(accumulated), InputValue::Float(rate)) => Ok(InputValue::Float(
                self.clamp_scalar(*accumulated + *rate * dt),
            )),
            (InputValue::FloatTensor(accumulated), InputValue::FloatTensor(rate)) => {
                if accumulated.shape != rate.shape || accumulated.values.len() != rate.values.len()
                {
                    bail!("integrate tensor shape mismatch");
                }
                Ok(InputValue::FloatTensor(FloatTensor {
                    shape: accumulated.shape.clone(),
                    values: accumulated
                        .values
                        .iter()
                        .zip(rate.values.iter())
                        .map(|(accumulated, rate)| self.clamp_scalar(*accumulated + *rate * dt))
                        .collect(),
                }))
            }
            (InputValue::ColorFrame(accumulated), InputValue::ColorFrame(rate)) => {
                if accumulated.layout != rate.layout
                    || accumulated.pixels.len() != rate.pixels.len()
                {
                    bail!("integrate frame layout mismatch");
                }
                Ok(InputValue::ColorFrame(ColorFrame {
                    layout: accumulated.layout.clone(),
                    pixels: accumulated
                        .pixels
                        .iter()
                        .zip(rate.pixels.iter())
                        .map(|(accumulated, rate)| RgbaColor {
                            r: self.clamp_scalar(accumulated.r + rate.r * dt),
                            g: self.clamp_scalar(accumulated.g + rate.g * dt),
                            b: self.clamp_scalar(accumulated.b + rate.b * dt),
                            a: self.clamp_scalar(accumulated.a + rate.a * dt),
                        })
                        .collect(),
                }))
            }
            _ => bail!("integrate input kind changed between evaluations"),
        }
    }
}

impl RuntimeNodeFromParameters for IntegrateNode {
    fn from_parameters(parameters: &HashMap<String, JsonValue>) -> NodeConstruction<Self> {
        let NodeConstruction {
            node: config,
            diagnostics,
        } = IntegrateParameters::from_parameters(parameters);
        let (node, mut extra_diagnostics) = Self::from_config(config);
        let mut diagnostics = diagnostics;
        diagnostics.append(&mut extra_diagnostics);
        NodeConstruction { node, diagnostics }
    }
}

pub(crate) struct IntegrateInputs {
    rate: AnyInputValue,
    reset: f32,
}

crate::node_runtime::impl_runtime_inputs!(IntegrateInputs {
    rate = AnyInputValue(InputValue::Float(0.0)),
    reset = 0.0,
});

pub(crate) struct IntegrateOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(IntegrateOutputs { value });

impl RuntimeNode for IntegrateNode {
    type Inputs = IntegrateInputs;
    type Outputs = IntegrateOutputs;

    /// Integrates the input rate against elapsed runtime seconds using explicit Euler steps.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let rate = inputs.rate.0;
        if inputs.reset >= 0.5 {
            self.reset_state(context.elapsed_seconds, &rate);
            return Ok(TypedNodeEvaluation::from_outputs(IntegrateOutputs {
                value: self
                    .accumulated
                    .clone()
                    .expect("integrate reset must initialize state"),
            }));
        }

        if !self.initialized {
            self.reset_state(context.elapsed_seconds, &rate);
            return Ok(TypedNodeEvaluation::from_outputs(IntegrateOutputs {
                value: self
                    .accumulated
                    .clone()
                    .expect("integrate initialization must set state"),
            }));
        }

        let dt = match self.last_elapsed_seconds {
            Some(previous_time) if context.elapsed_seconds >= previous_time => {
                (context.elapsed_seconds - previous_time) as f32
            }
            _ => 0.0,
        };
        self.last_elapsed_seconds = Some(context.elapsed_seconds);

        if dt > 0.0 {
            let accumulated = self
                .accumulated
                .as_ref()
                .expect("integrate must have initialized state before stepping");
            self.accumulated = Some(self.integrate_step(accumulated, &rate, dt)?);
        }

        Ok(TypedNodeEvaluation::from_outputs(IntegrateOutputs {
            value: self
                .accumulated
                .clone()
                .expect("integrate must have state when emitting output"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use shared::{ColorFrame, FloatTensor, InputValue, LedLayout, RgbaColor};

    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    use super::{IntegrateInputs, IntegrateNode};

    fn context(elapsed_seconds: f64) -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds,
            render_layout: None,
        }
    }

    #[test]
    fn first_sample_outputs_initial_value() {
        let mut node = IntegrateNode::default();

        let evaluation = node
            .evaluate(
                &context(0.0),
                IntegrateInputs {
                    rate: AnyInputValue(InputValue::Float(3.0)),
                    reset: 0.0,
                },
            )
            .expect("evaluate initial integrate sample");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
    }

    #[test]
    fn accumulates_rate_over_elapsed_seconds() {
        let mut node = IntegrateNode::default();

        node.evaluate(
            &context(0.0),
            IntegrateInputs {
                rate: AnyInputValue(InputValue::Float(2.0)),
                reset: 0.0,
            },
        )
        .expect("prime integrate");
        let evaluation = node
            .evaluate(
                &context(0.5),
                IntegrateInputs {
                    rate: AnyInputValue(InputValue::Float(2.0)),
                    reset: 0.0,
                },
            )
            .expect("integrate half second");

        assert_eq!(evaluation.outputs.value, InputValue::Float(1.0));
    }

    #[test]
    fn reset_restores_initial_value() {
        let mut node = IntegrateNode {
            initial_value: 5.0,
            ..IntegrateNode::default()
        };

        node.evaluate(
            &context(0.0),
            IntegrateInputs {
                rate: AnyInputValue(InputValue::Float(1.0)),
                reset: 0.0,
            },
        )
        .expect("prime integrate");
        node.evaluate(
            &context(1.0),
            IntegrateInputs {
                rate: AnyInputValue(InputValue::Float(1.0)),
                reset: 0.0,
            },
        )
        .expect("integrate one second");
        let evaluation = node
            .evaluate(
                &context(2.0),
                IntegrateInputs {
                    rate: AnyInputValue(InputValue::Float(1.0)),
                    reset: 1.0,
                },
            )
            .expect("reset integrate");

        assert_eq!(evaluation.outputs.value, InputValue::Float(5.0));
    }

    #[test]
    fn clamps_when_enabled() {
        let mut node = IntegrateNode {
            clamp_output: true,
            min: -1.0,
            max: 1.0,
            ..IntegrateNode::default()
        };

        node.evaluate(
            &context(0.0),
            IntegrateInputs {
                rate: AnyInputValue(InputValue::Float(10.0)),
                reset: 0.0,
            },
        )
        .expect("prime integrate");
        let evaluation = node
            .evaluate(
                &context(1.0),
                IntegrateInputs {
                    rate: AnyInputValue(InputValue::Float(10.0)),
                    reset: 0.0,
                },
            )
            .expect("integrate clamped");

        assert_eq!(evaluation.outputs.value, InputValue::Float(1.0));
    }

    #[test]
    fn integrates_float_tensors_elementwise() {
        let mut node = IntegrateNode::default();

        node.evaluate(
            &context(0.0),
            IntegrateInputs {
                rate: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                    shape: vec![2],
                    values: vec![2.0, -4.0],
                })),
                reset: 0.0,
            },
        )
        .expect("prime tensor integrate");
        let evaluation = node
            .evaluate(
                &context(0.5),
                IntegrateInputs {
                    rate: AnyInputValue(InputValue::FloatTensor(FloatTensor {
                        shape: vec![2],
                        values: vec![2.0, -4.0],
                    })),
                    reset: 0.0,
                },
            )
            .expect("integrate tensor");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::FloatTensor(FloatTensor {
                shape: vec![2],
                values: vec![1.0, -2.0],
            })
        );
    }

    #[test]
    fn integrates_color_frames_per_channel() {
        let mut node = IntegrateNode::default();
        let layout = LedLayout {
            id: "layout".to_owned(),
            pixel_count: 1,
            width: Some(1),
            height: Some(1),
        };

        node.evaluate(
            &context(0.0),
            IntegrateInputs {
                rate: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                    layout: layout.clone(),
                    pixels: vec![RgbaColor {
                        r: 2.0,
                        g: 0.0,
                        b: -2.0,
                        a: 1.0,
                    }],
                })),
                reset: 0.0,
            },
        )
        .expect("prime frame integrate");
        let evaluation = node
            .evaluate(
                &context(0.25),
                IntegrateInputs {
                    rate: AnyInputValue(InputValue::ColorFrame(ColorFrame {
                        layout: layout.clone(),
                        pixels: vec![RgbaColor {
                            r: 2.0,
                            g: 0.0,
                            b: -2.0,
                            a: 1.0,
                        }],
                    })),
                    reset: 0.0,
                },
            )
            .expect("integrate frame");

        assert_eq!(
            evaluation.outputs.value,
            InputValue::ColorFrame(ColorFrame {
                layout,
                pixels: vec![RgbaColor {
                    r: 0.5,
                    g: 0.0,
                    b: -0.5,
                    a: 0.25,
                }],
            })
        );
    }
}
