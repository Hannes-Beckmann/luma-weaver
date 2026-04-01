/// Implements `RuntimeInputs` for a struct by reading named inputs with fallback defaults.
macro_rules! impl_runtime_inputs {
    ($type:ty { $($field:ident = $default:expr),* $(,)? }) => {
        impl $crate::node_runtime::RuntimeInputs for $type {
            fn from_runtime_inputs(
                inputs: &std::collections::HashMap<String, ::shared::InputValue>,
            ) -> anyhow::Result<Self> {
                use anyhow::Context as _;

                Ok(Self {
                    $(
                        $field: match inputs.get(stringify!($field)) {
                            Some(value) => $crate::node_runtime::FromInputValue::from_input_value(value)
                                .with_context(|| format!("read input {}", stringify!($field)))?,
                            None => $default,
                        },
                    )*
                })
            }
        }
    };
}

/// Implements `RuntimeOutputs` for a struct by serializing named fields into runtime outputs.
macro_rules! impl_runtime_outputs {
    ($type:ty { $($field:ident),* $(,)? }) => {
        impl $crate::node_runtime::RuntimeOutputs for $type {
            fn into_runtime_outputs(
                self,
            ) -> anyhow::Result<std::collections::HashMap<String, ::shared::InputValue>> {
                use anyhow::Context as _;

                let mut outputs = std::collections::HashMap::new();
                $(
                    outputs.insert(
                        stringify!($field).to_owned(),
                        $crate::node_runtime::IntoInputValue::into_input_value(self.$field)
                            .with_context(|| format!("serialize output {}", stringify!($field)))?,
                    );
                )*
                Ok(outputs)
            }
        }
    };
}

/// Internal helper used by `impl_runtime_parameters!` to expand parameter parsing field by field.
///
/// This macro is not meant to be invoked directly outside the runtime parameter macro family.
macro_rules! impl_runtime_parameters_builder {
    ($parameters:ident, $diagnostics:ident, ($($statements:tt)*), ($($built:tt)*),) => {{
        $($statements)*
        Self { $($built)* }
    }};
    ($parameters:ident, $diagnostics:ident, ($($statements:tt)*), ($($built:tt)*), ..$rest:expr $(,)?) => {{
        $($statements)*
        Self {
            $($built)*
            ..$rest
        }
    }};
    ($parameters:ident, $diagnostics:ident, ($($statements:tt)*), ($($built:tt)*), $field:ident : $type:ty = $default:expr $(, $($rest:tt)*)?) => {
        $crate::node_runtime::impl_runtime_parameters_builder!(
            $parameters,
            $diagnostics,
            (
                $($statements)*
                let $field = match $crate::node_runtime::parameter_status::<$type>($parameters, stringify!($field)) {
                    $crate::node_runtime::ParameterStatus::Present(value) => value,
                    $crate::node_runtime::ParameterStatus::Missing => $default,
                    $crate::node_runtime::ParameterStatus::Invalid => {
                        $diagnostics.push($crate::node_runtime::invalid_parameter_diagnostic(
                            stringify!($field),
                            std::any::type_name::<$type>(),
                        ));
                        $default
                    }
                };
            ),
            (
                $($built)*
                $field: $field,
            ),
            $($($rest)*)?
        )
    };
    ($parameters:ident, $diagnostics:ident, ($($statements:tt)*), ($($built:tt)*), $field:ident : $type:ty => |$value:ident| $map:expr, default $default:expr $(, $($rest:tt)*)?) => {
        $crate::node_runtime::impl_runtime_parameters_builder!(
            $parameters,
            $diagnostics,
            (
                $($statements)*
                let $field = match $crate::node_runtime::parameter_status::<$type>($parameters, stringify!($field)) {
                    $crate::node_runtime::ParameterStatus::Present($value) => {
                        let adjustment = $crate::node_runtime::IntoParameterAdjustment::into_parameter_adjustment($map);
                        let (value, diagnostics) = adjustment.into_parts(stringify!($field));
                        $diagnostics.extend(diagnostics);
                        value
                    }
                    $crate::node_runtime::ParameterStatus::Missing => $default,
                    $crate::node_runtime::ParameterStatus::Invalid => {
                        $diagnostics.push($crate::node_runtime::invalid_parameter_diagnostic(
                            stringify!($field),
                            std::any::type_name::<$type>(),
                        ));
                        $default
                    }
                };
            ),
            (
                $($built)*
                $field: $field,
            ),
            $($($rest)*)?
        )
    };
}

/// Implements `RuntimeNodeFromParameters` for a struct using declarative field parsing rules.
///
/// Missing parameters fall back to the provided defaults. Invalid parameters emit automatic
/// construction diagnostics, and mapping closures can return `ParameterAdjustment`s to report
/// clamping or other normalization.
macro_rules! impl_runtime_parameters {
    ($type:ty { $($fields:tt)* }) => {
        impl $crate::node_runtime::RuntimeNodeFromParameters for $type {
            fn from_parameters(
                parameters: &std::collections::HashMap<String, serde_json::Value>,
            ) -> $crate::node_runtime::NodeConstruction<Self> {
                let mut diagnostics = Vec::new();
                let node = $crate::node_runtime::impl_runtime_parameters_builder!(
                    parameters,
                    diagnostics,
                    (),
                    (),
                    $($fields)*
                );
                $crate::node_runtime::NodeConstruction { node, diagnostics }
            }
        }
    };
}

/// Re-exports the runtime input macro for use by node implementations.
pub(crate) use impl_runtime_inputs;
/// Re-exports the runtime output macro for use by node implementations.
pub(crate) use impl_runtime_outputs;
/// Re-exports the runtime parameter macro for use by node implementations.
pub(crate) use impl_runtime_parameters;
/// Re-exports the internal runtime parameter builder macro for macro expansion support.
pub(crate) use impl_runtime_parameters_builder;
