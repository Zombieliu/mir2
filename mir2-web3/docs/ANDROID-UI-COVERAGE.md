# Android native UI coverage — 2026-09-08

Status: shared player UI assembly and offline Android UI baseline, **not whole
Android UI acceptance or a completed online client**. Work is isolated on
`codex/android-player-journey`; the original checkout and Windows backend
are not edited.

## Current priority — playable flow first

User correction on 2026-09-09: finish the actual game flow before requesting
physical-device acceptance. Prioritize real login/roster/StartGame → shared
scene/bootstrap → authoritative movement and basic player actions. Whole phone
UI/input follows that playable flow; offline polish and memory diagnosis must
not displace it. Physical-device purchase/testing is deferred, not a blocker
to implementing the client. Approved live Gateway still required for live claims.

2026-09-10 keyed-map generation: recovered the exact handoff generator via the
GitHub connector (blob matches local tree), executed an unmodified target-only
mirror with full-pack network fallback disabled. Map0 parses as700x700; 4703
entries emitted and verified byte-identical to local PNGs. Whole-map references
7672 include2969 missing sources (existing generator budget, NOT completeness).
A conservative offline65x111 region around302,634 has565 references/6 missing.
No new runtime code/APK/render-ready. Evidence and exact six keys:
`generated/player-qa/native-android-keyed-map-20260910/README.md`.

Earlier 2026-09-10 local tile build: `build-local-map-atlas.mjs` reuses the existing Web
shelf packer and writes only a fresh external output root. Generated 49 pages
from 2111 exported raw-upload tile frames across 10 libraries (14785921 bytes).
All output PNGs decode; hash-name prefixes, dimensions and rect bounds pass.
Existing-output rejection passes; packer tests5 pass/1 absent-standard-manifest
skip. Outputs remain under Android target/local-world-20260910, outside Git.
Keyed objects, complete Bichon coverage, Android staging/rendering still open.
Evidence: `generated/player-qa/native-android-local-map-atlas-20260910/README.md`.

Earlier 2026-09-10 resource follow-up located existing starter entity atlas pages and
Bichon raw map in the original checkout (read-only). The new Android
`audit-world-assets.mjs` verifies 7 PNG page hashes/sizes/headers, 9650 rectangle
bounds and `0.map.gz` decompression. UI-only input fails as expected. This is
local integrity, not resource release provenance or Android render acceptance.
Derived map-atlas/keyed manifests remain absent. Current-branch Windows assets.rs
was not cached and its automatic promisor fetch timed out; the older original
checkout resolver was inspected only as a discovery hint, not substituted.
Evidence: `generated/player-qa/native-android-local-assets-20260910/README.md`.

Earlier 2026-09-10 local `ced102ba9` feeds shared MapModel/EntityModelSet ingress from
the same validated snapshot. Shared serde validates terrain/entities; map center
uses server sceneView or authoritative self position when the viewport is absent.
Android94/preview97/API31 target check pass. No placeholder render plugins are
enabled. Standard map-atlas/native-map-keyed manifests were absent in both checked
checkouts; Android staging contains only original-ui. Other authorized asset roots
still need checking. No new APK, scene render or live acceptance.
Evidence: `generated/player-qa/native-android-scene-models-20260910/README.md`.

Earlier 2026-09-10 local `0ee203649` adds exact-request shared runtime world-data
receipts. A real native-queue/Bevy-update regression proves that enqueue alone
has no receipt, coalesced latest data is actually applied, and invalid world
schema reports rejection without replacing valid state. Android ignores stale
request IDs and keeps StartingGame even after Applied; decode rejection resets
and disconnects. Runtime215 serial/Android93/preview96 and Android target check
pass. This closes world-data acknowledgement only, NOT assets/render readiness.
No APK or device run for this increment; the APK below remains source `af5acf6ad`.
Evidence: `generated/player-qa/native-android-world-receipt-20260910/README.md`.

Earlier 2026-09-10 local source `af5acf6ad` retains the complete immutable Gateway
world snapshot through Java/JNI and forwards a wire-shape projection to shared
runtime world/HUD ingress. StartGame/self identity gates remain; pending snapshots
are not exposed before acceptance or replayed by later position packets. Host
event sizes/queue memory are bounded, and session/map boundaries request shared
data/scene resets. Nullable wire scalars use shared HUD defaults. Java TLS8,
Android92 and preview95 pass. This is **ingress**, not verified runtime bootstrap:
map/entity asset producers, render-ready acknowledgement, incremental gameplay
packet routing and the playable loop remain open. No live Gateway or device
acceptance. Evidence: `generated/player-qa/native-android-snapshot-ingress-20260910/README.md`.

