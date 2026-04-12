use serde_json::Value as JsonValue;

use super::FrontendApp;

impl FrontendApp {
    /// Starts the browser flow for selecting and uploading an image asset for a node parameter.
    pub(crate) fn begin_image_asset_upload(&mut self, node_id: String, parameter_name: String) {
        match crate::browser_file::pick_and_upload_image_asset(node_id, parameter_name) {
            Ok(receiver) => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.ui.browser_image_asset_events = Some(receiver);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = receiver;
                }
                self.ui.status = "Choose an image to upload".to_owned();
            }
            Err(message) => {
                self.ui.status = message;
            }
        }
    }

    /// Drains pending browser image-upload events and applies successful asset ids into the graph.
    pub(super) fn drain_browser_image_asset_events(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            use futures_channel::mpsc::TryRecvError;

            let Some(receiver) = &mut self.ui.browser_image_asset_events else {
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
                self.ui.browser_image_asset_events = None;
            }

            for event in events {
                match event {
                    crate::browser_file::BrowserImageAssetEvent::Uploaded {
                        node_id,
                        parameter_name,
                        asset_id,
                    } => self.apply_uploaded_image_asset(node_id, parameter_name, asset_id),
                    crate::browser_file::BrowserImageAssetEvent::Error(message) => {
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
                || self.ui.browser_image_asset_events.is_some()
        }
    }

    fn apply_uploaded_image_asset(
        &mut self,
        node_id: String,
        parameter_name: String,
        asset_id: String,
    ) {
        let Some(document) = self.active_graph_document_mut() else {
            self.ui.status = "Cannot apply uploaded image without an open graph".to_owned();
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

        self.ui.status = "Uploaded image asset".to_owned();
    }
}
