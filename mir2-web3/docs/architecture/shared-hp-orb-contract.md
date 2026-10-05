# Shared HP orb and browser display handoff

2026-10-01. The portable03 source boundary is closed: 322 indexed source/evidence
hashes match and the independent evidence audit is root-reviewed. The overall
Windows/Web/Android/iOS goal remains active.

## Player-visible scope

Use the same Rust Crystal HP orb geometry and image-node painter in Windows
and the existing opt-in WebGPU/shared WebGL2 UI surface. The first slice shares
only the HP fill image. Current/max values remain the existing visible HUD text.
MP, text/fonts/options, buttons, belt, map, quest tracking, locale and the world
renderer stay outside this code lease until their own replacements are verified.

Native Crystal clips the image from the bottom and moves the destination top
with the fill height. Web's current CSS fixes the destination top while changing
its height. A shared painter removes that difference and starts continuous
read-only HUD composition without capturing world input. This is a bounded
Candidate task, not whole-HUD or CP-02 completion.

## Shared painter

- Add a renderer-portable orb module under client-bevy/crystal_ui. Move the
  existing source/destination geometry and class/level HP-only rule into it,
  preserving the native public geometry names by re-export. Use a common
  image-node construction/update helper for native HP/MP and portable HP;
  another browser copy of the same calculations does not satisfy this task.
- Preserve 104x80 Prguse/4, HP's 50-pixel left half and 100-pixel Prguse/6
  below Warrior level26. Preserve integer truncation, clamp behavior for
  valid normalized model input and native coordinates. Native controls,
  text, options and typed component identities must retain their behavior.
- Portable HP reads the existing authoritative UiReadModel supplied by the
  Quest bridge. No local healing, HP inference, gameplay intent, timer or
  Gateway packet is added. Unknown/missing player or nonpositive maximum
  keeps the compatibility image owner; genuine HP0 with known maximum can
  paint an empty orb.
- Reuse the same owned UI camera/Window and App as Bag/Quest. Add no canvas,
  App, WASM instance or world-renderer path. The image root/nodes pass pointer
  focus and create no Button/Interaction/action consumer.

## Host protocol and layout

The existing Quest snapshot may include optional hpOrbSlot {left, top}, in
logical stage pixels. The browser reads the full orb slot origin from the
existing MainHud/CSS layout, excluding the current fill height. It validates
finite position, positive stage scale, actual canvas/frame bounds and HUD
presence. Geometry loss publishes null rather than retaining a stale slot.

The existing getMir2QuestUiStatus may expose an additional hpOrb descriptor:
supported, ready, generation, revision, hp, maxHp, hpOnly, image/source/destination
and current layout observation. Its frame freshness uses the existing live
status frame. Missing descriptor is unsupported; do not send the new optional
snapshot field to old runtimes that reject unknown fields. Lean GL2 retains
its React image and must not gain reachable UI painting.

Rust consumes the snapshot on the existing monotonic generation/revision
boundary. Its HP status requires a current player/slot, matching Window/layout,
loaded required image, current computed image-node geometry and the current
game generation. A stale model/slot must not acknowledge a new layout. Loading,
asset failure, suspension/layout loss, logout/remount and unsupported runtime
yield the image owner. HP never sets capturesPointer. This first display slice
only hands off on the main HUD with every window/menu closed. Bag, Quest, NPC,
other dialogs and More immediately restore React image ownership and publish
hpOrbSlot=null. Rust hides the image and clears readiness for a null slot;
closed-window reopening requires a fresh acknowledged slot/model/layout.

Web hides only .hud-orb-fill.hp while the current, fresh, identity-matching
shared image is ready and the live closed-window gate still permits it. Keep
the element for layout measurement and keep every other HUD element. The
React HP fallback also anchors its clipped destination at the bottom; measure
the unchanged full-image origin, including valid empty HP, without copying
the Rust crop calculations into JavaScript. The shared canvas stays above the retained base,
but pointerEvents and data-ui-interactive remain controlled exclusively by the
existing Bag/Quest input owner. A closed-window HUD-only canvas stays none.
Existing touch Quest/Bag ordering, More/PWA and compatibility mouse/joystick
routing retain their verified f506 rules. Conditional stacking changes apply
only during the closed-window HP handoff: base artwork lies below the canvas,
all retained MP/bars/text/buttons lie above it, and wide-touch HUD placement
preserves its measured origin. Opening any competing UI restores existing
scene/HUD stacking immediately. General window composition remains a later
gate; this slice must not claim continuous shared HP through open windows.

