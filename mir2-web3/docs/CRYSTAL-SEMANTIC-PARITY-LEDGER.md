# Crystal Semantic Parity Ledger Contract

Status: normative. This contract defines what may be counted as Crystal 1:1
parity. A feature implementation, passing unit test, screenshot, or compatible
result is not sufficient by itself.

## Scope

The ledger covers the Windows player experience and every authoritative
Gateway/Simulation behavior that can affect it:

- process startup, patch/version negotiation, account and character lifecycle;
- maps, zones, entities, movement, collision, AOI and disconnect lifecycle;
- combat, skills, buffs, AI, drops, items, equipment and character progression;
- quests, NPCs, shops, storage, economy, groups, guilds, chat and other social
  systems;
- scheduled events, persistence, reconnect, failure behavior, timing and RNG;
- client input, state transitions, HUD/UI, text, animation, effects, audio and
  visual layout;
- package, protocol, security and Web non-regression behavior needed to ship the
  same semantics safely.

The Crystal source tree and an original-client runtime trace are the reference.
Web behavior is useful regression evidence but is not a substitute for Crystal
evidence.

## Strict completion calculation

The denominator is every player-observable capability discovered in the pinned
Crystal revision. Discovery is complete only when `inventoryComplete` is true
and its semantic-leaf inventory report is hashed into the ledger.

A source-file inventory and a semantic-leaf inventory are different gates. The
source-file inventory proves only that every `.cs` file under the pinned
`Client`, `Server`, and `Shared` roots was read and hashed. It does not prove
that every packet branch, rejection path, timer, RNG draw, state transition, or
UI consequence in those files has a ledger leaf. Therefore:

- `sourceFileInventoryComplete` may become true after a clean, stable file scan;
- `semanticLeafInventoryComplete` remains false until a separately reviewed,
  trusted discovery pass has enumerated the player-observable leaves;
- top-level `inventoryComplete` is the conjunction of both gates and must never
  be inferred from source-file count alone.

The current source-file generator is intentionally incapable of asserting
`semanticLeafInventoryComplete=true`. Until the trusted semantic discovery pass
exists and is verified, the global denominator is open and the project-wide
percentage is undefined.

```text
verified parity = VERIFIED capabilities / all discovered capabilities
```

`BLOCKED_EXTERNAL`, `IMPLEMENTED_UNVERIFIED`, and intentionally unsupported
capabilities remain in the denominator. No capability may be hidden in an
"excluded", "not applicable", or umbrella row merely to raise the percentage.
A parent capability is VERIFIED only when all of its leaf capabilities are
VERIFIED.

The project may claim 100% only when:

1. source-file and semantic-leaf inventories are complete for the pinned
   Crystal revision;
2. every leaf capability is VERIFIED at the declared mir2-web3 implementation
   revision, independently from the pinned Crystal revision;
3. no automated P0/P1 issue remains;
4. Windows package, Web regression, persistence, stability and visual gates pass;
5. all referenced evidence exists, hashes correctly, and is freshness-bound to
   the release revision and package.

Human visual/feel acceptance follows Candidate completion and is never used to
waive missing automated evidence.

## Capability identity

Each leaf uses a stable ID:

```text
<DOMAIN>.<REFERENCE-TYPE>.<REFERENCE-SYMBOL>.<BEHAVIOR>
```

Examples:

```text
MOVE.CPACKET.WALK.ACCEPT
MOVE.CPACKET.WALK.REJECT_STATIC_COLLISION
QUEST.NPC.ASSISTANT_REQUEST.COMPLETE
UI.INGAME.INVENTORY.DRAG_TO_EQUIPMENT
```

One row represents one independently falsifiable behavior. Success and each
material rejection path are separate leaves when they produce different state,
packets, persistence or UI.

## Required row fields

Every ledger row must contain the following groups.

### Identity and reference

- stable capability ID and domain;
- short behavior description and severity if mismatched;
- pinned Crystal repository revision and the separately pinned mir2-web3
  implementation revision;
- repository-relative Crystal file, type/member and exact source span;
- any data-table, map, item, monster, NPC or asset identifiers read by the code.

### Golden contract

- complete preconditions and initial authoritative state;
- ordered player/server inputs, including packet fields and input source;
- deterministic clock/tick schedule;
- RNG algorithm, seed/state and ordered draws when randomness is involved;
- ordered authoritative state deltas;
- ordered owner/AOI/global packets and exact relevant fields;
- client state/UI/audio/animation consequences;
- persistence writes and the state observed after reload;
- timeout, duplicate, malformed, unauthorized and boundary behavior.

### mir2-web3 mapping

- implementation files and symbols;
- protocol translation and ownership boundary;
- test files and exact test names;
- known deviations, placeholders or compatibility shims;
- last verified mir2-web3 Git revision and formal package identity.

The machine ledger therefore carries two different immutable revisions:

