# syntax=docker/dockerfile:1
# The wst service on Alpine, with ffprobe. Built for the image's platform on
# the build host, without emulation: cargo-zigbuild compiles natively and
# links for the target, so a multi-platform build takes one compiler run per
# platform, not an emulated one.
#
#   docker build -t wanshitong .
#   docker build --platform linux/amd64,linux/arm64 -t wanshitong .
#
# A release puts binaries it already built in place of the `build` stage, laid
# out as it lays them out, `<dir>/<amd64|arm64>/wst`:
#
#   docker build --build-context build=dist -t wanshitong .

FROM --platform=$BUILDPLATFORM rust:1.98.1-slim-bookworm AS build
ARG TARGETARCH

# The build script compiles TypeScript with tsc, and downloads the Tailwind
# CLI itself. cargo-zigbuild's wheel brings zig with it.
RUN apt-get update \
    && apt-get install -y --no-install-recommends nodejs npm python3-pip \
    && rm -rf /var/lib/apt/lists/* \
    && npm install -g typescript \
    && pip install --no-cache-dir --break-system-packages cargo-zigbuild

# The image's toolchain is the one rust-toolchain.toml names, so rustup needn't
# fetch it again, nor the components only development uses.
ENV RUSTUP_TOOLCHAIN=1.98.1
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl ;; \
      arm64) echo aarch64-unknown-linux-musl ;; \
      *) echo "no Rust target for $TARGETARCH" >&2; exit 1 ;; \
    esac > /target \
    && rustup target add "$(cat /target)"

WORKDIR /src
COPY . .
# Queries are checked against the committed sqlx cache, not a database.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target,id=wst-target-$TARGETARCH \
    SQLX_OFFLINE=true cargo zigbuild --locked --release -p wst --target "$(cat /target)" \
    && install -D "target/$(cat /target)/release/wst" "/$TARGETARCH/wst"

FROM alpine:3.22
ARG TARGETARCH
# ffmpeg brings ffprobe. TMDB is reached over TLS, verified against the
# system's certificates.
RUN apk add --no-cache ffmpeg ca-certificates
COPY --from=build /$TARGETARCH/wst /usr/local/bin/wst

# Mount the three directories, or let them live in the container. Listening on
# every address, since the container's are the only ones to reach it by.
ENV WST_CONFIG_DIR=/config \
    WST_DATA_DIR=/data \
    WST_CACHE_DIR=/cache \
    WST__HOST=0.0.0.0
EXPOSE 6765
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD wget -qO- "http://127.0.0.1:${WST__PORT:-6765}/api/v1/health" || exit 1

# `docker run <image>` serves; `docker exec <container> wst unidentified` and
# the other client commands reach it inside the container.
ENTRYPOINT ["wst"]
CMD ["serve"]
