# syntax=docker/dockerfile:1.7
#
# Home Assistant builds a local/repository add-on with the ADD-ON DIRECTORY as the build context
# (ha-addon/ticknode), so the workspace is not available to COPY. The `fetch` stage therefore
# fetches the sources from GitHub (override TICKNODE_REF to pin a tag or commit). CI and local
# builds can use the checked-out workspace instead:
#   docker build --build-context source=. -f ha-addon/ticknode/Dockerfile ha-addon/ticknode
# (a named build context replaces the `source` stage).

ARG TICKNODE_REPO=https://github.com/pavver/TickNode.git
ARG TICKNODE_REF=master
ARG TRUNK_VERSION=0.21.14

FROM debian:bookworm-slim AS fetch
ARG TICKNODE_REPO
ARG TICKNODE_REF
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*
# The clone command never changes, so Docker would reuse a cached clone on "Rebuild". Adding the
# current commit description invalidates the cache whenever TICKNODE_REF points to a new commit.
ARG TICKNODE_GITHUB=pavver/TickNode
ADD https://api.github.com/repos/${TICKNODE_GITHUB}/commits/${TICKNODE_REF} /tmp/ticknode-ref.json
RUN git clone --depth 1 --branch "$TICKNODE_REF" "$TICKNODE_REPO" /src \
    || (git clone "$TICKNODE_REPO" /src && git -C /src checkout "$TICKNODE_REF")

# The workspace root; a `--build-context source=<dir>` replaces this stage.
FROM scratch AS source
COPY --from=fetch /src /

FROM rust:1-bookworm AS build
ARG TRUNK_VERSION
RUN rustup target add wasm32-unknown-unknown wasm32-wasip2 \
    && apt-get update && apt-get install -y --no-install-recommends curl ca-certificates binaryen && rm -rf /var/lib/apt/lists/*
# Prebuilt trunk; it downloads the wasm-bindgen-cli matching Cargo.lock by itself.
RUN set -eu; arch="$(uname -m)"; \
    curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${arch}-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin trunk
COPY --from=source / /src
WORKDIR /src
ENV CARGO_TARGET_DIR=/target
RUN cargo build --release -p ticknode-server
RUN cargo build --release --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p ha-plugin
RUN cd ticknode-editor && trunk build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /target/release/ticknode-server /usr/local/bin/ticknode-server
COPY --from=build /src/ticknode-editor/dist /opt/ticknode/web
COPY --from=build /target/wasm32-wasip2/release/*_plugin.wasm /opt/ticknode/plugins/
COPY run.sh /run.sh
RUN chmod +x /run.sh
CMD ["/run.sh"]
