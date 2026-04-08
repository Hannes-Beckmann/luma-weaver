/// Construction-time node building and parameter-driven initialization diagnostics.
mod construction;
/// Evaluation-time context and result types shared by runtime nodes and the executor.
mod context;
/// Fast typed conversions between generic runtime values and concrete Rust types.
mod conversions;
/// Parameter-adjustment helpers and construction-diagnostic utilities.
mod diagnostics;
/// Macros for declaring runtime input, output, and parameter boilerplate.
mod macros;
/// Raw parameter decoding helpers used during node construction.
mod parameters;
/// Built-in node registration and runtime factory wiring.
mod registry;
/// Core runtime traits implemented by concrete nodes and adapters.
mod traits;

/// Concrete runtime node implementations grouped by domain.
pub(crate) mod nodes;
/// Shared serde-based conversion helpers and generic input wrappers.
pub(crate) mod shared;
/// Tensor-specific runtime helpers used by color and math nodes.
pub(crate) mod tensor;

/// Construction result types and the trait for building nodes from graph parameters.
pub(crate) use construction::{NodeConstruction, RuntimeNodeFromParameters};
/// Evaluation context and typed/untyped evaluation result types.
pub(crate) use context::{
    NodeEvaluation, NodeEvaluationContext, NodeFrontendUpdate, TypedNodeEvaluation,
};
/// Fast typed conversion traits and evaluator adapter.
pub(crate) use conversions::{FastNodeEvaluator, FromInputValue, IntoInputValue};
/// Parameter-normalization helpers and construction-diagnostic builders.
pub(crate) use diagnostics::{
    IntoParameterAdjustment, clamp_f64_to_f32, clamp_u64_to_u16, clamp_u64_to_usize,
    invalid_parameter_diagnostic, max_f64_to_f32, max_u64_to_usize, non_empty_gradient,
};
/// Declarative macros for runtime node boilerplate.
pub(crate) use macros::{
    impl_runtime_inputs, impl_runtime_outputs, impl_runtime_parameters,
    impl_runtime_parameters_builder,
};
/// Typed parameter decoding helpers.
pub(crate) use parameters::{ParameterStatus, parameter_status};
/// Built-in registry construction and lookup.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use registry::build_builtin_node_registry;
pub(crate) use registry::{NodeRegistry, build_portable_node_registry};
/// Generic serde-based runtime input and output conversion helpers.
pub(crate) use shared::{AnyInputValue, deserialize_inputs, serialize_outputs};
/// Core runtime traits used throughout the execution engine.
pub(crate) use traits::{RuntimeInputs, RuntimeNode, RuntimeNodeEvaluator, RuntimeOutputs};
