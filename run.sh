#!/usr/bin/env bash
set -euo pipefail

if [[ -f /usr/lib/bashio/bashio.sh ]]; then
    # Home Assistant add-on environment
    # shellcheck disable=SC1091
    source /usr/lib/bashio/bashio.sh
    export RUST_LOG="${RUST_LOG:-$(bashio::config 'log_level')}"
else
    export RUST_LOG="${RUST_LOG:-info}"
fi

export APP_DATA_DIR="${APP_DATA_DIR:-/data}"
export FRONTEND_DIST_DIR="${FRONTEND_DIST_DIR:-/app/frontend-dist}"
export BACKEND_PORT="${BACKEND_PORT:-38123}"

exec /app/backend