Earlier host-data bridge increment carries immutable server player/map/x/y in
GatewaySession.View and JNI JSON, validating into Rust HostState rather than
parsing a notice. Non-world phases clear it. Java TLS fixtures8/8, Android88/88,
preview91/91 pass. Initial direct Gradle invocation lacked ANDROID_HOME; rerun
with the existing SDK path passed. Logs `/tmp/android-world-bridge-{java,rust,preview}.log`.
This stores a partial server projection only: shared world/render bootstrap,
full packet/read-model/effect routing, real Gateway login and visible map are
NOT complete. No new APK/screenshot/live acceptance claimed in this increment.

## Implemented

- Local `bc392f889`: Android IME temporarily collapses mail attachment/gold
  presentation and moves the same shared Send/Cancel under the fields, with
  44 logical-pixel targets. IME dismissal restores details and geometry.
  Shared585/normal87/preview90, both APKs and API31 draft/attachment/Cancel
  short flow pass. Evidence:
  `generated/player-qa/native-android-mail-compact-20260909/README.md`.
  Longer run was killed by lowmemorykiller at ~1.2GB RSS; memory/stability
  and process-death recovery remain open, not hidden by the short-flow pass.
- Local `5b909ce45`: ChatSettings now switches Title/466→467 with the tab;
  FILTER/CHAT BOX image families match actual staged PNG labels and actions.
  Failure-first tests, shared584/normal86/preview89, both APKs and API31
  tab/transparent/Apply pass. Evidence:
  `generated/player-qa/native-android-chat-skin-20260909/README.md`.
  Phone target sizing and remaining settings presentation still open.
- `a6af7cb6d`: chat presses are consumed before a changed model rebuilds
  the tree. API31 tab/transparent/Apply now works; source transparent frames
  replace global alpha tint, and offline settings gets a proper shared draft.
  Shared583/normal86/preview89, both APKs pass. Redundant root Pass patches
  removed; no-op Android clamp guard retained. Evidence:
  `generated/player-qa/native-android-chat-input-20260909/README.md`.
  Settings skin parity and mobile whole-screen layout remain open.
- Local `f2246b476` separates the shared belt presentation group and anchors
  it bottom-left when the phone gutter fits, with source-layout fallback.
  Rotation/Close use shared controls; IME hides the layer and preserves its
  preference. Shared581/normal85/preview87/Java8 and both APKs pass; API31
  horizontal/vertical/IME/restore/Close inspected. Evidence:
  `generated/player-qa/native-android-belt-edge-20260909/README.md`.
  Empty-belt evidence only; phone-sized targets and whole gameplay masks open.
- Android enables `client-bevy/native-player-ui`: the exact shared shell,
  HUD/overlays, minimap, chat, notices and quest/NPC plugins used by Windows.
  `native-ui` remains the desktop superset with the audio playback backend.
  Typed UI sound intents are shared separately; this does not implement Android audio.
- Most shared layers retain the 1024×768 fit. `74883f8b3` independently
  anchors minimap image/frame/actions to the top-right safe edge; the panel
  launcher follows below it with 64×48 OS-logical targets. Bottom HUD/chat
  and dialogs are not yet phone-wide responsive layouts.
- One touch owner feeds existing Crystal pointer/drag handlers. Secondary
  fingers cannot inherit a released drag; focus loss releases the pointer.
  This is not a completed multi-touch movement/combat controller.
- Login, character name, change-password, chat/social drafts, mail text,
  map search, locked storage password and guild/trade amount fields connect
  to the OS IME. Draft edits are focus-guarded; shared validation/reducers
  are reused. Done sends one press/release pair through shared keyboard input.
- Back dismisses editing or local panels before the game menu. No local
  editing operation changes inventory, balances, authentication or world position.
- New-character preview now uses the source offset-aware 16-frame preview
  renderer. Crystal `NewCharacterDialog.cs:95-109` places the anchor at
  dialog +(120,250), with `UseOffSet=true`; the old stretched rectangle
  incorrectly covered the Create button.
- Gradle stages external PNGs from ChrSel, Prguse, Prguse2, Title, Items,
  Help, MMap and StateItem. APKs, generated resources and keys remain ignored.

## Offline verification surface

