use serde::{Deserialize, Serialize};

/// Identifies the coarse-grained type of a graph value for connection validation and UI behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Any,
    Float,
    String,
    FloatTensor,
    Color,
    LedLayout,
    ColorFrame,
    MappedFrame,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "value")]
/// Represents a concrete runtime or persisted value carried through the graph system.
pub enum InputValue {
    Float(f32),
    String(String),
    FloatTensor(FloatTensor),
    Color(RgbaColor),
    LedLayout(LedLayout),
    ColorFrame(ColorFrame),
    MappedFrame(ColorFrame),
}

impl InputValue {
    /// Returns the coarse-grained value kind represented by this concrete value.
    pub fn value_kind(&self) -> ValueKind {
        match self {
            Self::Float(_) => ValueKind::Float,
            Self::String(_) => ValueKind::String,
            Self::FloatTensor(_) => ValueKind::FloatTensor,
            Self::Color(_) => ValueKind::Color,
            Self::LedLayout(_) => ValueKind::LedLayout,
            Self::ColorFrame(_) => ValueKind::ColorFrame,
            Self::MappedFrame(_) => ValueKind::MappedFrame,
        }
    }

    /// Returns the contained frame payload for either frame-valued variant.
    pub fn as_frame(&self) -> Option<&ColorFrame> {
        match self {
            Self::ColorFrame(frame) | Self::MappedFrame(frame) => Some(frame),
            _ => None,
        }
    }

    /// Returns the contained frame payload for either frame-valued variant.
    pub fn as_frame_mut(&mut self) -> Option<&mut ColorFrame> {
        match self {
            Self::ColorFrame(frame) | Self::MappedFrame(frame) => Some(frame),
            _ => None,
        }
    }

    /// Converts a frame payload into the requested frame-valued variant.
    pub fn from_frame_kind(kind: ValueKind, frame: ColorFrame) -> Option<Self> {
        match kind {
            ValueKind::ColorFrame => Some(Self::ColorFrame(frame)),
            ValueKind::MappedFrame => Some(Self::MappedFrame(frame)),
            _ => None,
        }
    }
}

impl ValueKind {
    /// Returns whether the kind represents one of the frame-carrying variants.
    pub fn is_frame(self) -> bool {
        matches!(self, Self::ColorFrame | Self::MappedFrame)
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Describes a concrete LED layout used for frame-producing nodes and sinks.
pub struct LedLayout {
    pub id: String,
    #[serde(default)]
    pub role: LedLayoutRole,
    pub pixel_count: usize,
    #[serde(default)]
    pub width: Option<usize>,
    #[serde(default)]
    pub height: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points_3d: Option<Vec<Vec3>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
/// Declares whether a layout is a render destination or a source capture layout.
pub enum LedLayoutRole {
    RenderTarget,
    Source,
}

impl Default for LedLayoutRole {
    fn default() -> Self {
        Self::RenderTarget
    }
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
/// Represents one LED position in a spatial render layout.
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
