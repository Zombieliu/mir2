# Native Android shared-code integration — 2026-10-01

Status: independent source integration, native APK build, automated regressions
and bounded offline emulator/input evidence. **Not a completed Android online
client, complete Bichon asset release, or physical-device acceptance.** The
Android IME and transformed-clipping repairs now pass the bounded actual input
check; final-source package/capture identity is recorded below.

## Source and isolation

| Role | Source |
| --- | --- |
| Android input branch | `codex/android-player-journey` at `5d417ca75a5fe845a7d6f6d9e16c720d55e19b36` |
| APK shared-code Windows baseline | `codex/playtest-registration` at `4b73525f379e5640ad19658730bd4874f7d49109` |
| Windows continuation verified during final QA | `9476f3845e89b9ede164fe18e9c353c1e44f7626` — two additional Windows updater/distribution commits; no Android/shared/runtime code change |
| Normal integration merge | `007df76c9c48569c28197cbcd8e6d41c5cd78da4` |
| APK / selected evidence source | `8d50cb8a369b399b957c4188c204ac4b48739b7b` |
| Integration branch | `codex/android-shared-sync` |

The independent worktree is
`/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync`. It was moved
from the task-owned temporary worktree after builds to preserve the artifacts.
The original dirty
checkout and existing Android branch remain unchanged. Neither Windows branch
was rewritten; this is not a force push, PR merge or production deployment.
The two source histories and their platform-specific acceptance limits remain
separate in the merged progress documents.

The final implementation includes **uiPreview-only** editor ownership and
mail-layout diagnostics. They log only activity/state/layout, not draft text or
credentials. Later documentation/screenshot commits do not change APK source.

## Implemented integration

- Adapt the Android host to current shared feature/API contracts while keeping
  `native-player-ui` independent of the desktop `native-ui` audio backend.
  The existing shared NPC, mail and chat layout/interaction semantics are
  retained rather than copied into an Android-only renderer. Explicit offline
  shared visual tests remain available without enabling Android audio.
- Add Android IME editor-epoch ownership so delayed input cannot write a
  different or reopened field/window. Field validation still precedes draft
  mutation; keyboard input does not submit mail or change gameplay authority.
- Route the Android editor's **whole-field replacement**, including an empty
  deletion, through shared `MailLetterEditor`. The source limit is 500 UTF-16
  units, not 256 Unicode scalar values. CRLF/CR normalize to LF; the fitting
  prefix never splits an emoji/grapheme. Selection is replaced as a complete
  document, composition is cleared, and the real shaped caret is shared with
  Android keyboard avoidance. Stale layout does not manufacture a caret.
- Preserve the process-lifetime `EntityAtlas` when a gameplay `SceneReset`
  clears scene/session entities, so a later valid scene can still resolve
  packaged assets without retaining stale world state.
- Validate each effect PNG against its own immutable metadata instead of a
  frozen file-count assumption. The current effect input contains **2,744
  PNGs**; the old fixed **2,067** count has been removed. This is integrity
  validation, not proof of all online skill/effect behavior.

Real emulator input exposed an ownership problem that screenshots alone had
not shown: the shared desktop Winit IME path took focus from Java `EditText`.
An **Android-only PostUpdate** suppression keeps keyboard ownership on the
Java adapter; actual typed mail body text is now visible on two lines in the
shared renderer. This does not establish sending/receiving mail.

Actual diagnostics corrected the earlier cancellation hypothesis: after the
first Back, `compose=true` and the draft was intact. Bevy 0.19's default clip
update applied translation but omitted `UiGlobalTransform` matrix scaling, so
the restored magnified mail window was clipped to its unscaled dimensions.
The Android host now recalculates affine clipping after `UiSystems::PostLayout`,
preserving ancestor intersections, overflow margins, visible-axis infinities,
`Display::None` and `OverrideClip`; it does not remove body clipping or modify
Windows geometry. Six new regressions pass. Actual emulator typing, first-Back
keyboard dismissal/draft restoration and clicking Close all produce the
expected frames. This does not establish online mail delivery or every keyboard
implementation. Desktop IME and Windows input behavior are unchanged.

## Recorded verification

These are the completed, stage-bound results in this integration round. Final
Android/Java checks and emulator evidence use the Android source tree at
`8d50cb8a3`; the shared suites/check ran before the final Android-only repairs
against the same unchanged shared/runtime source trees. The late Windows
updater commits do not change those trees. This is not Windows updater testing.

