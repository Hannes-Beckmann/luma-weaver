use anyhow::{Context, Result};
use image::RgbaImage;
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::Tree,
};
use shared::{ColorFrame, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor};
use std::io::ErrorKind;
use std::time::Duration;

use crate::platform_time::Instant;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};
use crate::services::image_asset_store::global_image_asset_store;
use crate::services::image_codec::parse_svg;

const IMAGE_FAILURE_RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(crate) struct ImageSourceNode {
    asset_id: String,
    fit_mode: String,
    cached_source: Option<CachedImageSource>,
}

crate::node_runtime::impl_runtime_parameters!(ImageSourceNode {
    asset_id: String = String::new(),
    fit_mode: String = "contain".to_owned(),
    ..Default::default()
});

pub(crate) struct ImageSourceOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(ImageSourceOutputs { frame });

impl RuntimeNode for ImageSourceNode {
    type Inputs = ();
    type Outputs = ImageSourceOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let layout_hint = context.render_layout.clone();
        let fit_mode = self.fit_mode();
        let asset_id = self.asset_id.trim().to_owned();
        if asset_id.is_empty() {
            return Ok(TypedNodeEvaluation {
                outputs: ImageSourceOutputs {
                    frame: transparent_frame(layout_hint.as_ref()),
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Warning,
                    code: Some("image_source_missing_source".to_owned()),
                    message: "No uploaded image selected.".to_owned(),
                }],
            });
        }

        let mut diagnostics = Vec::new();
        let source = match self.ensure_source(&asset_id) {
            Ok(source) => source,
            Err(error) => {
                if self.mark_failure_logged(&asset_id) {
                    tracing::warn!(
                        asset_id = %asset_id,
                        error = %format_error_chain(&error),
                        "failed to load image source asset"
                    );
                }
                diagnostics.push(NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("image_source_load_failed".to_owned()),
                    message: user_facing_load_error_message(&asset_id, &error),
                });
                return Ok(TypedNodeEvaluation {
                    outputs: ImageSourceOutputs {
                        frame: transparent_frame(layout_hint.as_ref()),
                    },
                    frontend_updates: Vec::new(),
                    diagnostics,
                });
            }
        };

        let layout = target_layout(layout_hint, source);
        Ok(TypedNodeEvaluation {
            outputs: ImageSourceOutputs {
                frame: render_source_to_frame(source, &layout, fit_mode)?,
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl ImageSourceNode {
    fn fit_mode(&self) -> ImageFitMode {
        match self.fit_mode.trim() {
            "stretch" => ImageFitMode::Stretch,
            "cover" => ImageFitMode::Cover,
            _ => ImageFitMode::Contain,
        }
    }

    fn ensure_source(&mut self, asset_id: &str) -> Result<&DecodedSource> {
        let failure_message = match self.cached_source.as_ref() {
            Some(CachedImageSource::Loaded {
                asset_id: cached_id,
                ..
            }) if cached_id == asset_id => {
                return match self.cached_source.as_ref() {
                    Some(CachedImageSource::Loaded { source, .. }) => Ok(source),
                    _ => anyhow::bail!("Image source cache changed unexpectedly"),
                };
            }
            Some(CachedImageSource::Failed {
                asset_id: cached_id,
                message,
                retry_after,
                ..
            }) if cached_id == asset_id && Instant::now() < *retry_after => Some(message.clone()),
            _ => None,
        };

        if let Some(message) = failure_message {
            anyhow::bail!("{message}");
        }

        match load_uploaded_image(asset_id) {
            Ok(source) => {
                self.cached_source = Some(CachedImageSource::Loaded {
                    asset_id: asset_id.to_owned(),
                    source,
                });
            }
            Err(error) => {
                let message = format_error_chain(&error);
                self.cached_source = Some(CachedImageSource::Failed {
                    asset_id: asset_id.to_owned(),
                    message: message.clone(),
                    retry_after: Instant::now() + IMAGE_FAILURE_RETRY_DELAY,
                    warning_emitted: false,
                });
                anyhow::bail!("{message}");
            }
        }

        match self.cached_source.as_ref() {
            Some(CachedImageSource::Loaded { source, .. }) => Ok(source),
            _ => anyhow::bail!("Image source did not produce a cached source"),
        }
    }

    fn mark_failure_logged(&mut self, asset_id: &str) -> bool {
        match self.cached_source.as_mut() {
            Some(CachedImageSource::Failed {
                asset_id: cached_id,
                warning_emitted,
                ..
            }) if cached_id == asset_id && !*warning_emitted => {
                *warning_emitted = true;
                true
            }
            _ => false,
        }
    }
}

enum CachedImageSource {
    Loaded {
        asset_id: String,
        source: DecodedSource,
    },
    Failed {
        asset_id: String,
        message: String,
        retry_after: Instant,
        warning_emitted: bool,
    },
}

#[derive(Clone, Copy)]
enum ImageFitMode {
    Stretch,
    Contain,
    Cover,
}

enum DecodedSource {
    Raster(RgbaImage),
    Svg(Tree),
}

fn load_uploaded_image(asset_id: &str) -> Result<DecodedSource> {
    let store = global_image_asset_store()
        .context("Uploaded image storage is unavailable in this backend process")?;
    let bytes = store
        .load_image_bytes(asset_id)
        .with_context(|| format!("Load uploaded image asset '{asset_id}'"))?;
    decode_image_source(&bytes, None)
}

fn decode_image_source(bytes: &[u8], content_type: Option<&str>) -> Result<DecodedSource> {
    if is_svg_payload(bytes, content_type) {
        return Ok(DecodedSource::Svg(parse_svg(
            bytes,
            "Parse SVG image source",
        )?));
    }

    Ok(DecodedSource::Raster(
        image::load_from_memory(bytes)
            .context("Decode image source")?
            .into_rgba8(),
    ))
}

fn is_svg_payload(bytes: &[u8], content_type: Option<&str>) -> bool {
    if content_type.is_some_and(|value| value.contains("image/svg+xml")) {
        return true;
    }

    crate::services::image_codec::is_svg_payload(bytes)
}

fn format_error_chain(error: &anyhow::Error) -> String {
    let mut chain = error.chain();
    let Some(first) = chain.next() else {
        return "Unknown image source error".to_owned();
    };

    let mut message = first.to_string();
    for cause in chain {
        let cause = cause.to_string();
        if !cause.is_empty() && cause != message {
            message.push_str(": ");
            message.push_str(&cause);
        }
    }
    message
}

fn user_facing_load_error_message(asset_id: &str, error: &anyhow::Error) -> String {
    if error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io_error| io_error.kind() == ErrorKind::NotFound)
    {
        return format!("Uploaded image asset '{asset_id}' is missing.");
    }

    if error.to_string().contains("image asset id") && error.to_string().contains("is invalid") {
        return "Uploaded image reference is invalid.".to_owned();
    }

    "Uploaded image asset could not be loaded.".to_owned()
}

fn target_layout(layout_hint: Option<LedLayout>, source: &DecodedSource) -> LedLayout {
    layout_hint.unwrap_or_else(|| match source {
        DecodedSource::Raster(source_image) => LedLayout {
            id: "image_source".to_owned(),
            pixel_count: source_image.width() as usize * source_image.height() as usize,
            width: Some(source_image.width() as usize),
            height: Some(source_image.height() as usize),
        },
        DecodedSource::Svg(tree) => {
            let size = tree.size().to_int_size();
            LedLayout {
                id: "image_source".to_owned(),
                pixel_count: size.width() as usize * size.height() as usize,
                width: Some(size.width() as usize),
                height: Some(size.height() as usize),
            }
        }
    })
}

fn transparent_frame(layout_hint: Option<&LedLayout>) -> ColorFrame {
    let layout = layout_hint.cloned().unwrap_or(LedLayout {
        id: "image_source".to_owned(),
        pixel_count: 0,
        width: None,
        height: None,
    });
    ColorFrame {
        pixels: vec![transparent_black(); layout.pixel_count],
        layout,
    }
}

fn render_image_to_frame(
    source_image: &RgbaImage,
    layout: &LedLayout,
    fit_mode: ImageFitMode,
) -> ColorFrame {
    let (target_width, target_height) = render_dimensions(layout);
    let pixels = (0..target_height)
        .flat_map(|y| {
            (0..target_width).map(move |x| {
                sample_source_image(source_image, x, y, target_width, target_height, fit_mode)
            })
        })
        .collect();

    ColorFrame {
        layout: layout.clone(),
        pixels,
    }
}

fn render_source_to_frame(
    source: &DecodedSource,
    layout: &LedLayout,
    fit_mode: ImageFitMode,
) -> Result<ColorFrame> {
    match source {
        DecodedSource::Raster(source_image) => {
            Ok(render_image_to_frame(source_image, layout, fit_mode))
        }
        DecodedSource::Svg(tree) => render_svg_to_frame(tree, layout, fit_mode),
    }
}

fn render_svg_to_frame(
    tree: &Tree,
    layout: &LedLayout,
    fit_mode: ImageFitMode,
) -> Result<ColorFrame> {
    let (target_width, target_height) = render_dimensions(layout);
    let mut pixmap = Pixmap::new(target_width as u32, target_height as u32)
        .context("Allocate SVG rasterization buffer")?;

    let svg_size = tree.size();
    let source_width = svg_size.width();
    let source_height = svg_size.height();
    let target_width_f32 = target_width as f32;
    let target_height_f32 = target_height as f32;

    let transform = match fit_mode {
        ImageFitMode::Stretch => Transform::from_row(
            target_width_f32 / source_width,
            0.0,
            0.0,
            target_height_f32 / source_height,
            0.0,
            0.0,
        ),
        ImageFitMode::Contain | ImageFitMode::Cover => {
            let scale_x = target_width_f32 / source_width;
            let scale_y = target_height_f32 / source_height;
            let scale = match fit_mode {
                ImageFitMode::Contain => scale_x.min(scale_y),
                ImageFitMode::Cover => scale_x.max(scale_y),
                ImageFitMode::Stretch => unreachable!(),
            };
            let rendered_width = source_width * scale;
            let rendered_height = source_height * scale;
            let offset_x = (target_width_f32 - rendered_width) * 0.5;
            let offset_y = (target_height_f32 - rendered_height) * 0.5;
            Transform::from_row(scale, 0.0, 0.0, scale, offset_x, offset_y)
        }
    };

    resvg::render(tree, transform, &mut pixmap.as_mut());

    let pixels = pixmap
        .pixels()
        .iter()
        .map(|pixel| {
            let color = pixel.demultiply();
            RgbaColor {
                r: color.red() as f32 / 255.0,
                g: color.green() as f32 / 255.0,
                b: color.blue() as f32 / 255.0,
                a: color.alpha() as f32 / 255.0,
            }
        })
        .collect();

    Ok(ColorFrame {
        layout: layout.clone(),
        pixels,
    })
}

fn render_dimensions(layout: &LedLayout) -> (usize, usize) {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 1 && height > 1 => (width, height),
        _ => (layout.pixel_count.max(1), 1),
    }
}

fn sample_source_image(
    source_image: &RgbaImage,
    x: usize,
    y: usize,
    target_width: usize,
    target_height: usize,
    fit_mode: ImageFitMode,
) -> RgbaColor {
    let source_width = source_image.width() as f32;
    let source_height = source_image.height() as f32;
    let target_width = target_width as f32;
    let target_height = target_height as f32;
    let target_x = x as f32 + 0.5;
    let target_y = y as f32 + 0.5;

    let (source_x, source_y) = match fit_mode {
        ImageFitMode::Stretch => (
            target_x / target_width * source_width,
            target_y / target_height * source_height,
        ),
        ImageFitMode::Contain | ImageFitMode::Cover => {
            let scale_x = target_width / source_width;
            let scale_y = target_height / source_height;
            let scale = match fit_mode {
                ImageFitMode::Contain => scale_x.min(scale_y),
                ImageFitMode::Cover => scale_x.max(scale_y),
                ImageFitMode::Stretch => unreachable!(),
            };

            let rendered_width = source_width * scale;
            let rendered_height = source_height * scale;
            let offset_x = (target_width - rendered_width) * 0.5;
            let offset_y = (target_height - rendered_height) * 0.5;
            let local_x = target_x - offset_x;
            let local_y = target_y - offset_y;

            if matches!(fit_mode, ImageFitMode::Contain)
                && (local_x < 0.0
                    || local_y < 0.0
                    || local_x >= rendered_width
                    || local_y >= rendered_height)
            {
                return transparent_black();
            }

            (local_x / scale, local_y / scale)
        }
    };

    rgba_from_image_pixel(source_image, source_x, source_y)
}

fn rgba_from_image_pixel(source_image: &RgbaImage, source_x: f32, source_y: f32) -> RgbaColor {
    let max_x = source_image.width().saturating_sub(1) as f32;
    let max_y = source_image.height().saturating_sub(1) as f32;
    let pixel = source_image.get_pixel(
        source_x.floor().clamp(0.0, max_x) as u32,
        source_y.floor().clamp(0.0, max_y) as u32,
    );
    RgbaColor {
        r: pixel[0] as f32 / 255.0,
        g: pixel[1] as f32 / 255.0,
        b: pixel[2] as f32 / 255.0,
        a: pixel[3] as f32 / 255.0,
    }
}

fn transparent_black() -> RgbaColor {
    RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use image::Rgba;
    use shared::LedLayout;
    use std::io::ErrorKind;
    use std::time::{Duration, Instant};

    use super::{
        DecodedSource, ImageFitMode, ImageSourceNode, decode_image_source, render_image_to_frame,
        render_source_to_frame, target_layout, transparent_black, user_facing_load_error_message,
    };
    use crate::node_runtime::{NodeEvaluationContext, RuntimeNode};

    fn sample_image() -> image::RgbaImage {
        image::RgbaImage::from_fn(2, 1, |x, _| match x {
            0 => Rgba([255, 0, 0, 255]),
            _ => Rgba([0, 0, 255, 255]),
        })
    }

    #[test]
    fn uses_source_dimensions_when_no_render_layout_is_available() {
        let layout = target_layout(None, &DecodedSource::Raster(sample_image()));

        assert_eq!(layout.pixel_count, 2);
        assert_eq!(layout.width, Some(2));
        assert_eq!(layout.height, Some(1));
    }

    #[test]
    fn contain_mode_letterboxes_when_target_is_taller_than_source() {
        let image = sample_image();
        let frame = render_image_to_frame(
            &image,
            &LedLayout {
                id: "matrix".to_owned(),
                pixel_count: 4,
                width: Some(2),
                height: Some(2),
            },
            ImageFitMode::Contain,
        );

        assert_eq!(frame.pixels[0].r, 1.0);
        assert_eq!(frame.pixels[1].b, 1.0);
        assert_eq!(frame.pixels[2], transparent_black());
        assert_eq!(frame.pixels[3], transparent_black());
    }

    #[test]
    fn svg_payload_uses_svg_dimensions_when_no_render_layout_is_available() {
        let source = decode_image_source(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><rect width="4" height="3" fill="#ff0000"/></svg>"##,
            Some("image/svg+xml"),
        )
        .expect("svg should parse");
        let layout = target_layout(None, &source);

        assert_eq!(layout.pixel_count, 12);
        assert_eq!(layout.width, Some(4));
        assert_eq!(layout.height, Some(3));
    }

    #[test]
    fn missing_uploaded_image_produces_warning_diagnostic() {
        let mut node = ImageSourceNode {
            ..Default::default()
        };

        let evaluation = node
            .evaluate(
                &NodeEvaluationContext {
                    graph_id: "graph".to_owned(),
                    graph_name: "Graph".to_owned(),
                    elapsed_seconds: 0.0,
                    render_layout: None,
                },
                (),
            )
            .expect("missing upload should not panic");

        assert!(evaluation.outputs.frame.pixels.is_empty());
        assert_eq!(evaluation.diagnostics.len(), 1);
        assert_eq!(
            evaluation.diagnostics[0].code.as_deref(),
            Some("image_source_missing_source")
        );
    }

    #[test]
    fn renders_svg_payload_into_target_layout() {
        let source = decode_image_source(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="1" height="1" fill="#ff0000"/><rect x="1" width="1" height="1" fill="#0000ff"/></svg>"##,
            Some("image/svg+xml"),
        )
        .expect("svg should parse");

        let frame = render_source_to_frame(
            &source,
            &LedLayout {
                id: "matrix".to_owned(),
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
            },
            ImageFitMode::Stretch,
        )
        .expect("svg should render");

        assert!(frame.pixels[0].r > 0.9);
        assert!(frame.pixels[1].b > 0.9);
    }

    #[test]
    fn missing_uploaded_image_uses_sanitized_diagnostic_message() {
        let error = std::fs::read("definitely-missing-image-asset")
            .context("read image asset")
            .context("Load uploaded image asset")
            .expect_err("missing file should error");

        let message = user_facing_load_error_message("asset-123", &error);

        assert_eq!(message, "Uploaded image asset 'asset-123' is missing.");
        assert!(matches!(
            error
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(ErrorKind::NotFound)
        ));
    }

    #[test]
    fn cached_failure_is_only_marked_for_logging_once() {
        let mut node = ImageSourceNode {
            cached_source: Some(super::CachedImageSource::Failed {
                asset_id: "asset-123".to_owned(),
                message: "missing".to_owned(),
                retry_after: Instant::now() + Duration::from_secs(5),
                warning_emitted: false,
            }),
            ..Default::default()
        };

        assert!(node.mark_failure_logged("asset-123"));
        assert!(!node.mark_failure_logged("asset-123"));
        assert!(!node.mark_failure_logged("other-asset"));
    }
}
