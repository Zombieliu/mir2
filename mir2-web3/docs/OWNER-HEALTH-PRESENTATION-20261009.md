# Owner HP/MP presentation Candidate — 2026-10-09

This is an unpublished backend Candidate after the exact R22 public delivery.
It preserves personal Session versus shared Zone authority. The current release
baseline is game7fea/Gatewaya41/feed17; this work changes none of those files,
services, installer objects or pointers. P1-P7 and native acceptance remain open.

## Resulting behavior

Real shared pool operations emit an exact owner-only HP/MP event, including
MP-only costs, one-HP changes with the same displayed percentage, and separate
intermediate EnergyShield/heal/damage operations. A silent metadata sync does
not manufacture a notification or settle death, experience or drops. Source
Flame/Twin preparation, normal casts, player hits, Plague and Reincarnation keep
their explicit operation/action ordering instead of reducing a whole tick to
its final net pools. Real accepted actions commit personal MP/cooldown and melee
progress even when their display packets are routed to the live channel.

Events bind the server-issued admission, actor ID, life generation and monotonic
health sequence. Local TCP/WS retains that capability until the final writer;
duplicate/reversed or expired events cannot become an unbound raw health reply.
The bounded live FIFO preserves normal controls around health operations and
cancels/resynchronizes an overloaded subscriber. Without a live subscriber, only
normal lifecycle/control packets go to the personal Source drain. Closed-at-cap
and teardown preserve those packets without replaying expired display proofs.

Ordinary Tick and both native-item input forms complete their Source/repository
transaction outside the Zone writer, then commit only the confirmed delta
against the prepared actor/life. Concurrent damage is preserved; a new death
or revival discards old recovery. Committed level ceilings remain attached to
the same admission even when its life changes. Source's positive-HP corpse
LevelUp keeps Dead=true and does not advance a life generation. Teardown reward
drains require a private runtime fence generation. Actual TownRevive commits
its new life before subsequent reward consumers; metadata tails cannot replay
the older refill over fresh damage. The pre-existing acknowledged private-death
compatibility case remains explicit, not a full Source life-floor acceptance.

The final mentorship integration exposed two pre-existing causes by static
parent comparison: stale durable levels could immediately end a newly accepted
relationship, and credit reconciliation could erase the projection difference
needed to notify a peer's logout. Actor graduation now uses the owner's current
confirmed Source level, including during fenced teardown; online peers use the
identity-checked current Zone experience profile rather than delayed viewport
metadata. The paired Store transaction applies the same captured levels only
to its strict graduation guard. Relationship epoch, persistence rollback and
peer character/XP/revision protections remain. A social-generation change
forces the current MentorUpdate despite a preceding durable-state merge.
Publishing a confirmed level change also advances that generation, so peers
receive the new level on ordinary Tick without a forced display refresh.

Remote Source RPC validates an event at its actual authenticated endpoint and
current lease/registration, without Tick, journal mutation, session creation
or endpoint fallback. Expired retained numbered frames stay until normal ACK;
final delivery skips them without renumbering. Old-map viewport and owner
location frames cannot inherit a new Source registration. Durable social
receipts stay on their existing recovery path. Validation failure or overflow
cancels the entire matching transport; cancellation sentinels never reach wire.

## Validation and limits

Simulation producer/mentor/rollback checks pass23/0; Gateway related lib checks pass
68/0; mentorship lifecycle checks pass6/0 and its ordinary TCP/WS story
passes1/0. Read-only review findings and all original failures are retained in
the [byte inventory](generated/player-qa/classic-20261009/owner-health-02/ARCHIVE.json)
and [scope/readme](generated/player-qa/classic-20261009/owner-health-02/README.md).
Several concurrency/control-order tests use prepared interleavings. The three
WS final-writer tests use controlled validators and real sockets. The seven RPC
tests use real authenticated loopback RPC with bounded native item/config data.
These distinctions are not natural combat or native-client acceptance.
The extra mentor tests prepare relationship/credit/old-save values, then use
actual Source credit consumption, ordinary Tick and the actual fenced checkpoint
drain. They prove confirmed-level handling and do not claim natural leveling.
The new Store test also preserves bad-epoch and BeforePersist/retry assertions.

Final check profiles only alter per-package test artifact identities to avoid
Windows locks held by another running game. Source/release profiles, installs,
player saves and public services are preserved. Full NativeCrystalWorld cold
initialization timeouts remain unproven; bounded RPC success is not that fix.

The full Source recipient contract is still open: inclusive range/group and
Revelation selection, monster Master/EXPOwner cross-map overlap, typed shared
Hero HP/MP identity, retained heal timing, leave revocation and zero-MaxMP rules.
OwnerLocation retains its priority lane; last validation/write is serialized
but not an atomic distributed transaction. A universal Source NPC/event ordered
outbox, natural crowded escape, later Boss/equipment/book acquisition and native
shop-wheel/visual acceptance are not certified. Earlier five reproduced parent
failures remain recorded. No 100% Candidate or full P1-P7 completion is claimed.

Real payment is not integrated. Monthly cards use operator-issued codes, account
expiry and renewals; public paid-access enforcement remains OFF. This work does
not add a payment provider or automatic purchase fulfillment.
