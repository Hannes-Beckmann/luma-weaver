#!/usr/bin/env bash
set -euo pipefail

rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown

if ! command -v trunk >/dev/null 2>&1; then
    cargo install trunk --locked
fi

