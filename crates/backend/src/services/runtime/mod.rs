/// Graph compilation, planning, execution, and task management for runtime playback.
pub(crate) mod compiler;
/// Per-tick node execution and runtime-update emission.
pub(crate) mod executor;
/// Long-lived runtime task management and persisted running-state handling.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod manager;
/// Render-context planning for compiled graphs.
pub(crate) mod planner;
/// Shared runtime data structures used across compilation and execution.
pub(crate) mod types;
