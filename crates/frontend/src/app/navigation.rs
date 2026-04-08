use super::FrontendApp;
use crate::state::AppView;

impl FrontendApp {
    #[cfg(target_arch = "wasm32")]
    /// Navigates one step backward in the browser history.
    ///
    /// This is only available on wasm targets and is a no-op when the browser window or history
    /// object cannot be accessed.
    pub(super) fn navigate_browser_back(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let _ = history.back();
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Navigates one step backward in the browser history.
    ///
    /// Native builds do not expose browser history, so this is a no-op.
    pub(super) fn navigate_browser_back(&self) {}

    #[cfg(target_arch = "wasm32")]
    /// Navigates one step forward in the browser history.
    ///
    /// This is only available on wasm targets and is a no-op when the browser window or history
    /// object cannot be accessed.
    pub(super) fn navigate_browser_forward(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let _ = history.forward();
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Navigates one step forward in the browser history.
    ///
    /// Native builds do not expose browser history, so this is a no-op.
    pub(super) fn navigate_browser_forward(&self) {}

    #[cfg(target_arch = "wasm32")]
    /// Initializes the current view from the browser path at startup.
    ///
    /// When the path resolves to a graph route, the editor is selected and the matching graph ID
    /// is staged for loading.
    pub(crate) fn initialize_from_browser_path(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(pathname) = window.location().pathname() else {
            return;
        };
        let base_path = app_base_path();

        if let Some(graph_id) = graph_id_from_path_with_base(&pathname, &base_path) {
            self.ui.selected_graph_id = Some(graph_id);
            self.ui.active_view = AppView::Editor;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Initializes the current view from the browser path at startup.
    ///
    /// Native builds do not use browser routing, so this is a no-op.
    pub(crate) fn initialize_from_browser_path(&mut self) {}

    #[cfg(target_arch = "wasm32")]
    /// Synchronizes the selected view with the current browser path.
    ///
    /// This keeps manual browser navigation, direct URL edits, and in-app navigation in sync by
    /// opening the addressed graph or returning to the dashboard as needed.
    pub(crate) fn sync_state_from_browser_path(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(pathname) = window.location().pathname() else {
            return;
        };
        let base_path = app_base_path();

        match graph_id_from_path_with_base(&pathname, &base_path) {
            Some(graph_id) => {
                let already_selected = self.ui.active_view == AppView::Editor
                    && self.ui.selected_graph_id.as_deref() == Some(graph_id.as_str());
                if !already_selected {
                    self.open_graph_internal(graph_id, false);
                }
            }
            None => {
                let already_on_dashboard = self.ui.active_view == AppView::Dashboard
                    && self.ui.selected_graph_id.is_none();
                if !already_on_dashboard {
                    self.return_to_dashboard_internal(false);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Synchronizes the selected view with the current browser path.
    ///
    /// Native builds do not use browser routing, so this is a no-op.
    pub(crate) fn sync_state_from_browser_path(&mut self) {}

    #[cfg(target_arch = "wasm32")]
    /// Replaces the current browser history entry to match the active frontend view.
    ///
    /// This is used for state synchronization when the current history entry should be updated in
    /// place rather than pushing a new navigation entry.
    pub(crate) fn sync_browser_path_for_current_view(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let path = route_path_for_view(self);

        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Replaces the current browser history entry to match the active frontend view.
    ///
    /// Native builds do not use browser routing, so this is a no-op.
    pub(crate) fn sync_browser_path_for_current_view(&self) {}

    #[cfg(target_arch = "wasm32")]
    /// Pushes a new browser history entry for the active frontend view.
    ///
    /// This is used for user-initiated navigation that should participate in browser back/forward
    /// behavior.
    pub(crate) fn push_browser_path_for_current_view(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let path = route_path_for_view(self);

        let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Pushes a new browser history entry for the active frontend view.
    ///
    /// Native builds do not use browser routing, so this is a no-op.
    pub(crate) fn push_browser_path_for_current_view(&self) {}

    /// Returns the metadata for the currently selected graph.
    pub(crate) fn selected_graph(&self) -> Option<&shared::GraphMetadata> {
        let selected_id = self.ui.selected_graph_id.as_deref()?;
        self.graphs
            .graph_documents
            .iter()
            .find(|graph| graph.id == selected_id)
    }

    /// Returns the loaded graph document when it matches the selected graph.
    ///
    /// This prevents callers from mutating a stale document after the selection changes.
    pub(crate) fn active_graph_document_mut(&mut self) -> Option<&mut shared::GraphDocument> {
        let selected_id = self.ui.selected_graph_id.as_deref()?;
        let loaded_id = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str())?;
        if loaded_id != selected_id {
            return None;
        }
        self.graphs.loaded_graph_document.as_mut()
    }

    /// Requests the selected graph document from the backend when it is not already loaded.
    ///
    /// Repeated requests for the same graph are suppressed while a previous request is still
    /// pending.
    pub(crate) fn ensure_selected_graph_document_requested(&mut self) {
        let Some(selected_graph_id) = self.ui.selected_graph_id.clone() else {
            return;
        };
        let already_loaded = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str() == selected_graph_id.as_str())
            .unwrap_or(false);
        if already_loaded {
            return;
        }

        if self.graphs.requested_graph_document_id.as_deref() == Some(selected_graph_id.as_str()) {
            return;
        }

        self.send(shared::ClientMessage::GetGraphDocument {
            id: selected_graph_id.clone(),
        });
        self.graphs.requested_graph_document_id = Some(selected_graph_id);
    }

    /// Switches the UI into the editor for `graph_id` and resets graph-scoped state.
    ///
    /// When `update_history` is `true`, the new editor path is pushed into browser history.
    pub(super) fn open_graph_internal(&mut self, graph_id: String, update_history: bool) {
        self.ui.selected_graph_id = Some(graph_id.clone());
        self.ui.node_menu_search.clear();
        self.ui.node_menu_graph_position = None;
        self.ui.rename_graph_dialog_open = false;
        self.ui.rename_graph_id = Some(graph_id.clone());
        self.ui.rename_graph_name.clear();
        self.graphs.loaded_graph_document = None;
        self.reset_graph_history(None);
        self.graphs.requested_graph_document_id = None;
        self.graphs.persisted_graph_document = None;
        self.graphs.save_in_flight_document = None;
        self.clear_pending_graph_update_tracking();
        self.graphs.snarl_viewport_initialized_graph_id = None;
        self.graphs.runtime_node_values.clear();
        self.ui.diagnostics_window_graph_id = None;
        self.ui.diagnostics_window_node_id = None;
        self.ui.active_view = AppView::Editor;
        if update_history {
            self.push_browser_path_for_current_view();
        }
        self.ensure_selected_graph_document_requested();
    }

    /// Opens a graph in the editor and records the navigation in browser history.
    pub(crate) fn open_graph(&mut self, graph_id: String) {
        self.open_graph_internal(graph_id, true);
    }

    /// Returns the UI to the dashboard and clears graph-scoped state.
    ///
    /// When `update_history` is `true`, the dashboard route is pushed into browser history.
    pub(super) fn return_to_dashboard_internal(&mut self, update_history: bool) {
        self.clear_selected_graph_session();
        self.ui.active_view = AppView::Dashboard;
        if update_history {
            self.push_browser_path_for_current_view();
        }
    }

    /// Returns the UI to the dashboard and records the navigation in browser history.
    pub(crate) fn return_to_dashboard(&mut self) {
        self.return_to_dashboard_internal(true);
    }

    /// Forces a reload of the currently selected graph document from the backend.
    pub(crate) fn reload_selected_graph(&mut self) {
        self.graphs.requested_graph_document_id = None;
        self.ensure_selected_graph_document_requested();
    }
}

/// Extracts a graph ID from a frontend route path.
///
/// The current route format is `/graphs/<graph_id>`.
fn graph_id_from_path(pathname: &str) -> Option<String> {
    graph_id_from_path_with_base(pathname, "")
}

fn graph_id_from_path_with_base(pathname: &str, base_path: &str) -> Option<String> {
    let pathname = strip_base_path(pathname, base_path)?;
    let mut segments = pathname.trim_matches('/').split('/');
    match (segments.next(), segments.next(), segments.next()) {
        (Some("graphs"), Some(graph_id), None) if !graph_id.is_empty() => Some(graph_id.to_owned()),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn route_path_for_view(app: &FrontendApp) -> String {
    let relative_path = match app.ui.active_view {
        AppView::Dashboard => "/".to_owned(),
        AppView::Editor => app
            .ui
            .selected_graph_id
            .as_deref()
            .map(|graph_id| format!("/graphs/{graph_id}"))
            .unwrap_or_else(|| "/".to_owned()),
    };
    join_base_path(&app_base_path(), &relative_path)
}

#[cfg(target_arch = "wasm32")]
fn app_base_path() -> String {
    let Some(window) = web_sys::window() else {
        return String::new();
    };
    let Ok(pathname) = window.location().pathname() else {
        return String::new();
    };
    infer_base_path_from_location(&pathname)
}

fn strip_base_path<'a>(pathname: &'a str, base_path: &str) -> Option<&'a str> {
    if base_path.is_empty() {
        return Some(pathname);
    }

    if pathname == base_path {
        return Some("/");
    }

    pathname
        .strip_prefix(base_path)
        .filter(|suffix| suffix.starts_with('/'))
}

#[cfg(target_arch = "wasm32")]
fn join_base_path(base_path: &str, relative_path: &str) -> String {
    if base_path.is_empty() {
        return relative_path.to_owned();
    }

    if relative_path == "/" {
        format!("{base_path}/")
    } else {
        format!("{base_path}{relative_path}")
    }
}

fn normalize_base_path(pathname: &str) -> String {
    let trimmed = pathname.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        String::new()
    } else {
        trimmed.to_owned()
    }
}

fn infer_base_path_from_location(pathname: &str) -> String {
    let trimmed = pathname.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return String::new();
    }

    if let Some((base_path, _)) = trimmed.split_once("/graphs/") {
        normalize_base_path(base_path)
    } else {
        normalize_base_path(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::{graph_id_from_path, graph_id_from_path_with_base, infer_base_path_from_location};

    #[test]
    fn parses_graph_route() {
        assert_eq!(graph_id_from_path("/graphs/demo"), Some("demo".to_owned()));
        assert_eq!(graph_id_from_path("graphs/demo"), Some("demo".to_owned()));
    }

    #[test]
    fn rejects_non_graph_routes() {
        assert_eq!(graph_id_from_path("/"), None);
        assert_eq!(graph_id_from_path("/graphs"), None);
        assert_eq!(graph_id_from_path("/graphs/demo/extra"), None);
    }

    #[test]
    fn parses_graph_route_under_base_path() {
        assert_eq!(
            graph_id_from_path_with_base("/luma-weaver/graphs/demo", "/luma-weaver"),
            Some("demo".to_owned())
        );
        assert_eq!(
            graph_id_from_path_with_base("/luma-weaver/", "/luma-weaver"),
            None
        );
    }

    #[test]
    fn infers_pages_base_path_from_dashboard_and_graph_routes() {
        assert_eq!(infer_base_path_from_location("/"), String::new());
        assert_eq!(
            infer_base_path_from_location("/luma-weaver/"),
            "/luma-weaver".to_owned()
        );
        assert_eq!(
            infer_base_path_from_location("/luma-weaver/graphs/demo"),
            "/luma-weaver".to_owned()
        );
    }
}
