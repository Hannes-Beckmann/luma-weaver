# Luma Weaver

<p align="center">
  <img src="assets/icon.svg" alt="Luma Weaver icon" width="180" />
</p>

Luma Weaver is a node-based lighting and animation editor built with Rust. It combines a browser-hosted `egui` frontend compiled to WebAssembly with an Axum backend that stores graph documents, executes them in real time, and serves the UI from the same process.

<p align="center">
  <img src="assets/example.gif" alt="Luma Weaver editor demo" width="900" />
</p>

Luma Weaver is designed for programmable LED workflows and reactive visuals, with built-in support for WLED discovery/output and Home Assistant MQTT number entities.

## Start Here

- Run it standalone with Docker or Docker Compose when you want persistent graphs and local integrations.
- Install it as a Home Assistant add-on when you want an always-on host with Home Assistant integration.
- Use the GitHub Pages demo when you want a lightweight preview of the editor.

Live GitHub Pages demo:

- https://hannes-beckmann.github.io/luma-weaver/

For longer-form docs, start with:

- [docs/index.md](docs/index.md): docs landing page
- [docs/user/README.md](docs/user/README.md): user guide entry point
- [docs/developer/architecture.md](docs/developer/architecture.md): system architecture
- [DOCS.md](DOCS.md): Home Assistant add-on notes

## What It Does

With Luma Weaver, you build animation graphs from reusable nodes and run them continuously. Nodes expose inputs and parameters so animations can be tuned precisely instead of being locked to fixed presets.

Current building blocks include:

- animation and pattern nodes such as linear sweep, circle sweep, plasma, twinkle stars, bouncing balls, and level bar
- math and signal nodes such as constants, add/subtract/multiply/divide, min/max/clamp, abs, map range, binary select, power/root/exp/log, rounding, and signal generator
- frame and color processing nodes such as tint, mix, blur, Laplacian edge/detail filtering, mask, brightness, and filters
- runtime/debug nodes such as plot, display, and a WLED dummy display
- network nodes for WLED output, WLED frame input, audio FFT receive, and Home Assistant MQTT numbers

## Quick Start

### Standalone With Docker Compose

```bash
docker compose up --build
```

Then open:

```text
http://localhost:38123/
```

The compose setup persists application data in a named Docker volume mounted at `/app/data`.

### Standalone With Docker

Build:

```bash
docker build -t luma-weaver .
```

Run:

```bash
docker run --rm -p 38123:38123 -v luma-weaver-data:/app/data luma-weaver
```

Useful environment variables:

- `APP_DATA_DIR`: persisted graphs, MQTT broker configs, and runtime state
- `FRONTEND_DIST_DIR`: compiled frontend asset directory
- `BACKEND_PORT`: HTTP/WebSocket server port, default `38123`
- `RUST_LOG`: backend log level such as `info` or `debug`

### Home Assistant Add-on

Add this repository as a custom add-on repository in Home Assistant:

```text
https://github.com/Hannes-Beckmann/luma-weaver
```

Then install `Luma Weaver`, start it, and open the web UI from Home Assistant or directly on port `38123`.

### Local Development

```bash
cargo check
cd crates/frontend && trunk build --release
cargo run -p backend
```

Then open:

```text
http://localhost:38123/
```

## Documentation Map

### User docs

- [docs/user/getting-started.md](docs/user/getting-started.md)
- [docs/user/editor.md](docs/user/editor.md)
- [docs/user/deployment.md](docs/user/deployment.md)
- [docs/user/demo-mode.md](docs/user/demo-mode.md)
- [docs/user/integrations.md](docs/user/integrations.md)
- [docs/user/node-catalog.md](docs/user/node-catalog.md)
- [docs/user/troubleshooting.md](docs/user/troubleshooting.md)

### Developer docs

- [docs/developer/architecture.md](docs/developer/architecture.md)
- [docs/developer/backend-objects.md](docs/developer/backend-objects.md)
- [docs/developer/protocol-runtime.md](docs/developer/protocol-runtime.md)
- [docs/developer/runtime-execution.md](docs/developer/runtime-execution.md)
- [docs/developer/contributing.md](docs/developer/contributing.md)
- [docs/developer/node-authoring.md](docs/developer/node-authoring.md)
- [docs/developer/workflows.md](docs/developer/workflows.md)

## Basic Architecture

The workspace is split into three crates:

- `crates/frontend`: the browser UI, built with `eframe`/`egui` and compiled to WebAssembly with `trunk`
- `crates/backend`: the HTTP/WebSocket server, graph storage, graph runtime, WLED discovery, and MQTT/Home Assistant integration
- `crates/shared`: shared protocol types, graph schema, validation, and built-in node definitions used by both frontend and backend

In backend mode, the flow looks like this:

1. The browser loads the WebAssembly frontend from the backend.
2. The frontend opens a WebSocket connection to `/ws`.
3. The backend loads stored graph documents and running-state metadata from disk.
4. Graphs are compiled into executable runtime tasks and ticked at their configured frequency.
5. Runtime outputs can be sent to integrations like WLED or exposed through Home Assistant MQTT entities.

Important backend endpoints:

- `/`: serves the frontend bundle
- `/ws`: frontend/backend messaging
- `/health`: health check endpoint for containers and add-on watchdogs

## Frontend Demo Pages

The repository includes a manual GitHub Pages workflow at `.github/workflows/publish-pages.yml`.

Running that workflow builds:

- the frontend preview at `/luma-weaver/`
- the `mdBook` docs site at `/luma-weaver/docs/`
- the generated Rust reference from `cargo doc` at `/luma-weaver/reference/`

The published frontend preview runs entirely in the browser without a backend, so backend-dependent features are intentionally unavailable there.

For local docs work, the `mdBook` project lives under `docs-book/` and reads its content from `docs/`.

Current live URL:

- https://hannes-beckmann.github.io/luma-weaver/

## Persistence And Integrations

Luma Weaver stores runtime data on disk. This includes:

- graph documents
- MQTT broker configuration
- the set of graphs that should resume in the running state on restart

In standalone Docker, this should be backed by a volume. In Home Assistant, it is stored in the add-on data directory.

The backend includes:

- mDNS-based WLED discovery and output support
- Home Assistant MQTT number entity support
- graph-scoped runtime and diagnostics state surfaced to the frontend

## Status

The project already contains a substantial runtime and node catalog, but it is still evolving. If you deploy it, expect the graph format, node set, and integrations to continue growing.



