use serde_json::Value as JsonValue;
use shared::{
    ColorGradient, ColorGradientStop, InputValue, MqttBrokerConfig, NodeDefinition, NodeParameter,
    ParameterUiHint, RgbaColor, WledInstance,
};

use super::model::{coerce_input_value_kind, parameters_with_defaults};
use super::{EditorInputPort, EditorSnarlNode};

/// Formats an input value for compact inline display in the editor.
pub(super) fn format_input_value(value: &InputValue) -> String {
    match value {
        InputValue::Float(value) => format!("{value:.3}"),
        InputValue::FloatTensor(tensor) => {
            format!(
                "tensor(shape={:?}, len={})",
                tensor.shape,
                tensor.values.len()
            )
        }
        InputValue::Color(color) => format!(
            "rgba({:.2}, {:.2}, {:.2}, {:.2})",
            color.r, color.g, color.b, color.a
        ),
        InputValue::LedLayout(layout) => {
            if let (Some(width), Some(height)) = (layout.width, layout.height) {
                format!(
                    "layout(id={}, leds={}, {}x{})",
                    layout.id, layout.pixel_count, width, height
                )
            } else {
                format!("layout(id={}, leds={})", layout.id, layout.pixel_count)
            }
        }
        InputValue::ColorFrame(frame) => format!(
            "frame(layout={}, leds={}, dims={})",
            frame.layout.id,
            frame.pixels.len(),
            frame_layout_dims_label(frame)
        ),
    }
}

/// Renders a runtime value preview in the node body.
///
/// Scalar values are shown as text, while colors and frames use compact visual previews.
pub(super) fn show_runtime_value(ui: &mut egui::Ui, value: &InputValue) {
    match value {
        InputValue::Float(value) => {
            ui.label(format!("{value:.3}"));
        }
        InputValue::FloatTensor(tensor) => {
            ui.label(format!(
                "float_tensor(shape={:?}, len={})",
                tensor.shape,
                tensor.values.len()
            ));
        }
        InputValue::Color(color) => {
            let color32 = egui::Color32::from_rgba_unmultiplied(
                (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                (color.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 18.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, color32);
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                egui::StrokeKind::Outside,
            );
        }
        InputValue::LedLayout(layout) => {
            if let (Some(width), Some(height)) = (layout.width, layout.height) {
                ui.label(format!(
                    "layout(id={}, leds={}, {}x{})",
                    layout.id, layout.pixel_count, width, height
                ));
            } else {
                ui.label(format!(
                    "layout(id={}, leds={})",
                    layout.id, layout.pixel_count
                ));
            }
        }
        InputValue::ColorFrame(frame) => {
            ui.label(format!(
                "frame(layout={}, leds={}, dims={})",
                frame.layout.id,
                frame.pixels.len(),
                frame_layout_dims_label(frame)
            ));
        }
    }
}

/// Draws a pixel preview for a color frame.
///
/// The preview preserves the frame aspect ratio and paints each source pixel into a scaled cell
/// within the available preview rectangle.
pub(super) fn draw_color_frame_preview(ui: &mut egui::Ui, frame: &shared::ColorFrame) {
    let (width, height) = frame_preview_dimensions(frame);
    let max_preview_size = egui::vec2(240.0, 240.0);
    let preview_size = if width == 1 || height == 1 {
        // One-dimensional strips read best as a horizontal ribbon instead of a tall sliver.
        egui::vec2(max_preview_size.x, 18.0)
    } else {
        let aspect = width as f32 / height as f32;
        if aspect >= 1.0 {
            egui::vec2(max_preview_size.x, max_preview_size.x / aspect)
        } else {
            egui::vec2(max_preview_size.y * aspect, max_preview_size.y)
        }
    };

    let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));

    let pixels_per_point = ui.ctx().pixels_per_point();
    let snap = |x: f32| (x * pixels_per_point).round() / pixels_per_point;

    for y in 0..height {
        let y0 = snap(egui::lerp(
            rect.top()..=rect.bottom(),
            y as f32 / height as f32,
        ));
        let y1 = rect.bottom();

        for x in 0..width {
            let x0 = snap(egui::lerp(
                rect.left()..=rect.right(),
                x as f32 / width as f32,
            ));
            let x1 = rect.right();

            let idx = y * width + x;
            let color = frame.pixels.get(idx).copied().unwrap_or(shared::RgbaColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });

            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),
                0.0,
                egui::Color32::from_rgb(
                    (color.r.clamp(0.0, 1.0) * color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (color.g.clamp(0.0, 1.0) * color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (color.b.clamp(0.0, 1.0) * color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
                ),
            );
        }
    }

    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
        egui::StrokeKind::Outside,
    );
}

