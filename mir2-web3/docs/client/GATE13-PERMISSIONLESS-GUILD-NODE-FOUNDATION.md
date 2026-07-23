# Gate 13 — Permissionless Guild-Node Foundation

Gate 13 replaces the earlier shared-secret enrollment boundary with a
cryptographic, chain-anchored admission path. A Zone Host now proves possession
of its Ed25519 node key, an operator registers that public identity and stake on
Sui testnet, a remote verifier certifies a bounded capacity challenge, and only
a Commonware-finalized registration plus a valid short-lived capacity
certificate enters the deterministic scheduling and reward projections.

This is a testnet foundation and an end-to-end acceptance, not a permissionless
mainnet launch.

## Trust and data flow

```text
Sui testnet NodeRegistry events
          |
          v
finalized GraphQL event snapshot
          |
          v
Commonware v2026.2.0 quorum-finalized control block
          |
          +--> deterministic guild-node membership
          |        |
          |        v
          |    Zone scheduling / verified execution
          |
          +--> reward eligibility + verified work receipts
                              |
                              v
                    Merkle reward batch

Ed25519 Zone Host --> nonce-bound remote capacity challenge
                              |
                              v
                  trusted short-lived capacity certificate
```

The Sui registry establishes operator ownership, stake, identity generation,
capacity claims, rotation, and revocation. Commonware is currently an
application-side deterministic finality adapter pinned to tag `v2026.2.0`; it
does not claim to be a deployed public validator network. The capacity verifier
must be in the local issuer trust set, and expiry is fail-closed.

Default Gateway development and tests remain on Rust `1.89.0`. The optional
`commonware-2026-2` feature uses Rust `1.95.0`, matching Gate 8, because the
upstream v2026.2.0 release uses stable standard-library duration constructors
not available in 1.89. The dependency stays pinned to the requested release
tag; it is not silently downgraded to make the older toolchain compile.

## Sui testnet deployment

The Move package pins the Sui framework to exact commit
`3c0f387ebb40b8be292d3b7bd3f5bee8ad226d33`.

| Item | Value |
| --- | --- |
| Network | `testnet` |
| Publisher | `0xf5fbee0ef2ad68b0340bfdc30af133c2bbaa7ab815944e04561d53ce4669b74f` |
| Package | `0x4201a90b22b8a6e000a032fff075be6bc6fdd531c6163465c902107ea285c53e` |
| Shared registry | `0x7622e3ec2b5664e584a147d530aaab8084d6e793325b8d71f1ae386da9a266a7` |
| Registry admin cap | `0xbab44e29812a6dc0f6f471e1051d53f733035b732fe2acdd227f976e4062bc8a` |
| Upgrade cap | `0x8d38ae4086fa5b4cdf88108b47e67af65545c9c2516ccc83de1c119d93f5000c` |
| Publish transaction / checkpoint | `GxxvU7FpBKH1ud2ukmXAR98BbNsTE7o15GZYn391fhm` / `363529670` |
| Active registration transaction / checkpoint | `FuvLLhCaNJswJcZCj2uRYdSC2YbHN79SZ8nEgdaEBVYH` / `363531316` |

The lifecycle acceptance also registered a separate identity, rotated it from
generation 1 to 2, revoked it, returned its 2,000,000 MIST stake, and confirmed
that its stable node ID remains retired:

| Operation | Transaction | Checkpoint |
| --- | --- | --- |
| Register | `FCUCkww8oTYMrgYsXfGA3TU9nfKWxLTXgmqDyDgKwawt` | `363530816` |
| Rotate | `CjXUqEWBp1oscXFZwFLf1FiMU66QySX2SgbQGP7qHGdf` | `363531011` |
| Revoke/refund | `GmUiPsHG77MhLVrM6PE4LBf5MCcfWq9SfUE37kFAHqmT` | `363531201` |

The authoritative machine-readable record is
`onchain/deployments/obelisk-node-registry-testnet.json`. The active
registration snapshot is
`docs/generated/gate13/testnet/active-registration.json`. Its
`node-active.test:7020` endpoint is test metadata, not a public service address.

## Identity and key handling

Generate and inspect a node seed:

```bash
cargo +1.89.0 run -p mir2-gateway --bin node_identity -- \
  generate /secure/path/node.key
cargo +1.89.0 run -p mir2-gateway --bin node_identity -- \
  inspect /secure/path/node.key
```

