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

        if let Some(parameter) = node
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == parameter_name)
        {
            parameter.value = JsonValue::from(asset_id.clone());
        } else {
            node.parameters.push(shared::NodeParameter {
                name: parameter_name,
                value: JsonValue::from(asset_id.clone()),
            });
        }

        self.ui.status = kind.success_status().to_owned();
    }
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
