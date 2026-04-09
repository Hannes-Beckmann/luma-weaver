# Architecture

This page describes the current high-level structure of `luma-weaver`.

Use this page for crate boundaries, module responsibilities, and deployment-level structure.

- For long-lived backend service instances, see [backend-objects.md](backend-objects.md).
- For protocol flow and WebSocket behavior, see [protocol-runtime.md](protocol-runtime.md).
- For graph compilation and runtime execution internals, see [runtime-execution.md](runtime-execution.md).

## Workspace Layout

The Rust workspace is split into three crates:

- `crates/backend`: Axum server, WebSocket API, graph persistence, runtime execution, WLED and MQTT services
- `crates/frontend`: `egui`/`eframe` browser UI compiled to WebAssembly with `trunk`
- `crates/shared`: graph schema, protocol types, validation, and built-in node definitions shared by frontend and backend

Top-level deployment and packaging files:

- `Dockerfile`
- `docker-compose.yml`
- `config.yaml`, `build.yaml`, `repository.yaml`
- `.github/workflows/*`

## Runtime Shape

At runtime, the system works roughly like this:

1. The backend starts and builds shared application state.
2. The backend loads persisted graph documents, MQTT broker configs, and running-state metadata.
3. The backend serves the compiled frontend bundle and exposes `/ws` plus `/health`.
4. The frontend connects over WebSocket and requests graph metadata, node definitions, runtime statuses, WLED instances, and MQTT broker configs.
5. Graphs are compiled into runtime tasks and ticked at their configured execution frequency.
6. Runtime outputs are surfaced to integrations such as WLED and Home Assistant MQTT, and diagnostics/runtime updates are pushed back to the frontend.

## Backend Responsibilities

Important backend areas:

- `crates/backend/src/app/startup.rs`: application startup and service wiring
- `crates/backend/src/api/http.rs`: HTTP routes and static frontend serving
- `crates/backend/src/api/websocket/...`: WebSocket request routing and outbound messages
- `crates/backend/src/services/runtime/...`: runtime planning, compilation, execution, and management
- `crates/backend/src/services/graph_store/...`: persisted graph storage
- `crates/backend/src/services/mqtt...`: MQTT broker storage and Home Assistant MQTT behavior
- `crates/backend/src/services/wled/...`: WLED discovery and transport
- `crates/backend/src/node_runtime/...`: runtime node traits, conversions, registry, and concrete node implementations

## Frontend Responsibilities

Important frontend areas:

- `crates/frontend/src/app/...`: app state, graph actions, history, navigation, messaging
- `crates/frontend/src/controllers/...`: subscription and message synchronization with the backend
- `crates/frontend/src/editor_view/...`: graph editor model, viewer, widgets, and UI behavior
- `crates/frontend/src/dashboard_view.rs`: dashboard-level graph management and monitoring UX
- `crates/frontend/src/transport.rs`: frontend transport abstraction used by the frontend
- `crates/frontend/index.html`: frontend entry page used for normal startup and Pages route restoration
- `crates/frontend/404.html`: GitHub Pages fallback used to redirect deep links back through the app entrypoint

## Shared Responsibilities

Important shared areas:

- `crates/shared/src/protocol.rs`: frontend/backend WebSocket contract
- `crates/shared/src/graph/node_definition.rs`: node schema types, categories, parameter UI hints
- `crates/shared/src/graph/builtin_nodes.rs`: built-in node catalog
- `crates/shared/src/validation.rs`: graph validation against shared schema

## Node System

Each built-in node spans multiple layers:

1. Shared schema definition in `crates/shared/src/graph/builtin_nodes.rs`
2. Backend runtime implementation under `crates/backend/src/node_runtime/nodes/...`
3. Registry entry in `crates/backend/src/node_runtime/registry.rs`
4. Frontend editor behavior driven from shared definitions and rendered in `crates/frontend/src/editor_view/...`

This separation is intentional:

- `shared` defines what the node is
- `backend` defines how the node behaves at runtime
- `frontend` renders and edits it using the shared definition

The GitHub Pages demo reuses a portable subset of runtime behavior that can be compiled into the frontend's wasm build. That is why some backend code is now structured to compile on `wasm32` while backend-only services remain gated to native targets.

## Pages Preview Path

The repository also has a static GitHub Pages preview path for the frontend.

Architecturally, that path relies on:

- `index.html` restoring redirected deep links before the wasm app boots
- `404.html` redirecting unknown static routes back through the app entrypoint
- navigation code under `crates/frontend/src/app/navigation.rs` keeping the project base path stable

For the publishing workflow itself, see [workflows.md](workflows.md).

## Diagnostics

Diagnostics are produced by runtime and schema-aware backend paths and surfaced to the frontend through shared protocol/state.

Important practical rule:

- prefer one shared diagnostic model with different filtered views rather than separate dashboard/editor-only diagnostic systems

That keeps runtime failures, schema issues, and node-level detail aligned across the UI.

## Persistence And Compatibility

Graphs are persisted on disk and may outlive the current node catalog or schema.

That means changes involving these areas need extra care:

- node IDs
- parameter names
- default values
- graph import/export behavior
- asset references if assets are introduced

If a change may invalidate or remove data, the behavior should be surfaced explicitly to users rather than silently handled.