`MIR2_ANDROID_VARIANT=uiPreview` builds a separate
`com.mir2.web3.uipreview` package with compile-time `ui-preview`.
Normal debug/release variants cannot select fixtures using Activity extras.
The preview Java host refuses to connect, log in or StartGame. Every specimen
has an OFFLINE UI PREVIEW label; no fake Gateway login or bootstrap is emitted.

The 33 named specimens cover (including inventory amount and mail compose):

| Area | Specimens |
| --- | --- |
| Shell | login, empty roster, roster, create, password, SafeKey, delete confirmation, connecting, starting, disconnected |
| Player | HUD, inventory, character, skills, quests, options, platform settings, menu |
| Services/social | cash shop, NPC shop, mail, big map, storage, group, guild, accepted trade, chat settings, NPC |
| Other | death, focused chat/IME, help |

Inventory, storage, mail, skills, quests, NPC goods/dialog and trade include
explicitly offline specimens. An empty cash-shop/map view is **empty-state evidence**, not proof of populated
catalogues, pagination or server actions. Password/SafeKey capture restrictions
remain enabled; a protected screenshot is not visual acceptance.

Run `bash apps/game-client/platform-android/capture-ui-preview.sh OUTPUT_DIR`
after installing the preview APK. It waits for each specimen-ready marker,
captures a screenshot and process log, and fails on panic/FATAL/missing asset errors.

## Still open — do not mark the whole UI done

- `native-android-memory-baseline-20260909`: 33 cold-launch specimens pass;
  Help idle RSS ~303MiB, but repeated mail editing grows from 513→820MiB
  across 20 samples, then is killed again at 1171124KB RSS. Native heap
  growth is reproducible; exact allocation/lifetime cause is not diagnosed.
  Prior short-flow passes do not close stability. Instrument before fixing.
- `8423b220c` fixes ordinary/rich hint coordinate conversion under UiScale.
  The earlier "old stage clamp" diagnosis was imprecise: window coordinates
  were being scaled twice. Failure-first/system tests shared581,
  normal84/preview86, both APKs and API31 Mini Map/Mail hint screenshots pass.
  [Hint evidence](generated/player-qa/native-android-hint-scale-20260909/README.md).
  Cutout/IME-rich-hint and whole-screen interaction acceptance remain open.
- `74883f8b3`: first edge-layout slice, shared minimap group/image aligned
  at Android safe edge, shared Bevy button hits verified after moving.
  Expanded → collapse → Mail → compose/IME on API31 passes; shared579,
  normal84/preview86, Java8 and both APKs pass. Hover hints still use old
  coordinates; bottom HUD/chat/world and complete input masks remain open.
  [Edge evidence](generated/player-qa/native-android-minimap-edge-20260909/README.md).
- `ce08b88c9` wraps the inventory amount title into two lines, preserving
  the specimen item name above icon/input. Failure-first layout test,
  shared578/normal83/preview85, both APK builds and API31 IME/Cancel pass.
  [Amount title evidence](generated/player-qa/native-android-amount-title-20260909/README.md).
  Longer-than-two-line names, localization and phone-wide layout remain open.
- `08f658a53` fixes shared Help text-row/footer overlap without deleting
  content or changing pages. Failure-first geometry test, shared577,
  normal83/preview85 and API 31 page1 → page2 touch/screenshots pass.
  [Help evidence](generated/player-qa/native-android-help-rows-20260909/README.md).
  Phone-specific guidance, target sizes and full-screen layout remain open.
- Offline refresh at `a8bb81954`: 33/33 specimen captures/log checks,
  shared576/normal83/preview85 and preview APK build pass. This is smoke,
  not all-interaction acceptance. Visual sampling retains centered 4:3 HUD,
  Help last-row/footer overlap, clipped amount title and mail action area
  obscured by IME. Full-screen layout and these usability gaps stay open.
  [Refresh evidence](generated/player-qa/native-android-33-refresh-20260909/README.md).
- Test-only `7770110db` verifies existing shared session reset from Android:
  Login/ConnectionLost clears ordinary player intents/pending/drafts on the
  next update, without reusing storage request IDs. normal83/preview85,
  shared pending31 and normal APK pass. No duplicate reset implementation.
  [Reset audit](generated/player-qa/native-android-reset-integration-20260909/README.md).
  Live generations, unknown outcomes and real account transitions remain open.
