# AGENTS.md

## Purpose

This file gives coding agents the project-specific context needed to work effectively in `luma-weaver`.

Use it alongside [README.md](README.md) and [DOCS.md](DOCS.md). Prefer concrete commands, small changes, and verification that matches the files you touched.

## Project Overview

Luma Weaver is a Rust workspace for a node-based lighting and animation editor.

- `crates/backend`: Axum HTTP/WebSocket server, graph persistence, runtime executor/compiler, WLED discovery/output, MQTT/Home Assistant integration
- `crates/frontend`: `egui`/`eframe` WebAssembly UI built with `trunk`
- `crates/shared`: graph schema, protocol types, validation, and node definitions shared by frontend and backend

Runtime shape:

1. The backend serves the frontend bundle.
2. The frontend connects to `/ws`.
3. Graph documents are loaded from disk and compiled into runtime tasks.
4. Runtime outputs drive integrations like WLED and Home Assistant MQTT.

## Repo Map

- `crates/shared/src/graph`: graph document model, import/export, node definitions
- `crates/shared/src/protocol.rs`: frontend/backend protocol
- `crates/backend/src/api`: HTTP and WebSocket endpoints
- `crates/backend/src/services/runtime`: graph planning, compilation, execution
- `crates/backend/src/node_runtime/nodes`: concrete node implementations
- `crates/backend/src/services/wled`: WLED discovery and DDP output
- `crates/backend/src/services/mqtt*`: MQTT broker storage and Home Assistant integration
- `crates/frontend/src/app`: app-level state, history, graph actions, messaging
- `crates/frontend/src/editor_view`: graph editor UI and widgets
- `docs`: audience-based project documentation for users and developers
- `.github/workflows`: CI, release preflight, publish workflows
- `config.yaml`, `build.yaml`, `repository.yaml`: Home Assistant add-on metadata

## Toolchain And Setup

The workspace uses stable Rust with `clippy`, `rustfmt`, and the `wasm32-unknown-unknown` target. See [rust-toolchain.toml](rust-toolchain.toml).

Common commands:

- `cargo fmt --all`
- `cargo test -p shared -p backend --locked`
- `cargo test -p frontend --locked`
- `cargo run -p backend`
- `trunk build --release` from `crates/frontend`
- `docker compose up --build`

## Working Rules

- Keep changes scoped to the user request. Do not mix feature work, refactors, and release/config churn unless required.
- When working from GitHub issues, check the issue body and comments for explicit prerequisite/dependency notes before implementing. If an issue depends on another issue, follow that sequencing unless the user explicitly overrides it.
- Prefer fixing the underlying shared contract in `shared` when frontend and backend disagree.
- For new nodes or node changes, check all three layers:
  - shared node definition / graph schema
  - backend runtime implementation
  - frontend editor/add-menu/parameter UI
- Preserve persisted graph compatibility unless the task explicitly allows a breaking change. If you must break compatibility, call it out clearly in the PR notes.
- Treat `config.yaml`, Docker, and workflow changes as release-sensitive. Keep them minimal and verify carefully.
- Do not invent new infrastructure or project structure when an existing pattern already exists nearby.
- Update docs in `docs`

## Event-Driven TODOs

Use this section as a maintenance checklist. When one of these events happens, explicitly consider the matching follow-up work before finishing.

### If a file is created, deleted, renamed, or its responsibility changes

- Update this `AGENTS.md` when the repo map, recommended commands, or event checklists are now stale.
- Keep contributor-facing docs consistent:
  - [README.md](README.md) for architecture, development commands, env vars, endpoints, deployment, or user-visible capabilities
  - [DOCS.md](DOCS.md) for Home Assistant add-on behavior and operator-facing notes
- If the change affects release or packaging behavior, re-check:
  - `.github/workflows/*.yml`
  - `Dockerfile`
  - `docker-compose.yml`
  - `config.yaml`
  - `build.yaml`
  - `repository.yaml`

### If you add, remove, rename, or materially change a node

- Update the shared node catalog in `crates/shared/src/graph/node_catalog.rs`.
- Update or add the backend runtime implementation under `crates/backend/src/node_runtime/nodes/...`.
- Register the runtime in `crates/backend/src/node_runtime/registry.rs`.
- Verify the frontend editor picks it up correctly through the shared node definitions, especially:
  - `crates/frontend/src/editor_view/viewer.rs` for add-menu grouping and search behavior
  - `crates/frontend/src/editor_view/model.rs` for defaults and persisted-node handling
  - `crates/frontend/src/editor_view/widgets.rs` when a new parameter UI hint or custom widget behavior is needed
- Add or update tests in the touched runtime module and any registry/schema coverage that is affected.
- Document the node if it changes the public node catalog:
  - update [README.md](README.md) when the catalog summary or capability list is now inaccurate
  - update `examples/Example.animation-graph.json` if the new node should appear in the sample graph or if a removed/renamed node breaks the example
- If the node ID, parameter names, or persisted graph shape change, call out compatibility impact in the PR and use breaking-change commit syntax when appropriate.

### If you change the shared protocol or WebSocket message flow

- Update `crates/shared/src/protocol.rs`.
- Update the backend send/receive path under `crates/backend/src/api/websocket/...`.
- Update the frontend handling in:
  - `crates/frontend/src/controllers/messages.rs`
  - `crates/frontend/src/controllers/subscriptions.rs`
  - `crates/frontend/src/transport.rs`
  - `crates/frontend/src/app/messaging.rs`
- Add or update protocol tests where practical.
- Update [README.md](README.md) if externally visible endpoints, message semantics, or runtime behavior changed.

### If you change persisted graph format, schema defaults, validation, or import/export behavior

