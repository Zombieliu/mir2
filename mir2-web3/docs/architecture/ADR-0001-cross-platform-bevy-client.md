# ADR-0001: One authoritative Mir2 client across Web, Windows, Android and iOS

- Status: Accepted
- Date: 2026-08-06
- Owners: Client and platform architecture

## Context

The shipped Web client currently combines a large React game shell with a Bevy
WASM runtime. React owns account screens, in-game UI, asset-atlas production and
parts of presentation/input orchestration; `mir2-bevy-runtime` owns an increasing
share of map/entity rendering and motion. Browser callbacks, `wasm-bindgen`,
`js-sys` clocks, a fixed canvas selector and thread-local pending snapshots are
still embedded in the runtime crate.

Windows, Android and iOS must not become independent games. At the same time,
client extraction must not move MMO authority out of `apps/simulation` and the
Gateway. The migration therefore needs both a platform boundary and an explicit
authority boundary.

## Decision

We will ship one server-authoritative game whose client presentation core and
Bevy renderer compile for multiple platform hosts.

```text
server/
  simulation                 authoritative world and transactions
  gateway                    sessions, routing, protocol boundary

packages/
  protocol                   versioned wire messages
  game-data                  source content and server rules

apps/game-client/
  client-core                replica/presentation math, intents, reconciliation
  client-bevy                shared rendering fallbacks and in-game UI
  platform-web               WASM/PWA host (planned extraction)
  platform-windows           native Bevy desktop host
  platform-android           native Android compile gate; Activity still planned
  platform-ios               Swift/Xcode host (planned)

apps/mir2-launcher-tauri     thin Windows/macOS/Linux WebView shell
apps/mir2-mobile             Capacitor Android/iOS WebView shells
```

The existing `runtime` crate remains the Web production adapter until the
planned crates replace it by verified vertical slices.

### Authority matrix

| Domain | Client may do | Final authority |
| --- | --- | --- |
| Input and movement | Produce intents, predict, interpolate, reconcile | Simulation validates position, collision and speed |
| Combat and skills | Aim, preview, animate and predict feedback | Simulation decides legal cast, hit, damage, cooldown and death |
| Progression and quests | Display replica and pending feedback | Simulation awards XP, levels, quest state and rewards |
| Inventory and equipment | Stage UI operations and optimistic affordances | Simulation commits ownership, slots, durability and stats |
| Economy and trade | Display quotes and submit transactions | Simulation/Gateway commit gold, trade, shop, auction and mail atomically |
| PK, group, guild, Sabuk | Display shared state and submit actions | Shared server runtime decides membership, PK state, ownership and settlement |
| Camera, animation, effects and HUD | Fully client-owned presentation | Client |
| Login, secure storage, billing and updates | Call the platform capability | Platform host plus Gateway verification |

`client-core` must never import server runtime modules or expose APIs that grant
items, XP, currency, ownership or success. Shared deterministic formulae may be
introduced later only when the server remains the deciding caller and parity is
covered by tests.

### Dependency rule

Dependencies point inward:

```text
platform host -> client-bevy -> client-core -> protocol/public content schema
                                      X
                                      | no server-simulation/platform SDK/DOM
```

`client-core` has no Bevy, DOM, window, Tauri, Android, Apple or Xbox dependency.
`client-bevy` adapts renderer-neutral state to Bevy types. Platform hosts own
only lifecycle, window/surface, input devices and platform capabilities.

### UI convergence

The target native clients use shared Bevy in-game UI. React remains the Web
account/launcher shell and a migration reference; it is not the source of a
second native gameplay UI. While both React and Bevy surfaces coexist, they must
consume the same read models and emit the same intent commands.

### Platform capabilities

A single synchronous `PlatformServices` god object is rejected. Capabilities
will be separate asynchronous interfaces such as identity, secure storage,
billing, notifications, updater, lifecycle, keyboard and achievements. Hosts
implement only capabilities they support.

Billing never grants an item directly. A host returns a signed receipt/token,
the Gateway verifies it with the platform provider, and only server authority
commits the entitlement.

### Content and versioning

Public render/content data and secret server rules will be separated before
native patching is enabled:

- `content-schema`: shared identifiers and validated shapes;
- `content-public`: maps and player-visible presentation metadata;
- `content-server`: drop tables, protected economy rules and operations data;
- `asset-manifest`: hashes, sizes, groups and signatures.

Every connection and update must identify four independent versions: launcher,
client build, protocol and content. Incompatible protocol/content combinations
fail before character entry. Asset updates are signed, downloaded to a staging
directory and activated atomically with rollback; the Tauri updater only owns
the launcher/native binaries.

## Delivery gates

1. **M0 — boundary extraction (this slice):** introduce dependency-free
   `client-core`; move snapshot buffering and motion math behind a Bevy adapter;
   keep Web behavior and APIs unchanged.
2. **M1 — host-independent client model:** move replica/read models, intents,
   prediction and reconciliation; add protocol fixtures and deterministic time.
3. **M2 — shared Bevy application:** create an app/plugin builder without DOM or
   fixed canvas assumptions; open a native Windows/macOS development window,
   load a map and connect to the same Gateway.
4. **M3 — Windows distribution:** Tauri 2 launcher wraps the current Web build
   first, then launches the signed Native Bevy sidecar when M2 parity gates pass.
5. **M4 — mobile hosts:** continuous Android/iOS compile gates, then early real
   devices for lifecycle, GPU, memory, touch, suspend/resume and network recovery.
