FROM rust:1.89.0-bookworm AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY apps/admin-api ./apps/admin-api
COPY apps/dubhe-node-desktop/src-tauri ./apps/dubhe-node-desktop/src-tauri
COPY apps/gateway ./apps/gateway
COPY apps/simulation ./apps/simulation
COPY packages/game-data ./packages/game-data
COPY packages/protocol ./packages/protocol
COPY vendor/dubhe-network-core ./vendor/dubhe-network-core
COPY infra/postgres/migrations ./infra/postgres/migrations
COPY infra/regional ./infra/regional

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway \
      --bin mir2-gateway \
      --bin zone_host \
      --bin zone_replicator \
      --bin zone_promoter \
      --bin node_identity \
      --bin home_relay \
      --bin home_agent \
      --bin home_tunnel_fixture \
      --bin home_tunnel_acceptance \
      --bin home_sandbox_policy \
      --bin home_beta_policy \
      --bin home_beta_local_acceptance \
      --bin home_telemetry_collector \
      --bin gate12_acceptance \
      --bin gate13_acceptance \
    && install -Dm755 target/release/mir2-gateway /out/mir2-gateway \
    && install -Dm755 target/release/zone_host /out/obelisk-zone-host \
    && install -Dm755 target/release/zone_replicator /out/obelisk-zone-replicator \
    && install -Dm755 target/release/zone_promoter /out/obelisk-zone-promoter \
    && install -Dm755 target/release/node_identity /out/obelisk-node-identity \
    && install -Dm755 target/release/home_relay /out/obelisk-home-relay \
    && install -Dm755 target/release/home_agent /out/obelisk-home-agent \
    && install -Dm755 target/release/home_tunnel_fixture /out/home-tunnel-fixture \
    && install -Dm755 target/release/home_tunnel_acceptance /out/home-tunnel-acceptance \
    && install -Dm755 target/release/home_sandbox_policy /out/home-sandbox-policy \
    && install -Dm755 target/release/home_beta_policy /out/home-beta-policy \
    && install -Dm755 target/release/home_beta_local_acceptance /out/home-beta-local-acceptance \
    && install -Dm755 target/release/home_telemetry_collector /out/home-telemetry-collector \
    && install -Dm755 target/release/gate12_acceptance /out/gate12-acceptance \
    && install -Dm755 target/release/gate13_acceptance /out/gate13-acceptance

FROM debian:bookworm-slim AS runtime
RUN install -d -o 65534 -g 65534 \
    /var/lib/obelisk \
    /var/lib/obelisk/replication-wal \
    /evidence
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

FROM runtime AS home-relay
COPY --from=builder /out/obelisk-home-relay /usr/local/bin/obelisk-home-relay
USER 65534:65534
EXPOSE 7443/udp 7444
CMD ["/usr/local/bin/obelisk-home-relay"]

FROM runtime AS home-agent
COPY --from=builder /out/obelisk-home-agent /usr/local/bin/obelisk-home-agent
USER 65534:65534
CMD ["/usr/local/bin/obelisk-home-agent"]

FROM runtime AS home-tunnel-fixture
COPY --from=builder /out/home-tunnel-fixture /usr/local/bin/home-tunnel-fixture
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/home-tunnel-fixture"]

FROM runtime AS home-tunnel-acceptance
COPY --from=builder /out/home-tunnel-acceptance /usr/local/bin/home-tunnel-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/home-tunnel-acceptance"]

FROM runtime AS home-sandbox-policy
COPY --from=builder /out/home-sandbox-policy /usr/local/bin/home-sandbox-policy
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/home-sandbox-policy"]

FROM runtime AS home-beta-policy
COPY --from=builder /out/home-beta-policy /usr/local/bin/home-beta-policy
COPY --from=builder /out/home-beta-local-acceptance /usr/local/bin/home-beta-local-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/home-beta-policy"]

FROM runtime AS home-telemetry-collector
COPY --from=builder /out/home-telemetry-collector /usr/local/bin/home-telemetry-collector
USER 65534:65534
EXPOSE 18080
ENTRYPOINT ["/usr/local/bin/home-telemetry-collector"]

FROM runtime AS zone-replicator
COPY --from=builder /out/obelisk-zone-replicator /usr/local/bin/obelisk-zone-replicator
COPY --from=builder /out/obelisk-zone-promoter /usr/local/bin/obelisk-zone-promoter
USER 65534:65534
CMD ["/usr/local/bin/obelisk-zone-replicator"]

FROM runtime AS acceptance
COPY --from=builder /out/gate12-acceptance /usr/local/bin/gate12-acceptance
COPY --from=builder /out/gate13-acceptance /usr/local/bin/gate13-acceptance
USER 65534:65534
CMD ["/usr/local/bin/gate12-acceptance"]

# Commonware v2026.2.0 uses stable library APIs newer than Rust 1.89. Keep the
# existing game binaries pinned above, and install the newer toolchain only in
# the Gate 14 build stage. Starting from the pinned image also avoids depending
# on a future Docker Hub tag being published at build time.
FROM rust:1.89.0-bookworm AS gate14-builder
WORKDIR /src

RUN rustup toolchain install 1.95.0 --profile minimal

