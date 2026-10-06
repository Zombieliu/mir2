# Android phone panel workspace — 2026-10-01

Bounded G4 presentation leaf after published source `4ec9b2df8`.
The frozen Windows baseline remains `3f5e61533235921369bc13a7760b4a56b0e467e5`.
The Windows-completeness goal remains **Active**, not complete.

## Baseline and source change

Actual v10 preview APK/source `452398d4c` showed BAG covering status/chat/left
joystick, and a small CHAR/SPELLS window. The unchanged original screenshots
are retained as `inventory-before.png` and `character-before.png`; their exact
APK identity is in the preceding item-geometry QA checkpoint.

- A common Android logical-pixel workspace is used by both panel focus and
  phone HUD adapters, rather than fitting the panel to the entire viewport.
- On supported wide landscape phones, the shared BAG/CHAR tree stays in the
  center; status, one visible chat history row and all six original belt use
  targets occupy a left lane; both thumb-control footprints stay clear.
- The original belt targets/counts reflow to 3x2 at48dp each, keeping shared
  actions, item identity and the user's orientation/visibility preferences.
  Closing the panel restores the normal phone HUD. History is not discarded.
- All four shared character pages use the same real focused panel. The only
  shared source change exposes `OverlayEquipment` to the Android host; there
  is no Windows layout, rule, model, authentication or packet modification.
- Authoring coordinates, image bytes and model state are not rewritten.
  Shared ordinary Button picking uses the moved real Bevy nodes; inventory
  drag continues through its existing inverse panel transform. Actual APK
  tab/cell/drag checks remain a separate gate below.
- Sidebar eligibility is restricted to ordinary core inventory/character
  views. Storage/trade and their companion windows do not share this lane.
  Keyboard/chat-focus and insufficient short/notched areas keep the previous
  layout, explicitly **not** a complete compact-phone or IME acceptance.
- Diagnostic version11 is `0.1.8-phone-panels`. Build/runtime evidence follows
  a clean source commit; setting this version alone proves no APK.

## Source checks and retained failures

- Failure-first focused run: both new character/bag tests FAIL on the old
  focus implementation; `before.log` records tiny CHAR and status occlusion.
- Final Android249/249 and ui-preview260/260, no ignores/failures.
- Shared native-player UI1203 pass /8 existing ignored, fresh serial run.
- Both actual arm64/API31 normal and ui-preview-feature target checks PASS.
- Five new tests cover all four character pages at densities1/2.75/3.5,
  unchanged authored geometry/close reset, BAG workspace bounds, safe insets,
  all six belt targets/counts/preferences and restored HUD/pointer bounds.
- `first-compile.log` retains a test's private-field misuse. `query-arity.log`
  retains the initial Bevy tuple-limit error, repaired with nested disjoint
  filters/data. `normal-float-assertion.log` and `preview-float-assertion.log`
  retain a too-strict new test (33.999996 versus34.0); only its numeric tolerance
  was corrected. These are not silently relabelled as final passes.
- No Java behavior changed; prior Java33+33 is historical, not newly rerun in
  this source leaf. Package compilation is a separate check.

## Open acceptance denominator

This source leaf does **not** close G4: compact landscape reflow, all window
controls' minimum touch sizes, character/body/skill tabs and scrolling, login
forms, IME, all shared service dialogs, languages, audio/performance, multi-finger
runtime evidence and whole UI remain OPEN. Source-only coordinate tests are not
device measurements. Current diagnostic asset packs remain partial/offline.

No online login/WSS, server-authoritative player loop, real item custody,
physical-device or human acceptance follows from the offline specimens below.

## Exact v11 packages and source binding

Both variants were built serially from clean source
`6fe6a17ac229e9ebf0181a1ce02062ae00db71f4`; all four source records match and
all four before/after status files are empty. Gradle build logs, generated
BuildConfig, APK badging, installed version records and `package-v11.json` are
retained. VersionCode11 / `0.1.8-phone-panels`, arm64-v8a, minimumAPI31 /
target35; the generated Gateway URL is empty in both variants. These are
diagnostic Debug packages, not a signed store release or remote Web page.

APKs remain **outside Git**, under local `apps/game-client/platform-android/target/`
`phone-panels-v11-20261001/final-apks/` relative to `mir2-web3`:

