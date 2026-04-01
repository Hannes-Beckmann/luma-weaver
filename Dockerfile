# syntax=docker/dockerfile:1.7
ARG BUILD_FROM=debian:bookworm-slim
ARG BUILD_ARCH=amd64
ARG BUILD_VERSION=dev

FROM rust:bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        binaryen \
        build-essential \
        ca-certificates \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
    && cargo install --locked trunk

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release -p backend

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cd /app/crates/frontend \
    && trunk build --release \
    && cargo build --locked --release -p backend \
    && install -D /app/target/release/backend /out/backend

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
        wget \
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