`generate` writes a mode-0600 seed and prints only the derived stable node ID
and public key. A non-loopback Zone Host requires
`MIR2_ZONE_HOST_SIGNING_KEY_FILE` or `MIR2_ZONE_HOST_SIGNING_KEY`; the signing
seed is redacted from debug output. Heartbeat replay protection is scoped by
node ID, key generation, and process ID so rotation can advance without
reusing an old replay window.

Private node, issuer, Sui keystore, mnemonic, and capability material must stay
outside the repository. Rotation and revocation require the on-chain owner
capability. An initial registration cryptographically derives the stable node
ID from its public key; later generations rely on the finalized on-chain
rotation history to preserve continuity.

## Finalized registration snapshot

From `onchain/`:

```bash
NODE_REGISTRY_NETWORK=testnet \
NODE_REGISTRY_PACKAGE_ID=0x4201a90b22b8a6e000a032fff075be6bc6fdd531c6163465c902107ea285c53e \
EXPECT_ACTIVE_NODE_ID=ed25519:e5b4574f4c34f6c53adb1ec87fd980199b403bfaf653492602298e16f1108482 \
pnpm node-registry:snapshot
```

The relayer reads Sui GraphQL events, orders them by checkpoint and event
sequence, and applies register, rotate, metadata, slash, and revoke transitions
idempotently. A registration is not scheduler-eligible at the Sui event-read
boundary: the Rust control log must also finalize the sync command through its
configured Commonware quorum.

## Remote capacity and reward acceptance

The Zone Host exposes `POST /v1/capacity-challenge` on its operator listener.
The request body is capped at 8 KiB, only one challenge runs at a time, command
count is capped by `MIR2_ZONE_HOST_CAPACITY_MAX_COMMANDS`, and requested
sessions/Zones cannot exceed configured capacity. The response commits the
nonce-bound workload transcript, measured success count and p95 latency, then
signs it with the registered Ed25519 node key.

The trusted verifier checks the node identity, finalized registration,
generation, challenge window, workload accounting, latency/SLO, and signature.
It then issues a signed short-lived capacity certificate. Scheduler and reward
projections require all of:

1. active finalized Sui registration;
2. a valid certificate from a trusted issuer;
3. certificate capacity no greater than the on-chain registration;
4. non-expired eligibility at placement and reward-finalization time;
5. verified work receipts finalized at the required quorum.

Revocation removes future membership and reward eligibility. It does not
rewrite already-finalized historical batches.

## Docker acceptance

The active testnet node key must correspond to the committed public
registration, while the capacity issuer key must correspond to the public
issuer in the deployment manifest:

```bash
export GATE13_NODE_SIGNING_KEY_FILE=/secure/path/active-testnet-node.key
export GATE13_CAPACITY_ISSUER_KEY_FILE=/secure/path/capacity-issuer.key
GATE13_EVIDENCE_DIR="$PWD/docs/generated/gate13/docker" \
  infra/gate13/run-acceptance.sh
```

Validate the exact Commonware release adapter separately:

```bash
cargo +1.95.0 test -p mir2-gateway --features commonware-2026-2 \
  consensus_log::tests::pinned_commonware_release_types_round_trip_block_coordinates \
  -- --test-threads=1
```

The acceptance starts a separately containerized, non-root, read-only Zone
Host, executes 2,000 remote challenge commands, verifies the signed response,
issues a one-hour certificate, proves two of four Commonware votes do not admit
the node, finalizes admission with the third vote, and produces a verified-work
Merkle reward batch. The committed Docker evidence records `accepted: true`,
p95 `1 ms`, finalized height `3`, and reward total `2000`.

Gate 12 is rerun after the identity change. Its regression evidence proves both
Ed25519 heartbeats and both remote capacity responses, a replicated live player
session, primary failure, authoritative standby output, and observed failover
in `814 ms`.

## Exact boundary

Gate 13 proves real asymmetric identity, Sui testnet lifecycle, finalized
membership, a remotely executed admission workload, certificate expiry, and
verified-work reward eligibility. It deliberately does not claim:

- hardware identity, TEE/TPM attestation, or that an operator cannot rent
  burst capacity only for admission;
- sustained production capacity under the real Mir2 simulation workload,
  adversarial traffic, or multi-region network faults;
- transport confidentiality/authentication before TLS/mTLS is added;
- a deployed public Commonware validator network;
- permissionless Sui mainnet operation, production token economics, slashing
  governance, or legal/compliance readiness;
- arbitrary untrusted game-code sandboxing.

Those are production hardening gates. Capacity certification should eventually
be renewed continuously with real replay workloads and cross-checked against
long-horizon verified work, rather than treated as a permanent benchmark.
