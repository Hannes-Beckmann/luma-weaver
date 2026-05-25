use serde_json::Value as JsonValue;

use super::FrontendApp;

#[derive(Clone, Copy)]
enum AssetUploadKind {
    Image,
    Layout,
}

impl AssetUploadKind {
    fn status_prompt(self) -> &'static str {
        match self {
            Self::Image => "Choose an image to upload",
            Self::Layout => "Choose a CSV or JSON layout to upload",
        }
    }

    fn success_status(self) -> &'static str {
        match self {
            Self::Image => "Uploaded image asset",
            Self::Layout => "Uploaded layout asset",
        }
    }
}

impl FrontendApp {
    /// Starts the browser flow for selecting and uploading an image asset for a node parameter.
    pub(crate) fn begin_image_asset_upload(&mut self, node_id: String, parameter_name: String) {
        self.begin_asset_upload(AssetUploadKind::Image, node_id, parameter_name);
    }

    /// Starts the browser flow for selecting and uploading a layout asset for a node parameter.
    pub(crate) fn begin_layout_asset_upload(&mut self, node_id: String, parameter_name: String) {
        self.begin_asset_upload(AssetUploadKind::Layout, node_id, parameter_name);
    }

    fn begin_asset_upload(
        &mut self,
        kind: AssetUploadKind,
        node_id: String,
        parameter_name: String,
    ) {
        match crate::browser_file::pick_and_upload_asset(kind.into(), node_id, parameter_name) {
            Ok(receiver) => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.ui.browser_asset_upload_events = Some(receiver);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = receiver;
                }
                self.ui.status = kind.status_prompt().to_owned();
            }
            Err(message) => {
                self.ui.status = message;
            }
        }
    }

    /// Drains pending browser asset-upload events and applies successful asset ids into the graph.
    pub(super) fn drain_browser_asset_upload_events(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            use futures_channel::mpsc::TryRecvError;

            let Some(receiver) = &mut self.ui.browser_asset_upload_events else {
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

            if !events.is_empty() || receiver_closed {
                self.ui.browser_asset_upload_events = None;
            }

            for event in events {
                match event {
                    crate::browser_file::BrowserAssetUploadEvent::Uploaded {
                        kind,
                        node_id,
                        parameter_name,
                        asset_id,
                    } => self.apply_uploaded_asset(kind.into(), node_id, parameter_name, asset_id),
                    crate::browser_file::BrowserAssetUploadEvent::Error(message) => {
                        self.ui.status = message;
                    }
                }
            }
        }
    }

    /// Returns whether browser-managed background work should keep scheduling frames.
    pub(crate) fn browser_background_work_needs_repaint(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.ui.browser_graph_file_events.is_some()
                || self.ui.browser_asset_upload_events.is_some()
        }
    }

    fn apply_uploaded_asset(
        &mut self,
        kind: AssetUploadKind,
        node_id: String,
        parameter_name: String,
        asset_id: String,
    ) {
        let Some(document) = self.active_graph_document_mut() else {
            self.ui.status = match kind {
                AssetUploadKind::Image => {
                    "Cannot apply uploaded image without an open graph".to_owned()
                }
                AssetUploadKind::Layout => {
                    "Cannot apply uploaded layout without an open graph".to_owned()
                }
            };
            return;
        };
        let Some(node) = document.nodes.iter_mut().find(|node| node.id == node_id) else {
            self.ui.status = "The edited node no longer exists in the open graph".to_owned();
            return;
        };

        let previous_asset_id = node
            .parameters
            .iter()
            .find(|parameter| parameter.name == parameter_name)
            .and_then(|parameter| parameter.value.as_str())
            .map(str::to_owned);

        if let Some(parameter) = node
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == parameter_name)
        {
            parameter.value = JsonValue::from(asset_id.clone());
        } else {
            node.parameters.push(shared::NodeParameter {
                name: parameter_name.clone(),
                value: JsonValue::from(asset_id.clone()),
            });
        }

        let replaced_layout_asset_id =
            orphaned_layout_asset_id(document, kind, &parameter_name, previous_asset_id, &asset_id);
        self.sync_live_snarl_from_loaded_document();
        self.ui.status = kind.success_status().to_owned();
        self.delete_replaced_layout_asset(replaced_layout_asset_id);
    }

    fn delete_replaced_layout_asset(&mut self, asset_id: Option<String>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = asset_id;
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(asset_id) = asset_id {
            use wasm_bindgen_futures::spawn_local;

            spawn_local(async move {
                let _ =
                    crate::browser_file::delete_asset(crate::browser_file::AssetUploadKind::Layout, asset_id)
                        .await;
            });
        }
    }
}

fn orphaned_layout_asset_id(
    document: &shared::GraphDocument,
    kind: AssetUploadKind,
    parameter_name: &str,
    previous_asset_id: Option<String>,
    uploaded_asset_id: &str,
) -> Option<String> {
    if !matches!(kind, AssetUploadKind::Layout) {
        return None;
    }

    let previous_asset_id = previous_asset_id?;
    if previous_asset_id.is_empty() || previous_asset_id == uploaded_asset_id {
        return None;
    }

    let still_referenced = document.nodes.iter().flat_map(|node| node.parameters.iter()).any(
        |parameter| {
            parameter.name == parameter_name
                && parameter.value.as_str() == Some(previous_asset_id.as_str())
        },
    );

    (!still_referenced).then_some(previous_asset_id)
}

impl From<AssetUploadKind> for crate::browser_file::AssetUploadKind {
    fn from(value: AssetUploadKind) -> Self {
        match value {
            AssetUploadKind::Image => Self::Image,
            AssetUploadKind::Layout => Self::Layout,
        }
    }
}

