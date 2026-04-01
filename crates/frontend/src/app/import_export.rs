use shared::{
    ClientMessage, GraphDocument, GraphExchangeFile, GraphImportCollisionPolicy, GraphImportMode,
};

use super::FrontendApp;

impl FrontendApp {
    /// Requests an export of the graph with the given ID from the backend.
    ///
    /// The backend responds with a versioned `GraphExchangeFile`, which is later turned into a
    /// browser download by [`Self::handle_graph_export`].
    pub(crate) fn request_graph_export(&mut self, graph_id: String) {
        self.send(ClientMessage::ExportGraphDocument { id: graph_id });
    }

    /// Starts the browser flow for importing a graph file.
    ///
    /// On wasm targets this stores the browser file-event receiver so the event loop can process
    /// the selected file asynchronously. On native targets the browser picker is not used.
    pub(crate) fn begin_graph_import(&mut self) {
        match crate::browser_file::pick_graph_import_file() {
            Ok(receiver) => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.ui.browser_graph_file_events = Some(receiver);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = receiver;
                }
                self.ui.status = "Choose a graph file to import".to_owned();
            }
            Err(message) => {
                self.ui.status = message;
            }
        }
    }

    /// Imports the pending graph file with the selected collision policy.
    ///
    /// This is used after the user resolves an ID collision in the dashboard import flow.
    pub(crate) fn import_pending_graph_file(
        &mut self,
        collision_policy: GraphImportCollisionPolicy,
    ) {
        let Some(file) = self.ui.pending_import_graph_file.take() else {
            return;
        };
        self.send(ClientMessage::ImportGraphDocument {
            file,
            collision_policy,
        });
    }

    /// Drains pending browser file-picker events for graph imports.
    ///
    /// Parsed files are forwarded into the normal import validation flow, while picker and parse
    /// failures are surfaced through the global UI status message.
    pub(super) fn drain_browser_graph_file_events(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            use futures_channel::mpsc::TryRecvError;

            let Some(receiver) = &mut self.ui.browser_graph_file_events else {
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
                self.ui.browser_graph_file_events = None;
            } else if receiver_closed {
                self.ui.browser_graph_file_events = None;
                return;
            }

            for event in events {
                match event {
                    crate::browser_file::BrowserGraphFileEvent::Parsed(file) => {
                        self.handle_import_graph_file(file);
                    }
                    crate::browser_file::BrowserGraphFileEvent::Error(message) => {
                        self.ui.status = message;
                    }
                }
            }
        }
    }

    /// Validates a parsed graph import file and either sends it to the backend or stages it for
    /// collision resolution.
    ///
    /// Files with invalid exchange headers are rejected locally. When the imported graph ID
    /// already exists, the file is stored in `pending_import_graph_file` so the user can choose
    /// between overwrite and import-copy behavior.
    fn handle_import_graph_file(&mut self, file: GraphExchangeFile) {
        if let Err(message) = file.validate_header() {
            self.ui.status = message;
            return;
        }

        let graph_id = file.document.metadata.id.trim().to_owned();
        let id_conflicts = !graph_id.is_empty()
            && self
                .graphs
                .graph_documents
                .iter()
                .any(|graph| graph.id == graph_id);

        if id_conflicts {
            self.ui.pending_import_graph_file = Some(file);
            self.ui.status = format!(
                "Graph id '{}' already exists. Choose how to import it.",
                graph_id
            );
            return;
        }

        self.send(ClientMessage::ImportGraphDocument {
            file,
            collision_policy: GraphImportCollisionPolicy::PreserveIfFree,
        });
    }

    /// Downloads an exported graph file through the browser.
    ///
    /// The graph name is sanitized into a stable filename before the exchange file is handed to
    /// the browser download helper.
    pub(crate) fn handle_graph_export(&mut self, file: GraphExchangeFile) {
        let filename = sanitize_graph_export_filename(&file.document.metadata.name);
        match crate::browser_file::download_graph_export(&filename, &file) {
            Ok(()) => {
                self.ui.status = format!("Exported graph as {filename}");
            }
            Err(message) => {
                self.ui.status = message;
            }
        }
    }

    /// Applies the result of a successful graph import.
    ///
    /// This clears any pending collision state and updates the dashboard status message to reflect
    /// whether the graph was imported as a new document or overwrote an existing one.
    pub(crate) fn handle_graph_imported(&mut self, document: GraphDocument, mode: GraphImportMode) {
        self.ui.pending_import_graph_file = None;
        self.ui.status = match mode {
            GraphImportMode::Imported => {
                format!("Imported graph '{}'", document.metadata.name)
            }
            GraphImportMode::Overwritten => {
                format!("Overwrote graph '{}'", document.metadata.name)
            }
        };
    }
}

/// Sanitizes a graph name for use as an export filename.
///
/// Non-alphanumeric characters are collapsed to underscores, empty names fall back to `graph`,
/// and the result always ends with the `.animation-graph.json` extension.
fn sanitize_graph_export_filename(graph_name: &str) -> String {
    let mut sanitized = graph_name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned();
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }
    if sanitized.is_empty() {
        sanitized = "graph".to_owned();
    }
    format!("{sanitized}.animation-graph.json")
}
