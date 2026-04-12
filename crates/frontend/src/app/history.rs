use eframe::egui;
use shared::GraphDocument;

use super::{FrontendApp, MAX_UNDO_HISTORY};
use crate::state::AppView;

impl FrontendApp {
    /// Replaces the current undo/redo history with a new committed baseline document.
    pub(super) fn reset_graph_history(&mut self, document: Option<GraphDocument>) {
        self.graphs.history_committed_document = document;
        self.graphs.undo_history.clear();
        self.graphs.redo_history.clear();
    }

    /// Commits a new undo-history snapshot when the document changed in a history-relevant way.
    ///
    /// Viewport-only differences are ignored so panning and zooming do not flood the undo stack.
    pub(crate) fn commit_graph_history_snapshot(&mut self, document: GraphDocument) {
        let should_commit = self
            .graphs
            .history_committed_document
            .as_ref()
            .map(|committed| {
                crate::controllers::autosave::canonicalize_graph_document_for_history(committed)
                    != crate::controllers::autosave::canonicalize_graph_document_for_history(
                        &document,
                    )
            })
            .unwrap_or(true);
        if !should_commit {
            return;
        }

        if let Some(previous) = self.graphs.history_committed_document.replace(document) {
            self.graphs.undo_history.push(previous);
            if self.graphs.undo_history.len() > MAX_UNDO_HISTORY {
                let overflow = self.graphs.undo_history.len() - MAX_UNDO_HISTORY;
                self.graphs.undo_history.drain(0..overflow);
            }
        }
        self.graphs.redo_history.clear();
    }

    /// Returns whether an older committed graph state is available for undo.
    pub(crate) fn can_undo_graph_edit(&self) -> bool {
        !self.graphs.undo_history.is_empty()
    }

    /// Returns whether a previously undone graph state is available for redo.
    pub(crate) fn can_redo_graph_edit(&self) -> bool {
        !self.graphs.redo_history.is_empty()
    }

    /// Restores the previous graph snapshot from the undo stack.
    pub(crate) fn undo_graph_edit(&mut self) {
        let Some(previous) = self.graphs.undo_history.pop() else {
            return;
        };
        let Some(current) = self.graphs.loaded_graph_document.clone() else {
            return;
        };
        self.graphs.redo_history.push(current);
        self.restore_graph_history_snapshot(previous);
        self.ui.status = "Undid graph edit".to_owned();
    }

    /// Restores the next graph snapshot from the redo stack.
    pub(crate) fn redo_graph_edit(&mut self) {
        let Some(next) = self.graphs.redo_history.pop() else {
            return;
        };
        let Some(current) = self.graphs.loaded_graph_document.clone() else {
            return;
        };
        self.graphs.undo_history.push(current);
        if self.graphs.undo_history.len() > MAX_UNDO_HISTORY {
            let overflow = self.graphs.undo_history.len() - MAX_UNDO_HISTORY;
            self.graphs.undo_history.drain(0..overflow);
        }
        self.restore_graph_history_snapshot(next);
        self.ui.status = "Redid graph edit".to_owned();
    }

    /// Handles keyboard and mouse-back/forward shortcuts for undo, redo, and browser navigation.
    ///
    /// Mouse side buttons are routed to undo and redo only while the editor canvas is hovered.
    pub(super) fn handle_history_shortcuts(&mut self, ctx: &egui::Context) {
        if self.ui.active_view != AppView::Editor || ctx.wants_keyboard_input() {
            return;
        }

        let (
            undo_pressed,
            redo_pressed,
            copy_pressed,
            paste_pressed,
            mouse_back_pressed,
            mouse_forward_pressed,
        ) = ctx.input(|input| {
            let undo_pressed = input.modifiers.command
                && !input.modifiers.shift
                && input.key_pressed(egui::Key::Z);
            let redo_pressed = (input.modifiers.command && input.key_pressed(egui::Key::Y))
                || (input.modifiers.command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::Z));
            let copy_pressed = input.modifiers.command
                && !input.modifiers.shift
                && input.key_pressed(egui::Key::C);
            let paste_pressed = input.modifiers.command
                && !input.modifiers.shift
                && input.key_pressed(egui::Key::V);
            let mouse_back_pressed = input.pointer.button_pressed(egui::PointerButton::Extra1);
            let mouse_forward_pressed = input.pointer.button_pressed(egui::PointerButton::Extra2);
            (
                undo_pressed,
                redo_pressed,
                copy_pressed,
                paste_pressed,
                mouse_back_pressed,
                mouse_forward_pressed,
            )
        });

        if undo_pressed {
            self.undo_graph_edit();
            return;
        }
        if redo_pressed {
            self.redo_graph_edit();
            return;
        }
        if copy_pressed {
            self.copy_selected_nodes_to_clipboard();
            return;
        }
        if paste_pressed {
            self.paste_nodes_from_clipboard();
            return;
        }
        if mouse_back_pressed {
            if self.ui.editor_canvas_hovered {
                self.undo_graph_edit();
            } else {
                self.navigate_browser_back();
            }
            return;
        }
        if mouse_forward_pressed {
            if self.ui.editor_canvas_hovered {
                self.redo_graph_edit();
            } else {
                self.navigate_browser_forward();
            }
        }
    }
}
