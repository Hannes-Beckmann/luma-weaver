use super::{NodeOutputDefinition, NodeParameter, NodeTypeId, ValueKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputKindRef<'a> {
    pub name: &'a str,
    pub kind: ValueKind,
}

pub(crate) fn input_kind(input_kinds: &[InputKindRef<'_>], name: &str) -> Option<ValueKind> {
    input_kinds
        .iter()
        .find(|input| input.name == name)
        .map(|input| input.kind)
}

pub(crate) fn infer_numeric_output_kind(
    node_type_id: &str,
    output: &NodeOutputDefinition,
    input_kinds: &[InputKindRef<'_>],
) -> ValueKind {
    let relevant_inputs: &[&str] = match node_type_id {
        NodeTypeId::ABS | NodeTypeId::EXPONENTIAL | NodeTypeId::ROUND => &["value"],
        NodeTypeId::CLAMP => &["value", "min", "max"],
        NodeTypeId::MAP_RANGE => &[
            "value",
            "source_min",
            "source_max",
            "target_min",
            "target_max",
        ],
        NodeTypeId::POWER => &["base", "exponent"],
        NodeTypeId::ROOT => &["value", "degree"],
        NodeTypeId::LOG => &["value", "base"],
        _ => &["a", "b"],
    };

    let kinds = relevant_inputs
        .iter()
        .map(|name| input_kind(input_kinds, name))
        .collect::<Vec<_>>();

    infer_preferred_kind(output, &kinds)
}

pub(crate) fn infer_preferred_kind(
    output: &NodeOutputDefinition,
    candidates: &[Option<ValueKind>],
) -> ValueKind {
    for preferred_kind in [
        ValueKind::ColorFrame,
        ValueKind::FloatTensor,
        ValueKind::Float,
    ] {
        if candidates
            .iter()
            .flatten()
            .any(|kind| *kind == preferred_kind)
            && output.accepts_kind(preferred_kind)
        {
            return preferred_kind;
        }
    }

    for candidate in candidates.iter().flatten() {
        if output.accepts_kind(*candidate) {
            return *candidate;
        }
    }

    output.value_kind
}

pub(crate) fn parameter_string(parameters: &[NodeParameter], name: &str) -> Option<String> {
    parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .and_then(|parameter| parameter.value.as_str().map(str::to_owned))
}
