use shared::{GraphClipboardFragment, NodePosition};

use super::FrontendApp;

impl FrontendApp {
    /// Copies the currently selected nodes to the system clipboard.
    pub(crate) fn copy_selected_nodes_to_clipboard(&mut self) {
        let selected_node_ids = self
            .ui
            .selected_graph_node_ids
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        self.ui.pending_clipboard_read_graph_id = None;
        if selected_node_ids.is_empty() {
            self.ui.status = "Select one or more nodes to copy".to_owned();
            return;
        }

        let Some(document) = self.active_graph_document_mut().cloned() else {
            self.ui.status = "No graph document is loaded".to_owned();
            return;
        };
        let Some(fragment) =
            crate::editor_view::clipboard_fragment_from_document(&document, &selected_node_ids)
        else {
            self.ui.status = "Nothing selected to copy".to_owned();
            return;
        };

        let payload = match serde_json::to_string_pretty(&fragment) {
            Ok(payload) => payload,
            Err(error) => {
                self.ui.status = format!("Failed to serialize clipboard fragment: {error}");
                return;
            }
        };

        match crate::browser_file::write_text_to_clipboard(payload) {
            Ok(receiver) => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.ui.browser_clipboard_events = Some(receiver);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = receiver;
                }
                self.ui.status = format!("Copying {} node(s) to clipboard", fragment.nodes.len());
            }
            Err(message) => {
                self.ui.status = message;
            }
        }
    }

    /// Starts a clipboard read so the current graph can paste any copied node fragment.
    pub(crate) fn paste_nodes_from_clipboard(&mut self) {
        let Some(target_graph_id) = self.ui.selected_graph_id.clone() else {
            self.ui.status = "No graph document is loaded".to_owned();
            return;
        };
        if self.active_graph_document_mut().is_none() {
            self.ui.status = "No graph document is loaded".to_owned();
            return;
        }

        match crate::browser_file::read_text_from_clipboard() {
            Ok(receiver) => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.ui.browser_clipboard_events = Some(receiver);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = receiver;
                }
                self.ui.pending_clipboard_read_graph_id = Some(target_graph_id);
                self.ui.status = "Reading clipboard".to_owned();
            }
            Err(message) => {
                self.ui.pending_clipboard_read_graph_id = None;
                self.ui.status = message;
            }
        }
    }

    /// Drains asynchronous clipboard events triggered by browser clipboard APIs.
    pub(super) fn drain_browser_clipboard_events(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            use futures_channel::mpsc::TryRecvError;

            let Some(receiver) = &mut self.ui.browser_clipboard_events else {
                return;
            };

            let mut events = Vec::new();
            let mut receiver_closed = false;
            loop {
                match receiver.try_recv() {
                    Ok(event) => events.push(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Closed) => {
                        receiver_closed = true;
                        break;
                    }
                }
            }

            if !events.is_empty() {
                self.ui.browser_clipboard_events = None;
            } else if receiver_closed {
                self.ui.browser_clipboard_events = None;
                return;
            }

            for event in events {
                match event {
                    crate::browser_file::BrowserClipboardEvent::Copied => {
                        let copied_count = self.ui.selected_graph_node_ids.len();
                        self.ui.status = format!("Copied {copied_count} node(s) to clipboard");
                    }
                    crate::browser_file::BrowserClipboardEvent::Read(text) => {
                        if let Err(message) =
                            self.ensure_pending_clipboard_paste_targets_active_graph()
                        {
                            self.ui.pending_clipboard_read_graph_id = None;
                            self.ui.status = message;
                            continue;
                        }
                        self.handle_clipboard_text(text);
                        self.ui.pending_clipboard_read_graph_id = None;
                    }
                    crate::browser_file::BrowserClipboardEvent::Error(message) => {
                        self.ui.pending_clipboard_read_graph_id = None;
                        self.ui.status = message;
                    }
                }
            }
        }
    }

    fn ensure_pending_clipboard_paste_targets_active_graph(&self) -> Result<(), String> {
        let Some(target_graph_id) = self.ui.pending_clipboard_read_graph_id.as_deref() else {
            return Ok(());
        };
        let selected_graph_id = self.ui.selected_graph_id.as_deref();
        let loaded_graph_id = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str());

        if selected_graph_id == Some(target_graph_id) && loaded_graph_id == Some(target_graph_id) {
            Ok(())
        } else {
            Err("Clipboard paste was cancelled because the active graph changed".to_owned())
        }
    }

    fn handle_clipboard_text(&mut self, text: String) {
        let fragment = match serde_json::from_str::<GraphClipboardFragment>(&text) {
            Ok(fragment) => fragment,
            Err(error) => {
                self.ui.status =
                    format!("Clipboard does not contain a valid graph fragment: {error}");
                return;
            }
        };
        if let Err(message) = fragment.validate_header() {
            self.ui.status = message;
            return;
        }

        let available_node_definitions = self.graphs.available_node_definitions.clone();
        let paste_origin = self.preferred_paste_origin();
        let (pasted_node_count, skipped_node_type_ids, inserted_node_ids) = {
            let Some(document) = self.active_graph_document_mut() else {
                self.ui.status = "No graph document is loaded".to_owned();
                return;
            };
            let result = crate::editor_view::paste_clipboard_fragment_into_document(
                document,
                &fragment,
                &available_node_definitions,
                paste_origin,
            );
            (
                result.inserted_node_ids.len(),
                result.skipped_node_type_ids,
                result.inserted_node_ids,
            )
        };

        self.ui.selected_graph_node_ids = inserted_node_ids;
        self.sync_live_snarl_from_loaded_document();
        self.ui.status = if pasted_node_count == 0 {
            if skipped_node_type_ids.is_empty() {
                "Clipboard fragment did not contain any pasteable nodes".to_owned()
            } else {
                format!(
                    "Skipped unsupported node types: {}",
                    skipped_node_type_ids.join(", ")
                )
            }
        } else if skipped_node_type_ids.is_empty() {
            format!("Pasted {pasted_node_count} node(s)")
        } else {
            format!(
                "Pasted {pasted_node_count} node(s), skipped unsupported node types: {}",
                skipped_node_type_ids.join(", ")
            )
        };
    }

    fn preferred_paste_origin(&self) -> Option<NodePosition> {
        if !self.ui.editor_canvas_hovered {
            return None;
        }
        self.ui
            .editor_pointer_graph_position
            .map(|(x, y)| NodePosition { x, y })
    }
}

#[cfg(test)]
mod tests {
    use shared::{GraphDocument, GraphMetadata};

    use crate::app::FrontendApp;

    #[test]
    fn pending_clipboard_paste_is_rejected_after_graph_switch() {
        let mut app = FrontendApp::default();
        app.ui.selected_graph_id = Some("graph-b".to_owned());
        app.ui.pending_clipboard_read_graph_id = Some("graph-a".to_owned());
        app.graphs.loaded_graph_document = Some(GraphDocument {
            metadata: GraphMetadata {
                id: "graph-b".to_owned(),
                name: "Graph B".to_owned(),
                execution_frequency_hz: 60,
            },
            ..GraphDocument::default()
        });

        assert_eq!(
            app.ensure_pending_clipboard_paste_targets_active_graph(),
            Err("Clipboard paste was cancelled because the active graph changed".to_owned())
        );
    }
}
