# Gate 8 — Commonware 2026.2.0 Finalized Control Log

Gate 8 places host scheduling and guild-node admission behind a Byzantine-finalized, replayable
control log. The dependency is pinned to the upstream release tag `v2026.2.0` and resolved commit
`dcd4f1026254376d1490cfdaedd6d3abd61aa793`—not a moving branch.

## Why the feature is explicit

Commonware `v2026.2.0` declares Rust `1.91.1`; the Mir2 workspace remains buildable with its
established Rust `1.89.0`. `commonware-consensus` is therefore an optional dependency enabled by
`commonware-2026-2`. Default Gateway checks stay on 1.89, while the Commonware acceptance lane uses
a newer compiler. The lockfile still fixes the full upstream graph and tag commit.

## Finality adapter

`CommonwareControlLog` is the application-side finality state machine for a Commonware Simplex
committee:

- a committee of `n` validators requires `n - floor((n - 1) / 3)` matching votes;
- blocks commit to epoch, Commonware height/view, parent, proposer, ordered commands and
  idempotency keys;
- height gaps, parent forks, unknown signers, duplicate commands and invalid digests fail closed;
- conflicting votes produce durable equivocation evidence;
- finalized blocks import in order for deterministic crash recovery/catch-up;
- epoch rotation requires the expected finalized height and no pending proposal.

The `commonware-2026-2` feature converts every application block to the exact upstream
`commonware_consensus::types::{Epoch, Height, View}` types. The network Simplex reporter feeds its
certificate signers into `import_finalized`; the same collector is used by the deterministic test
harness.

## No empty blocks

The control chain is event-driven. `propose` rejects an empty command list, so an idle game does not
manufacture blocks. Host registration/heartbeat, Zone placement/drain, and guild admission/revoke
events are the transactions that advance height.

## Projection into the live control plane

`FinalizedControlProjector` decodes only the versioned `obelisk.control.v1` namespace and applies
finalized commands to the Gate 6 scheduler and Gate 7 guild security registry. It tracks the last
applied height and rejects gaps. No unfinalized proposal can change a placement or admit a guild
node.

## Acceptance

```bash
# Existing workspace/compiler lane
cargo +1.89.0 test -p mir2-gateway --lib consensus_log::tests -- --test-threads=1
cargo +1.89.0 check -p mir2-gateway --all-targets

# Pinned Commonware release lane (upstream requires Rust >= 1.91.1)
cargo +1.95.0 test -p mir2-gateway --features commonware-2026-2 \
  --lib consensus_log::tests -- --test-threads=1
```

Acceptance proves 3-of-4 finalization, absence of empty blocks, replay/fork rejection, finalized
projection into real placement/admission state, and exact upstream epoch/height/view conversion.