/// Draws a simple line plot for a sequence of float samples.
pub(super) fn draw_float_plot(ui: &mut egui::Ui, samples: &[f32]) {
    let desired_size = egui::vec2(ui.available_width().max(180.0), 84.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = ui.visuals().extreme_bg_color;
    let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color);
    let grid = egui::Color32::from_gray(80);
    let line = egui::Color32::from_rgb(97, 175, 239);

    painter.rect(
        rect,
        6.0,
        bg,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );

    let mid_y = rect.center().y;
    painter.line_segment(
        [
            egui::pos2(rect.left(), mid_y),
            egui::pos2(rect.right(), mid_y),
        ],
        egui::Stroke::new(1.0, grid),
    );

    if samples.len() < 2 {
        return;
    }

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for sample in samples {
        min = min.min(*sample);
        max = max.max(*sample);
    }
    if !min.is_finite() || !max.is_finite() {
        return;
    }
    let center = (min + max) * 0.5;
    let span = (max - min).abs().max(0.1);
    let lower = center - span * 0.55;
    let upper = center + span * 0.55;
    let to_screen = |index: usize, sample: f32| {
        let t = index as f32 / (samples.len() - 1) as f32;
        let x = egui::lerp(rect.left()..=rect.right(), t);
        let normalized = ((sample - lower) / (upper - lower)).clamp(0.0, 1.0);
        let y = egui::lerp(rect.bottom()..=rect.top(), normalized);
        egui::pos2(x, y)
    };
    let points = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| to_screen(index, *sample))
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.75, line)));
    painter.text(
        rect.left_top() + egui::vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!("{:.2} .. {:.2}", lower, upper),
        egui::TextStyle::Small.resolve(ui.style()),
        stroke.color,
    );
}

/// Renders an inline editor for a disconnected input port value.
///
/// Values are first coerced to the port's declared kind so stale or invalid persisted values do
/// not leak into the editor UI.
pub(super) fn edit_input_value(ui: &mut egui::Ui, input: &mut EditorInputPort) {
    input.value = coerce_input_value_kind(input.value.clone(), input.value_kind);
    match &mut input.value {
        InputValue::Float(value) => {
            ui.add(
                egui::DragValue::new(value)
                    .speed(0.01)
                    .range(-10_000.0..=10_000.0),
            );
        }
        InputValue::FloatTensor(tensor) => {
            ui.label(format!(
                "tensor {:?} ({})",
                tensor.shape,
                tensor.values.len()
            ));
        }
        InputValue::Color(color) => {
            let mut rgba = [color.r, color.g, color.b, color.a];
            ui.color_edit_button_rgba_unmultiplied(&mut rgba);
            color.r = rgba[0];
            color.g = rgba[1];
            color.b = rgba[2];
            color.a = rgba[3];
        }
        InputValue::LedLayout(layout) => {
            ui.label(format!("{} LEDs ({})", layout.pixel_count, layout.id));
        }
        InputValue::ColorFrame(frame) => {
            ui.label(format!("{} LEDs ({})", frame.pixels.len(), frame.layout.id));
        }
    }
}

/// Ensures that a node's parameter list contains any missing schema defaults.
pub(super) fn ensure_parameter_defaults(
    parameters: &mut Vec<NodeParameter>,
    node_type_id: &str,
    available_node_definitions: &[NodeDefinition],
) {
    let merged = parameters_with_defaults(parameters, node_type_id, available_node_definitions);
    *parameters = merged;
}

