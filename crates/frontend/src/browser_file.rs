use futures_channel::mpsc;
use shared::GraphExchangeFile;

/// Represents the outcome of a browser-managed graph import file read.
pub(crate) enum BrowserGraphFileEvent {
    Parsed(GraphExchangeFile),
    Error(String),
}

/// Represents the outcome of a browser-managed image upload.
pub(crate) enum BrowserImageAssetEvent {
    Uploaded {
        node_id: String,
        parameter_name: String,
        asset_id: String,
    },
    Error(String),
}

/// Represents the outcome of a browser-managed clipboard interaction.
pub(crate) enum BrowserClipboardEvent {
    Copied,
    Read(String),
    Error(String),
}

#[cfg(target_arch = "wasm32")]
/// Opens the browser file picker for graph import and returns a stream of parse results.
///
/// The returned receiver yields either a successfully parsed `GraphExchangeFile` or a user-facing
/// error message once the selected file has been read and parsed.
pub(crate) fn pick_graph_import_file()
-> Result<mpsc::UnboundedReceiver<BrowserGraphFileEvent>, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable".to_owned());
    };
    let Some(document) = window.document() else {
        return Err("Browser document is unavailable".to_owned());
    };
    let input = document
        .create_element("input")
        .map_err(|_| "Failed to create file input".to_owned())?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|_| "Failed to create file input".to_owned())?;
    input.set_type("file");
    input.set_accept(".animation-graph.json,application/json,.json");

    let (sender, receiver) = mpsc::unbounded();
    let onchange_sender = sender.clone();
    let onchange_input = input.clone();
    let onchange = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(files) = onchange_input.files() else {
            let _ = onchange_sender.unbounded_send(BrowserGraphFileEvent::Error(
                "No file was selected".to_owned(),
            ));
            return;
        };
        let Some(file) = files.get(0) else {
            let _ = onchange_sender.unbounded_send(BrowserGraphFileEvent::Error(
                "No file was selected".to_owned(),
            ));
            return;
        };

        let Ok(reader) = web_sys::FileReader::new() else {
            let _ = onchange_sender.unbounded_send(BrowserGraphFileEvent::Error(
                "Failed to create browser file reader".to_owned(),
            ));
            return;
        };

        let load_sender = onchange_sender.clone();
        let load_reader = reader.clone();
        let onload = Closure::once(Box::new(move |_event: web_sys::Event| {
            let result = load_reader
                .result()
                .ok()
                .and_then(|value| value.as_string())
                .ok_or_else(|| "Failed to read graph file".to_owned())
                .and_then(|text| {
                    serde_json::from_str::<GraphExchangeFile>(&text)
                        .map_err(|error| format!("Failed to parse graph file: {error}"))
                });

            let event = match result {
                Ok(file) => BrowserGraphFileEvent::Parsed(file),
                Err(message) => BrowserGraphFileEvent::Error(message),
            };
            let _ = load_sender.unbounded_send(event);
        }) as Box<dyn FnOnce(_)>);
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        let error_sender = onchange_sender.clone();
        let onerror = Closure::once(Box::new(move |_event: web_sys::Event| {
            let _ = error_sender.unbounded_send(BrowserGraphFileEvent::Error(
                "Failed to read graph file".to_owned(),
            ));
        }) as Box<dyn FnOnce(_)>);
        reader.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        if reader.read_as_text(&file).is_err() {
            let _ = onchange_sender.unbounded_send(BrowserGraphFileEvent::Error(
                "Failed to start reading graph file".to_owned(),
            ));
        }
    }) as Box<dyn FnMut(_)>);

    input.set_onchange(Some(onchange.as_ref().unchecked_ref()));
    onchange.forget();
    input.click();

    Ok(receiver)
}

#[cfg(not(target_arch = "wasm32"))]
/// Reports that graph import via the browser file picker is unavailable on non-wasm builds.
pub(crate) fn pick_graph_import_file()
-> Result<mpsc::UnboundedReceiver<BrowserGraphFileEvent>, String> {
    Err("Graph import is only available in the browser build".to_owned())
}

