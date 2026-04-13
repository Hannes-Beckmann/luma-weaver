# Editor Guide

This page describes the main frontend workflow in Luma Weaver.

## Dashboard

The dashboard is the graph-management view.

From here you can typically:

- create a graph
- open a graph in the editor
- rename a graph
- delete a graph
- inspect runtime state
- inspect graph-level diagnostics

The dashboard is also the safest place to orient yourself when you have multiple saved graphs.

## Editor

The editor opens one graph document at a time.

Inside the editor you can:

- add nodes
- connect nodes
- change parameter values
- select and move multiple nodes together
- copy and paste selected nodes
- inspect runtime values for debug-oriented nodes
- run the graph
- step or pause the runtime
- inspect diagnostics

## Common Editing Workflow

The usual graph-editing loop looks like this:

1. Open a graph from the dashboard.
2. Add nodes from the node menu.
3. Connect outputs to compatible inputs.
4. Adjust parameters and defaults.
5. Start the runtime and inspect the result.
6. Use diagnostics or debug-oriented nodes when behavior is not what you expect.

## Selection And Clipboard

The editor supports multi-selection on the node canvas.

You can:

- select multiple nodes
- drag one selected node to move the full selected group
- copy the selected nodes to the system clipboard
- paste copied nodes into the current graph
- copy from one graph or browser window and paste into another

Selection gestures:

- `Shift`+click: add a node to the current selection
- `Ctrl`+click or `Cmd`+click: remove a node from the current selection
- `Shift`+drag: box-select multiple nodes
- `Ctrl`+click or `Cmd`+click on empty canvas space: clear the current selection

When you copy a selection:

- only the selected nodes are included
- connections between the selected nodes are preserved
- connections to nodes outside the selection are not copied

When you paste:

- pasted nodes receive fresh node ids
- copied internal connections are restored
- the paste location prefers the current canvas pointer position when the canvas is hovered
- unsupported node types are skipped instead of failing the whole paste

## Dashboard Actions

From the dashboard you can currently:

- create a graph document
- import a graph
- export a graph
- manage MQTT broker configs
- rename a graph
- remove a graph
- start, pause, or stop a graph without opening it
- open graph-level diagnostics

The `Open` action switches from the dashboard into the editor for the selected graph.

## Editor Header Actions

When a graph is open, the editor header gives you graph-level controls such as:

- run
- pause
- step by a configurable number of ticks
- stop
- copy selected nodes
- paste copied nodes
- export
- focus the canvas
- reload the selected graph from storage
- return to the dashboard
- undo and redo

These controls let you move between editing, runtime inspection, and recovery without leaving the graph.

## Keyboard Shortcuts

Useful editor shortcuts:

- `Ctrl+C` or `Cmd+C`: copy the currently selected nodes
- `Ctrl+V` or `Cmd+V`: paste copied nodes into the open graph
- `Ctrl+Z` or `Cmd+Z`: undo
- `Ctrl+Y` or `Cmd+Shift+Z`: redo

## Imports And Exports

Graph import and export are available in backend mode.

Import behavior includes collision handling when a graph with the same id already exists. Export lets you save a graph document for sharing or backup.

The GitHub Pages demo is not intended to provide the full import/export and persistence story.

## Graph Lifecycle

Each graph has both an editable document and a runtime state.

Common lifecycle actions:

- `Open`: loads the graph into the editor
- `Start`: starts or resumes execution
- `Pause`: pauses execution while preserving runtime state
- `Step`: advances the graph a limited number of ticks
- `Stop`: stops the runtime

## Saving And Persistence

In normal use, graph changes are persisted on disk through the backend.

Persisted data includes:

- graph documents
- runtime resume state
- MQTT broker configuration

In demo mode, persistence is intentionally limited because the demo runs without a backend.

## Diagnostics

Diagnostics are shown both at graph level and node level.

They are useful when:

- a node is misconfigured
- a runtime compile step fails
- an integration is not available
- the current graph contains invalid or unsupported state

Use diagnostics to understand why a graph does not start cleanly or why a node is not behaving as expected.

## Runtime Inspection

Some nodes expose runtime-oriented previews or values, such as:

- `Plot`
- `Display`
- debug-oriented frame previews

These are especially useful when tuning math, signal, or animation graphs before sending output to a real integration target.