6. **M5 — Xbox:** retain gamepad, safe-area and lifecycle abstractions now;
   create `platform-xbox` only after Windows Native is stable and GDKX access is
   approved.

Each milestone must keep Web green. Native availability does not replace
browser acceptance, and simulator/compile evidence does not replace real-device
acceptance.

## M0 acceptance checks

- `client-core` builds with zero renderer/platform dependencies;
- its interpolation and motion tests pass natively;
- `mir2-bevy-runtime` delegates those calculations to `client-core`;
- the runtime native tests pass;
- WebGPU and WebGL2 WASM artifacts rebuild;
- the Player Web production build and one real gameplay smoke remain green.

## Consequences

This creates a slower but safe migration: existing Web delivery remains intact
while shared code grows through tested seams. Some adapters and duplicate UI
read-model plumbing will exist temporarily. The payoff is one gameplay client,
one protocol and one server authority across all release targets rather than
four drifting implementations.

## 2026-09-30 clarification: consistency and reuse across desktop, Web and mobile

The user explicitly prioritizes multi-platform consistency and reuse because
Android/iOS delivery follows Windows and Web. This confirms the shared-client
direction above. Closing Web gaps by maintaining a second independent gameplay
controller is not the default migration strategy. The work below is planned;
this clarification records no new implementation, build or device acceptance.

### Current boundary and intended ownership

Windows enables `client-bevy/native-ui`; the production Web runtime does not.
Quest guidance, practice, supplies and the Crystal UI are behind that feature,
while Web still owns separate TypeScript interaction and React presentation.
Android has shared reducer and host-bridge foundations, but those do not establish
complete native gameplay or device acceptance. `platform-web` and a native
`platform-ios` remain unfinished extraction targets. Existing Capacitor shells
are separate delivery paths, not proof of native-client feature convergence.

Shared ownership must include:

- client replica/read models and UI state transitions;
- movement/combat intent scheduling and presentation reconciliation;
- quest, inventory, equipment, trade, mail and shop interaction policies;
- operation correlation, pending/unknown/result handling and session recovery
  decisions, with protocol decoding kept at the adapter boundary;
- Bevy in-game UI components, render assets and presentation behavior.

Platform hosts supply transport I/O, clocks, resource loading/cache, persistence,
identity/secure storage, window or canvas lifecycle, IME/clipboard and normalized
input. Extract OS/browser dependencies from current native modules rather than
merely renaming or enabling the `native-ui` feature. Keep independent platform
capabilities small, following the capability rule above. Shared client code must
not import authoritative server rules or acquire gameplay authority.

Consistency means equivalent actions, state and results. Desktop, touch and
gamepad presentations may use different layouts, hit areas and bindings while
emitting the same intents. Mobile must support safe areas, soft keyboards and
foreground/background transitions; the desktop 1024x768 acceptance viewport is
not a mandatory mobile layout. View-specific changes still require their own
visual and input evidence.

### Migration sequence and acceptance

1. **CP-00 — map and isolate the boundary.** Map current Windows and Web intent,
   protocol, receipt and read-model fields before extending the frozen M1-A
   primitives. Identify native-only dependencies and establish native/WASM build
   checks for each extracted common module. Retain existing protocol semantics.
2. **CP-01 — shared newcomer quest slice.** Extract the quest endpoint metadata,
   enabled-profile policy, action eligibility and feedback into a common module
   consumed by both hosts. Explicit zero-NPC diary templates must work alongside
   ordinary NPC quests; absent metadata must not authorize diary operations.
   Generate or validate quest text against the same public content contract,
   including the current N16 Zombie2 target. An interim React view may consume
   the WASM-exported common model/actions, without duplicating those decisions.
   Gate completion on an ordinary N2 accept/equip/finish/save/relogin flow in
   both actual clients against matched Gateway/content, plus rejection and stale
   response checks. Source tests or a Node protocol driver alone are insufficient.
3. **CP-02 — executable shared browser UI.** Build the `platform-web` host around
   the existing WASM renderer and extracted common client. Prove a bounded Bevy
   task/HUD/inventory surface with browser input, fonts, resources and actual
   gateway responses. Retire the equivalent React owner only after this slice
   passes; each surface and operation must have one active owner. React may
   remain for launcher/account integrations. Preserve existing Web identity and
   accessibility/platform capabilities during the transition.
4. **CP-03 — early mobile validation.** Exercise the same slice through the
   existing Android foundation and a bounded iOS host when its build environment
   is available. Verify touch layout, safe areas, IME, suspend/resume and memory
   on real devices early, before expanding desktop-specific UI assumptions.
   Compilation and desktop host tests are separate from device acceptance.
5. **CP-04 — converge remaining interactions.** Migrate navigation/supplies,
   movement/combat input, trade/mail/storage/shop, settings and recovery one
   bounded surface at a time. Use the audited Windows behavior as the reference
   for its verified improvements, retain working Web/mobile capabilities, and
   derive gameplay truth from the shared server/content. Keep unaccepted native
   behavior explicitly unaccepted instead of inheriting an acceptance label.

For subsequent client work, place common behavior and regression fixtures in the
shared layer first. Platform tests then verify that keyboard/mouse, browser and
touch adapters produce equivalent intents and render the resulting state. Track
shared implementation, per-host integration and actual device/player acceptance
separately; a fix in shared source does not prove any deployed artifact contains
it. Existing capacity and release follow-ups retain their evidence and status.