#[cfg(target_arch = "wasm32")]
/// Serializes a graph export file and triggers a browser download for it.
pub(crate) fn download_graph_export(
    filename: &str,
    file: &GraphExchangeFile,
) -> Result<(), String> {
    use js_sys::Array;
    use wasm_bindgen::JsCast;

    let payload = serde_json::to_string_pretty(file)
        .map_err(|error| format!("Failed to serialize graph export: {error}"))?;
    let data = Array::new();
    data.push(&wasm_bindgen::JsValue::from_str(&payload));
    let blob = web_sys::Blob::new_with_str_sequence(&data)
        .map_err(|_| "Failed to create download blob".to_owned())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Failed to create download URL".to_owned())?;

    let Some(window) = web_sys::window() else {
        let _ = web_sys::Url::revoke_object_url(&url);
        return Err("Browser window is unavailable".to_owned());
    };
    let Some(document) = window.document() else {
        let _ = web_sys::Url::revoke_object_url(&url);
        return Err("Browser document is unavailable".to_owned());
    };
    let anchor = document
        .create_element("a")
        .map_err(|_| "Failed to create download link".to_owned())?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "Failed to create download link".to_owned())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Reports that browser-managed graph export downloads are unavailable on non-wasm builds.
pub(crate) fn download_graph_export(
    _filename: &str,
    _file: &GraphExchangeFile,
) -> Result<(), String> {
    Err("Graph export is only available in the browser build".to_owned())
}

#[cfg(target_arch = "wasm32")]
/// Writes text to the browser clipboard and reports the async result through a receiver.
pub(crate) fn write_text_to_clipboard(
    text: String,
) -> Result<mpsc::UnboundedReceiver<BrowserClipboardEvent>, String> {
    use wasm_bindgen_futures::spawn_local;

    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable".to_owned());
    };
    let clipboard = window.navigator().clipboard();
    let (sender, receiver) = mpsc::unbounded();

    spawn_local(async move {
        let event = match wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&text)).await {
            Ok(_) => BrowserClipboardEvent::Copied,
            Err(_) => BrowserClipboardEvent::Error("Failed to write to clipboard".to_owned()),
        };
        let _ = sender.unbounded_send(event);
    });

    Ok(receiver)
}

#[cfg(not(target_arch = "wasm32"))]
/// Reports that clipboard writes are unavailable on non-wasm builds.
pub(crate) fn write_text_to_clipboard(
    _text: String,
) -> Result<mpsc::UnboundedReceiver<BrowserClipboardEvent>, String> {
    Err("Clipboard access is only available in the browser build".to_owned())
}

#[cfg(target_arch = "wasm32")]
/// Reads text from the browser clipboard and reports the async result through a receiver.
pub(crate) fn read_text_from_clipboard()
-> Result<mpsc::UnboundedReceiver<BrowserClipboardEvent>, String> {
    use wasm_bindgen_futures::spawn_local;

    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable".to_owned());
    };
    let clipboard = window.navigator().clipboard();
    let (sender, receiver) = mpsc::unbounded();

    spawn_local(async move {
        let event = match wasm_bindgen_futures::JsFuture::from(clipboard.read_text()).await {
            Ok(value) => BrowserClipboardEvent::Read(value.as_string().unwrap_or_default()),
            Err(_) => BrowserClipboardEvent::Error("Failed to read clipboard".to_owned()),
        };
        let _ = sender.unbounded_send(event);
    });

    Ok(receiver)
}

#[cfg(not(target_arch = "wasm32"))]
/// Reports that clipboard reads are unavailable on non-wasm builds.
pub(crate) fn read_text_from_clipboard()
-> Result<mpsc::UnboundedReceiver<BrowserClipboardEvent>, String> {
    Err("Clipboard access is only available in the browser build".to_owned())
}

