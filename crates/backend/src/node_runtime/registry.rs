use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use shared::NodeDiagnostic;
use shared::{NodeDefinition, NodeTypeId, builtin_node_definitions};

use crate::node_runtime::nodes;
use crate::node_runtime::{
    FastNodeEvaluator, RuntimeInputs, RuntimeNode, RuntimeNodeEvaluator, RuntimeNodeFromParameters,
    RuntimeOutputs,
};

type FactoryFn = fn(&HashMap<String, JsonValue>) -> Box<dyn RuntimeNodeEvaluator>;
type DiagnosticsFn = fn(&HashMap<String, JsonValue>) -> Vec<NodeDiagnostic>;

/// Builds a boxed fast-path evaluator for a typed runtime node.
///
/// The node is first constructed from its persisted parameters and then wrapped in
/// `FastNodeEvaluator`, which handles typed input/output conversion at runtime.
fn build_fast<T>(parameters: &HashMap<String, JsonValue>) -> Box<dyn RuntimeNodeEvaluator>
where
    T: RuntimeNodeFromParameters + RuntimeNode,
    T::Inputs: RuntimeInputs,
    T::Outputs: RuntimeOutputs,
{
    Box::new(FastNodeEvaluator(T::from_parameters(parameters).node))
}

/// Collects construction-time diagnostics for a typed runtime node.
///
/// These diagnostics are emitted after graph compilation so parameter normalization and invalid
/// values can be surfaced before the first tick runs.
fn construction_diagnostics<T>(parameters: &HashMap<String, JsonValue>) -> Vec<NodeDiagnostic>
where
    T: RuntimeNodeFromParameters + RuntimeNode,
    T::Inputs: RuntimeInputs,
    T::Outputs: RuntimeOutputs,
{
    T::from_parameters(parameters).diagnostics
}

pub(crate) struct RegisteredNodeType {
    pub(crate) definition: NodeDefinition,
    evaluator_factory: FactoryFn,
    diagnostics_factory: DiagnosticsFn,
}

pub(crate) struct NodeRegistry {
    by_id: HashMap<String, RegisteredNodeType>,
    ordered_definitions: Vec<NodeDefinition>,
}

impl NodeRegistry {
    /// Creates an empty node registry.
    pub(crate) fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            ordered_definitions: Vec::new(),
        }
    }

    /// Registers a node definition together with its evaluator and diagnostics factories.
    ///
    /// Duplicate node IDs are rejected so the shared node catalog and runtime registry stay in
    /// one-to-one correspondence.
    pub(crate) fn register(&mut self, entry: RegisteredNodeType) -> anyhow::Result<()> {
        let node_id = entry.definition.id.clone();
        anyhow::ensure!(
            !self.by_id.contains_key(node_id.as_str()),
            "duplicate node registration for {}",
            node_id
        );
        self.ordered_definitions.push(entry.definition.clone());
        self.by_id.insert(node_id, entry);
        Ok(())
    }

    /// Registers a typed runtime node using the fast evaluator path.
    ///
    /// This is the common path for built-in nodes whose inputs, parameters, and outputs are all
    /// described by the shared node schema.
    pub(crate) fn register_fast_node<T>(&mut self, definition: NodeDefinition) -> anyhow::Result<()>
    where
        T: RuntimeNodeFromParameters + RuntimeNode,
        T::Inputs: RuntimeInputs,
        T::Outputs: RuntimeOutputs,
    {
        self.register(RegisteredNodeType {
            definition,
            evaluator_factory: build_fast::<T>,
            diagnostics_factory: construction_diagnostics::<T>,
        })
    }

    /// Returns the node definition registered for `node_type_id`.
    pub(crate) fn definition(&self, node_type_id: &str) -> Option<&NodeDefinition> {
        self.by_id.get(node_type_id).map(|entry| &entry.definition)
    }

    /// Returns all registered node definitions in registration order.
    pub(crate) fn definitions(&self) -> &[NodeDefinition] {
        &self.ordered_definitions
    }

    /// Builds a runtime evaluator for `node_type_id` using the provided parameters.
    pub(crate) fn evaluator_for(
        &self,
        node_type_id: &str,
        parameters: &HashMap<String, JsonValue>,
    ) -> Option<Box<dyn RuntimeNodeEvaluator>> {
        self.by_id
            .get(node_type_id)
            .map(|entry| (entry.evaluator_factory)(parameters))
    }

    /// Returns any construction diagnostics for `node_type_id` and `parameters`.
    pub(crate) fn construction_diagnostics_for(
        &self,
        node_type_id: &str,
        parameters: &HashMap<String, JsonValue>,
    ) -> Option<Vec<NodeDiagnostic>> {
        self.by_id
            .get(node_type_id)
            .map(|entry| (entry.diagnostics_factory)(parameters))
    }
}

