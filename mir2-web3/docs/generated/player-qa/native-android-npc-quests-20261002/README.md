# Native Android phone NPC quests — v30 / 2026-10-02

Outcome: **Done with concerns for this bounded offline presentation leaf**, not
completion of the Windows-alignment goal. Exact clean source:
`2c9ad6f96ebc47e07056abbf5a6117c278836083`. The full gameplay denominator remains
`3d735745f1117d42a7859e87604a106351dca935`; the separately observed Windows branch
is `6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`. Its backward move does not authorize
downgrading the denominator, rewriting Windows, or claiming a newly passed gate.

## Source and behavior

Exactly five source files change: shared `quest_phone.rs` / `quest_ui.rs`,
Android `phone_quests.rs` / `ui_preview.rs`, and Android app `build.gradle`.
The optional shared phone NPC view uses the existing controller, quest/reward
model, selection, validation, and close actions. Desktop default 316×466
geometry and five-row presentation remain tested. Authentication, eligibility,
reward grants, server rules, Windows host and render backend are not changed.

The phone NPC window has all eight specimen rows, independent quest-list and
message/reward scroll areas, ≥48dp controls, and nominal 14/16dp text. Separate
NPC view snapshots keep controls alive on unchanged frames and do not invalidate
a held confirmation button when only the NPC payload changes. Covered diary or
detail controls cannot act through the NPC window. Owner changes, reset and
focus loss cancel the relevant gesture. UI-only preview population leaves host
phase DISCONNECTED, shell Login and transport disabled.

[Source scope](raw/source-v30.json), [failure classification](raw/failure-classification.json),
and [measured geometry](raw/npc-geometry.json) retain exact inputs. Raw float font
sizes include 13.999999046325684dp; nominal rounding is recorded separately.
Clipped rows/rewards require scrolling, not simultaneous visibility.

## Failure-first and final gates

Three compiled NPC regressions first fail 0/3, then pass. The full phone suite
subsequently catches an introduced held-confirmation regression at 23/1; its
exact failed source is retained, and the separate view-cache fix passes 24/0.
The first broad preview run fails only the scene-inventory assertion (43 versus
the new 44 scenes); its exact source/log is retained and classified as assertion
maintenance, not a gameplay fix.

Final fresh source-bound gates: Android normal **327/327**, preview **352/352**,
shared native UI **1254 passed + 10 pre-existing ignored**, phone suite **24/24**
(already included in shared total), forced fresh Java **41/41 per variant**, and
actual arm64 API 31 checks for both variants. All six command records bind 41
source inputs before/after to the committed source. Original failures/warnings
are retained rather than replaced by green-only output.

[Final gates](raw/baseline-source-2/gate-commands.json),
[Java results and original XML](raw/baseline-source-2/java-results.json),
[initial NPC red](raw/npc-red.log),
[introduced modal regression](raw/phone-suite-final.log),
[final phone suite](raw/phone-suite-final-2.log),
[initial preview inventory failure](raw/baseline-source/android-preview-final.log).

## Packages and install

Both are native Bevy diagnostic APKs, versionCode **30**,
versionName **0.1.27-phone-npc-quests**, arm64-v8a, min API 31 / target API 35.
Gradle Debug variants contain a Rust release cdylib; these are not store Release
or WebView packages. Both Gateway URLs are empty; preview network is disabled.

- Debug: 387084853 bytes; SHA-256
  `cd33fd3b939b720aa33b6843ec20ca48b8c53575f59a7a74129ea6283f3b07b1`.
  [Local APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/quest-npc-v30-20261002-VVRFSM/final-apks/mir2-native-phone-npc-quests-debug-v30.apk).
- Preview: 391049577 bytes; SHA-256
  `c0680e22afdcf8d84470fa3ddbcdb3c772351ff3406b1c90ca07b8ef352e37b3`.
  [Local APK](/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync/mir2-web3/apps/game-client/platform-android/target/quest-npc-v30-20261002-VVRFSM/final-apks/mir2-native-phone-npc-quests-preview-v30.apk).

