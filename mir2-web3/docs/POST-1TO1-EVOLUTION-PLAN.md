# Post-1:1 Evolution Plan

Last updated: 2026-04-26

Purpose: define how to evolve this project after the Crystal / Mir2 1:1 Candidate baseline without confusing product changes with parity regressions.

Modernization RFC: `docs/TECH-MODERNIZATION-RFC.md` captures the current first-principles architecture discussion for Postgres, Redis, global single-world zoning, Bevy + NextJS, operations backend, and NPC DSL direction.

## Current Baseline

The current project state is a verified MMORPG foundation with a Crystal / Mir2 compatibility baseline:

- Automation status: `100.0% Candidate`.
- Backend/server tracked-slice parity: `99.70%`.
- Real full-project accepted 1:1: `roughly 90.0%`.
- Remaining 1:1 blockers: live Crystal packet comparison, missing local `Server.MirDB` import evidence, and human visual/feel acceptance.

This baseline should be preserved as a regression reference. Future product work is allowed to diverge from Crystal, but each divergence should be explicit.

## Direction

The project is moving from pure 1:1 reconstruction toward a custom MMORPG built on the proven Mir2-style foundation.

Crystal behavior remains useful as:

- a compatibility reference;
- a packet/protocol and gameplay sanity check;
- a source of imported data and edge-case behavior;
- a fallback oracle when a behavior should remain Mir2-like.

Crystal behavior is no longer automatically the product target once a feature is intentionally redesigned.

## Change Categories

### Preserve As Baseline

Keep these stable unless a migration plan and tests exist:

- protocol packet codecs and trace harnesses;
- deterministic simulation tests;
- account/session boundaries;
- save/load compatibility during migrations;
- generated Crystal data import scripts;
- Stage 5 smoke coverage and screenshot manifests;
- documented 1:1 parity evidence.

### Allowed To Evolve

These areas are expected to change for the new MMORPG direction:

- database schema and persistence backend;
- cache layer and runtime state storage;
- login, account, and character-selection UI;
- frontend layout, art direction, HUD, and panel style;
- NPC script text format and parser;
- quest authoring model;
- economy, shop, auction, mail, and item progression;
- combat pacing, skill design, monster tuning, and world events;
- admin/live-ops tooling.

### Needs Extra Care

These are high-risk because they touch both product behavior and baseline compatibility:

- account identifiers, character IDs, item unique IDs, and save schema versions;
- inventory/storage/equipment persistence;
- NPC script execution state and saved flags;
- packet-visible behavior for live clients;
- cache invalidation around character state, world state, and item state;
- login/session security.

## Planned Workstreams

### 1. Database And Persistence

Goal: replace or extend the current local JSON/account-store model with a production-ready database layer.

Planned decisions:

- choose the first production database target;
- define account, character, inventory, storage, mail, guild, auction, NPC flag, and world-event tables;
- decide migration strategy from current save JSON;
- keep deterministic test fixtures for simulation and gateway tests;
- add schema versioning and rollback guidance.

Acceptance:

- existing account/login/start-game tests still pass through the new storage adapter;
- migration tests cover old save JSON into the new schema;
- persistence tests cover disconnect, reconnect, save/reload under load, storage, mail, and NPC flags.

### 2. Cache And Runtime State

Goal: add a cache layer without breaking deterministic gameplay tests or persistence correctness.

Planned decisions:

- define which data can be cached: account session, character snapshot, map metadata, item manifest, NPC scripts, leaderboard-like views;
- define which data must remain authoritative in the simulation/runtime;
- define cache invalidation on item movement, storage, mail, auction, NPC flag changes, and character logout;
- decide whether cache is in-process first or external later.

Acceptance:

- tests prove cache misses and cache hits return identical gameplay-visible state;
- write-through or invalidation behavior is covered for inventory/storage/mail/NPC flags;
- load tests verify no stale character state after reconnect.

### 3. Login And Account UI

Goal: redesign the login and select flow as product UI, no longer constrained to Crystal pixels unless explicitly required.

Allowed changes:

- new landing/login layout;
- account creation and password UX;
- character select/create/delete presentation;
- language selector placement;
- error and loading states;
- mobile/compact behavior.

Acceptance:

- Stage 5 login/select smoke remains updated for the new UI;
- old Crystal visual parity rows are moved to accepted-divergence or product-design rows;
- keyboard submit, account creation, delete confirmation, recreate, and start-game paths remain automated.

### 4. NPC Script Text And Parser

Goal: support a cleaner authoring format while keeping a compatibility bridge for imported Crystal scripts where useful.

Planned decisions:

- define the new script text format or DSL;
- decide whether Crystal script import compiles into the new format or remains a separate compatibility parser;
- define parser diagnostics, line/section references, and safe failure behavior;
- define persistent NPC variables and player flag storage;
- define localization and link/input syntax.

Acceptance:

- current Crystal NPC parser tests remain green or are explicitly split into compatibility tests;
- new parser has unit tests for SAY, ACT, IF/ELSE, links, inputs, variables, item/gold checks, quest checks, and service open actions;
- runtime NPC interaction smoke covers both imported compatibility scripts and new product scripts during the transition.

### 5. Product Gameplay Layer

Goal: build new MMORPG systems on top of the stable runtime.

Potential changes:

- new starter flow;
- new quests and NPC text;
- revised progression and item economy;
- new monster tuning and combat pacing;
- new shop/auction/mail rules;
- new guild/social loops;
- world events and live operations.

Acceptance:

- product behavior has its own tests and screenshots;
- Crystal parity docs are not used to reject intentional product changes;
- any compatibility-breaking decision is recorded as an accepted divergence.

## Documentation Rules

When a future change intentionally diverges from Crystal:

1. Update this file or a dedicated product spec.
2. Mark the old 1:1 expectation as preserved baseline, accepted divergence, or deprecated.
3. Add/adjust tests for the new target behavior.
4. Keep `docs/CRYSTAL-1TO1-ROADMAP.md` truthful: do not rewrite Candidate evidence as Accepted.
5. Keep Windows/live Crystal blocker language until those gates are actually closed.

## Immediate Next Planning Tasks

- Finalize `docs/TECH-MODERNIZATION-RFC.md`.
- Draft the database target schema and adapter boundary.
- Draft the cache boundary and invalidation rules.
- Draft the new login/select UI product requirements.
- Draft the NPC script parser strategy: compatibility parser, new developer DSL, or compiler from Crystal scripts to new script IR.
- Decide whether the next branch should be named as a product-evolution branch rather than a parity round.
