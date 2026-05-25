# Changelog

## 0.3.0

### Features

- Added spatial LED layout workflows, including optional `Spatial3d` sink layouts and mapped-frame graph paths.
- Added a global 3D sink preview with runtime-fed previews for spatial WLED targets and dummy displays.
- Added the `Transform` node for spatial frame translation and rotation workflows.
- Added uploaded layout asset support for spatial layout configuration.
- Added Home Assistant MQTT graph controls for starting, stopping, and observing graph execution state.

### Fixes

- Preserved mapped-frame runtime previews over binary WebSocket transport.
- Fixed WLED target runtime preview forwarding so spatial target previews reach the frontend.
- Fixed layout upload validation to return client errors for malformed CSV/JSON uploads.
- Fixed uploaded asset state synchronization in the editor so selected files appear immediately.
- Fixed layout asset replacement cleanup so orphaned layout uploads are deleted when they are no longer referenced.
- Added editor feedback for rejected connections, including contextual guidance for invalid graph wires.

### Documentation And Maintenance

- Expanded architecture and integration docs for spatial layouts, mapped frames, and Home Assistant graph controls.

## 0.2.0

### Breaking Changes

- Aligned built-in node IDs with the current node taxonomy. Persisted graphs using the previous built-in node IDs may need migration.
- Added audio FFT receiver modes with updated audio input behavior.

### Features

- Added WLED UDP raw input support.
- Reworked the node add-menu hierarchy.
- Added Home Assistant broker flags and graph-scoped Home Assistant MQTT number grouping.
- Added conditional visibility for node parameters.
- Expanded float math, tensor, frame, and filter node workflows.
- Added moving median, differentiate, integrate, Laplacian, binary select, uploaded image source, and disable-input node capabilities.
- Added selection clipboard support in the graph editor.
- Added browser demo mode and GitHub Pages publishing for the frontend preview.
- Added tensor filter workflows and the PDE heat/wave example graph.

### Fixes

- Improved long one-dimensional frame previews and frame input layout stability.
- Surfaced runtime diagnostics from the dashboard.
- Restored GitHub Pages deep links.
- Fixed frontend clock handling for WebAssembly demo mode.
- Kept color gradient stops independently editable.

### Documentation And Maintenance

- Added repository guidance for coding agents.
- Scaffolded audience-based documentation and published docs/reference pages.
- Improved CI, Docker cache reuse, cargo-chef planning, frontend rustdoc builds, and release note automation.
- Renamed node catalog terminology and aligned backend node directories with menu categories.

## 0.1.0

- Initial Home Assistant add-on packaging.
