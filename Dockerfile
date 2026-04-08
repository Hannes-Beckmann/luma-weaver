# syntax=docker/dockerfile:1.7
ARG BUILD_FROM=debian:bookworm-slim
ARG BUILD_ARCH=amd64
ARG BUILD_VERSION=dev

FROM rust:bookworm AS chef-base

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        binaryen \
        build-essential \
        binutils \
        ca-certificates \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/tmp/cargo-target \
    rustup target add wasm32-unknown-unknown \
    && CARGO_TARGET_DIR=/tmp/cargo-target cargo install --locked cargo-chef \
    && CARGO_TARGET_DIR=/tmp/cargo-target cargo install --locked trunk \
    && CARGO_TARGET_DIR=/tmp/cargo-target cargo install --locked wasm-bindgen-cli

WORKDIR /app

FROM chef-base AS planner

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/backend/Cargo.toml crates/backend/Cargo.toml
COPY crates/frontend/Cargo.toml crates/frontend/Cargo.toml
COPY crates/shared/Cargo.toml crates/shared/Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

FROM chef-base AS builder

WORKDIR /app

COPY --from=planner /app/recipe.json /app/recipe.json
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY crates ./crates
COPY examples ./examples

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release -p backend

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cd /app/crates/frontend \
    && trunk build --release \
    && install -D /app/target/release/backend /out/backend \
    && strip /out/backend

FROM ${BUILD_FROM} AS runtime

ARG BUILD_ARCH
ARG BUILD_VERSION

LABEL \
    io.hass.version="${BUILD_VERSION}" \
    io.hass.type="addon" \
    io.hass.arch="${BUILD_ARCH}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /out/backend /app/backend
COPY --from=builder /app/crates/frontend/dist /app/frontend-dist
COPY run.sh /run.sh

RUN chmod a+x /run.sh \
    && mkdir -p /app/data

ENV APP_DATA_DIR=/app/data
ENV FRONTEND_DIST_DIR=/app/frontend-dist
ENV BACKEND_PORT=38123
ENV RUST_LOG=info

EXPOSE 38123
CMD ["/run.sh"]
