FROM rust:1.98-slim-bookworm AS build
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends build-essential && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && groupadd --gid 10001 lattice && useradd --uid 10001 --gid lattice --no-create-home lattice && mkdir /data && chown lattice:lattice /data
COPY --from=build /src/target/release/bedrock /usr/local/bin/bedrock
USER 10001:10001
ENV BEDROCK_BIND=0.0.0.0:4173 BEDROCK_DB=/data/lattice.sqlite3
VOLUME ["/data"]
EXPOSE 4173
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s CMD curl --fail --silent http://127.0.0.1:4173/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/bedrock"]
