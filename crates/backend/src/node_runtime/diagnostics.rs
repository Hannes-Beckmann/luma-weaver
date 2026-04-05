use ::shared::{ColorGradient, NodeDiagnostic, NodeDiagnosticSeverity};

/// Stores a partially constructed parameter diagnostic before the field name is known.
pub(crate) struct PendingParameterDiagnostic {
    pub(crate) severity: NodeDiagnosticSeverity,
    pub(crate) code_suffix: &'static str,
    pub(crate) message: String,
}

/// Couples a normalized parameter value with any diagnostics produced while deriving it.
pub(crate) struct ParameterAdjustment<T> {
    pub(crate) value: T,
    pending_diagnostics: Vec<PendingParameterDiagnostic>,
}

impl<T> ParameterAdjustment<T> {
    /// Wraps a value that required no normalization and produced no diagnostics.
    pub(crate) fn unchanged(value: T) -> Self {
        Self {
            value,
            pending_diagnostics: Vec::new(),
        }
    }

    /// Wraps a value together with a single pending diagnostic describing the adjustment.
    pub(crate) fn with_diagnostic(
        value: T,
        severity: NodeDiagnosticSeverity,
        code_suffix: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            value,
            pending_diagnostics: vec![PendingParameterDiagnostic {
                severity,
                code_suffix,
                message: message.into(),
            }],
        }
    }

    /// Finalizes the adjustment into its value and field-scoped diagnostics.
    ///
    /// The field name is used to build stable diagnostic codes such as
    /// `parameter_clamped.circle_count`.
    pub(crate) fn into_parts(self, field_name: &str) -> (T, Vec<NodeDiagnostic>) {
        let diagnostics = self
            .pending_diagnostics
            .into_iter()
            .map(|diagnostic| NodeDiagnostic {
                severity: diagnostic.severity,
                code: Some(format!(
                    "parameter_{}.{}",
                    diagnostic.code_suffix, field_name
                )),
                message: diagnostic.message,
            })
            .collect();
        (self.value, diagnostics)
    }
}

/// Converts a value or helper wrapper into the standard parameter-adjustment form.
pub(crate) trait IntoParameterAdjustment<T> {
    /// Returns the normalized value and any pending diagnostics associated with it.
    fn into_parameter_adjustment(self) -> ParameterAdjustment<T>;
}

impl<T> IntoParameterAdjustment<T> for T {
    /// Treats a plain value as an unchanged parameter adjustment.
    fn into_parameter_adjustment(self) -> ParameterAdjustment<T> {
        ParameterAdjustment::unchanged(self)
    }
}

impl<T> IntoParameterAdjustment<T> for ParameterAdjustment<T> {
    /// Reuses an adjustment value without modification.
    fn into_parameter_adjustment(self) -> ParameterAdjustment<T> {
        self
    }
}

/// Builds a warning diagnostic for a parameter that failed type-safe deserialization.
pub(crate) fn invalid_parameter_diagnostic(
    field_name: &str,
    expected_type: &str,
) -> NodeDiagnostic {
    NodeDiagnostic {
        severity: NodeDiagnosticSeverity::Warning,
        code: Some(format!("parameter_invalid.{field_name}")),
        message: format!(
            "Parameter '{}' has an invalid value; expected {}. Using the default instead.",
            field_name, expected_type
        ),
    }
}

/// Clamps an unsigned integer parameter into a `usize` range and reports when the value changed.
pub(crate) fn clamp_u64_to_usize(value: u64, min: u64, max: u64) -> ParameterAdjustment<usize> {
    let clamped = value.clamp(min, max);
    if clamped == value {
        ParameterAdjustment::unchanged(clamped as usize)
    } else {
        ParameterAdjustment::with_diagnostic(
            clamped as usize,
            NodeDiagnosticSeverity::Warning,
            "clamped",
            format!("Parameter value {} was clamped to {}.", value, clamped),
        )
    }
}

/// Clamps a floating-point parameter into an `f32` range and reports when the value changed.
pub(crate) fn clamp_f64_to_f32(value: f64, min: f64, max: f64) -> ParameterAdjustment<f32> {
    let clamped = value.clamp(min, max);
    if (clamped - value).abs() <= f64::EPSILON {
        ParameterAdjustment::unchanged(clamped as f32)
    } else {
        ParameterAdjustment::with_diagnostic(
            clamped as f32,
            NodeDiagnosticSeverity::Warning,
            "clamped",
            format!("Parameter value {} was clamped to {}.", value, clamped),
        )
    }
}

/// Enforces a minimum bound for a `u64` parameter converted to `usize`.
pub(crate) fn max_u64_to_usize(value: u64, min: u64) -> ParameterAdjustment<usize> {
    clamp_u64_to_usize(value, min, u64::MAX)
}

/// Clamps an unsigned integer parameter into a `u16` range and reports when the value changed.
pub(crate) fn clamp_u64_to_u16(value: u64, min: u64, max: u64) -> ParameterAdjustment<u16> {
    let clamped = value.clamp(min, max);
    if clamped == value {
        ParameterAdjustment::unchanged(clamped as u16)
    } else {
        ParameterAdjustment::with_diagnostic(
            clamped as u16,
            NodeDiagnosticSeverity::Warning,
            "clamped",
            format!("Parameter value {} was clamped to {}.", value, clamped),
        )
    }
}

/// Enforces a minimum bound for an `f64` parameter converted to `f32`.
pub(crate) fn max_f64_to_f32(value: f64, min: f64) -> ParameterAdjustment<f32> {
    let adjusted = value.max(min);
    if (adjusted - value).abs() <= f64::EPSILON {
        ParameterAdjustment::unchanged(adjusted as f32)
    } else {
        ParameterAdjustment::with_diagnostic(
            adjusted as f32,
            NodeDiagnosticSeverity::Warning,
            "clamped",
            format!("Parameter value {} was raised to {}.", value, adjusted),
        )
    }
}

/// Rejects empty gradients and falls back to the provided default gradient.
pub(crate) fn non_empty_gradient(
    value: ColorGradient,
    fallback: ColorGradient,
) -> ParameterAdjustment<ColorGradient> {
    if value.stops.is_empty() {
        ParameterAdjustment::with_diagnostic(
            fallback,
            NodeDiagnosticSeverity::Warning,
            "invalid",
            "Gradient was empty. Using the default gradient instead.".to_owned(),
        )
    } else {
        ParameterAdjustment::unchanged(value)
    }
}
