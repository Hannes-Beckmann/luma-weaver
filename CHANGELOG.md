# Changelog

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
