use std::sync::Arc;

use anyhow::Result;
use shared::{
    ColorFrame, ColorGradient, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor,
};

use crate::color_math::sample_gradient_hsv;
use crate::node_runtime::nodes::color::filter_utils::layout_dimensions;
use crate::node_runtime::{NodeEvaluationContext, RuntimeNode, TypedNodeEvaluation};

const EPSILON: f32 = 1.0e-5;
const POSITION_EPSILON: f32 = 1.0e-4;
const MAX_DT_SECONDS: f32 = 0.25;
const MAX_CHUNK_SECONDS: f32 = 0.05;

pub(crate) struct BouncingBallsNode {
    circle_count: usize,
    radius_variance: f32,
    gradient: ColorGradient,
    state: Option<SimulationState>,
    render_cache: Option<RenderCache>,
    last_elapsed_seconds: Option<f64>,
}

impl Default for BouncingBallsNode {
    /// Builds the default simulation parameters for the bouncing-balls effect.
    fn default() -> Self {
        Self {
            circle_count: 6,
            radius_variance: 0.35,
            gradient: default_gradient(),
            state: None,
            render_cache: None,
            last_elapsed_seconds: None,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(BouncingBallsNode {
    circle_count: u64 => |value| crate::node_runtime::clamp_u64_to_usize(value, 1, 64), default 6usize,
    radius_variance: f64 => |value| crate::node_runtime::clamp_f64_to_f32(value, 0.0, 1.0), default 0.35f32,
    gradient: ColorGradient => |value| crate::node_runtime::non_empty_gradient(value, default_gradient()), default default_gradient(),
    ..Self::default()
});

pub(crate) struct BouncingBallsInputs {
    speed: f32,
    radius: f32,
}

crate::node_runtime::impl_runtime_inputs!(BouncingBallsInputs {
    speed = 0.3,
    radius = 0.12,
});

pub(crate) struct BouncingBallsOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(BouncingBallsOutputs { frame });

impl RuntimeNode for BouncingBallsNode {
    type Inputs = BouncingBallsInputs;
    type Outputs = BouncingBallsOutputs;

    /// Advances the bouncing-balls simulation and renders the current state into the active layout.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let Some(layout) = context.render_layout.clone() else {
            return Ok(TypedNodeEvaluation {
                outputs: BouncingBallsOutputs {
                    frame: ColorFrame {
                        layout: LedLayout {
                            id: "bouncing_balls:unbound".to_owned(),
                            pixel_count: 0,
                            width: None,
                            height: None,
                        },
                        pixels: Vec::new(),
                    },
                },
                frontend_updates: Vec::new(),
                diagnostics: vec![NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Warning,
                    code: Some("bouncing_balls_missing_render_layout".to_owned()),
                    message:
                        "Bouncing Balls has no render layout, so it cannot render a frame yet."
                            .to_owned(),
                }],
            });
        };
        let mut diagnostics = Vec::new();

        let pixel_radius = inputs.radius.max(0.5);
        let pixel_speed = inputs.speed.max(0.0);
        if (pixel_radius - inputs.radius).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("bouncing_balls_radius_clamped".to_owned()),
                message: format!(
                    "Radius {} is too small; using {} instead.",
                    inputs.radius, pixel_radius
                ),
            });
        }
        if (pixel_speed - inputs.speed).abs() > f32::EPSILON {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("bouncing_balls_speed_clamped".to_owned()),
                message: format!(
                    "Speed {} is too small; using {} instead.",
                    inputs.speed, pixel_speed
                ),
            });
        }
        let is_1d = is_1d_layout(&layout);
        let world_size = world_size_for_layout(&layout);
        let pixel_size = pixel_size_for_layout(&layout);
        let base_radius = (pixel_radius * pixel_size).max(pixel_size * 0.5);
        let speed = pixel_speed * pixel_size;

        self.ensure_state(self.circle_count, world_size, is_1d);
        let dt = self.delta_seconds(context.elapsed_seconds);
        let radii = self.current_radii(base_radius, self.radius_variance, world_size, is_1d);

        if let Some(state) = &mut self.state {
            clamp_to_bounds(&mut state.balls, &radii, state.world_size, state.is_1d);
            separate_overlaps(&mut state.balls, &radii, state.world_size, state.is_1d);
            renormalize_speeds(&mut state.balls, speed, state.is_1d);
            simulate(state, &radii, speed, dt);
        }

        let render_cache = self.render_cache_for_layout(&layout);
        let pixels = if let Some(state) = &self.state {
            render_balls(&render_cache, &state.balls, &radii)
        } else {
            vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                layout.pixel_count
            ]
        };

        Ok(TypedNodeEvaluation {
            outputs: BouncingBallsOutputs {
                frame: ColorFrame { layout, pixels },
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl BouncingBallsNode {
    /// Rebuilds the simulation state when the ball count or layout shape changes.
    fn ensure_state(&mut self, circle_count: usize, world_size: Vec2, is_1d: bool) {
        let needs_reset = self.state.as_ref().is_none_or(|state| {
            state.balls.len() != circle_count
                || state.world_size != world_size
                || state.is_1d != is_1d
        });
        if !needs_reset {
            return;
        }

        let mut balls = Vec::with_capacity(circle_count);
        for index in 0..circle_count {
            let seed = hash_u32(index as u32 ^ 0xA0F1_3D5B);
            let direction = if is_1d {
                let x = if hash_to_unit(seed) < 0.5 { -1.0 } else { 1.0 };
                Vec2::new(x, 0.0)
            } else {
                let angle = hash_to_unit(seed) * std::f32::consts::TAU;
                Vec2::new(angle.cos(), angle.sin()).normalized()
            };
            let size_factor = hash_to_signed_unit(hash_u32(seed ^ 0x9E37_79B9));
            let color_position = if circle_count == 1 {
                0.0
            } else {
                index as f32 / (circle_count - 1) as f32
            };
            balls.push(BallState {
                position: Vec2::ZERO,
                velocity: direction,
                size_factor,
                color: sample_gradient_hsv(&self.gradient, color_position),
            });
        }

        let max_radius = max_radius_for_layout(world_size, is_1d);
        let radii = balls
            .iter()
            .map(|ball| {
                let scaled = 0.12 * (1.0 + self.radius_variance * ball.size_factor);
                scaled.clamp(0.004, max_radius)
            })
            .collect::<Vec<_>>();
        place_balls_without_overlap(&mut balls, &radii, world_size, is_1d);
        self.state = Some(SimulationState {
            world_size,
            balls,
            is_1d,
        });
    }

    /// Returns the elapsed simulation time since the previous evaluation, clamped to a safe step.
    fn delta_seconds(&mut self, elapsed_seconds: f64) -> f32 {
        let dt = match self.last_elapsed_seconds {
            Some(last_elapsed_seconds) if elapsed_seconds >= last_elapsed_seconds => {
                (elapsed_seconds - last_elapsed_seconds) as f32
            }
            _ => 0.0,
        };
        self.last_elapsed_seconds = Some(elapsed_seconds);
        dt.clamp(0.0, MAX_DT_SECONDS)
    }

    /// Computes the current radius of each ball from the configured base radius and variance.
    fn current_radii(
        &self,
        base_radius: f32,
        radius_variance: f32,
        world_size: Vec2,
        is_1d: bool,
    ) -> Vec<f32> {
        let Some(state) = &self.state else {
            return Vec::new();
        };

        let max_radius = max_radius_for_layout(world_size, is_1d);
        state
            .balls
            .iter()
            .map(|ball| {
                let scaled = base_radius * (1.0 + radius_variance * ball.size_factor);
                scaled.clamp(0.004, max_radius)
            })
            .collect()
    }

    /// Returns cached world-space pixel positions for the current render layout.
    fn render_cache_for_layout(&mut self, layout: &LedLayout) -> RenderCacheRef {
        let needs_refresh = self
            .render_cache
            .as_ref()
            .is_none_or(|cache| cache.layout != *layout);
        if needs_refresh {
            self.render_cache = Some(RenderCache::new(layout));
        }
        let cache = self
            .render_cache
            .as_ref()
            .expect("render cache must exist after refresh");
        RenderCacheRef {
            pixel_size: cache.pixel_size,
            width: cache.width,
            height: cache.height,
            pixel_positions: Arc::clone(&cache.pixel_positions),
        }
    }
}

#[derive(Clone)]
struct SimulationState {
    world_size: Vec2,
    balls: Vec<BallState>,
    is_1d: bool,
}

struct RenderCache {
    layout: LedLayout,
    pixel_size: f32,
    width: usize,
    height: usize,
    pixel_positions: Arc<[Vec2]>,
}

struct RenderCacheRef {
    pixel_size: f32,
    width: usize,
    height: usize,
    pixel_positions: Arc<[Vec2]>,
}

impl RenderCache {
    /// Precomputes world-space positions and pixel sizing for one render layout.
    fn new(layout: &LedLayout) -> Self {
        let (width, height) = layout_dimensions(layout);
        let pixel_positions = (0..layout.pixel_count)
            .map(|index| pixel_world_position(index, layout))
            .collect::<Vec<_>>()
            .into();
        Self {
            layout: layout.clone(),
            pixel_size: pixel_size_for_layout(layout),
            width,
            height,
            pixel_positions,
        }
    }
}

#[derive(Clone)]
struct BallState {
    position: Vec2,
    velocity: Vec2,
    size_factor: f32,
    color: RgbaColor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    const ZERO: Self = Self { x: 0.0, y: 0.0 };

    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    fn length_squared(self) -> f32 {
        self.dot(self)
    }

    fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    fn normalized(self) -> Self {
        let length = self.length();
        if length <= EPSILON {
            Self::new(1.0, 0.0)
        } else {
            self / length
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl std::ops::Div<f32> for Vec2 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

/// Builds the default palette used to color the simulated balls.
fn default_gradient() -> ColorGradient {
    ColorGradient {
        stops: vec![
            stop(0.0, 0.09, 0.64, 0.98),
            stop(0.5, 0.98, 0.73, 0.16),
            stop(1.0, 0.96, 0.24, 0.43),
        ],
    }
}

/// Creates one opaque gradient stop for the default bouncing-balls palette.
fn stop(position: f32, r: f32, g: f32, b: f32) -> shared::ColorGradientStop {
    shared::ColorGradientStop {
        position,
        color: RgbaColor { r, g, b, a: 1.0 },
    }
}

/// Converts a render layout into the normalized simulation-space dimensions.
fn world_size_for_layout(layout: &LedLayout) -> Vec2 {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => {
            Vec2::new(width as f32 / height as f32, 1.0)
        }
        _ if layout.pixel_count > 1 => {
            let pixel_height = 1.0 / (layout.pixel_count - 1) as f32;
            Vec2::new(1.0, pixel_height)
        }
        _ => Vec2::new(1.0, 1.0),
    }
}

/// Returns whether the layout should be treated as a one-dimensional strip.
fn is_1d_layout(layout: &LedLayout) -> bool {
    !matches!(
        (layout.width, layout.height),
        (Some(width), Some(height)) if width > 1 && height > 1
    )
}

/// Returns the vertical centerline used for one-dimensional strip rendering.
fn centerline_y(world_size: Vec2) -> f32 {
    world_size.y * 0.5
}

/// Returns the maximum legal ball radius for the current simulated world.
fn max_radius_for_layout(world_size: Vec2, is_1d: bool) -> f32 {
    let limiting_dimension = if is_1d {
        world_size.x
    } else {
        world_size.x.min(world_size.y)
    };
    (0.5 * limiting_dimension - POSITION_EPSILON).max(0.004)
}

/// Renormalizes every ball velocity so the simulation keeps a consistent configured speed.
fn renormalize_speeds(balls: &mut [BallState], speed: f32, is_1d: bool) {
    for ball in balls {
        ball.velocity = set_speed_preserving_direction(ball.velocity, speed, is_1d);
    }
}

/// Rebuilds a velocity vector with the requested speed while preserving its direction.
fn set_speed_preserving_direction(velocity: Vec2, speed: f32, is_1d: bool) -> Vec2 {
    if speed <= EPSILON {
        Vec2::ZERO
    } else if is_1d {
        let direction = if velocity.x < 0.0 { -1.0 } else { 1.0 };
        Vec2::new(direction * speed, 0.0)
    } else {
        velocity.normalized() * speed
    }
}

/// Advances the simulation, splitting long frame deltas into smaller stable chunks.
fn simulate(state: &mut SimulationState, radii: &[f32], speed: f32, dt: f32) {
    if dt <= EPSILON {
        return;
    }

    let mut remaining = dt;
    while remaining > EPSILON {
        let chunk = remaining.min(MAX_CHUNK_SECONDS);
        simulate_chunk(state, radii, speed, chunk);
        remaining -= chunk;
    }
}

/// Simulates a short time slice, processing wall and circle collisions in chronological order.
fn simulate_chunk(state: &mut SimulationState, radii: &[f32], speed: f32, dt: f32) {
    if dt <= EPSILON {
        return;
    }

    let max_events = (state.balls.len() * state.balls.len()).max(16);
    let mut remaining = dt;
    let mut events = 0usize;

    while remaining > EPSILON && events < max_events {
        let Some(event_time) = find_next_collision(state, radii, remaining) else {
            advance_positions(&mut state.balls, remaining);
            break;
        };

        if event_time > EPSILON {
            advance_positions(&mut state.balls, event_time);
            remaining -= event_time;
        }

        let processed = process_collisions(state, radii, speed);
        if !processed {
            advance_positions(&mut state.balls, remaining.min(EPSILON));
            remaining = (remaining - remaining.min(EPSILON)).max(0.0);
        }
        events += 1;
    }

    if remaining > EPSILON {
        advance_positions(&mut state.balls, remaining);
    }

    clamp_to_bounds(&mut state.balls, radii, state.world_size, state.is_1d);
    separate_overlaps(&mut state.balls, radii, state.world_size, state.is_1d);
    renormalize_speeds(&mut state.balls, speed, state.is_1d);
}

/// Returns the earliest upcoming collision time within the given time slice, if any.
fn find_next_collision(state: &SimulationState, radii: &[f32], max_time: f32) -> Option<f32> {
    let mut earliest = max_time + EPSILON;

    for (index, ball) in state.balls.iter().enumerate() {
        if let Some(time) =
            boundary_collision_time(ball, radii[index], state.world_size, max_time, state.is_1d)
        {
            earliest = earliest.min(time);
        }
    }

    for first in 0..state.balls.len() {
        for second in (first + 1)..state.balls.len() {
            if let Some(time) = circle_collision_time(
                &state.balls[first],
                radii[first],
                &state.balls[second],
                radii[second],
                max_time,
            ) {
                earliest = earliest.min(time);
            }
        }
    }

    if earliest <= max_time {
        Some(earliest.max(0.0))
    } else {
        None
    }
}

/// Returns the next time at which a ball will touch a simulation boundary.
fn boundary_collision_time(
    ball: &BallState,
    radius: f32,
    world_size: Vec2,
    max_time: f32,
    is_1d: bool,
) -> Option<f32> {
    let mut earliest = max_time + EPSILON;

    if ball.velocity.x > EPSILON {
        let t = (world_size.x - radius - ball.position.x) / ball.velocity.x;
        if t >= -EPSILON && t <= max_time + EPSILON {
            earliest = earliest.min(t.max(0.0));
        }
    } else if ball.velocity.x < -EPSILON {
        let t = (radius - ball.position.x) / ball.velocity.x;
        if t >= -EPSILON && t <= max_time + EPSILON {
            earliest = earliest.min(t.max(0.0));
        }
    }

    if !is_1d && ball.velocity.y > EPSILON {
        let t = (world_size.y - radius - ball.position.y) / ball.velocity.y;
        if t >= -EPSILON && t <= max_time + EPSILON {
            earliest = earliest.min(t.max(0.0));
        }
    } else if !is_1d && ball.velocity.y < -EPSILON {
        let t = (radius - ball.position.y) / ball.velocity.y;
        if t >= -EPSILON && t <= max_time + EPSILON {
            earliest = earliest.min(t.max(0.0));
        }
    }

    if earliest <= max_time {
        Some(earliest)
    } else {
        None
    }
}

/// Returns the next time at which two moving circles will first make contact.
fn circle_collision_time(
    first: &BallState,
    first_radius: f32,
    second: &BallState,
    second_radius: f32,
    max_time: f32,
) -> Option<f32> {
    let delta = second.position - first.position;
    let relative_velocity = second.velocity - first.velocity;
    let combined_radius = first_radius + second_radius;
    let c = delta.length_squared() - combined_radius * combined_radius;
    if c <= POSITION_EPSILON {
        return Some(0.0);
    }

    let a = relative_velocity.length_squared();
    if a <= EPSILON {
        return None;
    }

    let b = 2.0 * delta.dot(relative_velocity);
    if b >= 0.0 {
        return None;
    }

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }

    let t = (-b - discriminant.sqrt()) / (2.0 * a);
    if t >= -EPSILON && t <= max_time + EPSILON {
        Some(t.max(0.0))
    } else {
        None
    }
}

/// Advances all ball positions by `dt` without resolving collisions.
fn advance_positions(balls: &mut [BallState], dt: f32) {
    for ball in balls {
        ball.position += ball.velocity * dt;
    }
}

/// Resolves wall and ball collisions at the current positions and reapplies speed normalization.
fn process_collisions(state: &mut SimulationState, radii: &[f32], speed: f32) -> bool {
    let mut collided = false;

    for (index, ball) in state.balls.iter_mut().enumerate() {
        let radius = radii[index];
        let min_x = radius;
        let max_x = state.world_size.x - radius;

        if ball.position.x <= min_x + POSITION_EPSILON && ball.velocity.x < 0.0 {
            ball.position.x = min_x;
            ball.velocity.x = -ball.velocity.x;
            collided = true;
        } else if ball.position.x >= max_x - POSITION_EPSILON && ball.velocity.x > 0.0 {
            ball.position.x = max_x;
            ball.velocity.x = -ball.velocity.x;
            collided = true;
        }

        if state.is_1d {
            ball.position.y = centerline_y(state.world_size);
            ball.velocity.y = 0.0;
        } else {
            let min_y = radius;
            let max_y = state.world_size.y - radius;
            if ball.position.y <= min_y + POSITION_EPSILON && ball.velocity.y < 0.0 {
                ball.position.y = min_y;
                ball.velocity.y = -ball.velocity.y;
                collided = true;
            } else if ball.position.y >= max_y - POSITION_EPSILON && ball.velocity.y > 0.0 {
                ball.position.y = max_y;
                ball.velocity.y = -ball.velocity.y;
                collided = true;
            }
        }
    }

    for first in 0..state.balls.len() {
        for second in (first + 1)..state.balls.len() {
            let delta = state.balls[second].position - state.balls[first].position;
            let distance = delta.length();
            let combined_radius = radii[first] + radii[second];
            if distance > combined_radius + POSITION_EPSILON {
                continue;
            }

            let normal = if distance <= EPSILON {
                Vec2::new(1.0, 0.0)
            } else {
                delta / distance
            };
            let relative_velocity = state.balls[first].velocity - state.balls[second].velocity;
            let approach_speed = relative_velocity.dot(normal);
            if approach_speed <= 0.0 {
                continue;
            }

            state.balls[first].velocity -= normal * approach_speed;
            state.balls[second].velocity += normal * approach_speed;
            collided = true;
        }
    }

    if collided {
        separate_overlaps(&mut state.balls, radii, state.world_size, state.is_1d);
        renormalize_speeds(&mut state.balls, speed, state.is_1d);
    }

    collided
}

/// Clamps all balls back inside the simulated world after numerical drift or collision response.
fn clamp_to_bounds(balls: &mut [BallState], radii: &[f32], world_size: Vec2, is_1d: bool) {
    for (index, ball) in balls.iter_mut().enumerate() {
        let radius = radii[index];
        ball.position.x = ball
            .position
            .x
            .clamp(radius, (world_size.x - radius).max(radius));
        if is_1d {
            ball.position.y = centerline_y(world_size);
            ball.velocity.y = 0.0;
        } else {
            ball.position.y = ball
                .position
                .y
                .clamp(radius, (world_size.y - radius).max(radius));
        }
    }
}

/// Separates overlapping balls after collision handling to keep the simulation stable.
fn separate_overlaps(balls: &mut [BallState], radii: &[f32], world_size: Vec2, is_1d: bool) {
    if balls.is_empty() {
        return;
    }

    for _ in 0..16 {
        let mut moved = false;
        for first in 0..balls.len() {
            for second in (first + 1)..balls.len() {
                let delta = balls[second].position - balls[first].position;
                let distance = delta.length();
                let min_distance = radii[first] + radii[second];
                if distance + POSITION_EPSILON >= min_distance {
                    continue;
                }

                let normal = if distance <= EPSILON {
                    if is_1d {
                        Vec2::new(1.0, 0.0)
                    } else {
                        let angle = hash_to_unit(hash_u32((first as u32) << 16 | second as u32))
                            * std::f32::consts::TAU;
                        Vec2::new(angle.cos(), angle.sin())
                    }
                } else {
                    delta / distance
                };
                let correction = normal * ((min_distance - distance + POSITION_EPSILON) * 0.5);
                balls[first].position -= correction;
                balls[second].position += correction;
                moved = true;
            }
        }

        clamp_to_bounds(balls, radii, world_size, is_1d);
        if !moved {
            break;
        }
    }

    if balls.iter().all(|ball| ball.position == Vec2::ZERO) {
        place_balls_without_overlap(balls, radii, world_size, is_1d);
    }
}

/// Places the initial balls into non-overlapping positions within the simulation bounds.
fn place_balls_without_overlap(
    balls: &mut [BallState],
    radii: &[f32],
    world_size: Vec2,
    is_1d: bool,
) {
    let ball_count = balls.len().max(1);
    for index in 0..balls.len() {
        let radius = radii[index];
        let mut placed = false;
        for attempt in 0..128u32 {
            let seed = hash_u32(index as u32 ^ attempt.wrapping_mul(0x45D9_F3B));
            let x = lerp(radius, world_size.x - radius, hash_to_unit(seed));
            let y = if is_1d {
                centerline_y(world_size)
            } else {
                lerp(
                    radius,
                    world_size.y - radius,
                    hash_to_unit(hash_u32(seed ^ 0x27D4_EB2D)),
                )
            };
            let candidate = Vec2::new(x, y);
            if balls[..index]
                .iter()
                .enumerate()
                .all(|(other_index, other)| {
                    (other.position - candidate).length() + POSITION_EPSILON
                        >= radii[other_index] + radius
                })
            {
                balls[index].position = candidate;
                placed = true;
                break;
            }
        }

        if !placed {
            if is_1d {
                let x = if balls.len() == 1 {
                    world_size.x * 0.5
                } else {
                    lerp(
                        radius,
                        world_size.x - radius,
                        index as f32 / (balls.len() - 1) as f32,
                    )
                };
                balls[index].position = Vec2::new(x, centerline_y(world_size));
            } else {
                let angle = (index as f32 / ball_count as f32) * std::f32::consts::TAU;
                let center = Vec2::new(world_size.x * 0.5, world_size.y * 0.5);
                let orbit = 0.15 * world_size.x.min(world_size.y);
                balls[index].position = Vec2::new(
                    center.x + orbit * angle.cos(),
                    center.y + orbit * angle.sin(),
                );
            }
        }
    }

    clamp_to_bounds(balls, radii, world_size, is_1d);
}

/// Renders the simulated balls into the current layout by splatting each ball into nearby pixels.
fn render_balls(
    render_cache: &RenderCacheRef,
    balls: &[BallState],
    radii: &[f32],
) -> Vec<RgbaColor> {
    let mut pixels = vec![
        RgbaColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        render_cache.pixel_positions.len()
    ];

    match (render_cache.width, render_cache.height) {
        (width, height) if width > 1 && height > 1 => {
            let world_width = render_cache.pixel_positions[width - 1].x.max(EPSILON);
            let world_height = render_cache.pixel_positions[(height - 1) * width]
                .y
                .max(EPSILON);
            let x_scale = (width - 1) as f32 / world_width;
            let y_scale = (height - 1) as f32 / world_height;

            for (ball, radius) in balls.iter().zip(radii) {
                let max_distance = radius + render_cache.pixel_size;
                let max_distance_squared = max_distance * max_distance;
                let min_x =
                    world_to_pixel_lower_bound(ball.position.x - max_distance, x_scale, width);
                let max_x =
                    world_to_pixel_upper_bound(ball.position.x + max_distance, x_scale, width);
                let min_y =
                    world_to_pixel_lower_bound(ball.position.y - max_distance, y_scale, height);
                let max_y =
                    world_to_pixel_upper_bound(ball.position.y + max_distance, y_scale, height);

                for y in min_y..=max_y {
                    let row_offset = y * width;
                    for x in min_x..=max_x {
                        let index = row_offset + x;
                        let position = render_cache.pixel_positions[index];
                        let delta = position - ball.position;
                        let distance_squared = delta.length_squared();
                        if distance_squared >= max_distance_squared {
                            continue;
                        }
                        let distance = distance_squared.sqrt();
                        let edge = radius - distance;
                        let coverage =
                            ((edge / render_cache.pixel_size) * 0.5 + 0.5).clamp(0.0, 1.0);
                        if coverage > pixels[index].a {
                            pixels[index] = RgbaColor {
                                a: coverage,
                                ..ball.color
                            };
                        }
                    }
                }
            }
        }
        _ => {
            let pixel_count = render_cache.pixel_positions.len();
            let scale = (pixel_count.saturating_sub(1) as f32).max(1.0);

            for (ball, radius) in balls.iter().zip(radii) {
                let max_distance = radius + render_cache.pixel_size;
                let max_distance_squared = max_distance * max_distance;
                let min_x =
                    world_to_pixel_lower_bound(ball.position.x - max_distance, scale, pixel_count);
                let max_x =
                    world_to_pixel_upper_bound(ball.position.x + max_distance, scale, pixel_count);

                for index in min_x..=max_x {
                    let position = render_cache.pixel_positions[index];
                    let delta = position - ball.position;
                    let distance_squared = delta.length_squared();
                    if distance_squared >= max_distance_squared {
                        continue;
                    }
                    let distance = distance_squared.sqrt();
                    let edge = radius - distance;
                    let coverage = ((edge / render_cache.pixel_size) * 0.5 + 0.5).clamp(0.0, 1.0);
                    if coverage > pixels[index].a {
                        pixels[index] = RgbaColor {
                            a: coverage,
                            ..ball.color
                        };
                    }
                }
            }
        }
    }

    pixels
}

fn world_to_pixel_lower_bound(value: f32, scale: f32, upper_len: usize) -> usize {
    (value * scale)
        .floor()
        .clamp(0.0, upper_len.saturating_sub(1) as f32) as usize
}

fn world_to_pixel_upper_bound(value: f32, scale: f32, upper_len: usize) -> usize {
    (value * scale)
        .ceil()
        .clamp(0.0, upper_len.saturating_sub(1) as f32) as usize
}

fn pixel_world_position(index: usize, layout: &LedLayout) -> Vec2 {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 1 && height > 1 => {
            let world = world_size_for_layout(layout);
            let x = (index % width) as f32 / (width - 1) as f32 * world.x;
            let y = (index / width).min(height - 1) as f32 / (height - 1) as f32 * world.y;
            Vec2::new(x, y)
        }
        _ if layout.pixel_count > 1 => {
            let x = index as f32 / (layout.pixel_count - 1) as f32;
            Vec2::new(x, centerline_y(world_size_for_layout(layout)))
        }
        _ => Vec2::new(0.5, 0.5),
    }
}

fn pixel_size_for_layout(layout: &LedLayout) -> f32 {
    match (layout.width, layout.height) {
        (Some(width), Some(height)) if width > 1 && height > 1 => {
            let world = world_size_for_layout(layout);
            (world.x / (width - 1) as f32).min(world.y / (height - 1) as f32)
        }
        _ if layout.pixel_count > 1 => 1.0 / (layout.pixel_count - 1) as f32,
        _ => 1.0,
    }
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

fn hash_to_unit(x: u32) -> f32 {
    x as f32 / u32::MAX as f32
}

fn hash_to_signed_unit(x: u32) -> f32 {
    hash_to_unit(x) * 2.0 - 1.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::{
        BallState, SimulationState, Vec2, boundary_collision_time, circle_collision_time,
        process_collisions, set_speed_preserving_direction,
    };
    use shared::RgbaColor;

    #[test]
    fn boundary_collision_time_hits_exact_contact() {
        let ball = BallState {
            position: Vec2::new(0.25, 0.5),
            velocity: Vec2::new(-0.5, 0.0),
            size_factor: 0.0,
            color: RgbaColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        };

        let time = boundary_collision_time(&ball, 0.1, Vec2::new(1.0, 1.0), 1.0, false).unwrap();
        assert!((time - 0.3).abs() < 1.0e-5);
    }

    #[test]
    fn circle_collision_time_detects_head_on_contact() {
        let first = BallState {
            position: Vec2::new(0.3, 0.5),
            velocity: Vec2::new(0.4, 0.0),
            size_factor: 0.0,
            color: RgbaColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        };
        let second = BallState {
            position: Vec2::new(0.7, 0.5),
            velocity: Vec2::new(-0.4, 0.0),
            size_factor: 0.0,
            color: RgbaColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        };

        let time = circle_collision_time(&first, 0.1, &second, 0.1, 1.0).unwrap();
        assert!((time - 0.25).abs() < 1.0e-5);
    }

    #[test]
    fn process_collisions_reflects_wall_and_preserves_speed() {
        let mut state = SimulationState {
            world_size: Vec2::new(1.0, 1.0),
            balls: vec![BallState {
                position: Vec2::new(0.1, 0.4),
                velocity: Vec2::new(-0.6, 0.2),
                size_factor: 0.0,
                color: RgbaColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            }],
            is_1d: false,
        };

        let collided = process_collisions(&mut state, &[0.1], 0.5);
        assert!(collided);
        assert!(state.balls[0].velocity.x > 0.0);
        assert!((state.balls[0].velocity.length() - 0.5).abs() < 1.0e-5);
    }

    #[test]
    fn speed_normalization_keeps_requested_magnitude() {
        let velocity = set_speed_preserving_direction(Vec2::new(3.0, 4.0), 0.75, false);
        assert!((velocity.length() - 0.75).abs() < 1.0e-5);
    }

    #[test]
    fn speed_normalization_locks_1d_motion_to_x_axis() {
        let velocity = set_speed_preserving_direction(Vec2::new(-0.2, 0.9), 0.75, true);
        assert!((velocity.x + 0.75).abs() < 1.0e-5);
        assert!(velocity.y.abs() < 1.0e-5);
    }
}
