# Native Android v21 — received NPC Exit restores HUD

2026-10-02. Frozen Windows `3d735745f1117d42a7859e87604a106351dca935`.
Exact diagnostic source `545a34abb34415e61a6dd2f274f48914f0d0b698`.
Whole Windows-completeness goal remains **Active**, not complete.

## Actual result and retained failures

The primary inspected all seven original uncropped2340×1080 frames. Two fresh
offline NPC processes show the received title/body/Exit. Actual taps at263,319
remove the dialogue and restore HP/MP, belt, joystick and phone action controls:
[Exit before](raw/v21/touch-device/npc-ingress-before.png),
[Exit after](raw/v21/touch-device/npc-ingress-after.png).
A further fresh-process Exit followed by Menu at2210,958 actually opens the
[phone panel rail](raw/v21/menu-device/npc-ingress-menu.png). This verifies that
one restored control is pickable, not full combat/multitouch acceptance.

The cause was specific to the feature fixture: `npc-ingress` pinned a manual
`UiPanel::NpcDialog`, which continued blocking shared world/HUD guards after
the received model closed. Only that initialization now uses `UiPanel::None`;
the received open model still blocks actions. The legacy manual `npc` fixture
remains unchanged. No shared, Windows, server, authentication, reward or
save rule was rewritten. A compiled regression fails0/1 before this repair,
passes1/1 afterwards and is included in the preview total. The Node template
syntax setup error before any gate ran is recorded separately, not a bug red.

**Renderer still fails:** four distinct PID captures contain33 GL0x506 entries
(normal login33, three received-NPC processes0). Count one latest cumulative
log per PID; do not erase failed login because later processes have zero. The
older [v20 HUD/readability failures and73 GL entries](../native-android-quest-preview-20261002/README.md)
and v19's135 entries remain at their exact old sources.
**Phone quest readability remains FAIL/not rerun here**, not repaired by the
NPC fixture change. These model scenes deliberately request no map pack; the
black playfield is not a world/Zone/render-ready or real network acceptance.

## Exact gates, packages and device

Final serial/offline Android315/315 and preview338/338 pass; fresh Java41/41 in
each variant with42 tasks rerun and zero failure/error/skip; both actual API31
arm64 checks pass. All35 input hashes equal committed source before/after all
five gates. [Source binding](raw/v21/source-v21.json) and original logs/XML
retain those boundaries. Shared/runtime/Windows source did not change; their
older gates, including broader Windows failures, were not freshly rerun here.

Both packages start/end clean at the exact source above. Version21/name
`0.1.18-npc-exit`, minAPI31/target35, arm64-v8a, native Bevy/GameActivity, not a
WebView. Rust release cdylib inside Gradle Debug diagnostic variants, not store
Release. Both Gateway URLs empty; preview is offline-only.

| Local APK, not Git | Bytes | SHA-256 |
| --- | ---: | --- |
| mir2-native-npc-exit-debug-v21.apk | 386804381 | `6e4b7dfaa8b240a0b32f671bef256713f02bd4505613b20cd9e91dc11e1ecf59` |
| mir2-native-npc-exit-preview-v21.apk | 390797349 | `03c344fa50b6a57962974b226b752f5cc482db82bee37404d289bc36b19f76ec` |

Local directory: `apps/game-client/platform-android/target/quest-exit-v21-20261002/final-apks/`.
[Package audit](raw/v21/package-v21.json) records exact absolute locations,
source and selected6647 PNG/three metadata checks per package; UI_32bit inputs
also equal frozen Windows Git. Not all resources/maps/audio/update acceptance.

Only existing emulator5554/Mir2_API_31_ARM64/Android12/API31, native1080×2340,
density440/landscape2340×1080 used. Update installs preserve data; installed
base.apk SHA-256 is checked before captures/taps. Task apps are stopped after
each run; no clear/wipe/AVD change, real credentials or physical device used.

## Integrity and next leaf

[Manifest](manifest.json) binds138 original artifacts, including seven PNGs,
compiled red/green and the separate setup record. `raw/** -text` applies before
staging. No originals are trimmed/cropped/painted or rebound to another source;
no APK/cache/extracted assets/password/key is curated. Run `git-integrity.mjs
--cached` before commit, or `git-integrity.mjs HEAD` after, from repository root
to verify actual Git blobs and35 committed input hashes.
[Runtime audit](runtime-audit.json) retains renderer/readability/open gates.

The new evidence parent is Git-ignored: the initial ordinary add committed
only the five tracked docs and raw index/HEAD checks failed. This is retained
in [publication setup](publication-setup.json), not an integrity pass. Only
this reviewed proof directory is explicitly added in a normal follow-up
commit; working/index/HEAD byte checks must pass before remote publication.

Next: phone quest layout/readability with actual shared picking, bounded GPU
diagnosis, full NI-11 receipts and NI-12–20/model/input/resource denominator.
Real HTTPS/WSS/login/Zone/save, complete UI/actions and physical/human gates
remain OPEN. Do not infer that authenticated live NPC Exit had this fixture cause.