- `crystalRevision`: the reference behavior/source revision;
- `implementationRevision`: the mir2-web3 code and asset revision being judged.

They must never be collapsed into one `referenceRevision`. A VERIFIED leaf's
`verifiedRevision` equals `implementationRevision`, while its Crystal source
inventory and original traces equal `crystalRevision`.

### Evidence

- original Crystal trace path and SHA-256;
- mir2-web3 trace path and SHA-256;
- normalized semantic diff path and SHA-256;
- persistence-before/after evidence where relevant;
- negative-test evidence;
- original/native signed image pair and visual review where relevant;
- Web regression evidence for shared code paths;
- verifier version, policy hash, creation time, expiry/challenge and signer pin.

## Status state machine

Allowed statuses are:

- `UNMAPPED`: discovered in Crystal but not mapped to mir2-web3;
- `MAPPED`: source and intended implementation are located, contract incomplete;
- `CONTRACT_READY`: golden contract and original evidence are complete;
- `IMPLEMENTED_UNVERIFIED`: implementation exists without current matching proof;
- `TRACE_MISMATCH`: current evidence demonstrates a semantic difference;
- `VERIFIED`: every required evidence field passes at the current revision;
- `BLOCKED_EXTERNAL`: evidence depends on unavailable original data, hardware,
  credentials or a human action. It still counts as incomplete.

Valid forward flow is:

```text
UNMAPPED -> MAPPED -> CONTRACT_READY -> IMPLEMENTED_UNVERIFIED -> VERIFIED
                                      -> TRACE_MISMATCH -> IMPLEMENTED_UNVERIFIED
```

Any source, contract, implementation, verifier, package or relevant asset change
invalidates stale VERIFIED evidence. The row returns to the earliest status no
longer proved by current evidence.

## Evidence rules

1. Evidence files are immutable and created with no-overwrite semantics.
2. Every capability evidence reference carries SHA-256, schema version,
   creation time, evidence kind, both revisions, verifier version and the
   trusted policy hash. Package/visual evidence additionally carries package
   identity, expiry, challenge and signer pin. Source-file and semantic-leaf
   inventory evidence are separate records bound to the Crystal revision
   recorded inside their reports.
3. Runtime evidence must originate from the real Crystal implementation or the
   real Gateway/Simulation/Windows path. Mocks may test helpers but cannot prove
   parity.
4. Normalization may remove nondeterministic IDs only through a reviewed mapping;
   it may not discard ordering, rejection reasons, timing windows or state.
5. Visual evidence requires independently attested original and Windows captures
   of the same declared scene. Identical files, reused hashes and caller-selected
   trust pins are rejected.
6. A score threshold is fixed by trusted policy. CLI arguments may raise but may
   never lower it.
7. A passing result must include negative and boundary cases, not only a happy
   path.
8. A unit test that restates mir2-web3 behavior without observing Crystal is
   implementation evidence, not parity evidence.
9. `VERIFIED` requires, at minimum, separate Crystal trace, implementation
   trace, normalized semantic diff and negative-test evidence. Persistence
   behavior requires reload evidence. UI/audio/animation/effect/asset leaves
   additionally require independently attested original/native images and a
   visual review. No single self-authored report may stand in for all kinds.

## Domain inventory

The semantic-leaf discovery pass must enumerate, at minimum, leaves under:

```text
BOOT AUTH CHARACTER PROTOCOL SESSION SAVE
MAP ZONE AOI ENTITY MOVEMENT COLLISION
COMBAT DAMAGE DEATH RESPAWN SKILL BUFF STATUS AI RNG TIMING
DROP PICKUP ITEM INVENTORY EQUIPMENT PROGRESSION
QUEST NPC SHOP STORAGE CRAFT ECONOMY
CHAT GROUP GUILD TRADE MAIL AUCTION SOCIAL
EVENT INSTANCE PVP SIEGE
INPUT SHELL HUD UI TEXT ANIMATION EFFECT AUDIO ASSET
SECURITY PACKAGE WEB_REGRESSION OPERATIONS
```

This list is a discovery floor, not a closed list. Crystal source inspection may
add domains and leaves. Merely finding every file or every public symbol does
not close this gate: materially distinct success, rejection, timeout,
duplicate, persistence, packet-order, and client-consequence branches remain
separate leaves.

## First execution wave

The first wave establishes the machinery and closes the existing Bichon vertical
slice without narrowing the final objective:

1. generate the pinned Crystal source-file inventory, then the semantic-leaf
   inventory and initial ledger;
2. prove account creation/login, character creation/start, Bichon zone join;
3. prove walk/turn/collision/AOI and correction semantics;
4. prove Q1 -> Q2, combat, real ground drop, pickup, inventory, completion reward;
5. prove logout/disconnect, save and exact reload state;
6. prove Windows input/UI effects and shared Web non-regression;
7. bind signed package and original/native visual evidence to the same revision.

No first-wave success changes the denominator or completion definition for the
remaining Crystal systems.
