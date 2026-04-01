use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use mdns_sd::{ServiceDaemon, ServiceEvent};
use shared::WledInstance;
use tokio::sync::RwLock;

use crate::messaging::event_bus::EventBus;

const WLED_SERVICE_TYPE: &str = "_wled._tcp.local.";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(1);

/// Spawns the blocking WLED discovery loop on a dedicated worker thread.
///
/// The loop keeps the shared instance list synchronized with mDNS discovery results and emits
/// change notifications through the event bus whenever the visible device set changes.
pub(crate) fn spawn_wled_discovery_task(
    instances: Arc<RwLock<Vec<WledInstance>>>,
    event_bus: EventBus,
) {
    tokio::task::spawn_blocking(move || run_discovery_loop(instances, event_bus));
}

/// Runs the blocking WLED mDNS discovery loop.
///
/// The loop tracks resolved services, removes devices when they disappear, opportunistically
/// fills in missing LED counts, and publishes a freshly sorted device list whenever it changes.
fn run_discovery_loop(instances: Arc<RwLock<Vec<WledInstance>>>, event_bus: EventBus) {
    let mdns = match ServiceDaemon::new() {
        Ok(mdns) => mdns,
        Err(error) => {
            tracing::error!(%error, "failed to start mDNS daemon for wled discovery");
            return;
        }
    };

    let receiver = match mdns.browse(WLED_SERVICE_TYPE) {
        Ok(receiver) => receiver,
        Err(error) => {
            tracing::error!(%error, service_type = WLED_SERVICE_TYPE, "failed to browse wled mDNS service");
            return;
        }
    };

    tracing::info!(
        service_type = WLED_SERVICE_TYPE,
        "wled mDNS discovery started"
    );

    let runtime = tokio::runtime::Handle::current();
    let mut discovered = HashMap::<String, WledInstance>::new();
    let mut last_published = Vec::<WledInstance>::new();
    loop {
        match receiver.recv_timeout(RECEIVE_TIMEOUT) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let instance = wled_instance_from_resolved_service(&info);
                discovered.insert(info.get_fullname().to_owned(), instance);
            }
            Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                discovered.remove(&fullname);
            }
            Ok(_) => {}
            Err(flume::RecvTimeoutError::Timeout) => {}
            Err(flume::RecvTimeoutError::Disconnected) => {
                tracing::warn!("wled discovery receiver disconnected");
                break;
            }
        }

        refresh_missing_led_counts(&mut discovered);

        let mut next = discovered.values().cloned().collect::<Vec<_>>();
        next.sort_by(|a, b| a.id.cmp(&b.id));
        if next == last_published {
            continue;
        }

        runtime.block_on(async {
            let mut guard = instances.write().await;
            *guard = next.clone();
        });
        event_bus.emit_wled_instances_changed(next.clone());
        last_published = next;
    }
}

/// Builds a `WledInstance` from a resolved mDNS service record.
///
/// The instance host is derived from the best address available in the service record, the name
/// prefers the advertised `name` property, and the LED count is fetched eagerly over HTTP when
/// possible.
fn wled_instance_from_resolved_service(info: &mdns_sd::ServiceInfo) -> WledInstance {
    let host = resolved_host(info);
    let name = info
        .get_properties()
        .get_property_val_str("name")
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| instance_name(info.get_fullname()));
    let led_count = fetch_led_count(&host);

    WledInstance {
        id: info.get_fullname().to_owned(),
        name,
        host,
        led_count,
    }
}

/// Resolves the best host string for a discovered WLED service.
///
/// IP addresses are preferred when present. The default HTTP port is omitted so the result can be
/// used directly in URLs and user-facing displays.
fn resolved_host(info: &mdns_sd::ServiceInfo) -> String {
    let hostname = info.get_hostname().trim_end_matches('.').to_owned();
    if let Some(ip) = info.get_addresses().iter().next() {
        if info.get_port() == 80 {
            return ip.to_string();
        }
        return format!("{}:{}", ip, info.get_port());
    }

    if info.get_port() == 80 {
        hostname
    } else {
        format!("{hostname}:{}", info.get_port())
    }
}

/// Derives a display name from the full mDNS service name.
fn instance_name(fullname: &str) -> String {
    fullname
        .split("._wled._tcp.local.")
        .next()
        .unwrap_or(fullname)
        .to_owned()
}

/// Refreshes missing LED counts for discovered devices.
///
/// Devices that already have a known LED count are left untouched.
fn refresh_missing_led_counts(discovered: &mut HashMap<String, WledInstance>) {
    for instance in discovered.values_mut() {
        if instance.led_count.is_some() {
            continue;
        }
        instance.led_count = fetch_led_count(&instance.host);
    }
}

/// Fetches the configured LED count from a WLED device over HTTP.
///
/// Returns `None` when the device cannot be reached, the response is malformed, or the JSON
/// payload does not contain a usable LED count field.
fn fetch_led_count(host: &str) -> Option<usize> {
    let address = if host.contains(':') {
        host.to_owned()
    } else {
        format!("{host}:80")
    };
    let resolved = address.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&resolved, Duration::from_millis(750)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(750)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(750)));

    let request = format!("GET /json/info HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;

    let response = read_http_response(&mut stream).ok()?;
    let body = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .and_then(|index| response.get(index + 4..))
        .unwrap_or(&response);
    let json = serde_json::from_slice::<serde_json::Value>(body).ok()?;

    json.get("leds")
        .and_then(|leds| leds.get("count"))
        .and_then(|count| count.as_u64())
        .map(|count| count as usize)
        .or_else(|| {
            json.get("info")
                .and_then(|info| info.get("leds"))
                .and_then(|leds| leds.get("count"))
                .and_then(|count| count.as_u64())
                .map(|count| count as usize)
        })
}

/// Reads an entire HTTP response from a blocking TCP stream.
///
/// When a `Content-Length` header is present, reading stops as soon as the expected number of
/// bytes has been received. Otherwise the function reads until EOF.
fn read_http_response(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut expected_total_len = None;

    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(response);
        }

        response.extend_from_slice(&buffer[..read]);

        if expected_total_len.is_none() {
            expected_total_len = expected_http_response_len(&response);
        }

        if let Some(expected_total_len) = expected_total_len {
            if response.len() >= expected_total_len {
                return Ok(response);
            }
        }
    }
}

/// Returns the total HTTP response length implied by the current header bytes.
///
/// The returned value includes both the header section and the response body. `None` is returned
/// until a full header block with a valid `Content-Length` field is available.
fn expected_http_response_len(response: &[u8]) -> Option<usize> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?
        + 4;
    let headers = std::str::from_utf8(&response[..header_end]).ok()?;

    for line in headers.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        let content_length = value.trim().parse::<usize>().ok()?;
        return Some(header_end + content_length);
    }

    None
}