/// Renders the editor widget for a parameter according to its shared UI hint.
///
/// The parameter value is created on demand when missing and written back into the JSON-backed
/// parameter list after every edit.
pub(super) fn edit_parameter_value(
    ui: &mut egui::Ui,
    parameters: &mut Vec<NodeParameter>,
    name: &str,
    ui_hint: &ParameterUiHint,
    default_value: JsonValue,
    wled_instances: &[WledInstance],
    mqtt_broker_configs: &[MqttBrokerConfig],
) {
    let value = parameter_value_mut(parameters, name, default_value);
    match ui_hint {
        ParameterUiHint::DragFloat { speed, min, max } => {
            let mut float_value = value.as_f64().unwrap_or(0.0);
            if ui
                .add(
                    egui::DragValue::new(&mut float_value)
                        .speed(*speed)
                        .range(*min..=*max),
                )
                .changed()
            {
                *value = JsonValue::from(float_value);
            }
        }
        ParameterUiHint::ColorPicker => {
            let mut color =
                serde_json::from_value::<RgbaColor>(value.clone()).unwrap_or(RgbaColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                });
            let mut rgba = [color.r, color.g, color.b, color.a];
            if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                color.r = rgba[0];
                color.g = rgba[1];
                color.b = rgba[2];
                color.a = rgba[3];
                *value = serde_json::to_value(color).unwrap_or(JsonValue::Null);
            }
        }
        ParameterUiHint::ColorGradient => {
            let mut gradient = serde_json::from_value::<ColorGradient>(value.clone())
                .unwrap_or_else(|_| default_editor_gradient());
            edit_color_gradient_button(ui, name, &mut gradient);
            *value = serde_json::to_value(normalize_gradient(gradient)).unwrap_or(JsonValue::Null);
        }
        ParameterUiHint::Checkbox => {
            let mut bool_value = value.as_bool().unwrap_or(false);
            if ui.checkbox(&mut bool_value, "").changed() {
                *value = JsonValue::from(bool_value);
            }
        }
        ParameterUiHint::TextSingleLine => {
            let mut text = value.as_str().unwrap_or("").to_owned();
            if ui
                .add_sized(
                    [205.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut text),
                )
                .changed()
            {
                *value = JsonValue::from(text);
            }
        }
        ParameterUiHint::EnumSelect { options } => {
            let mut selected = value.as_str().unwrap_or("").to_owned();
            let selected_label = options
                .iter()
                .find(|option| option.value == selected)
                .map(|option| option.label.clone())
                .unwrap_or_else(|| {
                    options
                        .first()
                        .map(|option| option.label.clone())
                        .unwrap_or_default()
                });
            egui::ComboBox::from_id_salt(ui.id().with(("enum_select", name)))
                .selected_text(selected_label)
                .show_ui(ui, |ui| {
                    for option in options {
                        if ui
                            .selectable_label(selected == option.value, option.label.clone())
                            .clicked()
                        {
                            selected = option.value.to_owned();
                        }
                    }
                });
            if selected.is_empty() {
                if let Some(option) = options.first() {
                    selected = option.value.to_owned();
                }
            }
            *value = JsonValue::from(selected);
        }
        ParameterUiHint::IntegerDrag { speed, min, max } => {
            let mut int_value = value.as_i64().unwrap_or(0);
            if ui
                .add(
                    egui::DragValue::new(&mut int_value)
                        .speed(*speed)
                        .range(*min..=*max),
                )
                .changed()
            {
                *value = JsonValue::from(int_value);
            }
        }
        ParameterUiHint::WledInstanceOrHost => {
            let mut target = value.as_str().unwrap_or("").to_owned();
            let mut selected_led_count = None;

            ui.vertical(|ui| {
                egui::ComboBox::from_id_salt(ui.id().with(("wled_target", name)))
                    .selected_text(if target.is_empty() {
                        "Select discovered instance"
                    } else {
                        "Discovered instances"
                    })
                    .show_ui(ui, |ui| {
                        for instance in wled_instances {
                            let label = if instance.name.is_empty() {
                                instance.host.clone()
                            } else {
                                format!("{} ({})", instance.name, instance.host)
                            };
                            if ui
                                .selectable_label(target == instance.host, label)
                                .clicked()
                            {
                                target = instance.host.clone();
                                selected_led_count = instance.led_count;
                            }
                        }
                    });

                if ui.text_edit_singleline(&mut target).changed() {
                    *value = JsonValue::from(target.clone());
                }
            });
            if value.as_str().unwrap_or("") != target {
                *value = JsonValue::from(target.clone());
            }
            if let Some(led_count) = selected_led_count {
                *parameter_value_mut(parameters, "led_count", JsonValue::from(led_count as i64)) =
                    JsonValue::from(led_count as i64);
            }
        }
        ParameterUiHint::MqttBrokerSelect => {
            let mut selected = value.as_str().unwrap_or("").to_owned();
            let selected_broker = mqtt_broker_configs
                .iter()
                .find(|broker| broker.id == selected);

            egui::ComboBox::from_id_salt(ui.id().with(("mqtt_broker", name)))
                .selected_text(selected_broker.map_or_else(
                    || {
                        if selected.is_empty() {
                            "Select broker".to_owned()
                        } else {
                            selected.clone()
                        }
                    },
                    |broker| {
                        let label = if broker.display_name.trim().is_empty() {
                            broker.id.clone()
                        } else {
                            format!("{} ({})", broker.display_name, broker.id)
                        };
                        if broker.is_home_assistant {
                            label
                        } else {
                            format!("{label} (not Home Assistant)")
                        }
                    },
                ))
                .show_ui(ui, |ui| {
                    for broker in mqtt_broker_configs
                        .iter()
                        .filter(|broker| broker.is_home_assistant)
                    {
                        let label = if broker.display_name.trim().is_empty() {
                            broker.id.clone()
                        } else {
                            format!("{} ({})", broker.display_name, broker.id)
                        };
                        if ui.selectable_label(selected == broker.id, label).clicked() {
                            selected = broker.id.clone();
                        }
                    }
                });
            *value = JsonValue::from(selected);
        }
    }
}

