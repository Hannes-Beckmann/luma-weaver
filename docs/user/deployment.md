# Deployment

This page describes the main ways to run Luma Weaver outside the source tree.

Use this as the canonical user-facing deployment page.

- Standalone and Home Assistant deployment choices live here.
- The GitHub Pages preview is documented here first and in more detail on [demo-mode.md](demo-mode.md).

## Choose A Deployment Style

There are three practical ways to use Luma Weaver today:

- Docker Compose for the quickest standalone setup
- Docker for more direct container control
- Home Assistant add-on for Home Assistant-hosted operation

Use the GitHub Pages demo only when you want a lightweight browser preview rather than a full deployment.

## Docker Compose

The quickest standalone path is:

```bash
docker compose up --build
```

Then open:

```text
http://localhost:38123/
```

Important behavior:

- the frontend is served by the backend
- application data is persisted in the `luma-weaver-data` Docker volume
- the compose file uses `network_mode: host`

Host networking is useful for LAN-based discovery features such as WLED discovery.

## Docker

Build the image:

```bash
docker build -t luma-weaver .
```

Run it:

```bash
docker run --rm -p 38123:38123 -v luma-weaver-data:/app/data luma-weaver
```

Useful environment variables:

- `APP_DATA_DIR`: persisted graphs, MQTT broker configs, and runtime state
- `FRONTEND_DIST_DIR`: frontend asset directory served by the backend
- `BACKEND_PORT`: HTTP and WebSocket port, default `38123`
- `RUST_LOG`: backend log level such as `info` or `debug`

## Home Assistant Add-on

To install Luma Weaver as a custom Home Assistant add-on:

1. Add `https://github.com/Hannes-Beckmann/luma-weaver` as a custom add-on repository.
2. Install the `Luma Weaver` add-on.
3. Start it.
4. Open the web UI from Home Assistant or directly on port `38123`.

Current add-on behavior:

- uses Home Assistant's `/data` volume for persistence
- runs on the host network
- exposes the web UI on port `38123`
- uses `/health` as the watchdog endpoint

## Demo Mode On GitHub Pages

The browser-only demo is published separately from backend deployments.

Live demo:

- https://hannes-beckmann.github.io/luma-weaver/

The Pages demo is useful for previewing the editor and portable runtime behavior, but it does not replace a full backend deployment.

For URL behavior, limitations, and publishing details, see [demo-mode.md](demo-mode.md).

## Persistence Expectations

Backend mode persists:

- graph documents
- runtime resume state
- MQTT broker configuration

If you want work to survive restarts, make sure your deployment keeps `/app/data` or the equivalent add-on data path persistent.