- Check `crates/shared/src/graph/...` and `crates/shared/src/validation.rs` for any corresponding schema and validation updates.
- Update backend load/store behavior if persistence expectations changed.
- Verify old documents still load when backward compatibility is expected.
- Update `examples/Example.animation-graph.json` when the canonical example no longer matches current schema or defaults.
- Document compatibility notes in the PR. If migration is required, say so explicitly.

### If you add or change an environment variable, port, endpoint, or container path

- Update the implementation source:
  - backend env handling in `crates/backend/src/main.rs`, `crates/backend/src/app/startup.rs`, or `crates/backend/src/api/http.rs`
  - container settings in `Dockerfile` and `docker-compose.yml`
  - Home Assistant settings in `config.yaml`
- Keep [README.md](README.md) environment-variable and deployment sections in sync.
- Keep [DOCS.md](DOCS.md) in sync when add-on behavior changes.
- Re-check `/health`, `/ws`, `BACKEND_PORT`, `APP_DATA_DIR`, and `FRONTEND_DIST_DIR` references if related behavior changed.

### If you change Home Assistant add-on metadata or release packaging

- Keep `config.yaml`, `build.yaml`, `repository.yaml`, `Dockerfile`, and the workflows under `.github/workflows` consistent with each other.
- Keep [README.md](README.md) and [DOCS.md](DOCS.md) deployment instructions aligned with the new behavior.
- If you change versioning or release behavior, also review `.github/release.yml`, [CHANGELOG.md](CHANGELOG.md), and tag/version assumptions in the workflows.

### If graphs gain assets or asset-like stored content

- Update persistence assumptions and storage behavior in backend graph/data handling.
- Keep [README.md](README.md) and any asset-related docs in sync with how assets are added, stored, referenced, and loaded.
- Revisit graph size/statistics assumptions if storage footprint is surfaced to users.
- Check whether import/export, copy/paste, and graph portability need asset-aware behavior.

### If you add or rename user-facing text or configuration labels

- Update `translations/en.yaml` when the text belongs to add-on configuration or other translation-backed UI.
- Keep README examples and terminology consistent with the new naming.
- When renaming a visible node, setting, or feature, check the sample graph and docs for stale old names.

## Verification

Match verification to the change:

- Shared model/protocol changes: `cargo test -p shared -p backend --locked`
- Backend/runtime/integration changes: `cargo test -p backend --locked`
- Frontend UI/state changes: `cargo test -p frontend --locked` and `trunk build --release` from `crates/frontend`
- Workflow, Docker, or add-on metadata changes: read the matching workflow in `.github/workflows` and verify any referenced commands or files still line up

Before finishing, run the smallest meaningful command set that covers the edited files. If you could not run a relevant check, say so explicitly.

## Testing Expectations

- Add or update tests when changing runtime logic, protocol behavior, graph persistence, or non-trivial UI state logic.
- Prefer unit tests near the changed module when that pattern already exists.
- Do not claim manual verification you did not perform.

## Git And PR Workflow

Follow the existing branch naming style visible in recent repository history:

- `feat/<slug>` for features
- `fix/<slug>` for bug fixes
- `ci/<slug>` for workflow/build pipeline changes
- `chore/<slug>` or `docs/<slug>` for maintenance/documentation

Commit and PR guidance:

- Keep commits focused and reviewable.
- Use Conventional Commit subjects so changelog generation from commits on `main` is reliable.
- Format the first line as `<type>(<optional-scope>): <imperative summary>`.
- Allowed primary types:
  - `feat`: user-facing features
  - `fix`: bug fixes and regressions
  - `docs`: documentation-only changes
  - `ci`: workflow, Docker, release, or build pipeline changes
  - `chore`: maintenance that does not change product behavior
  - `refactor`: internal restructuring without intended behavior change
  - `test`: test-only changes
- Keep the subject line changelog-ready:
  - lowercase type
  - imperative summary
  - no trailing period
  - no issue number prefix
- Good examples:
  - `feat(nodes): add UDP raw mode to WLED sink`
  - `fix(runtime): preserve graph state when websocket reconnects`
  - `docs(readme): clarify frontend build prerequisites`
  - `ci(docker): exclude frontend from default cargo-chef members`
- If a change is breaking, add `!` after the type or scope and describe the break in the footer, for example:
  - `feat(graph)!: remove legacy sinus node`
  - `BREAKING CHANGE: persisted graphs using math.sinus must be migrated`
- PR descriptions should include:
  - summary
  - why
  - testing
  - compatibility notes when graphs, config, or deployment behavior change
- If changes are squash-merged, the PR title must follow the same Conventional Commit format because that title may become the commit on `main`.

Open separate branches and PRs for independent issues. Only parallelize work when the write scopes do not overlap materially.

## Release And Packaging Notes

- `config.yaml` version must stay aligned with release tags of the form `vX.Y.Z`.
- CI validates add-on metadata and runs Rust tests plus a frontend trunk build.
- Pushes and manual CI runs also execute a Docker smoke build.
- Tagged releases publish multi-arch GHCR images and a GitHub release.
- `.github/release.yml` groups GitHub release notes by PR labels (`feat`, `fix`, `docs`, `ci`, `chore`), while any future changelog generated from commits on `main` should rely on the Conventional Commit subject line format above.

If you touch release-sensitive files, inspect:

- `.github/workflows/ci.yml`
- `.github/workflows/release-preflight.yml`
- `.github/workflows/publish-addon.yml`
- `config.yaml`
- `Dockerfile`

## When In Doubt

- Read the nearest existing implementation before adding a new pattern.
- Prefer the smallest change that preserves current architecture.
- Surface uncertainty early when a task may affect graph compatibility, runtime behavior, or release packaging.