| Check | Recorded result | Acceptance boundary |
| --- | --- | --- |
| Android Rust host | 202 passed / 0 failed | Host/contract regressions, not real login |
| Android preview Rust host | 210 passed / 0 failed | Offline preview and fixture path |
| Shared `native-player-ui` | 1,164 passed / 0 failed / 7 ignored | Ignored visual fixtures are not counted as executed passes |
| Shared runtime | 288 passed / 0 failed / 1 ignored | Runtime contracts, not an online player-loop receipt |
| Java Debug unit tests | 30 passed / 0 failed | In-memory TLS/host fixtures |
| Java uiPreview unit tests | 30 passed / 0 failed | Second variant; 60 Java passes total |
| macOS `native-ui` Cargo check | Passed | Desktop feature compilation on macOS; **not Windows test execution** |
| `aarch64` / API 31 native packaging | Passed | Build/package gate only |
| Final 34-scene emulator refresh | 34 ready markers and captures, no panic/FATAL/PathNotFound marker; source `8d50cb8a3` | Offline fixtures; not every interaction or human visual acceptance |
| Actual mail keyboard input | Two body lines, first Back retains draft; Close disposes composition | Offline local editing only, not mail delivery |
| Actual chat settings input | Chat Box tab opens; Two selected; Apply closes the panel and removes the white chat background | Local shared UI state, not online chat delivery |
| Actual world background/resume | Home then activity resume preserves PID `18355` and restores the map frame | Same-process offline lifecycle, not reconnect/process-death recovery |
| Normal Debug startup | Installed and displays `Test server not configured.` | No credentials entered and no server connection |

No Windows executable/test suite, approved Gateway login, full online player
journey or physical Android device result is inferred from these checks.

## APK identity

Both native variants use `versionName=0.1.1-shared-sync`, `versionCode=4`,
`minSdk=31` (Android 12+), `targetSdk=35`, and `arm64-v8a` only.

- Debug is the normal native Bevy/GameActivity application, **not Capacitor or
  WebView**. No Gateway is configured; its unconfigured-server state is an
  intentional fail-closed result, not a successful authentication flow.
- uiPreview is a separate package with explicit offline fixtures and network
  access disabled. A seeded preview screen must not be reported as a real
  account, server character, map position or login receipt.

Both APKs were built from committed source
`8d50cb8a369b399b957c4188c204ac4b48739b7b`, using Rust 1.95.0,
NDK 26.1.10909125 and JDK 17. Locations below are relative to the independent
worktree, not downloadable Git files:

| Variant / package | APK path | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Debug / `com.mir2.web3` | `mir2-web3/apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk` | 527813008 | `1f1acd5e03a790554185012423936f45ddf7d447c9e193fe44b85cdba00111aa` |
| uiPreview / `com.mir2.web3.uipreview` | `mir2-web3/apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk` | 534836976 | `8fe46d14877c855d7bff33168dae15e10f38049d7b76beb36ad896556ce922ad` |

These are diagnostic native packages, not optimized store releases. Gradle
reports that it could not strip `libmir2_platform_android.so` and packages it
as-is. Package size/performance and low-end acceptance remain open. APKs,
build caches, credentials and signing keys are excluded from Git.

## Emulator and local evidence