/// Formats the visible dimensions of a frame layout for display.
fn frame_layout_dims_label(frame: &shared::ColorFrame) -> String {
    if let (Some(width), Some(height)) = (frame.layout.width, frame.layout.height) {
        return format!("{width}x{height}");
    }
    format!("1x{}", frame.pixels.len().max(1))
}

/// Returns the preview grid dimensions used to render a frame in the editor.
///
/// True 2D layouts keep their native dimensions. One-dimensional strips are flattened into a
/// horizontal ribbon so long LED chains stay readable in the preview.
fn frame_preview_dimensions(frame: &shared::ColorFrame) -> (usize, usize) {
    if let (Some(width), Some(height)) = (frame.layout.width, frame.layout.height)
        && width > 1
        && height > 1
    {
        return (width, height);
    }

    (frame.pixels.len().max(1), 1)
}

/// Returns a mutable reference to the named parameter value, inserting the default when missing.
fn parameter_value_mut<'a>(
    parameters: &'a mut Vec<NodeParameter>,
    name: &str,
    default_value: JsonValue,
) -> &'a mut JsonValue {
    if let Some(index) = parameters
        .iter()
        .position(|parameter| parameter.name == name)
    {
        return &mut parameters[index].value;
    }
    parameters.push(NodeParameter {
        name: name.to_owned(),
        value: default_value,
    });
    let index = parameters.len() - 1;
    &mut parameters[index].value
}

/// Renders the compact gradient preview button and popup editor for a gradient parameter.
fn edit_color_gradient_button(ui: &mut egui::Ui, name: &str, gradient: &mut ColorGradient) {
    *gradient = normalize_gradient(gradient.clone());

    let popup_id = ui.id().with(("gradient_dialog", name));
    let mut open = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(popup_id))
        .unwrap_or(false);

    ui.horizontal(|ui| {
        let desired_size = egui::vec2(96.0, ui.spacing().interact_size.y);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        paint_gradient_preview(ui, rect, gradient);

        let stroke = if response.hovered() {
            egui::Stroke::new(2.0, ui.visuals().widgets.hovered.bg_stroke.color)
        } else {
            egui::Stroke::new(0.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
        };
        ui.painter()
            .rect_stroke(rect, 0, stroke, egui::StrokeKind::Outside);

        if response.clicked() {
            open = true;
        }
    });

    if open {
        egui::Window::new(format!("{name} Gradient"))
            .id(popup_id)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                edit_color_gradient_contents(ui, gradient);
            });
    }

    ui.ctx().data_mut(|data| data.insert_temp(popup_id, open));
    *gradient = normalize_gradient(gradient.clone());
}