- Local source `13f001d0b` discards unsent shared Gateway effects at disconnect,
  reconnect start, logout and inactive/unfocused boundaries, preserving local effects.
  API 31 queued revive → Home → resume logs one discarded command; normal82/
  preview84 and both APK gates pass. Other queues/in-flight/generation wiring remain.
  [Session evidence](generated/player-qa/native-android-ui-session-20260909/README.md).
- Local source `06a5dea1a` adds death-only Android Revive touch action.
  API 31 touch emits a shared TownRevive intent, without restoring HP;
  alive state hides the target. normal80/preview82 and both APK gates pass.
  [Revive evidence](generated/player-qa/native-android-revive-touch-20260909/README.md).
  This is UI-to-queue only: real socket/receipt and lifecycle wiring remain open.
- Local source `bed749479` adds a guarded notice-body IME touch target.
  API 31 repeated Back/retap retains the draft; Cancel restores the notice.
  Shared576, normal78/preview80 and both APK gates pass. Live publication
  and physical-device acceptance remain open; source is local only.
  [Retap evidence](generated/player-qa/native-android-guild-retap-20260909/README.md).
- Local source `77edc7afb` fixes guild notice IME panning text offscreen.
  API 31 eight-line input and Back/Cancel are verified, normal78/preview80
  and both APK gates pass. Retap is addressed above; live publication remains open.
  [Guild evidence](generated/player-qa/native-android-guild-ime-20260909/README.md).
  Remote remains Draft `99a1cd121`; this source is local only.
- Local source `ac1a37d55` fixes touch rail Close leaving Help open, using the
  shared complete-close method. API 31 before/after replay and host tests
  76/76 normal, 78/78 preview pass; modal/inactive guards remain intact.
  [Close evidence](generated/player-qa/native-android-rail-close-20260908/README.md).
  Read-only remote check remains `99a1cd121`, Draft; no push retry this round.
- Latest local editor source `fafbc716f` adds trade/guild amount IME geometry
  and multiline mail/notice input. Trade amount reopening/cancel and two-line
  mail text have API 31 evidence; guild-specific populated cases remain open.
  Android tests 74/74 normal, 76/76 preview; shared UI 575/575. These later
  commits/evidence are not yet confirmed remote: PR head last verified `99a1cd121`
  after HTTP 408/SSH push failures. [Latest editor evidence](generated/player-qa/native-android-editors-20260908/README.md).
- Source `79d6d7ce2` fixes GameActivity Back bypass, shared modal cancellation,
  same-field IME reopening and inventory amount bounds. Source `99a1cd121`
  bounds mail compose with six-row attachment paging. API 31 amount/recipient
  input, Back priority, Help close and page-two attachment selection have
  targeted evidence. Other nested fields and multiline IME are still open.
  [IME and mail evidence](generated/player-qa/native-android-ime-20260908/README.md).
- The two initial touch gaps have targeted API 31 regression evidence at
  source `62344b0e5`: same-frame tap edges and hit-test ordering are fixed;
  bag/help keep a 24 OS-logical-pixel top gutter. Bag dragging and short-tap
  inspect → Panels → Close now pass. See
  [touch regression evidence](generated/player-qa/native-android-touch-20260908/README.md).
  This does not establish every window's touch behavior or physical-device acceptance.
- The real login socket still lacks the shared gameplay bootstrap/read-model
  projection and complete gameplay/transaction intent dispatch. Real StartGame
  remains on the transition screen, rather than fabricating a playable map.
- Character creation/deletion/password changes and gameplay operations need
  actual request/result wiring and an approved Gateway test environment.
- Populated quests/skills/shops/maps, all nested dialogs and item operations
  need scenario-level interaction evidence, not merely a screenshot of a root panel.
- Dedicated virtual joystick, multi-touch combat/skill controls and the actual
  revival round trip require further Android work; desktop keyboard affordances
  in the shared UI do not prove phone usability.
- Android audio, exact Crystal visual/feel parity, physical-device touch/IME,
  lifecycle/reconnect, low-end performance, signing and human acceptance remain open.
- The original chat frame PNG is white in this source pack. This work does
  not silently recolour source assets or claim original-client A/B acceptance.
- Full Windows audio-enabled regression is not established here: this local
  crates.io cache lacks Bevy audio 0.19. Shared visual-feature tests are a
  separate denominator.

Evidence: [Android player-UI baseline](generated/player-qa/native-android-player-ui-20260908/README.md).
No production deployment, real account login or save mutation was performed.
