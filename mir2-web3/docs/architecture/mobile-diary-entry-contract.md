# Mobile More Diary entry: bounded contract

2026-10-01, CP-03 follow-up to the frozen WebGL2 shared UI prototype109d.
The Web is a normal player's complete install-free entrance. Landscape touch
players need a discoverable Diary action that reaches the same Quest state,
shared renderer and existing compatibility owner as other platforms.

## Behavior and ownership

Add one localized Quest button to the existing mobile More panel before Char
and Bag. Reuse Shell/Page `onToggleQuestLog`, tutorial panel notification and
the existing touch secondary-window exclusion. Close the More row after its
action. Do not add a Quest reducer, request schema, operation ledger, quick-slot
shortcut or inventory-tab alias. Original Crystal HUD/menu art is retained.

When this action opens Diary while the original system menu is open, dismiss
that local menu so it cannot block the shared Quest owner. A narrowly scoped
touch Diary-opening effect in the existing Game UI scene is allowed; do not
close unrelated dialogs or change desktop menu behavior. Verify the actual
More+system-menu route instead of assuming the menu is unreachable.

Raise compact panel targets from36 to40 CSS px (regular44 unchanged). Reserve
the new four-target top row from touch-landscape PWA actions and permit long
localized action labels to wrap within the available area. Install/fullscreen
must stay readable and hittable; this does not redesign combat/joystick targets
or establish all-HUD touch acceptance. Use measured actual viewport/stage
geometry, wait for existing menu animation to settle and record hit-test points.

## Bounded writers

Root owns this contract, queue/roadmap/gaps/goal docs, integration and actual
browser QA. Sol/high may edit only:

- `apps/web/app/components/original-client-mobile-controls.tsx`
- `apps/web/app/components/original-client-stage-presentation.ts`
- `apps/web/app/original-client-shell.tsx` (mobile prop wiring only)
- `apps/web/app/components/original-client-game-ui-scene.tsx` (touch menu dismissal only)
- `apps/web/app/pwa-game-shell.css`
- `apps/web/scripts/test-responsive-stage.mjs` (existing meaningful size regression)

Do not edit Page, Rust/runtime artifacts, auth, gateway, game state or unrelated
staged work. Back up each write-set file before editing, retain focused test
and TypeScript logs and freeze final source before root begins actual QA.
Luna/medium reads sealed evidence independently; no concurrent browser writer.

Actual first-attempt844×390 touch login shows PWA actions covering the ordinary
English/Chinese/Portuguese button centers. The blocked waits and screenshots
are retained before credentials/gameplay. A bounded same-round CSS repair is
authorized: center/limit the short touch-landscape login PWA group, clear the
language/audio controls and login form, and retain usable install/fullscreen.
Keep the initial source freeze and create a separate final freeze after repair.

Two subsequent600 React Diary observations authorize bounded same-round PWA
repairs: keep install/fullscreen in one row and wrap text within each button;
then hide PWA only while the real React Diary window mounts during touch
gameplay and restore it on close. Do not alter shared readiness/owner flags to
obtain that handoff. Retain separate before/final/final2/final3 source variants.
The last is the accepted product source for this slice.600's335px shared Quest
minimum remains intact; fixing entry/close overlap does not accept tiny fallback
readability. A compact shared layout is the next bounded task.

The failed Chinese640 test used a stale600 More coordinate and emitted ordinary
world movement. Its captures,20 walks and two further normal facing-restoration
steps remain archived. Root recovered through ordinary Revive/movement/logout,
not store/state injection. Stable explicit QA dependencies now require matching
viewport/frame/canvas/More geometry and repeated requestAnimationFrame hit
inspection; DOM taps use native CSS coordinates, while drawn shared controls
use their existing logical-to-canvas mapping. HTML lang is not the game-locale
oracle; record ordinary pressed language buttons and translated entry labels.

## Verification

Run the responsive-stage and PWA checks plus adjacent shared Bag/Shell tests,
then TypeScript without incremental output. The existing109d four runtime
artifacts must stay byte-identical; this slice changes only the Web host.

At actual600×320,640×360 and844×390, verify More/Quest/Char/Bag target dimensions,
viewport containment, PWA separation and elementFromPoint at center and four
documented interior corner samples. Inspect localized English, Chinese and
long Western Quest labels through ordinary UI language selection. PWA copy
currently follows `navigator.language` independently: record a browser
accept-language emulation and fresh navigation for its en/zh/pt variants,
without injecting translated DOM or game state. This does not repair that
existing separate language preference. Do not
claim physical safe-area support from desktop geometry.

The current HTML declaration remains en for other selected UI languages and is
an open accessibility gap. Portuguese Quit/Logout both use Sair; distinguish
their existing data-system-menu-action when exercising normal logout. Neither
is a localization repair in this bounded entry task.

With ordinary CDP touch input exercise More→Quest→detail/paging→close→More→Bag,
then the menu-open More→Quest case. Verify the More row closes, shared Quest
and Bag retain one current foreground owner, no UI gesture emits world movement
and portrait compatibility remains usable. On normal logout record a public
save and ordinary relogin restoration. Recheck default WebGL2 React and WebGPU
shared paths without inheriting tests from the preceding109d archive.

Compilation, CSS predictions, actual desktop touch emulation, physical
Android/iOS, startup/memory/performance and final human acceptance remain
separate gates. A failed route remains archived and prompts a bounded repair.
