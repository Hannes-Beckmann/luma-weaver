# Troubleshooting

This page lists common issues when running Luma Weaver.

## The App Does Not Open

Check:

- that the backend is running
- that you are opening the correct port, usually `38123`
- that container or add-on logs do not show a startup failure

For Docker:

- confirm the container is running
- confirm the port mapping is present if you are not using host networking

## The Frontend Loads But Nothing Updates

In backend mode, the frontend depends on the WebSocket connection to `/ws`.

Check:

- that the backend is reachable
- that the frontend is served from the same backend instance
- that the browser console does not show repeated connection failures

## WLED Devices Do Not Appear

Check:

- that the host network can see the target devices
- that discovery is not blocked by container networking choices

Host networking is often the most reliable setup for LAN discovery.

## Home Assistant MQTT Features Do Not Work

Check:

- that a broker config exists
- that the broker is marked for Home Assistant use when required
- that the broker itself is reachable from the backend host

## A Graph Does Not Start

Check graph and node diagnostics first.

Common causes:

- invalid parameter combinations
- missing required integration configuration
- unsupported nodes in demo mode
- runtime compile failures