COPY Cargo.toml Cargo.lock ./
COPY apps/admin-api ./apps/admin-api
COPY apps/dubhe-node-desktop/src-tauri ./apps/dubhe-node-desktop/src-tauri
COPY apps/gateway ./apps/gateway
COPY apps/simulation ./apps/simulation
COPY packages/game-data ./packages/game-data
COPY packages/protocol ./packages/protocol
COPY vendor/dubhe-network-core ./vendor/dubhe-network-core
COPY infra/postgres/migrations ./infra/postgres/migrations
COPY infra/regional ./infra/regional

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo +1.95.0 build --locked --release -p mir2-gateway \
      --features commonware-2026-2 \
      --bin gate14_validator \
      --bin gate14_gateway \
      --bin gate14_projector \
    && install -Dm755 target/release/gate14_validator /out/gate14-validator \
    && install -Dm755 target/release/gate14_gateway /out/gate14-gateway \
    && install -Dm755 target/release/gate14_projector /out/gate14-projector

FROM runtime AS gate14
COPY --from=gate14-builder /out/gate14-validator /usr/local/bin/gate14-validator
COPY --from=gate14-builder /out/gate14-gateway /usr/local/bin/gate14-gateway
COPY --from=gate14-builder /out/gate14-projector /usr/local/bin/gate14-projector
# The validator creates Commonware journal partitions on first boot. Named
# volumes are mounted root-owned by Docker, so this POC image starts as root;
# production packaging should replace this with an init/chown step and then
# drop privileges before the process starts.
EXPOSE 9300 9400 9500 9600

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

FROM builder AS gate16-v4-baseline-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate16_checkpoint_load \
    && install -Dm755 target/release/gate16_checkpoint_load /out/gate16-checkpoint-load

FROM runtime AS gate16-v4-baseline
COPY --from=gate16-v4-baseline-builder /out/gate16-checkpoint-load /usr/local/bin/gate16-checkpoint-load
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate16-checkpoint-load"]

FROM builder AS gate16-v5-certification-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate16_v5_certification \
    && install -Dm755 target/release/gate16_v5_certification /out/gate16-v5-certification

FROM runtime AS gate16-v5-certification
COPY --from=gate16-v5-certification-builder /out/gate16-v5-certification /usr/local/bin/gate16-v5-certification
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate16-v5-certification"]

FROM builder AS gate17-acceptance-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate17_acceptance \
    && install -Dm755 target/release/gate17_acceptance /out/gate17-acceptance

FROM runtime AS gate17-acceptance
COPY --from=gate17-acceptance-builder /out/gate17-acceptance /usr/local/bin/gate17-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate17-acceptance"]

FROM builder AS gate18-economy-producer-acceptance-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate18_economy_producer_acceptance \
    && install -Dm755 target/release/gate18_economy_producer_acceptance /out/gate18-economy-producer-acceptance

FROM runtime AS gate18-economy-producer-acceptance
COPY --from=gate18-economy-producer-acceptance-builder /out/gate18-economy-producer-acceptance /usr/local/bin/gate18-economy-producer-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate18-economy-producer-acceptance"]

FROM builder AS gate18-remote-economy-acceptance-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate18_remote_economy_acceptance \
    && install -Dm755 target/release/gate18_remote_economy_acceptance /out/gate18-remote-economy-acceptance

FROM runtime AS gate18-remote-economy-acceptance
COPY --from=gate18-remote-economy-acceptance-builder /out/gate18-remote-economy-acceptance /usr/local/bin/gate18-remote-economy-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate18-remote-economy-acceptance"]

FROM builder AS gate18-gameplay-acceptance-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate18_gameplay_acceptance \
    && install -Dm755 target/release/gate18_gameplay_acceptance /out/gate18-gameplay-acceptance

FROM runtime AS gate18-gameplay-acceptance
COPY --from=gate18-gameplay-acceptance-builder /out/gate18-gameplay-acceptance /usr/local/bin/gate18-gameplay-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate18-gameplay-acceptance"]

FROM builder AS gate18-load-acceptance-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate18_load_acceptance \
    && install -Dm755 target/release/gate18_load_acceptance /out/gate18-load-acceptance

FROM runtime AS gate18-load-acceptance
COPY --from=gate18-load-acceptance-builder /out/gate18-load-acceptance /usr/local/bin/gate18-load-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate18-load-acceptance"]

FROM builder AS gate18-migration-acceptance-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate18_migration_acceptance \
    && install -Dm755 target/release/gate18_migration_acceptance /out/gate18-migration-acceptance

FROM runtime AS gate18-migration-acceptance
COPY --from=gate18-migration-acceptance-builder /out/gate18-migration-acceptance /usr/local/bin/gate18-migration-acceptance
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate18-migration-acceptance"]

FROM builder AS gate19-zone-failover-controller-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate19_zone_failover_controller \
    && install -Dm755 target/release/gate19_zone_failover_controller /out/gate19-zone-failover-controller

FROM runtime AS gate19-zone-failover-controller
COPY --from=gate19-zone-failover-controller-builder /out/gate19-zone-failover-controller /usr/local/bin/gate19-zone-failover-controller
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate19-zone-failover-controller"]

FROM builder AS gate19-infra-probe-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate19_infra_probe \
    && install -Dm755 target/release/gate19_infra_probe /out/gate19-infra-probe

FROM runtime AS gate19-infra-probe
COPY --from=gate19-infra-probe-builder /out/gate19-infra-probe /usr/local/bin/gate19-infra-probe
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate19-infra-probe"]

FROM builder AS gate19-zone-seed-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p mir2-gateway --bin gate19_zone_seed \
    && install -Dm755 target/release/gate19_zone_seed /out/gate19-zone-seed

FROM runtime AS gate19-zone-seed
COPY --from=gate19-zone-seed-builder /out/gate19-zone-seed /usr/local/bin/gate19-zone-seed
USER 65534:65534
ENTRYPOINT ["/usr/local/bin/gate19-zone-seed"]