#[cfg(target_arch = "wasm32")]
/// Opens the browser file picker for an image upload and returns a stream of upload results.
pub(crate) fn pick_and_upload_image_asset(
    node_id: String,
    parameter_name: String,
) -> Result<mpsc::UnboundedReceiver<BrowserImageAssetEvent>, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable".to_owned());
    };
    let Some(document) = window.document() else {
        return Err("Browser document is unavailable".to_owned());
    };
    let input = document
        .create_element("input")
        .map_err(|_| "Failed to create file input".to_owned())?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|_| "Failed to create file input".to_owned())?;
    input.set_type("file");
    input.set_accept("image/*");

    let (sender, receiver) = mpsc::unbounded();
    let onchange_sender = sender.clone();
    let onchange_input = input.clone();
    let onchange = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(files) = onchange_input.files() else {
            let _ = onchange_sender.unbounded_send(BrowserImageAssetEvent::Error(
                "No file was selected".to_owned(),
            ));
            return;
        };
        let Some(file) = files.get(0) else {
            let _ = onchange_sender.unbounded_send(BrowserImageAssetEvent::Error(
                "No file was selected".to_owned(),
            ));
            return;
        };

        let Ok(reader) = web_sys::FileReader::new() else {
            let _ = onchange_sender.unbounded_send(BrowserImageAssetEvent::Error(
                "Failed to create browser file reader".to_owned(),
            ));
            return;
        };

        let load_sender = onchange_sender.clone();
        let load_reader = reader.clone();
        let upload_node_id = node_id.clone();
        let upload_parameter_name = parameter_name.clone();
        let onload = Closure::once(Box::new(move |_event: web_sys::Event| {
            let Some(result) = load_reader.result().ok() else {
                let _ = load_sender.unbounded_send(BrowserImageAssetEvent::Error(
                    "Failed to read image file".to_owned(),
                ));
                return;
            };
            let array = js_sys::Uint8Array::new(&result);
            let bytes = array.to_vec();
            let upload_sender = load_sender.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let event = match upload_image_asset(bytes).await {
                    Ok(asset_id) => BrowserImageAssetEvent::Uploaded {
                        node_id: upload_node_id,
                        parameter_name: upload_parameter_name,
                        asset_id,
                    },
                    Err(message) => BrowserImageAssetEvent::Error(message),
                };
                let _ = upload_sender.unbounded_send(event);
            });
        }) as Box<dyn FnOnce(_)>);
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        let error_sender = onchange_sender.clone();
        let onerror = Closure::once(Box::new(move |_event: web_sys::Event| {
            let _ = error_sender.unbounded_send(BrowserImageAssetEvent::Error(
                "Failed to read image file".to_owned(),
            ));
        }) as Box<dyn FnOnce(_)>);
        reader.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        if reader.read_as_array_buffer(&file).is_err() {
            let _ = onchange_sender.unbounded_send(BrowserImageAssetEvent::Error(
                "Failed to start reading image file".to_owned(),
            ));
        }
    }) as Box<dyn FnMut(_)>);

    input.set_onchange(Some(onchange.as_ref().unchecked_ref()));
    onchange.forget();
    input.click();

    Ok(receiver)
}

#[cfg(target_arch = "wasm32")]
async fn upload_image_asset(bytes: Vec<u8>) -> Result<String, String> {
    use serde::Deserialize;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    #[derive(Deserialize)]
    struct UploadImageAssetResponse {
        asset_id: String,
    }

    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&js_sys::Uint8Array::from(bytes.as_slice()).into());

    let request = web_sys::Request::new_with_str_and_init("/api/assets/images", &init)
        .map_err(|_| "Failed to build upload request".to_owned())?;

    let Some(window) = web_sys::window() else {
        return Err("Browser window is unavailable".to_owned());
    };
    let response_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "Image upload request failed".to_owned())?;
    let response = response_value
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "Image upload response was invalid".to_owned())?;
    let response_text = JsFuture::from(
        response
            .text()
            .map_err(|_| "Failed to read image upload response".to_owned())?,
    )
    .await
    .map_err(|_| "Failed to read image upload response".to_owned())?
    .as_string()
    .unwrap_or_default();

    if !response.ok() {
        let fallback = format!("Image upload failed with status {}", response.status());
        return Err(if response_text.trim().is_empty() {
            fallback
        } else {
            response_text
        });
    }

    serde_json::from_str::<UploadImageAssetResponse>(&response_text)
        .map(|response| response.asset_id)
        .map_err(|error| format!("Failed to parse image upload response: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
/// Reports that image uploads via the browser file picker are unavailable on non-wasm builds.
pub(crate) fn pick_and_upload_image_asset(
    _node_id: String,
    _parameter_name: String,
) -> Result<mpsc::UnboundedReceiver<BrowserImageAssetEvent>, String> {
    Err("Image uploads are only available in the browser build".to_owned())
}
