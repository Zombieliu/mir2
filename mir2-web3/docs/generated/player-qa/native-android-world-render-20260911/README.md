# Native Android authoritative world-render checkpoint (2026-09-11)

## Scope and source

- Branch: `codex/android-player-journey`
- Implementation commit: `75d0eac4604fc632c7679edc299a5b1a16565a56`
- Host: the isolated macOS Android worktree; the original checkout was not
  switched, stashed, reset, or cleaned.
- Scope: native Android map/object/entity rendering and render-ready handoff.
  No Windows backend task, production deploy, live save mutation, or auth
  bypass is included.

## Implemented

- The Android producer converts accepted authoritative world snapshots into
  the shared `MapRenderState` and `EntityRenderState` contracts.
- Bichon type-100 cells now resolve ordinary atlas draws plus keyed and
  additive standalone objects. Missing source objects stay unresolved and
  block readiness; no synthetic replacement is used.
- Server object IDs, kinds, grid positions, directions, class and sprite
  descriptors drive visible self/player/monster/NPC object entries. Player
  body, hair, primary/secondary weapon and mount layers use the shared
  content atlas and Crystal standing frame bands. A missing requested pose
  does not fall back to frame zero.
- Map/entity state is queued before bounded raw-RGBA image batches. Bulk image
  resources no longer consume the state snapshot eviction quota. Each Bevy
  frame ingests at most 8 MiB per image family while allowing one oversized
  page to make progress.
- StartGame/map-change completion is released only when the same exact request
  remains complete for two consecutive rendered frames: map live count is
  nonzero, visible map/entity unresolved counts are zero, and the self actor
  has at least one live layer. A newer snapshot replaces the one deferred load;
  stale work and stale receipts cannot release a later scene.
- The Java WSS state-machine regression now covers an in-game world refresh and
  `MapChanged -> STARTING_GAME -> destination world snapshot -> IN_GAME`.

## Immutable local asset evidence

Assets are generated/staged outside Git and are not part of either commit.

| Artifact | Result |
| --- | --- |
| Raw Bichon `0.map` | 12,740,008 bytes; SHA-256 `ed4783215ffa989658f79892c2dd6720753fb111e1102cb14da8e6182d88d7f6` |
| Map atlas manifest | 49 pages, 2,111 sources, 14,785,921 compressed image bytes; SHA-256 `b2ec2e3a73125e781ac8aeead31294e88e6510b1dfd62b2b00c2c2407ad609ce` |
| Keyed object manifest | 7,672 references; 4,703 emitted; 2,969 missing outside the checked viewport; 28,768,420 image bytes; SHA-256 `f69e6ffa100f895d8cf337c3ecc0737a8fabfb142af93004bdd0cccea50c610b` |

The checked `(302,634)` viewport resolves 607 atlas draws and 242 standalone
draws (849 total) with zero missing visible bindings. This is a viewport
checkpoint, not a claim that the incomplete local export covers every Bichon
object or every game map.

## APKs

| APK | Bytes | SHA-256 | Purpose |
| --- | ---: | --- | --- |
| `android/app/build/outputs/apk/uiPreview/app-uiPreview.apk` | 374,415,965 | `184a8bfe505e1f06b696c2ff4903c5d01e033253a64c0e1780d01b656a591419` | Explicit offline `world-render` fixture; separate package and visibly labelled `NOT LIVE GAMEPLAY` |
| `android/app/build/outputs/apk/debug/app-debug.apk` | 330,063,709 | `c65a51641af558e5496a9e1cf6dc4f2a904d172a4dc20eed15509602d8922603` | Normal native client, built with an empty Gateway URL for safe configuration-negative verification |

APKs, debug signing material, caches, source resource packs, screenshots and
logs remain local/ignored and are not committed.

## Automated gates

- `platform-android`: 110 passed, 0 failed, including real local atlas/map
  parsing and strict no-frame-zero fallback.
- shared Bevy runtime: 232 passed, 0 failed.
- Java `GatewaySessionTest`: 8 passed, 0 skipped/failed/errors after a forced
  Gradle rerun. It uses TLS MockWebServer and is protocol evidence, not real
  account acceptance.
- `MIR2_ANDROID_MODE=check`: Rust 1.95.0 arm64 API 31 target check passed.
- Both UI Preview and normal debug package gates passed from implementation
  commit `75d0eac46`.

## API 31 emulator evidence

- AVD: `Pixel_5_API_31`
- Device model/ABI/API: `sdk_gphone64_arm64`, `arm64-v8a`, API 31
- Physical framebuffer: 1080x2340 at density 440; captured landscape frame:
  2340x1080. The existing AVD was retained and not wiped.
- Software SwiftShader rendering was used, so this is not physical GPU/device
  performance evidence.
- Cold-start strict log:
  `ANDROID_WORLD_RENDER_READY ... center_x=302 center_y=634 map_tiles=849 entities=2 entity_layers=2`.
- Immediate post-ready screenshot (local only):
  `platform-android/target/evidence/android-native-world-render-20260911/pixel5-api31-world-render-184a8bfe-ready.png`,
  SHA-256 `789200562d6fbbf74a76996f9bb2890eddef094a3c384449460403d2ed6dfc22`.
  It visibly contains terrain, buildings, shadows, HUD/minimap, self actor and
  monster; no post-receipt black-frame race was observed.
- Home/background then foreground preserved the same process (`6969`) and a
  visible resumed frame; this proves only the offline Activity/render lifecycle.
- The normal APK cold-started into the shared Crystal login screen with
  `MIR2_GATEWAY_URL = ""` and visibly reported `Test server not configured.`
  No fake roster, login result, or world state was injected.

## Acceptance boundary and remaining work

| Gate | Status | Reason |
| --- | --- | --- |
| Native authoritative map/object/entity code | Implemented and automated | Exact snapshot/request contracts and strict render receipt are covered |
| API 31 visible world frame | Passed on emulator fixture | Offline, labelled fixture only |
| Real HTTPS/WSS login -> roster -> StartGame -> rendered world | **Not run / open** | No approved `MIR2_GATEWAY_WS_URL` or test account was available; credentials were not sought or persisted |
| Complete live gameplay command loop | **Open** | Outbound reducer commands are wired by later commit `f1ee2148c`, but ordinary inbound gameplay/receipt projection and live validation remain incomplete |
| Physical Android device acceptance | **Not run / open** | `adb devices -l` listed only `emulator-5554`; no physical phone was connected |
| Whole-map/all-map object closure | **Open** | The local Bichon export lacks 2,969 referenced standalone sources and no other complete map pack was supplied |
| Touch/IME/network/background/soak on real device | **Open** | Requires an approved endpoint/account, physical phone, and real player journey |

This checkpoint therefore closes the bounded native rendering implementation
and emulator-visible-frame slice. It does not claim a real online player loop,
physical-device acceptance, global Candidate completion, or human acceptance.
