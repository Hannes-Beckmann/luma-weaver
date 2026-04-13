# Node Catalog Overview

This page gives a user-facing overview of the node families currently present in Luma Weaver.

It is not intended to replace source-level schema definitions. Instead, it helps users understand the kinds of nodes available when building graphs.

## Inputs

These nodes introduce values or external sources into a graph.

Examples:

- Float Constant
- Color Constant
- WLED Sink
- Audio FFT Receiver
- Home Assistant MQTT Number

Use these when a graph needs a starting value, a live input, or an external control source.

## Generators

These nodes create animated frame output procedurally.

Examples:

- Linear Sweep
- Circle Sweep
- Plasma
- Twinkle Stars
- Bouncing Balls
- Level Bar
- Solid Frame

Use these when you want to synthesize patterns rather than consume external frame data.

## Math

These nodes transform scalar values and tensors.

Examples:

- Add Float
- Subtract Float
- Multiply Float
- Divide Float
- Min/Max
- Clamp
- Power, Root, Exponential, Log
- Map Range
- Round
- Signal Generator

Use these to derive control signals and modulate animation parameters.

## Frame Operations

These nodes manipulate frame or color data.

Examples:

- Tint Frame
- Mask Frame
- Mix Color
- Alpha Over
- Frame Brightness
- Scale Color

Use these to combine, recolor, or reshape visual output.

## Temporal Filters

These nodes work across time.

Examples:

- Delay
- Differentiate
- Integrate
- Fade
- Moving Average
- Moving Median

Use these when you want smoothing, trailing, accumulation, or other history-aware behavior.

## Spatial Filters

These nodes process frame data in image-like ways.

Examples:

- Box Blur
- Gaussian Blur
- Median Filter
- Laplacian Filter

Use these to soften, sharpen, or otherwise filter generated frame output.

## Outputs

These nodes drive runtime targets or expose final values.

Examples:

- Display
- Plot
- WLED Target

Use these when you want to inspect, visualize, or send graph output somewhere useful.

## Debug

These nodes help inspect graph state while building or testing.

Examples:

- WLED Dummy Display
- runtime-oriented preview nodes such as Plot and Display, depending on how you use them

Use these to understand what a graph is doing before wiring it into real hardware or integrations.

## Notes About Availability

Not every node is available in the GitHub Pages demo.

If a graph does not start cleanly there, check whether it depends on backend-only integrations or runtime behavior.
