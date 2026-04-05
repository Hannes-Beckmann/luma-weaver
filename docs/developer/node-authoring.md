# Node Authoring

This page describes the normal workflow for adding or changing built-in nodes in `luma-weaver`.

## Mental Model

A built-in node is not implemented in just one place.

Most node changes require coordinated updates in:

1. shared schema
2. backend runtime
3. backend registry
4. frontend editor behavior
5. tests
6. docs/examples when user-visible behavior changes

## Core Files

Shared schema:

- `crates/shared/src/graph/node_definition.rs`
- `crates/shared/src/graph/builtin_nodes.rs`

Backend runtime:

- `crates/backend/src/node_runtime/nodes/...`
- `crates/backend/src/node_runtime/registry.rs`

Frontend editor:

- `crates/frontend/src/editor_view/viewer.rs`
- `crates/frontend/src/editor_view/model.rs`
- `crates/frontend/src/editor_view/widgets.rs`

## Add A New Built-In Node

Typical sequence:

1. Add or reuse a `NodeTypeId` when needed.
2. Add the node definition to `crates/shared/src/graph/builtin_nodes.rs`.
3. Implement the runtime node in the appropriate backend module.
4. Register it in `crates/backend/src/node_runtime/registry.rs`.
5. Verify the frontend add-menu placement and parameter rendering.
6. Add or update tests.
7. Update docs/examples when the public node catalog changed.

## Change An Existing Node

Review all of these before finishing:

- node ID stability
- parameter names and defaults
- accepted input/output kinds
- category/menu placement
- runtime diagnostics
- persisted graph compatibility

If the node is renamed, removed, or materially reworked, also review unknown-node and graph migration implications.

Built-in node IDs should use the same category prefix as the add-menu taxonomy and backend module
layout, for example:

- `inputs.*`
- `generators.*`
- `math.*`
- `frame_operations.*`
- `temporal_filters.*`
- `spatial_filters.*`
- `outputs.*`
- `debug.*`

## Defaults And Diagnostics

Defaults should represent a clean first-use state.

Avoid situations where a newly inserted node:

- immediately clamps its defaults
- emits warnings by default
- starts in a confusing invalid state unless it is genuinely missing required external configuration

When shared defaults and runtime normalization differ, bring them back into alignment.

## Mode-Heavy Nodes

If a node introduces multiple modes with different relevant controls:

- prefer shared-schema-driven mode selection
- use conditional parameter visibility when available
- avoid exposing irrelevant controls in every mode

Examples of mode-heavy work include different transport/protocol modes, smoothing modes, or source modes like URL versus upload.

## Category Placement

Node category placement affects both users and contributors.

When adding a node:

- verify it is discoverable in the editor menu
- verify the category choice remains coherent with the current node menu structure
- note that menu organization and implementation organization are related but not always identical
- keep the stable node type prefix aligned with the implementation family used in the codebase

The current add menu uses the user-facing category as the top-level grouping and then sub-groups
nodes by the implementation prefix in the stable node type ID, such as `core.`, `math.`,
`color.`, `anim.`, `net.`, or `debug.`.

That means a node can still live in a user-friendly category like `Inputs` or `Outputs` while the
menu also shows contributors which code area it comes from.

## Compatibility And Unknown Nodes

Persisted graphs can contain node types the current build no longer understands.

That means node work should consider:

- unknown-node warnings
- runtime compile/start diagnostics
- whether destructive cleanup happens on load or edit

Do not assume schema changes are harmless just because the runtime compiles.

## Verification

Use the smallest meaningful check set for the node you touched:

- `cargo test -p shared -p backend --locked`
- `cargo test -p backend --locked`
- `cargo test -p frontend --locked` when UI behavior changed
- `trunk build --release` from `crates/frontend` when frontend/editor rendering changed

## Docs And Examples

Update these when relevant:

- [../../README.md](../../README.md)
- [../../AGENTS.md](../../AGENTS.md) if contributor guidance changed
- `examples/Example.animation-graph.json` when the sample graph should demonstrate the new behavior or no longer matches the catalog
