use anyhow::Result;
use shared::InputValue;

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, NodeFrontendUpdate, RuntimeNode,
    RuntimeNodeFromParameters, TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct TypeDebugNode;

impl RuntimeNodeFromParameters for TypeDebugNode {}

pub(crate) struct TypeDebugInputs {
    value: Option<AnyInputValue>,
}

crate::node_runtime::impl_runtime_inputs!(TypeDebugInputs {
    value = None,
});

impl RuntimeNode for TypeDebugNode {
    type Inputs = TypeDebugInputs;
    type Outputs = ();

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        Ok(TypedNodeEvaluation::with_frontend_updates(
            (),
            vec![NodeFrontendUpdate {
                name: "type".to_owned(),
                value: InputValue::String(type_label(inputs.value.as_ref().map(|value| &value.0))),
            }],
        ))
    }
}

fn type_label(value: Option<&InputValue>) -> String {
    match value {
        None => "None".to_owned(),
        Some(InputValue::Float(_)) => "Float".to_owned(),
        Some(InputValue::String(_)) => "String".to_owned(),
        Some(InputValue::FloatTensor(tensor)) => format!("FloatTensor shape={:?}", tensor.shape),
        Some(InputValue::Color(_)) => "Color".to_owned(),
        Some(InputValue::LedLayout(layout)) => match (layout.width, layout.height) {
            (Some(width), Some(height)) => format!("LedLayout shape=[{}, {}]", height, width),
            _ => format!("LedLayout shape=[{}]", layout.pixel_count),
        },
        Some(InputValue::ColorFrame(frame)) => match (frame.layout.width, frame.layout.height) {
            (Some(width), Some(height)) => format!("ColorFrame shape=[{}, {}]", height, width),
            _ => format!("ColorFrame shape=[{}]", frame.pixels.len()),
        },
    }
}

#[cfg(test)]
mod tests {
    use shared::InputValue;

    use super::{TypeDebugInputs, TypeDebugNode};
    use crate::node_runtime::{AnyInputValue, NodeEvaluationContext, RuntimeNode};

    fn evaluation_context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "graph".to_owned(),
            graph_name: "Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    #[test]
    fn emits_concrete_type_name_for_connected_input() {
        let mut node = TypeDebugNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                TypeDebugInputs {
                    value: Some(AnyInputValue(InputValue::FloatTensor(
                        shared::FloatTensor {
                            shape: vec![2, 2],
                            values: vec![0.0; 4],
                        },
                    ))),
                },
            )
            .expect("evaluate type debug node");

        assert_eq!(
            evaluation.frontend_updates[0].value,
            InputValue::String("FloatTensor shape=[2, 2]".to_owned())
        );
    }

    #[test]
    fn emits_none_when_input_is_disconnected() {
        let mut node = TypeDebugNode;
        let evaluation = node
            .evaluate(&evaluation_context(), TypeDebugInputs { value: None })
            .expect("evaluate disconnected type debug node");

        assert_eq!(
            evaluation.frontend_updates[0].value,
            InputValue::String("None".to_owned())
        );
    }

    #[test]
    fn includes_frame_shape_information() {
        let mut node = TypeDebugNode;
        let evaluation = node
            .evaluate(
                &evaluation_context(),
                TypeDebugInputs {
                    value: Some(AnyInputValue(InputValue::ColorFrame(shared::ColorFrame {
                        layout: shared::LedLayout {
                            id: "grid".to_owned(),
                            pixel_count: 6,
                            width: Some(3),
                            height: Some(2),
                        },
                        pixels: vec![
                            shared::RgbaColor {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            };
                            6
                        ],
                    }))),
                },
            )
            .expect("evaluate type debug frame");

        assert_eq!(
            evaluation.frontend_updates[0].value,
            InputValue::String("ColorFrame shape=[2, 3]".to_owned())
        );
    }
}
