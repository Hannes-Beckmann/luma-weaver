pub(crate) mod display;
pub(crate) mod plot;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod wled_target;