impl From<crate::browser_file::AssetUploadKind> for AssetUploadKind {
    fn from(value: crate::browser_file::AssetUploadKind) -> Self {
        match value {
            crate::browser_file::AssetUploadKind::Image => Self::Image,
            crate::browser_file::AssetUploadKind::Layout => Self::Layout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrontendApp;
    use crate::editor_view::{
        build_snarl_from_document, snarl_node_parameter_value, snarl_node_titles,
    };
    use shared::{
        GraphDocument, GraphMetadata, NodeCategory, NodeConnectionDefinition, NodeMetadata,
        NodeParameter, NodeParameterDefinition, NodeSchema, NodeTypeId, ParameterDefaultValue,
        ParameterUiHint,
    };

    fn layout_upload_definition() -> NodeSchema {
        NodeSchema {
            id: NodeTypeId::WLED_TARGET.to_owned(),
            display_name: "Wled Target".to_owned(),
            category: NodeCategory::Outputs,
            needs_io: false,
            render_layouts: vec![
                shared::RenderLayoutKind::Index1d,
                shared::RenderLayoutKind::Matrix2d,
                shared::RenderLayoutKind::Spatial3d,
            ],
            inputs: vec![],
            outputs: vec![],
            parameters: vec![NodeParameterDefinition::new(
                "layout_asset_id",
                "Layout Asset".to_owned(),
                ParameterDefaultValue::String(String::new()),
                ParameterUiHint::TextSingleLine,
            )],
            connection: NodeConnectionDefinition {
                max_input_connections: 1,
                require_value_kind_match: true,
            },
            runtime_updates: None,
        }
    }

    fn graph_document() -> GraphDocument {
        GraphDocument {
            metadata: GraphMetadata {
                id: "graph-a".to_owned(),
                name: "Graph A".to_owned(),
                execution_frequency_hz: 60,
                home_assistant_broker_id: String::new(),
            },
            nodes: vec![shared::GraphNode {
                id: "node-1".to_owned(),
                metadata: NodeMetadata {
                    name: "Target".to_owned(),
                },
                node_type: NodeTypeId::new(NodeTypeId::WLED_TARGET),
                viewport: shared::NodeViewport::default(),
                input_values: Vec::new(),
                parameters: vec![NodeParameter {
                    name: "layout_asset_id".to_owned(),
                    value: serde_json::Value::from(String::new()),
                }],
            }],
            ..GraphDocument::default()
        }
    }

    #[test]
    fn uploaded_asset_resyncs_live_snarl_parameter_values() {
        let mut app = FrontendApp::default();
        let document = graph_document();
        let definitions = vec![layout_upload_definition()];
        app.ui.selected_graph_id = Some("graph-a".to_owned());
        app.graphs.available_node_definitions = definitions.clone();
        app.graphs.loaded_graph_document = Some(document.clone());
        app.graphs.live_snarl_graph_id = Some("graph-a".to_owned());
        app.graphs.live_snarl = Some(build_snarl_from_document(
            &document,
            &definitions,
            &app.graphs.runtime_node_values,
        ));

        app.apply_uploaded_asset(
            super::AssetUploadKind::Layout,
            "node-1".to_owned(),
            "layout_asset_id".to_owned(),
            "asset-123".to_owned(),
        );

        let updated_value = app
            .graphs
            .loaded_graph_document
            .as_ref()
            .and_then(|document| document.nodes.first())
            .and_then(|node| node.parameters.first())
            .and_then(|parameter| parameter.value.as_str());
        assert_eq!(updated_value, Some("asset-123"));

        let live_snarl = app.graphs.live_snarl.as_ref().expect("live snarl");
        let parameter_value = snarl_node_parameter_value(live_snarl, "node-1", "layout_asset_id")
            .and_then(|value| value.as_str().map(str::to_owned));
        assert_eq!(parameter_value.as_deref(), Some("asset-123"));
        assert_eq!(snarl_node_titles(live_snarl), vec!["Target".to_owned()]);
    }

    #[test]
    fn replacing_unique_layout_asset_marks_old_asset_for_deletion() {
        let mut document = graph_document();
        document.nodes[0].parameters[0].value = serde_json::Value::from("old-layout");

        document.nodes[0].parameters[0].value = serde_json::Value::from("new-layout");

        let deleted_asset = super::orphaned_layout_asset_id(
            &document,
            super::AssetUploadKind::Layout,
            "layout_asset_id",
            Some("old-layout".to_owned()),
            "new-layout",
        );

        assert_eq!(deleted_asset.as_deref(), Some("old-layout"));
    }

    #[test]
    fn replacing_shared_layout_asset_keeps_old_asset() {
        let mut document = graph_document();
        document.nodes.push(shared::GraphNode {
            id: "node-2".to_owned(),
            metadata: NodeMetadata {
                name: "Target 2".to_owned(),
            },
            node_type: NodeTypeId::new(NodeTypeId::WLED_TARGET),
            viewport: shared::NodeViewport::default(),
            input_values: Vec::new(),
            parameters: vec![NodeParameter {
                name: "layout_asset_id".to_owned(),
                value: serde_json::Value::from("shared-layout"),
            }],
        });
        document.nodes[0].parameters[0].value = serde_json::Value::from("replacement-layout");

        let deleted_asset = super::orphaned_layout_asset_id(
            &document,
            super::AssetUploadKind::Layout,
            "layout_asset_id",
            Some("shared-layout".to_owned()),
            "replacement-layout",
        );

        assert!(deleted_asset.is_none());
    }
}
