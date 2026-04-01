use anyhow::Result;
use shared::RgbaColor;

use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

pub(crate) struct ColorConstantNode {
    color: RgbaColor,
}

impl Default for ColorConstantNode {
    fn default() -> Self {
        Self {
            color: RgbaColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(ColorConstantNode {
    color: RgbaColor = RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
});

pub(crate) struct ColorConstantOutputs {
    color: RgbaColor,
}

crate::node_runtime::impl_runtime_outputs!(ColorConstantOutputs { color });

impl RuntimeNode for ColorConstantNode {
    type Inputs = ();
    type Outputs = ColorConstantOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        Ok(TypedNodeEvaluation::from_outputs(ColorConstantOutputs {
            color: self.color,
        }))
    }
}
