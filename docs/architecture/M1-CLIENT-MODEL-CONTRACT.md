# M1 client model contract

- Status: Frozen for M1-A
- Date: 2026-08-06
- Depends on: ADR-0001
- Scope: client presentation and protocol adaptation only

## Outcome

M1 creates a deterministic, host-independent client model without transferring
game authority out of Simulation and Gateway. The Web client remains the
behavioral reference until each extracted vertical slice passes parity checks.

The data flow is:

    platform input
      -> normalized intent
      -> protocol adapter
      -> Gateway
      -> authoritative Simulation

    authoritative protocol snapshot
      -> decode/version adapter
      -> revision gate
      -> client replica
      -> renderer read model
      -> Bevy or temporary React surface

The protocol adapter is an edge. Wire packets, legacy Crystal aliases and
transport concerns do not enter client-core.

## Frozen invariants

1. client-core contains no Bevy, DOM, WASM, Tauri, Android, Apple, Xbox or server
   runtime dependency.
2. A client intent expresses a request. It never expresses a successful hit,
   granted item, awarded XP, accepted trade or committed ownership.
3. Simulation and Gateway remain final authority for position, combat,
   progression, inventory, economy, PK, group, guild and Sabuk state.
4. Presentation prediction is disposable. Any authoritative correction wins.
5. Duplicate and stale delivery revisions cannot roll the local replica back.
6. Tests can freeze and advance time without reading browser or operating-system
   clocks.
7. Existing Web command formats and exported wasm-bindgen APIs remain compatible
   throughout M1-A.

## Frozen public primitives

The following renderer-neutral primitives are the only M1 interfaces frozen by
this slice:

- clock::Clock and clock::ManualClock
- intent::IntentSequence
- intent::IntentEnvelope<T>
- intent::IntentSequencer and SequenceExhausted
- reconciliation::SnapshotRevision
- reconciliation::RevisionGate and RevisionDecision
- the M0 interpolation and motion types

Intent sequence numbers are local metadata for ordering, diagnostics,
prediction bookkeeping and acknowledgement correlation. They are not permission
to execute an action.

Clock values are opaque client-local milliseconds. Values may be compared only
after the platform/protocol edge has placed them in the same time domain.
client-core does not call Date.now, performance.now, SystemTime or any platform
clock. Epoch timestamps carried by the existing Web protocol must be normalized
at the adapter edge before a future replica relies on them.

SnapshotRevision is a monotonic delivery revision chosen by the adapter from an
actual server revision/tick when available, or from a local receive counter when
the wire format has none. It is separate from character level, map position,
client intent sequence and wall-clock time.

## Intentionally not frozen yet

M1-A must not invent these types:

- concrete Walk, Run, Turn, Attack, Cast, UseItem or Trade payload enums;
- the complete ClientReplica aggregate;
- wire packet replacements or renamed Gateway commands;
- a final prediction/correction algorithm;
- platform identity, billing, storage, updater or notification traits;
- native-window, mobile-lifecycle or Tauri launcher APIs.

Those interfaces require an explicit mapping of the current TypeScript world
snapshot, Gateway commands, acknowledgements and correction packets. They will
be introduced in M1-B only after Sol reviews that mapping.

## M1-A Flash implementation boundary

Flash may implement one narrow vertical slice: deterministic motion time
injection.

Allowed files:

- apps/game-client/client-core/**
- apps/game-client/runtime/src/interpolation.rs
- apps/game-client/runtime/src/motion.rs
- apps/game-client/runtime/Cargo.toml
- apps/game-client/runtime/Cargo.lock
- focused tests or fixtures directly required by this slice

Required behavior:

- place platform clock acquisition behind a host/Bevy adapter;
- make motion-table update tests independent of real time;
- preserve the existing movement-start and duration interpretation;
- preserve all wasm-bindgen exports and Gateway command payloads;
- keep authoritative snapshot correction behavior unchanged;
- stop and report if a frozen public primitive must change.

Forbidden without a new Sol review:

- apps/simulation/**
- apps/gateway/**
- packages/protocol/**
- gameplay React components
- economy, combat, progression, inventory, PK, guild or Sabuk rules
- platform shells, Tauri, native Windows, Android, iOS or Xbox scaffolding
- dependency upgrades, generated assets, deployments, pushes or history rewrites

## M1-A acceptance

All of the following must pass from the repository project directory:

    cargo +1.89.0 fmt --manifest-path apps/game-client/client-core/Cargo.toml --check
    cargo +1.89.0 test --manifest-path apps/game-client/client-core/Cargo.toml
    cargo +1.89.0 fmt --manifest-path apps/game-client/runtime/Cargo.toml --check
    cargo +1.89.0 test --manifest-path apps/game-client/runtime/Cargo.toml
    npm --prefix apps/web run runtime:build:dev
    npm --prefix apps/web run smoke:bevy-runtime-backends
    git diff --check

Flash must also report:

- changed files and why each file is inside the allowlist;
- test counts and exact failing command if any;
- cargo tree evidence that client-core still has no external dependencies;
- confirmation that no protocol, server-authority or public wasm API changed;
- any generated files left untracked or modified.

Passing native tests or WASM compilation is not real-device acceptance. Windows,
Android and iOS lifecycle/GPU/device gates remain later milestones.
