use anyhow::Result;

use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

#[derive(Clone, Copy)]
enum Waveform {
    Sinus,
    Triangle,
    Sawtooth,
    Rectangle,
}

pub(crate) struct SignalGeneratorNode {
    waveform: Waveform,
    frequency: f64,
    amplitude: f64,
    phase: f64,
}

impl Default for SignalGeneratorNode {
    fn default() -> Self {
        Self {
            waveform: Waveform::Sinus,
            frequency: 1.0,
            amplitude: 1.0,
            phase: 0.0,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(SignalGeneratorNode {
    waveform: String => |value| Waveform::from_id(&value), default Waveform::Sinus,
    frequency: f64 = 1.0,
    amplitude: f64 = 1.0,
    phase: f64 = 0.0,
});

pub(crate) struct SignalGeneratorOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(SignalGeneratorOutputs { value });

impl RuntimeNode for SignalGeneratorNode {
    type Inputs = ();
    type Outputs = SignalGeneratorOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let cycles = self.frequency * context.elapsed_seconds + self.phase;
        let phase = fract01(cycles as f32);
        let value = self.waveform.sample(phase) * self.amplitude as f32;
        Ok(TypedNodeEvaluation::from_outputs(SignalGeneratorOutputs {
            value,
        }))
    }
}

impl Waveform {
    fn from_id(id: &str) -> Self {
        match id {
            "triangle" => Self::Triangle,
            "sawtooth" => Self::Sawtooth,
            "rectangle" => Self::Rectangle,
            _ => Self::Sinus,
        }
    }

    fn sample(self, phase: f32) -> f32 {
        match self {
            Self::Sinus => (std::f32::consts::TAU * phase).sin(),
            Self::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            Self::Sawtooth => phase * 2.0 - 1.0,
            Self::Rectangle => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

fn fract01(v: f32) -> f32 {
    let f = v.fract();
    if f < 0.0 { f + 1.0 } else { f }
}
