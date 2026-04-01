# Luma Weaver

This add-on runs the Luma Weaver backend and serves the bundled web UI on port `38123`.

## Installation

1. Add this repository to Home Assistant as a custom add-on repository.
2. Install the `Luma Weaver` add-on.
3. Start the add-on.
4. Open `http://homeassistant.local:38123/` or use the add-on's `Open Web UI` button.

## Notes

- The add-on uses Home Assistant's `/data` volume for persistent runtime data.
- It runs on the host network so LAN discovery features such as WLED discovery can work correctly.
- The only exposed configuration option today is the backend log level.
