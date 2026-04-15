use std::collections::HashMap;

use anyhow::Result;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity, ValueKind};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeInputs, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct BinarySelectNode;

impl RuntimeNodeFromParameters for BinarySelectNode {}

pub(crate) struct BinarySelectInputs {
    selector: f32,
    a: AnyInputValue,
    b: AnyInputValue,
}

impl RuntimeInputs for BinarySelectInputs {
    fn from_runtime_inputs(inputs: &HashMap<String, InputValue>) -> Result<Self> {
        Ok(Self {
            selector: match inputs.get("selector") {
                Some(InputValue::Float(value)) => *value,
                Some(_) | None => 0.0,
            },
            a: AnyInputValue(inputs.get("a").cloned().unwrap_or(InputValue::Float(0.0))),
            b: AnyInputValue(inputs.get("b").cloned().unwrap_or(InputValue::Float(0.0))),
        })
    }
}

pub(crate) struct BinarySelectOutputs {
    value: InputValue,
}

crate::node_runtime::impl_runtime_outputs!(BinarySelectOutputs { value });

impl RuntimeNode for BinarySelectNode {
    type Inputs = BinarySelectInputs;
    type Outputs = BinarySelectOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let a = inputs.a.0;
        let b = inputs.b.0;
        let a_kind = value_kind(&a);
        let b_kind = value_kind(&b);

        if a_kind != b_kind {
            return Ok(TypedNodeEvaluation {
                outputs: BinarySelectOutputs {
                    value: InputValue::Float(0.0),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("binary_select_kind_mismatch".to_owned()),
                    message: format!(
                        "Binary Select inputs 'a' and 'b' must have the same kind, found {:?} and {:?}.",
                        a_kind, b_kind
                    ),
                }],
            });
        }

        let selected = if selects_b(inputs.selector) { b } else { a };

        Ok(TypedNodeEvaluation::from_outputs(BinarySelectOutputs {
            value: selected,
        }))
    }
}

fn selects_b(selector: f32) -> bool {
    selector >= 0.5
}

fn value_kind(value: &InputValue) -> ValueKind {
    match value {
        InputValue::Float(_) => ValueKind::Float,
        InputValue::String(_) => ValueKind::String,
        InputValue::FloatTensor(_) => ValueKind::FloatTensor,
        InputValue::Color(_) => ValueKind::Color,
        InputValue::LedLayout(_) => ValueKind::LedLayout,
        InputValue::ColorFrame(_) => ValueKind::ColorFrame,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use shared::{
        ColorFrame, FloatTensor, InputValue, LedLayout, NodeDiagnosticSeverity, RgbaColor,
    };

    use super::{BinarySelectInputs, BinarySelectNode, selects_b};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeInputs, RuntimeNode};

    fn evaluation_context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "graph".to_owned(),
            graph_name: "Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn selector_threshold_matches_issue_contract() {
        assert!(!selects_b(0.49));
        assert!(selects_b(0.5));
        assert!(selects_b(1.0));
    }

    #[test]
    fn selector_below_threshold_returns_a() {
        let mut node = BinarySelectNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                BinarySelectInputs {
                    selector: 0.25,
                    a: AnyInputValue(InputValue::Float(2.0)),
                    b: AnyInputValue(InputValue::Float(7.0)),
                },
            )
            .expect("evaluate binary select");

        assert_eq!(evaluation.outputs.value, InputValue::Float(2.0));
    }

    #[test]
    fn selector_at_threshold_returns_b() {
        let mut node = BinarySelectNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                BinarySelectInputs {
                    selector: 0.5,
                    a: AnyInputValue(InputValue::Float(2.0)),
                    b: AnyInputValue(InputValue::Float(7.0)),
                },
            )
            .expect("evaluate binary select");

        assert_eq!(evaluation.outputs.value, InputValue::Float(7.0));
    }

    #[test]
    fn preserves_selected_value_kind_for_frames() {
        let mut node = BinarySelectNode;
        let other_frame = InputValue::ColorFrame(ColorFrame {
            layout: LedLayout {
                id: "frame".to_owned(),
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
            },
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                },
                RgbaColor {
                    r: 1.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
            ],
        });
        let frame = InputValue::ColorFrame(ColorFrame {
            layout: LedLayout {
                id: "frame".to_owned(),
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
            },
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                RgbaColor {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
            ],
        });

        let evaluation = node
            .evaluate(
                &evaluation_context(),
                BinarySelectInputs {
                    selector: 1.0,
                    a: AnyInputValue(other_frame),
                    b: AnyInputValue(frame.clone()),
                },
            )
            .expect("evaluate binary select");

        assert_eq!(evaluation.outputs.value, frame);
    }

    #[test]
    fn preserves_selected_value_kind_for_tensors() {
        let mut node = BinarySelectNode;
        let other_tensor = InputValue::FloatTensor(FloatTensor {
            shape: vec![3],
            values: vec![1.0, 0.5, 0.25],
        });
        let tensor = InputValue::FloatTensor(FloatTensor {
            shape: vec![3],
            values: vec![0.25, 0.5, 0.75],
        });

        let evaluation = node
            .evaluate(
                &evaluation_context(),
                BinarySelectInputs {
                    selector: 1.0,
                    a: AnyInputValue(other_tensor),
                    b: AnyInputValue(tensor.clone()),
                },
            )
            .expect("evaluate binary select");

        assert_eq!(evaluation.outputs.value, tensor);
    }

    #[test]
    fn runtime_defaults_fall_back_to_zero_for_missing_inputs() {
        let inputs = BinarySelectInputs::from_runtime_inputs(&HashMap::new())
            .expect("default runtime inputs");

        assert_eq!(inputs.selector, 0.0);
        assert_eq!(inputs.a.0, InputValue::Float(0.0));
        assert_eq!(inputs.b.0, InputValue::Float(0.0));
    }

    #[test]
    fn mismatched_branch_kinds_emit_error_and_safe_fallback() {
        let mut node = BinarySelectNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                BinarySelectInputs {
                    selector: 0.0,
                    a: AnyInputValue(InputValue::Float(1.0)),
                    b: AnyInputValue(InputValue::Color(RgbaColor {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    })),
                },
            )
            .expect("evaluate binary select");

        assert_eq!(evaluation.outputs.value, InputValue::Float(0.0));
        assert!(evaluation.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("binary_select_kind_mismatch")
                && diagnostic.severity == NodeDiagnosticSeverity::Error
        }));
    }
}