/// Renders the full gradient editor contents inside the popup window.
fn edit_color_gradient_contents(ui: &mut egui::Ui, gradient: &mut ColorGradient) {
    ui.vertical(|ui| {
        let desired_size = egui::vec2(ui.available_width().max(180.0), 22.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        paint_gradient_preview(ui, rect, gradient);

        let mut remove_index = None;
        let can_remove_stop = gradient.stops.len() > 2;
        for (index, stop) in gradient.stops.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", index + 1));
                ui.add(egui::Slider::new(&mut stop.position, 0.0..=1.0).show_value(true));

                let mut rgba = [stop.color.r, stop.color.g, stop.color.b, stop.color.a];
                if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                    stop.color.r = rgba[0];
                    stop.color.g = rgba[1];
                    stop.color.b = rgba[2];
                    stop.color.a = rgba[3];
                }

                if can_remove_stop && ui.small_button("Remove").clicked() {
                    remove_index = Some(index);
                }
            });
        }

        if let Some(index) = remove_index {
            gradient.stops.remove(index);
        }

        if ui.small_button("Add Stop").clicked() {
            let position = 0.5;
            let color = sample_gradient(gradient, position);
            gradient.stops.push(ColorGradientStop { position, color });
        }
    });
}

/// Paints a horizontal gradient preview into `rect`.
///
/// The preview is sampled at approximately one sample per screen pixel to avoid visible seams.
fn paint_gradient_preview(ui: &egui::Ui, rect: egui::Rect, gradient: &ColorGradient) {
    let pixels_per_point = ui.ctx().pixels_per_point();
    let samples = pixels_per_point * rect.width();
    for index in 0..samples as usize {
        let t0 = index as f32 / samples as f32;
        let pixels_per_point = ui.ctx().pixels_per_point();
        let snap = |x: f32| (x * pixels_per_point).round() / pixels_per_point;

        let x0 = snap(egui::lerp(
            rect.left()..=rect.right(),
            index as f32 / samples as f32,
        ));
        let color = sample_gradient(gradient, t0);
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x0, rect.top()),
                egui::pos2(rect.right(), rect.bottom()),
            ),
            0,
            egui::Color32::from_rgb(
                (color.r.clamp(0.0, 1.0) * color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
                (color.g.clamp(0.0, 1.0) * color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
                (color.b.clamp(0.0, 1.0) * color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ),
        );
    }
}

/// Samples a gradient at `position` using HSV interpolation between stops.
fn sample_gradient(gradient: &ColorGradient, position: f32) -> RgbaColor {
    if gradient.stops.is_empty() {
        return RgbaColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
    }

    let mut stops = gradient.stops.clone();
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));
    let position = position.clamp(0.0, 1.0);
    if position <= stops[0].position {
        return stops[0].color;
    }
    for window in stops.windows(2) {
        let left = window[0];
        let right = window[1];
        if position <= right.position {
            let span = (right.position - left.position).max(f32::EPSILON);
            let factor = (position - left.position) / span;
            return mix_rgba_hsv(left.color, right.color, factor);
        }
    }
    stops.last().map(|stop| stop.color).unwrap_or(RgbaColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    })
}

/// Normalizes a gradient into the editor's canonical form.
///
/// Stop positions and color channels are quantized into the unit range, stops are sorted, and an
/// empty or single-stop gradient is expanded into a valid two-stop gradient.
fn normalize_gradient(mut gradient: ColorGradient) -> ColorGradient {
    if gradient.stops.is_empty() {
        return default_editor_gradient();
    }

    for stop in &mut gradient.stops {
        stop.position = quantize_unit_float(stop.position);
        stop.color.r = quantize_unit_float(stop.color.r);
        stop.color.g = quantize_unit_float(stop.color.g);
        stop.color.b = quantize_unit_float(stop.color.b);
        stop.color.a = quantize_unit_float(stop.color.a);
    }
    gradient
        .stops
        .sort_by(|a, b| a.position.total_cmp(&b.position));
    gradient.stops.dedup_by(|a, b| a.position == b.position);

    if gradient.stops.len() == 1 {
        let only = gradient.stops[0];
        gradient.stops.push(ColorGradientStop {
            position: 1.0,
            color: only.color,
        });
    }

    gradient
}

