use anyhow::Result;
use shared::{InputValue, RgbaColor};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, RuntimeOutputs,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MultiplyColorNode;

impl RuntimeNodeFromParameters for MultiplyColorNode {}

pub(crate) struct MultiplyColorInputs {
    a: RgbaColor,
    b: RgbaColor,
}

crate::node_runtime::impl_runtime_inputs!(MultiplyColorInputs {
    a = RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
    b = RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
});

pub(crate) struct MultiplyColorOutputs {
    color: RgbaColor,
}

impl RuntimeOutputs for MultiplyColorOutputs {
    fn into_runtime_outputs(self) -> anyhow::Result<std::collections::HashMap<String, InputValue>> {
        let mut outputs = std::collections::HashMap::new();
        outputs.insert("color".to_owned(), InputValue::Color(self.color));
        Ok(outputs)
    }
}

impl RuntimeNode for MultiplyColorNode {
    type Inputs = MultiplyColorInputs;
    type Outputs = MultiplyColorOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        Ok(TypedNodeEvaluation::from_outputs(MultiplyColorOutputs {
            color: RgbaColor {
                r: (inputs.a.r * inputs.b.r).clamp(0.0, 1.0),
                g: (inputs.a.g * inputs.b.g).clamp(0.0, 1.0),
                b: (inputs.a.b * inputs.b.b).clamp(0.0, 1.0),
                a: (inputs.a.a * inputs.b.a).clamp(0.0, 1.0),
            },
        }))
    }
}
