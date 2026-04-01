# luma-weaver

Rust workspace for a browser-hosted `egui` frontend and an HTTP/WebSocket backend.

## Layout

- `crates/shared`: shared serde models for messages exchanged over WebSocket.
- `crates/frontend`: `egui` web app compiled to WebAssembly with `trunk`.
- `crates/backend`: Axum server that exposes `/ws` and serves the frontend bundle.
- `.devcontainer`: development container with Rust, wasm target, and `trunk`.

## Development

Inside the devcontainer:

```bash
cargo check
trunk build crates/frontend/index.html --release
cargo run -p backend
```

The backend serves static frontend assets from `crates/frontend/dist` and exposes the WebSocket endpoint at `/ws`.

## Home Assistant add-on

This repository now also includes Home Assistant add-on metadata at the repository root:

- `repository.yaml`
- `config.yaml`
- `build.yaml`

For Home Assistant, add this repository as a custom add-on repository, install `Luma Weaver`, and access the UI on port `38123`.

