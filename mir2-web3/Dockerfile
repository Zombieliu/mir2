FROM rust:1.89.0-bookworm AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY apps/admin-api ./apps/admin-api
COPY apps/gateway ./apps/gateway
COPY apps/simulation ./apps/simulation
COPY packages/game-data ./packages/game-data
COPY packages/protocol ./packages/protocol
COPY infra/postgres/migrations ./infra/postgres/migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway \
      --bin mir2-gateway \
      --bin zone_host \
      --bin zone_replicator \
      --bin node_identity \
      --bin gate12_acceptance \
      --bin gate13_acceptance \
    && install -Dm755 target/release/mir2-gateway /out/mir2-gateway \
    && install -Dm755 target/release/zone_host /out/obelisk-zone-host \
    && install -Dm755 target/release/zone_replicator /out/obelisk-zone-replicator \
    && install -Dm755 target/release/node_identity /out/obelisk-node-identity \
    && install -Dm755 target/release/gate12_acceptance /out/gate12-acceptance \
    && install -Dm755 target/release/gate13_acceptance /out/gate13-acceptance

FROM debian:bookworm-slim AS runtime
RUN install -d -o 65534 -g 65534 /var/lib/obelisk /evidence
ENV RUST_BACKTRACE=1
WORKDIR /var/lib/obelisk

FROM runtime AS zone-host
COPY --from=builder /out/obelisk-zone-host /usr/local/bin/obelisk-zone-host
COPY --from=builder /out/obelisk-node-identity /usr/local/bin/obelisk-node-identity
USER 65534:65534
EXPOSE 7020 9100
HEALTHCHECK --interval=5s --timeout=2s --start-period=10s --retries=20 \
  CMD /bin/bash -ec "exec 3<>/dev/tcp/127.0.0.1/9100 && printf 'GET /readyz HTTP/1.0\r\n\r\n' >&3 && grep -q '200 OK' <&3"
CMD ["/usr/local/bin/obelisk-zone-host"]

FROM runtime AS gateway
COPY --from=builder /out/mir2-gateway /usr/local/bin/mir2-gateway
USER 65534:65534
EXPOSE 7000 7010
HEALTHCHECK --interval=5s --timeout=2s --start-period=15s --retries=20 \
  CMD /bin/bash -ec "exec 3<>/dev/tcp/127.0.0.1/7010 && printf 'GET /health HTTP/1.0\r\n\r\n' >&3 && grep -q '200 OK' <&3"
CMD ["/usr/local/bin/mir2-gateway"]

FROM runtime AS zone-replicator
COPY --from=builder /out/obelisk-zone-replicator /usr/local/bin/obelisk-zone-replicator
USER 65534:65534
CMD ["/usr/local/bin/obelisk-zone-replicator"]

FROM runtime AS acceptance
COPY --from=builder /out/gate12-acceptance /usr/local/bin/gate12-acceptance
COPY --from=builder /out/gate13-acceptance /usr/local/bin/gate13-acceptance
USER 65534:65534
CMD ["/usr/local/bin/gate12-acceptance"]

FROM builder AS capacity-benchmark-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-simulation --example zone_load \
    && install -Dm755 target/release/examples/zone_load /out/dubhe-zone-load

FROM runtime AS capacity-benchmark
COPY --from=capacity-benchmark-builder /out/dubhe-zone-load /usr/local/bin/dubhe-zone-load
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/dubhe-zone-load"]