## Model allocation and exact file leases

The coordinator owns architecture, schema agreement, final source review,
builds/publication to new local artifact directories and actual clients.
Sol/high owns bounded implementation; Luna/medium owns fixed evidence after
testing. No quota/model availability is inferred. One writer per file.

Rust writer lease:
- apps/game-client/client-bevy/src/crystal_ui/hud.rs
- apps/game-client/client-bevy/src/crystal_ui/hud_portable.rs
- apps/game-client/client-bevy/src/crystal_ui/mod.rs
- apps/game-client/client-bevy/src/crystal_ui/hud_orb.rs (new)
- apps/game-client/client-bevy/src/portable_hp_orb_ui.rs (new, local tests allowed)
- apps/game-client/client-bevy/src/lib.rs
- apps/game-client/runtime/src/quest_ui_host.rs

Web writer lease:
- apps/web/lib/bevy-quest-ui.ts
- apps/web/lib/use-bevy-quest-ui.ts
- apps/web/lib/bevy-hp-orb.ts (new)
- apps/web/app/page.tsx
- apps/web/app/original-client-shell.tsx
- apps/web/app/components/original-client-shell-types.ts
- apps/web/app/globals.css
- apps/web/app/components/original-client-game-ui-scene.tsx (local window presence only)
- apps/web/app/components/original-client-mobile-controls.tsx (More presence only)
- apps/web/scripts/test-bevy-hp-orb-ui.mjs (new)
- apps/web/scripts/test-bevy-quest-ui.mjs (only if its existing fixtures require extension)
- apps/web/scripts/test-mobile-input.mjs (existing fixture hook and More callback sequence)

The two additional component files have independent before snapshots in
lease-extension-before.json / lease-mobile-before.json; the mobile test has
lease-mobile-test-before.json. Their separate HP
callbacks publish actual local window/More presence during layout, before
paint; they do not change the existing Quest callback or movement/gameplay
handlers and require no scene traversal or MutationObserver.

No other source/manifest/build/script/config file is leased. No worker runs
services, rebuilds or publishes runtimes, edits native stores, touches staged
server/catalog work or updates global docs. Preserve before copies/hashes;
announce additional necessary files before editing them.

## Verification

- Shared/native geometry and painter regressions at empty/partial/full HP,
  split/low-level Warrior and level26 boundary; native existing HUD tests.
  Portable headless model/layout/reset tests prove image-only focus and
  computed readiness rather than merely testing a duplicate implementation.
- Thin adapter tests: missing old-runtime support, invalid geometry, asset not
  ready, stale generation/revision/frame, model/layout refresh, logout and
  suspension handoff. Existing Quest/Bag/input/TypeScript tests remain relevant.
- Three current WASM builds and native feature/build checks, unchanged byte
  limits, complete immutable source/JS/WASM identity. Retain old f506 artifacts.
  Actual Next/portable output and isolation gate are repeated only for the new
  changed build; the 360 MiB cap stays unchanged.
- Actual ordinary browser GPU/shared GL2/lean routes: HP values, source/clip and
  image ownership on current received hashes; closed-window world movement,
  immediate Bag/Diary/More fallback and normal controls, retained MP/text/bar
  layers, resizing/touch/DPR and asset-fault fallback.
  Restore coordinates/equipment through ordinary input, normal Logout/save,
  retain failures and close owned services. Synthetic partial-HP tests are
  separate from actual ordinary server HP evidence.
- Rebuilt native image proof must reference the new shared source; existing
  accepted Windows saves and ordinary gameplay remain preserved. Emulation,
  physical phones, full HUD/performance/public WAN and human acceptance remain
  separate open gates.

After tests/screenshots pass, update queue/roadmap/frontend QA and goal evidence.
Do not mark the implementation complete from a source patch or build alone.