/// Quantizes a unit float so repeated editor edits remain stable in JSON.
fn quantize_unit_float(value: f32) -> f32 {
    const STEP: f32 = 1_000_000.0;
    ((value.clamp(0.0, 1.0) * STEP).round() / STEP).clamp(0.0, 1.0)
}

/// Returns the fallback gradient used by the editor when a parameter has no valid gradient value.
fn default_editor_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            ColorGradientStop {
                position: 0.0,
                color: RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            },
            ColorGradientStop {
                position: 1.0,
                color: RgbaColor {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
            },
        ],
    }
}

/// Mixes two RGBA colors in HSV space using `factor`.
fn mix_rgba_hsv(background: RgbaColor, foreground: RgbaColor, factor: f32) -> RgbaColor {
    let factor = factor.clamp(0.0, 1.0);
    let (bh, bs, bv) = rgba_to_hsv(background);
    let (fh, fs, fv) = rgba_to_hsv(foreground);
    let hue = bh + shortest_hue_delta(bh, fh) * factor;
    let saturation = bs + (fs - bs) * factor;
    let value = bv + (fv - bv) * factor;
    let mut color = hsv_to_rgba(hue, saturation, value);
    color.a = background.a + (foreground.a - background.a) * factor;
    color
}

/// Converts an RGBA color to HSV.
fn rgba_to_hsv(color: RgbaColor) -> (f32, f32, f32) {
    let max = color.r.max(color.g).max(color.b);
    let min = color.r.min(color.g).min(color.b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == color.r {
        60.0 * ((color.g - color.b) / delta).rem_euclid(6.0)
    } else if max == color.g {
        60.0 * (((color.b - color.r) / delta) + 2.0)
    } else {
        60.0 * (((color.r - color.g) / delta) + 4.0)
    };
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue, saturation, max)
}

/// Converts HSV components to an opaque RGBA color.
fn hsv_to_rgba(hue_degrees: f32, saturation: f32, value: f32) -> RgbaColor {
    let h = hue_degrees.rem_euclid(360.0) / 60.0;
    let c = value * saturation;
    let x = c * (1.0 - ((h.rem_euclid(2.0)) - 1.0).abs());
    let (r, g, b) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = value - c;
    RgbaColor {
        r: (r + m).clamp(0.0, 1.0),
        g: (g + m).clamp(0.0, 1.0),
        b: (b + m).clamp(0.0, 1.0),
        a: 1.0,
    }
}

/// Returns the shortest signed hue delta from `from_degrees` to `to_degrees`.
fn shortest_hue_delta(from_degrees: f32, to_degrees: f32) -> f32 {
    let delta = (to_degrees - from_degrees).rem_euclid(360.0);
    if delta > 180.0 { delta - 360.0 } else { delta }
}

/// Converts an RGBA color into an `egui::Color32`.
fn color32_from_rgba(color: RgbaColor) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// Returns the maximum width needed to align the input labels of a node.
pub(super) fn max_input_label_width(ui: &egui::Ui, node: &EditorSnarlNode) -> f32 {
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().text_color();
    let widest = node
        .inputs
        .iter()
        .map(|port| {
            ui.painter()
                .layout_no_wrap(port.display_name.clone(), font_id.clone(), color)
                .rect
                .width()
        })
        .fold(0.0f32, f32::max);
    widest + 8.0
}

#[cfg(test)]
mod tests {
    use super::frame_preview_dimensions;
    use shared::{ColorFrame, LedLayout, RgbaColor};

    #[test]
    fn one_dimensional_layout_is_rendered_as_horizontal_strip() {
        let frame = ColorFrame {
            layout: LedLayout {
                id: "strip".to_owned(),
                pixel_count: 153,
                width: Some(1),
                height: Some(153),
            },
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };
                153
            ],
        };

        assert_eq!(frame_preview_dimensions(&frame), (153, 1));
    }

    #[test]
    fn true_two_dimensional_layout_keeps_native_dimensions() {
        let frame = ColorFrame {
            layout: LedLayout {
                id: "matrix".to_owned(),
                pixel_count: 12,
                width: Some(4),
                height: Some(3),
            },
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };
                12
            ],
        };

        assert_eq!(frame_preview_dimensions(&frame), (4, 3));
    }
}