- AVD: `Mir2_API_31_ARM64`, Android 12 / API 31, `arm64-v8a`;
  fingerprint `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
- Landscape frame: 2340 × 1080; density 440. Existing userdata was not wiped.
- World fixture: render-ready at (302,634), 849 map draws, 8 entities,
  13 entity layers and two local effect instances. These are fixtures, not
  server-owned characters or proof of online position authority.
- One post-resume memory snapshot: PSS 416787 KiB, RSS 516316 KiB.
  This is not a sustained memory/FPS/thermal or low-end performance pass.
- Full 34 captures, process logs, build logs and test logs are retained locally
  in `mir2-web3/apps/game-client/platform-android/target/shared-sync-qa-20261001/`
  (Git-ignored). The capture's source status contains only the coordinator's
  pending documentation updates; application code was committed and clean.
- Password/safe-key previews intentionally protect screenshots with FLAG_SECURE;
  their blank capture is not evidence that confidential UI rendered correctly.

## Resource identity and limits

All inputs below live under the integration worktree's Git-ignored build cache,
not Windows drive-letter paths or Git-tracked binary resources. SHA-256 values
were read from the exact local manifest bytes during this documentation task.

Build-time root was `/private/tmp/mir2-android-shared-sync.x1tuTx/build-cache`;
the unchanged owned cache was moved to
`mir2-web3/apps/game-client/platform-android/target/shared-sync-build-cache/`
under the persistent worktree after verification.

| Input root under the retained build cache | Manifest | SHA-256 |
| --- | --- | --- |
| `shared-ui-assets` | `original-ui/manifest.generated.json` | `890ae5d8f0fca13246654c26c5826575887bf22faa7d44504c88d0dbd0307cca` |
| `local-world-20260911-objects` | `generated/map-atlas/manifest.json` | `b2ec2e3a73125e781ac8aeead31294e88e6510b1dfd62b2b00c2c2407ad609ce` |
| `local-world-20260911-objects` | `generated/native-map-keyed/manifest.json` | `f69e6ffa100f895d8cf337c3ecc0737a8fabfb142af93004bdd0cccea50c610b` |
| `entity-proof-assets-with-archer-bow-20260912` | `bevy-entity-atlases/manifest.json` | `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9` |

The entity override has pack ID
`android-archer-mount-bow-proof-20260912`: five pages and 7,848 rects. It is a
licensed local proof-pack override, **not an approved public Web-release
alignment**. Matching local integrity/provenance checks do not prove that an
external browser, Android build and Gateway use one publicly released pack.

The map proof remains the bounded Bichon viewport near **(302,634)** with
**849 draws**. The full map refers to **7,672 keys**, of which **2,969 source
frames are missing**. This must not be called complete Bichon coverage. Missing
required assets still fail closed; no invented replacement scene or sprite
counts as an authoritative render-ready result.

## Selected final evidence

These are actual final-source emulator captures, not planned screenshot slots.
They establish only the bounded observations below. Visual/human acceptance
and online/physical-device acceptance remain separate.

| Screenshot | Observation | Boundary |
| --- | --- | --- |
| [world-render.png](world-render.png) | Native map, players, monster, item labels/effects visible | Bounded offline assets only |
| [mail-body.png](mail-body.png) | Actual `SyncMail` + newline + `SecondLine`; Send/Close above the keyboard | Local draft only |
| [mail-dismiss.png](mail-dismiss.png) | First Back hides IME, both lines and full shared letter frame remain | Affine body/window clipping retained |
| [mail-close.png](mail-close.png) | Close disposes the composer; inbox remains | `compose=false`, not a send receipt |
| [chat-box-tab.png](chat-box-tab.png) | Chat Box tab visibly opens | Actual pointer input |
| [chat-box-two.png](chat-box-two.png) | Two changes to the selected appearance | Actual local shared state |
| [chat-settings-applied.png](chat-settings-applied.png) | Apply closes panel; white chat background becomes transparent | Not network delivery |
| [npcshop.png](npcshop.png) | Shared NPC/shop and beside-shop inventory composition visible | Seeded fixture, no purchase |
| [world-resume.png](world-resume.png) | Map remains visible after Home/resume in the same process | Not server session resumption |
| [debug-unconfigured.png](debug-unconfigured.png) | Normal Debug explicitly says test server is unconfigured | Not real login |

## Remaining delivery gates

1. Finish phone-first HUD/chat/window layout, touch-target sizing, multi-touch
   joystick/combat and keyboard/device variations. Full-screen map rendering
   does not mean every desktop-scaled shared HUD/window is phone-adapted.
2. Keep Android-host unsupported boundaries visible: the new registration
   interface and detailed quest NPC/Harvest interactions remain explicitly
   unsupported here. Importing shared UI/protocol code does not implement
   their Android network-host handlers or justify a fabricated success.
3. Configure only an approved test Gateway and perform actual authentication,
   character roster/create/select, StartGame, authoritative map/position,
   movement/action and logout/reentry checks. Credentials are not discovered
   from caches or recorded in logs; raw `account_id` is not a login assertion.
4. Complete the online loop with normal server rules, including reconnect,
   background handling and saved-position restoration. No production deploy,
   authentication bypass, duplicate mobile combat/save rules or real save edits.
5. Obtain separately labelled physical-device evidence for touch/multitouch,
   IME, lifecycle, network loss/recovery, rendering and performance. Emulator,
   offline fixtures and build success do not close this gate.

Current flags: `accepted=false`, `visualAccepted=false`,
`realLoginVerified=false`, `onlinePlayerLoopVerified=false`,
`physicalDeviceVerified=false`, `globalParityPercent=null`.