/// Builds the registry containing all built-in node types.
///
/// This function pairs the shared built-in node definitions with their backend runtime
/// implementations and returns the result as a shared `Arc`.
pub(crate) fn build_builtin_node_registry() -> anyhow::Result<Arc<NodeRegistry>> {
    let mut registry = NodeRegistry::new();
    let definitions_by_id = builtin_node_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();

    macro_rules! register_builtin {
        ($node_type:expr, $runtime:ty) => {
            registry.register_fast_node::<$runtime>(
                definitions_by_id
                    .get($node_type)
                    .cloned()
                    .expect("builtin node definition must exist"),
            )?;
        };
    }

    register_builtin!(
        NodeTypeId::FLOAT_CONSTANT,
        nodes::inputs::float_constant::FloatConstantNode
    );
    register_builtin!(
        NodeTypeId::COLOR_CONSTANT,
        nodes::inputs::color_constant::ColorConstantNode
    );
    register_builtin!(NodeTypeId::DELAY, nodes::temporal_filters::delay::DelayNode);
    register_builtin!(NodeTypeId::DISPLAY, nodes::outputs::display::DisplayNode);
    register_builtin!(NodeTypeId::PLOT, nodes::outputs::plot::PlotNode);
    register_builtin!(
        NodeTypeId::WLED_TARGET,
        nodes::outputs::wled_target::WledTargetNode
    );
    register_builtin!(
        NodeTypeId::WLED_SINK,
        nodes::inputs::wled_sink::WledSinkNode
    );
    register_builtin!(
        NodeTypeId::AUDIO_FFT_RECEIVER,
        nodes::inputs::audio_fft_receiver::AudioFftReceiverNode
    );
    register_builtin!(
        NodeTypeId::HA_MQTT_NUMBER,
        nodes::inputs::ha_mqtt_number::HomeAssistantMqttNumberNode
    );
    register_builtin!(
        NodeTypeId::SIGNAL_GENERATOR,
        nodes::inputs::signal_generator::SignalGeneratorNode
    );
    register_builtin!(NodeTypeId::ADD_FLOAT, nodes::math::add_float::AddFloatNode);
    register_builtin!(
        NodeTypeId::SUBTRACT_FLOAT,
        nodes::math::subtract_float::SubtractFloatNode
    );
    register_builtin!(
        NodeTypeId::DIVIDE_FLOAT,
        nodes::math::divide_float::DivideFloatNode
    );
    register_builtin!(
        NodeTypeId::MIN_MAX_FLOAT,
        nodes::math::min_max_float::MinMaxFloatNode
    );
    register_builtin!(
        NodeTypeId::MULTIPLY_FLOAT,
        nodes::math::multiply_float::MultiplyFloatNode
    );
    register_builtin!(NodeTypeId::ABS_FLOAT, nodes::math::abs_float::AbsFloatNode);
    register_builtin!(
        NodeTypeId::CLAMP_FLOAT,
        nodes::math::clamp_float::ClampFloatNode
    );
    register_builtin!(
        NodeTypeId::POWER_FLOAT,
        nodes::math::power_float::PowerFloatNode
    );
    register_builtin!(
        NodeTypeId::ROOT_FLOAT,
        nodes::math::root_float::RootFloatNode
    );
    register_builtin!(
        NodeTypeId::EXPONENTIAL_FLOAT,
        nodes::math::exponential_float::ExponentialFloatNode
    );
    register_builtin!(NodeTypeId::LOG_FLOAT, nodes::math::log_float::LogFloatNode);
    register_builtin!(
        NodeTypeId::MAP_RANGE_FLOAT,
        nodes::math::map_range_float::MapRangeFloatNode
    );
    register_builtin!(
        NodeTypeId::ROUND_FLOAT,
        nodes::math::round_float::RoundFloatNode
    );
    register_builtin!(
        NodeTypeId::SCALE_TENSOR,
        nodes::math::scale_tensor::ScaleTensorNode
    );
    register_builtin!(
        NodeTypeId::SCALE_COLOR,
        nodes::frame_operations::scale_color::ScaleColorNode
    );
    register_builtin!(
        NodeTypeId::MULTIPLY_COLOR,
        nodes::frame_operations::multiply_color::MultiplyColorNode
    );
    register_builtin!(
        NodeTypeId::TINT_FRAME,
        nodes::frame_operations::tint_frame::TintFrameNode
    );
    register_builtin!(
        NodeTypeId::MASK_FRAME,
        nodes::frame_operations::mask_frame::MaskFrameNode
    );
    register_builtin!(
        NodeTypeId::MIX_COLOR,
        nodes::frame_operations::mix_color::MixColorNode
    );
    register_builtin!(
        NodeTypeId::ALPHA_OVER,
        nodes::frame_operations::alpha_over::AlphaOverNode
    );
    register_builtin!(NodeTypeId::FADE, nodes::temporal_filters::fade::FadeNode);
    register_builtin!(
        NodeTypeId::MOVING_AVERAGE,
        nodes::temporal_filters::moving_average::MovingAverageNode
    );
    register_builtin!(
        NodeTypeId::BOX_BLUR,
        nodes::spatial_filters::box_blur::BoxBlurNode
    );
    register_builtin!(
        NodeTypeId::GAUSSIAN_BLUR,
        nodes::spatial_filters::gaussian_blur::GaussianBlurNode
    );
    register_builtin!(
        NodeTypeId::MEDIAN_FILTER,
        nodes::spatial_filters::median_filter::MedianFilterNode
    );
    register_builtin!(
        NodeTypeId::SPECTRUM_ANALYZER,
        nodes::generators::spectrum_analyzer::SpectrumAnalyzerNode
    );
    register_builtin!(
        NodeTypeId::SOLID_FRAME,
        nodes::generators::solid_frame::SolidFrameNode
    );
    register_builtin!(
        NodeTypeId::RAINBOW_SWEEP,
        nodes::generators::rainbow_sweep::RainbowSweepNode
    );
    register_builtin!(
        NodeTypeId::CIRCLE_SWEEP,
        nodes::generators::circle_sweep::CircleSweepNode
    );
    register_builtin!(
        NodeTypeId::LEVEL_BAR,
        nodes::generators::level_bar::LevelBarNode
    );
    register_builtin!(
        NodeTypeId::BOUNCING_BALLS,
        nodes::generators::bouncing_balls::BouncingBallsNode
    );
    register_builtin!(
        NodeTypeId::TWINKLE_STARS,
        nodes::generators::twinkle_stars::TwinkleStarsNode
    );
    register_builtin!(NodeTypeId::PLASMA, nodes::generators::plasma::PlasmaNode);
    register_builtin!(
        NodeTypeId::FRAME_BRIGHTNESS,
        nodes::frame_operations::frame_brightness::FrameBrightnessNode
    );
    register_builtin!(
        NodeTypeId::WLED_DUMMY_DISPLAY,
        nodes::debug::wled_dummy_display::WledDummyDisplayNode
    );

    anyhow::ensure!(
        registry.definition(NodeTypeId::FLOAT_CONSTANT).is_some(),
        "builtin node registry must contain inputs.float_constant"
    );

    Ok(Arc::new(registry))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value as JsonValue;
    use shared::{NodeCategory, NodeDefinition, builtin_node_definition};

    use super::{NodeRegistry, RegisteredNodeType, build_builtin_node_registry};
    use crate::node_runtime::{
        NodeEvaluationContext, RuntimeNode, RuntimeNodeEvaluator, RuntimeNodeFromParameters,
        TypedNodeEvaluation,
    };

    /// Builds a factory placeholder for duplicate-registration tests.
    fn dummy_factory(_parameters: &HashMap<String, JsonValue>) -> Box<dyn RuntimeNodeEvaluator> {
        panic!("dummy factory should not be called")
    }

    /// Returns an empty diagnostics list for duplicate-registration tests.
    fn dummy_diagnostics(_parameters: &HashMap<String, JsonValue>) -> Vec<shared::NodeDiagnostic> {
        Vec::new()
    }

    #[derive(Default)]
    struct TestNode;

    impl RuntimeNodeFromParameters for TestNode {}

    impl RuntimeNode for TestNode {
        type Inputs = ();
        type Outputs = TestOutputs;

        /// Evaluates the test node and emits a constant float output.
        fn evaluate(
            &mut self,
            _context: &NodeEvaluationContext,
            _inputs: Self::Inputs,
        ) -> anyhow::Result<TypedNodeEvaluation<Self::Outputs>> {
            Ok(TypedNodeEvaluation::from_outputs(TestOutputs {
                value: 42.0,
            }))
        }
    }

    #[derive(serde::Serialize)]
    struct TestOutputs {
        value: f32,
    }

    #[test]
    /// Tests that the built-in registry contains the expected built-in node entries.
    fn builtin_registry_contains_nodes() {
        let registry = build_builtin_node_registry().expect("build builtin registry");
        assert!(registry.definitions().len() > 10);
        assert!(
            registry
                .definition(shared::NodeTypeId::MULTIPLY_FLOAT)
                .is_some()
        );
    }

    #[test]
    /// Tests that registering the same node ID twice returns an error.
    fn duplicate_node_ids_are_rejected() {
        let mut registry = NodeRegistry::new();
        let definition = builtin_node_definition(shared::NodeTypeId::FLOAT_CONSTANT)
            .expect("builtin float constant definition");
        registry
            .register(RegisteredNodeType {
                definition: definition.clone(),
                evaluator_factory: dummy_factory,
                diagnostics_factory: dummy_diagnostics,
            })
            .expect("register first node");

        let error = registry
            .register(RegisteredNodeType {
                definition,
                evaluator_factory: dummy_factory,
                diagnostics_factory: dummy_diagnostics,
            })
            .expect_err("duplicate node registration should fail");
        assert!(error.to_string().contains("duplicate node registration"));
    }

    #[test]
    /// Tests that custom Rust nodes can be registered through the same API as built-ins.
    fn custom_rust_nodes_register_through_same_api() {
        let mut registry = NodeRegistry::new();
        registry
            .register_fast_node::<TestNode>(NodeDefinition {
                id: "custom.test".to_owned(),
                display_name: "Custom Test".to_owned(),
                category: NodeCategory::Debug,
                inputs: Vec::new(),
                outputs: vec![shared::NodeOutputDefinition {
                    name: "value".to_owned(),
                    display_name: "Value".to_owned(),
                    value_kind: shared::ValueKind::Float,
                    accepted_kinds: Vec::new(),
                }],
                parameters: Vec::new(),
                connection: shared::NodeConnectionDefinition {
                    max_input_connections: 1,
                    require_value_kind_match: true,
                },
                runtime_updates: None,
            })
            .expect("register custom node");

        assert!(registry.definition("custom.test").is_some());
        assert!(
            registry
                .evaluator_for("custom.test", &HashMap::new())
                .is_some()
        );
    }
}
