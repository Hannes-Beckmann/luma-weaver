use std::time::Duration;

use eframe::egui;
use shared::{ClientMessage, ColorGradient, GraphDocument, GraphViewport};
use tracing::debug;

use crate::app::FrontendApp;
use crate::state::AppView;

impl FrontendApp {
    /// Tracks editor changes, records undo history at debounce boundaries, and sends autosave requests.
    ///
    /// Graph documents are canonicalized before comparison so harmless formatting differences do not
    /// keep the document dirty forever. Viewport-only changes are excluded from history snapshots.
    pub(crate) fn schedule_graph_document_update(&mut self, ctx: &egui::Context) {
        if self.ui.active_view != AppView::Editor {
            return;
        }

        let now_secs = Self::now_secs(ctx);
        let Some(document) = self.active_graph_document_mut().cloned() else {
            return;
        };
        let canonical_document = canonicalize_graph_document(&document);
        let persisted_canonical = self
            .graphs
            .persisted_graph_document
            .as_ref()
            .map(canonicalize_graph_document);

        if persisted_canonical.as_ref() != Some(&canonical_document) {
            let changed_since_last_observation =
                self.graphs.graph_update_last_observed_document.as_ref() != Some(&document);
            if !self.graphs.pending_graph_update {
                self.graphs.graph_update_dirty_since_secs = Some(now_secs);
            }
            self.graphs.pending_graph_update = true;
            if changed_since_last_observation {
                self.graphs.graph_update_last_change_secs = Some(now_secs);
                self.graphs.graph_update_last_observed_document = Some(document.clone());
            }
        }

        let history_changed_from_committed = self
            .graphs
            .history_committed_document
            .as_ref()
            .map(|committed| {
                canonicalize_graph_document_for_history(committed)
                    != canonicalize_graph_document_for_history(&document)
            })
            .unwrap_or(false);

        if history_changed_from_committed && !self.graphs.redo_history.is_empty() {
            self.graphs.redo_history.clear();
        }

        if !self.graphs.pending_graph_update {
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(50));

        let Some(dirty_since) = self.graphs.graph_update_dirty_since_secs else {
            return;
        };
        let Some(last_change) = self.graphs.graph_update_last_change_secs else {
            return;
        };

        let debounced = now_secs - last_change >= 0.4;
        let max_delay_reached = now_secs - dirty_since >= 3.0;

        if debounced || max_delay_reached {
            self.commit_graph_history_snapshot(document.clone());

            if self.connection.is_connected()
                && self.graphs.save_in_flight_document.as_ref() != Some(&canonical_document)
            {
                debug!(graph_id = %document.metadata.id, "frontend sending graph save request");
                self.send(ClientMessage::UpdateGraphDocument {
                    document: document.clone(),
                });
                self.graphs.save_in_flight_document = Some(canonical_document);
                self.ui.status = "Saving graph document...".to_owned();
            }
        }
    }
}

/// Normalizes a graph document for persistence comparisons.
///
/// This currently canonicalizes gradient-valued parameters so autosave and equality checks are not
/// sensitive to tiny float jitter.
pub(crate) fn canonicalize_graph_document(document: &GraphDocument) -> GraphDocument {
    let mut document = document.clone();
    for node in &mut document.nodes {
        for parameter in &mut node.parameters {
            if let Ok(gradient) = serde_json::from_value::<ColorGradient>(parameter.value.clone()) {
                parameter.value = serde_json::to_value(canonicalize_gradient(gradient))
                    .unwrap_or(parameter.value.clone());
            }
        }
    }
    document
}

/// Normalizes a graph document for undo-history comparisons while ignoring viewport movement.
pub(crate) fn canonicalize_graph_document_for_history(document: &GraphDocument) -> GraphDocument {
    let mut document = canonicalize_graph_document(document);
    document.viewport = GraphViewport::default();
    document
}

/// Normalizes a gradient by quantizing colors and positions without reordering stops.
fn canonicalize_gradient(mut gradient: ColorGradient) -> ColorGradient {
    for stop in &mut gradient.stops {
        stop.position = quantize_unit_float(stop.position);
        stop.color.r = quantize_unit_float(stop.color.r);
        stop.color.g = quantize_unit_float(stop.color.g);
        stop.color.b = quantize_unit_float(stop.color.b);
        stop.color.a = quantize_unit_float(stop.color.a);
    }
    gradient
}

/// Quantizes a unit float into a stable six-decimal representation in the inclusive `0..=1` range.
fn quantize_unit_float(value: f32) -> f32 {
    const STEP: f32 = 1_000_000.0;
    ((value.clamp(0.0, 1.0) * STEP).round() / STEP).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::canonicalize_gradient;
    use shared::{ColorGradient, ColorGradientStop, RgbaColor};

    #[test]
    fn canonicalize_gradient_preserves_stop_order_and_duplicate_positions() {
        let gradient = ColorGradient {
            stops: vec![
                stop(1.0, 0.0, 0.0, 1.0),
                stop(0.5, 1.0, 0.0, 0.0),
                stop(0.5, 0.0, 1.0, 0.0),
            ],
        };

        let canonical = canonicalize_gradient(gradient);

        assert_eq!(canonical.stops.len(), 3);
        assert_eq!(canonical.stops[0].position, 1.0);
        assert_eq!(canonical.stops[1].position, 0.5);
        assert_eq!(canonical.stops[2].position, 0.5);
    }

    fn stop(position: f32, r: f32, g: f32, b: f32) -> ColorGradientStop {
        ColorGradientStop {
            position,
            color: RgbaColor { r, g, b, a: 1.0 },
        }
    }
}
