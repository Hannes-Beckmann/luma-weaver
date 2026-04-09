# Integrations

This page summarizes the main external integrations available in Luma Weaver.

## WLED

The backend includes WLED discovery and output support.

Current behavior includes:

- mDNS-based discovery of WLED instances on the local network
- backend tracking of discovered instances
- frontend visibility into available devices so graph nodes can target them

Because discovery and device communication happen in the backend, WLED features are only available in the normal backend-hosted application.

## Home Assistant MQTT

Luma Weaver can expose values through Home Assistant MQTT `number` entities.

The backend is responsible for:

- broker configuration storage
- broker connections
- discovery payloads
- command/state topics
- synchronization for registered graph nodes

Reusable broker configs can be marked as Home Assistant brokers in the UI. Only those marked brokers are offered to Home Assistant nodes.

## Demo Mode Limitations

The GitHub Pages demo does not include live WLED or MQTT integration behavior. It is meant as an editor preview, not as a real integration host.

## Deployment Guidance

Use:

- Docker or Docker Compose when you want a standalone service with integrations
- the Home Assistant add-on when you want Home Assistant-hosted operation
- the browser demo when you only need a lightweight editor preview