[Package binding](raw/package-baseline.json) and
[resource/source audit](raw/source-and-package-baseline.json) verify each
selected 6647 PNGs + three metadata files against approved existing inputs;
four UI_32bit files match frozen Git. This is **not full resource-pack acceptance**.
[Installation](raw/installation.json) verifies prior v29 hashes, actual
data-preserving v30 upgrades, and installed v30 hashes. No uninstall, wipe,
credential input, save mutation, APK/cache/key commit or production deployment.

## Actual emulator interaction and original frames

Device: Mir2_API_31_ARM64, emulator-5554, Android 12/API 31,
sdk_gphone64_arm64, landscape 2340×1080 / density 440. No physical device attached.
The primary agent inspected **all 26 original PNGs**, without resizing/cropping:
17 manual NPC frames, three manual confirmation frames, and six baseline frames.
[Per-frame observations and hashes](raw/manual-frame-ledger.json).

Actual touch sequence: unselected Finish → existing shared reward-selection
alert → OK; quest 2 → right swipe/Down/Up; left Down/swipe/end clamp reaches
quest 8 without changing the right viewport; quest 8 resets its message offset;
right Down reveals both rewards → select B → HOME/resume retains B and offsets;
left Up leaves B selected → Leave closes NPC/list → resume stays closed.
Finish is not tapped after choosing B, and no grant is accepted.

[Alert](raw/ui-npc-quests/8-tap.png),
[quest 8 / reward B after resume](raw/ui-npc-quests/104-home.png),
[Leave and resume retain closed state](raw/ui-npc-quests/129-home.png),
[actual commands](raw/ui-npc-quests/commands.json).
The confirmation regression check uses actual No then same-process resume;
the one-quest diary/detail remains. The separate baseline is normal login
(`Test server not configured`), licensed offline world-render, and received
offline Daily quest metadata. Received offline projection is not authenticated
receipt, and manual NPC/confirmation specimens are not server-granted quests.

## Render gate still FAIL

Canonical cumulative counts use each PID once, not cold + latest twice:

| Scene | PID | GL 0x506 | Fatal/panic named in sample |
| --- | --- | --- | --- |
| Normal login | 20183 | 0 | 0 |
| Offline world-render | 20270 | 0 | 0 |
| Received offline quests | 20358 | 28 | 0 |
| Manual NPC quests | 19449 | 10 | 0 |
| Manual confirmation | 20005 | 0 | 0 |
| Total | five distinct processes | **38** | **0** |

The two renderer files remain byte-identical to published `277ab0a6566b5241c418844e5366865547cd03e0`.
No game GPU fix is implemented; the bounded dependency expansion question remains
unanswered. These counts are not comparable trends across versions/scenes/runs.
Visible frames and no named fatal in these samples do not close zero-error,
long-run stability, live gameplay or physical-device gates.

## Durable byte verification and open acceptance

[Manifest](manifest.json) binds **315 original payloads**; `raw/** -text` preserves
binary/CRLF bytes in Git. From repository root:

```sh
node mir2-web3/docs/generated/player-qa/native-android-npc-quests-20261002/verify-evidence.mjs working
node mir2-web3/docs/generated/player-qa/native-android-npc-quests-20261002/verify-evidence.mjs index
node mir2-web3/docs/generated/player-qa/native-android-npc-quests-20261002/verify-evidence.mjs HEAD
```

Before/after original-checkout records compare only Git HEAD/branch/status, not
recursive hashes of untracked content. No original checkout or Windows changes
were made. This leaf advances NI-10 phone NPC presentation only. Full quest
actions/routes/Help, localization, compact/IME/multitouch, complete NI-11–20,
actual authenticated JNI/HTTPS/WSS, Zone/movement/save, complete resources,
audio/updater and physical-device/player acceptance remain **OPEN**.
Whole Windows-alignment goal remains **Active**; historical failures are retained.
