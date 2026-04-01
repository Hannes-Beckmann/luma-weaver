use futures_channel::mpsc;
use shared::GraphExchangeFile;

/// Represents the outcome of a browser-managed graph import file read.
pub(crate) enum BrowserGraphFileEvent {
    Parsed(GraphExchangeFile),
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
