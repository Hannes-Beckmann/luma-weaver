use anyhow::Result;
use shared::{ColorFrame, InputValue, RgbaColor};

use crate::node_runtime::tensor::{
    coerce_color_frame, coerce_float_tensor, infer_broadcast_shape, layout_from_shape,
    mix_color_frames,
};
use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};

#[derive(Default)]
pub(crate) struct MixColorNode;

impl RuntimeNodeFromParameters for MixColorNode {}

#[derive(Clone)]
pub(crate) struct MixColorInputs {
    foreground: AnyInputValue,
    background: AnyInputValue,
    factor: AnyInputValue,
}

crate::node_runtime::impl_runtime_inputs!(MixColorInputs {
    foreground = default_white_value(),
    background = default_white_value(),
    factor = default_factor_value(),
});

pub(crate) struct MixColorOutputs {
    color: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(MixColorOutputs { color });

impl RuntimeNode for MixColorNode {
    type Inputs = MixColorInputs;
    type Outputs = MixColorOutputs;

    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let foreground = inputs.foreground.0;
        let background = inputs.background.0;
        let factor = inputs.factor.0;

        let shape = infer_broadcast_shape(&[&foreground, &background, &factor])?;
        let fallback_layout = layout_from_shape(&shape, "mix_color");
        let foreground = coerce_color_frame(&foreground, &shape, &fallback_layout.id)?;
        let background = coerce_color_frame(&background, &shape, &fallback_layout.id)?;
        let factor = coerce_float_tensor(&factor, &shape)?;
        let color = mix_color_frames(&foreground, &background, &factor)?;

        Ok(TypedNodeEvaluation::from_outputs(MixColorOutputs { color }))
    }
}

fn default_white_value() -> AnyInputValue {
    AnyInputValue(InputValue::Color(RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    }))
}

fn default_factor_value() -> AnyInputValue {
    AnyInputValue(InputValue::Float(0.0))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use shared::{InputValue, LedLayout, RgbaColor};

    use crate::node_runtime::{
        FastNodeEvaluator, NodeEvaluationContext, RuntimeInputs, RuntimeNode, RuntimeNodeEvaluator,
        RuntimeOutputs,
    };

    use super::{MixColorInputs, MixColorNode, MixColorOutputs};

    #[test]
    fn profile_mix_color_with_framework_conversion() {
        let width = 27usize;
        let height = 25usize;
        let pixel_count = width * height;
        let layout = LedLayout {
            id: "profile".to_owned(),
            pixel_count,
            width: Some(width),
            height: Some(height),
        };

        let foreground = shared::ColorFrame {
            layout: layout.clone(),
            pixels: (0..pixel_count)
                .map(|index| {
                    let t = index as f32 / pixel_count as f32;
                    RgbaColor {
                        r: t,
                        g: 1.0 - t,
                        b: (0.25 + t * 0.5).clamp(0.0, 1.0),
                        a: 1.0,
                    }
                })
                .collect(),
        };
        let background = shared::ColorFrame {
            layout: layout.clone(),
            pixels: (0..pixel_count)
                .map(|index| {
                    let t = ((pixel_count - index) as f32) / pixel_count as f32;
                    RgbaColor {
                        r: (0.1 + t * 0.7).clamp(0.0, 1.0),
                        g: (0.05 + t * 0.2).clamp(0.0, 1.0),
                        b: (0.2 + t * 0.6).clamp(0.0, 1.0),
                        a: 1.0,
                    }
                })
                .collect(),
        };

        let raw_inputs = HashMap::from([
            (
                "foreground".to_owned(),
                InputValue::ColorFrame(foreground.clone()),
            ),
            (
                "background".to_owned(),
                InputValue::ColorFrame(background.clone()),
            ),
            ("factor".to_owned(), InputValue::Float(0.5)),
        ]);

        let typed_inputs =
            MixColorInputs::from_runtime_inputs(&raw_inputs).expect("deserialize mix inputs");
        let context = NodeEvaluationContext {
            graph_id: "test-graph".to_owned(),
            graph_name: "Test Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        };

        let mut warmup_node = MixColorNode;
        RuntimeNode::evaluate(&mut warmup_node, &context, typed_inputs)
            .expect("warm up typed mix evaluate");

        let iterations = 1_000usize;

        let deserialize_started = Instant::now();
        for _ in 0..iterations {
            let _ =
                MixColorInputs::from_runtime_inputs(&raw_inputs).expect("deserialize mix inputs");
        }
        let deserialize_total = deserialize_started.elapsed();

        let typed_inputs =
            MixColorInputs::from_runtime_inputs(&raw_inputs).expect("deserialize mix inputs");
        let typed_evaluate_started = Instant::now();
        for _ in 0..iterations {
            let mut node = MixColorNode;
            let _ = RuntimeNode::evaluate(&mut node, &context, typed_inputs.clone())
                .expect("typed mix evaluate");
        }
        let typed_evaluate_total = typed_evaluate_started.elapsed();

        let output = {
            let mut node = MixColorNode;
            let inputs =
                MixColorInputs::from_runtime_inputs(&raw_inputs).expect("deserialize mix inputs");
            RuntimeNode::evaluate(&mut node, &context, inputs)
                .expect("typed mix evaluate")
                .outputs
        };

        let serialize_started = Instant::now();
        for _ in 0..iterations {
            let _ = MixColorOutputs {
                color: output.color.clone(),
            }
            .into_runtime_outputs()
            .expect("serialize mix outputs");
        }
        let serialize_total = serialize_started.elapsed();

        let full_started = Instant::now();
        for _ in 0..iterations {
            let mut node = FastNodeEvaluator(MixColorNode);
            let _ = RuntimeNodeEvaluator::evaluate(&mut node, &context, &raw_inputs)
                .expect("full mix evaluator");
        }
        let full_total = full_started.elapsed();

        let per_iter = |duration: Duration| {
            Duration::from_secs_f64(duration.as_secs_f64() / iterations as f64)
        };

        tracing::info!(
            width,
            height,
            pixel_count,
            iterations,
            deserialize_millis = deserialize_total.as_secs_f64() * 1000.0,
            deserialize_avg_millis = per_iter(deserialize_total).as_secs_f64() * 1000.0,
            typed_evaluate_millis = typed_evaluate_total.as_secs_f64() * 1000.0,
            typed_evaluate_avg_millis = per_iter(typed_evaluate_total).as_secs_f64() * 1000.0,
            serialize_millis = serialize_total.as_secs_f64() * 1000.0,
            serialize_avg_millis = per_iter(serialize_total).as_secs_f64() * 1000.0,
            full_millis = full_total.as_secs_f64() * 1000.0,
            full_avg_millis = per_iter(full_total).as_secs_f64() * 1000.0,
            "mix_color profile"
        );
    }
}
