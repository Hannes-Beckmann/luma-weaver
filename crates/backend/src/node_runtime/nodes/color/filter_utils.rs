use shared::LedLayout;

pub(crate) fn layout_dimensions(layout: &LedLayout) -> (usize, usize) {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => (layout.pixel_count.max(1), 1),
    }
}

pub(crate) fn clamped_index(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    pixel_count: usize,
) -> usize {
    let clamped_x = x.min(width.saturating_sub(1));
    let clamped_y = y.min(height.saturating_sub(1));
    (clamped_y * width + clamped_x).min(pixel_count.saturating_sub(1))
}