| Variant / filename | Bytes | SHA-256 |
| --- | ---: | --- |
| Debug / `mir2-native-phone-panels-debug-v11.apk` | 386070706 | `8b6f054b1381985d4308af05e730ece8647207c02d3ee21d367f9390880580ee` |
| Offline preview / `mir2-native-phone-panels-preview-v11.apk` | 389924694 | `df56ab3705bb4887bac90b1b8d9602e5c2596fa56eef55165a85cac34ea10916` |

`verify-package.mjs` compares each actual APK's6195 original Items/StateItem PNGs,
448 MagIcon/MagIcon2 PNGs and both item metadata files byte-for-byte against the
approved unchanged v9 pack. All match. Items metadata SHA:
`ec0eca414ac6c34e17a218f0ed4396f30607c2485482f8760af7706acddcedfe`;
StateItem metadata SHA:
`7d812a76e9a54b1d0535968a8c21ee2064577e7012572daf79b242221ca72e9a`.
This is bounded pack identity, **not** complete game-resource release acceptance.
`package-audit-first.log` preserves the audit's initial wrong BuildConfig field
name; the script was corrected to the actual `MIR2_GATEWAY_URL`, with final PASS.

## Actual emulator observations

Device: `emulator-5554`, Android12/API31 `sdk_gphone64_arm64`, physical1080x2340,
landscape capture2340x1080, density440. The exact fingerprint and unedited13
captures, installation/start records and PID logs are retained. No physical
phone was connected; no accounts, passwords or save data were used. Both owned
diagnostic apps were force-stopped after this bounded test; no app-data clear,
AVD wipe, production connection or deployment was performed.

- **PASS, bounded presentation:** `preview-inventory.png` and
  `preview-character.png` show the common center workspace clear of status,
  chat and both thumb controls, with six belt targets visible at the left.
  These specimens have empty belt slots; no use or quantity receipt is proved.
- **PASS, bounded actual clicking:** BAG Quest tab at physical(1450,185),
  return ItemsI at(960,185) and Close at(1858,166) respond. Closing restores the
  normal HUD (`preview-inventory-closed.png`). The immediate Quest capture has
  transient missing tab artwork; caption stability remains OPEN.
- ItemsII at(1200,185), including a350ms held input, stays disabled because
  this specimen's second bag is not expanded. Attempts are retained, **not**
  counted as an unlock or enabled-page pass.
- **PASS, bounded character navigation:** STATS I(1280,256), STATS II(1436,256),
  SPELLS(1598,256), CHAR(1116,256) and Close(1668,77) respond at their rendered
  positions; return/close captures show the normal states. Source sword frame30
  remains visible on CHAR. Full armour/skin coverage is not established.
- **OPEN, actual data:** both stats pages have no numeric rows because the
  character preview never supplied `crystal_stats`, which the shared renderer
  requires. This is not proof of a font/layout defect or real stat ingress.
  Character's SPELLS page contains generic fixtures; the separate typed
  skills-scene/original-icon evidence is not relabelled as this scene's pass.
- **FAIL / OPEN, phone completeness:** authored character/body/skill tabs are
  still below the all-controls48dp requirement. Actual frame/item drag, belt
  drag/drop/use, multitouch, compact/IME, login and all service windows remain
  unaccepted. Center-focus geometry tests do not prove drag/drop coordinates.
- **FAIL / OPEN, start performance:** inventory `am start -W` timed out at
 15844ms, normal Debug at13174ms. Character returned OK/Wait8541ms. Later visible
  frames do not erase those timeouts. `debug-login.png` still shows a small login
  form and `Test server not configured`, not a successful login.
- Captured final PID5324/5739/5871 logs have0 ERROR/fatal/panic/signal lines in
 647/336/498 captured lines respectively. This bounded log observation is not a
  whole-device crash, memory, startup or performance acceptance.

`runtime-audit.mjs` checks capture dimensions/hash identities, package/install/
device records and records the manual observations in `runtime-v11.json`.
Screenshot hashes are integrity evidence, not an automatic UI verdict.

Next source leaf: connect the frozen Windows authoritative vitals/stat/wallet
presentation to Android's existing shared model, with lifecycle/owner tests and
server-shaped offline specimens; do not manufacture UI values or mark NI-08,
G2/G3/G4/G5/G6 complete on this package evidence.
