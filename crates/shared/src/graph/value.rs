use serde::{Deserialize, Serialize};

/// Identifies the coarse-grained type of a graph value for connection validation and UI behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Any,
    Float,
    FloatTensor,
    Color,
    LedLayout,
    ColorFrame,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "value")]
/// Represents a concrete runtime or persisted value carried through the graph system.
pub enum InputValue {
    Float(f32),
    FloatTensor(FloatTensor),
    Color(RgbaColor),
    LedLayout(LedLayout),
    ColorFrame(ColorFrame),
}

impl InputValue {
    /// Returns the coarse-grained value kind represented by this concrete value.
    pub fn value_kind(&self) -> ValueKind {
        match self {
            Self::Float(_) => ValueKind::Float,
            Self::FloatTensor(_) => ValueKind::FloatTensor,
            Self::Color(_) => ValueKind::Color,
            Self::LedLayout(_) => ValueKind::LedLayout,
            Self::ColorFrame(_) => ValueKind::ColorFrame,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents a dense tensor of `f32` values together with its logical shape.
pub struct FloatTensor {
    #[serde(default)]
    pub shape: Vec<usize>,
    #[serde(default)]
    pub values: Vec<f32>,
}

impl FloatTensor {
    /// Returns the logical number of elements implied by the tensor shape.
    ///
    /// Empty shapes are treated as a scalar containing one element.
    pub fn element_count(&self) -> usize {
        if self.shape.is_empty() {
            1
        } else {
            self.shape.iter().copied().product()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Describes a concrete LED layout used for frame-producing nodes and sinks.
pub struct LedLayout {
    pub id: String,
    pub pixel_count: usize,
    #[serde(default)]
    pub width: Option<usize>,
    #[serde(default)]
    pub height: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents a full RGBA frame paired with the layout it was rendered for.
pub struct ColorFrame {
    pub layout: LedLayout,
    pub pixels: Vec<RgbaColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents a color gradient as an ordered list of color stops.
pub struct ColorGradient {
    pub stops: Vec<ColorGradientStop>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
/// Represents one stop in a color gradient.
pub struct ColorGradientStop {
    pub position: f32,
    pub color: RgbaColor,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
/// Represents a normalized RGBA color using `0.0..=1.0` channel values.
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
