# Gate 7 — Untrusted Guild-Node Verification

Gate 7 makes guild-operated compute an explicitly untrusted execution tier. A guild node never
becomes authoritative merely because it owns a Zone placement or can answer Zone RPC.

## Admission and least privilege

`GuildNodeSecurityRegistry` is the local projection of the operator admission log. Each admission
binds a stable node id to an operator, expiry, and a small capability set. Unknown, expired,
revoked, capability-mismatched, or quarantined nodes are excluded before any command is sent.

The current capabilities separate Zone execution from checkpoint replication. Gate 8 will order
admit/revoke operations through the Commonware control-plane log; the root operator remains the
only component allowed to construct those records.

## Deterministic execution quorum

`VerifiedGuildZoneTransport` fans every authoritative operation out to independent admitted
replicas. For `execute`, every response commitment includes:

- canonical Crystal packet frames and ordering;
- command kind, packet count, snapshot tick and active identity;
- the complete post-command `WorldSnapshot`.

Only a matching threshold is returned to the Gateway. A 2-of-3 placement tolerates one unavailable
or Byzantine guild node. Snapshot, identity, connect, save, mail refresh, and close operations also
require a quorum. The live outbound stream comes from one eligible node, but subsequent commands
and snapshots expose any divergent state before it can commit another player transition.

## Automatic quarantine

Each mismatch or failed verifier response records a strike. At the configured threshold the node is
quarantined for a bounded interval; its admission stays visible for audit while it is excluded from
new quorum work. Agreements are counted and decay one outstanding strike. Revocation is immediate.

This boundary composes with the earlier controls:

- Gate 5 owner leases fence stale Zone writers;
- Gate 6 placement generations fence stale schedulers and draining hosts;
- Gate 7 digest quorums fence incorrect execution results.

No guild node holds the account-store, settlement, admission, or reward-signing keys. Its useful
output is a deterministic execution candidate, not a final economic transaction.

## Acceptance

```bash
cargo +1.89.0 test -p mir2-gateway --lib node_security::tests -- --test-threads=1
cargo +1.89.0 check -p mir2-gateway --all-targets
```

The acceptance test runs two honest deterministic runtimes and one response-tampering runtime. The
2-of-3 result exactly matches the honest packet, the divergent node is quarantined, and an expired
admission fails closed before execution.

Gate 8 continues in [`GATE8-COMMONWARE-CONTROL-LOG.md`](GATE8-COMMONWARE-CONTROL-LOG.md).
