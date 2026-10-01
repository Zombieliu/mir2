# Native Android received chat source leaf — 2026-10-01

NI-09 bounded progress, not complete Windows chat/UI/player acceptance.
Frozen Windows: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
Parent: `c418c5ab12a887cb0aac8d8237360526657e1d67`.
The source checkpoint below preceded the exact v14 package follow-up at the end
of this report. v14 / `0.1.11-received-chat` is now built/installed from clean
`253b6682f73715b896745ca85a6923be98d0b892`, but focused chat imagery **fails**:
only the system line is visible; the peer line is clipped. v13 lacks this source.

## Change and authority boundaries

- Extract the exact frozen Windows `transform_chat_line` into one shared native
  module; Windows imports it, Android uses the same function. `Chat.message` and
  `ObjectChat.text` remain distinct, channel spelling/defaults and whitespace
  remain unchanged. No duplicated channel/filter/gameplay rule or local echo.
- Java forwards only public Chat/ObjectChat after accepted StartGame plus this
  connection's owner snapshot. A peer object's ID need not equal the owner.
  Before bootstrap, roster/login, unknown/admin packet and terminal paths stay shut.
  Existing16KiB transport admission and authentication remain unchanged.
- Android binds only the already validated player snapshot. It retains same-owner
  chat during map loading; changing owner/name without a data-reset boundary
  rejects the snapshot and revokes gameplay, rather than retaining another
  character's delivered history. Disconnect/render failures clear the host FIFO
  and invoke the existing shared DataReset.
- FIFO32 /128KiB, per-packet/model16KiB. Flush retries only the unaccepted suffix;
  it never coalesces lines, deduplicates legitimate repeated messages or replays
  the accepted prefix. Overflow/malformed recognized payload revokes the session.
- Compile-time preview `chat`/`chat-settings` scenes generate two explicitly
  OFFLINE server-shaped packets through this same adapter, and enqueue each once.
  They do not authenticate or send anything to a Gateway. Shared ChatModel,
  filtering/scrolling/phone decoration remain the existing implementation.

**NI-19 remains open:** the unchanged shared runtime queue may evict an already
accepted ChatLine for critical/ACK pressure. Android host FIFO retry is not
end-to-end lossless delivery. Full outbound/echo/filter/scroll/input/IME/network
and lifecycle acceptance must still be exercised separately.

## Actual source controls

| Gate | Measured result |
| --- | --- |
| Android host /actual preview feature |267 /281 passed,0 failed |
| Shared native-player UI |1205 passed,8 existing ignored,0 failed |
| Mac Windows-host chat filters |2 passed; not Windows-device/full-suite acceptance |
| Java Debug /uiPreview |37 each, five suites each,0 failures/errors/skips |
| Java task execution |42/42 actually executed with `--rerun-tasks` |
| API31 arm64 normal /preview target checks | Both passed |
| Frozen extraction | Signature/body equal except visibility/type namespace/formatting |
| Bounded independent read-only source review | No reproducible remaining P0/P1/P2 reported |

Eight adapter tests cover exact fields, peer/system order, authenticated bootstrap,
retry prefix/suffix, character reset, malformed inputs, line/byte capacity and
legitimate repeated messages; host regressions cover phase/render reset and
terminal same-batch stale Chat/View rejection. Java uses synthetic local TLS,
not a real account/server. The reviewer did not execute tests, builds or device QA.

Failure-first evidence is retained: the first patch used an incorrect working
directory, so its filtered Java run found no tests (`java-before.log`), not a
product failure. The corrected actual new test compiled and failed because no
chat was forwarded (`java-before-corrected.log`/XML; the test then dereferenced
its missing received line). Only after that was the allowlist implemented.
One evidence script was initially run outside repository root and could not
locate XML; the correct root invocation succeeds. Neither setup error is a pass.

Run both audit scripts from repository root with the exact ignored QA target as
their argument. `source-results.json` records12 relevant source hashes and actual
test/target results. APK/cache/original licensed images/keys are not committed.

## Still open

Next build both v14 APKs from a clean source commit, bind SHA-256/version/inputs,
install on the dedicated API31 AVD and inspect the actual shared received-chat
lines. This does not replace approved HTTPS/WSS ordinary-account testing or a
physical phone. Phone HUD weight is absent; full UI/touch/IME/NPC/services/quests/
mail/social/Hero/audio/resources/startup stability and the full frozen Windows
denominator stay open. Existing startup timeouts/emulator host-crash evidence
remains in the [v13 report](../native-android-weight-bars-20261001/README.md).
Remote ref/PR publication must be freshly verified; prior verified head is
f04a74399 and transport failures do not authorize force-push/production changes.

## Exact v14 package and actual emulator follow-up

`verify-package.mjs` /`package-v14.json` bind the12 tested source hashes to clean
253b6682f, before/after status and both APKs. Each actual APK's6647 selected
PNG files and Items/StateItem/UI_32bit metadata match the approved input bytes;
the four weight frames and metadata also match frozen Git. This is not acceptance
of the complete aligned resource release. Rust release cdylibs are inside Gradle
Debug diagnostic variants, not store Release/production signing.

APK directory (relative to repository root, ignored and **not committed**):
`mir2-web3/apps/game-client/platform-android/target/chat-ingress-v14-20261001/final-apks/`.

| APK | Bytes | SHA-256 |
| --- | --- | --- |
| `mir2-native-received-chat-debug-v14.apk` |386270237 |`0f89f4ad5d2d6e39f8385e4e8db027176b72b496ef5a46035d0cc93eed61136d` |
| `mir2-native-received-chat-preview-v14.apk` |390043101 |`2f5eeff2e8871456f98bf9cb118f5d8c1020b682ce76aaa3a9c17916b2931a5d` |

Both `install -r` report Success, installed version14 verified. No app/AVD data
clear, emulator restart/GPU change or Pixel5 action. Only Mir2_API_31_ARM64 /
emulator-5554, Android12 sdk_gphone64_arm64; physical2340x1080 landscape,
density440 (2.75). Device fingerprint is in `device-fingerprint.txt`.
SDK startup waits448/486/367ms report ok; these are not first-frame timings or
startup/soak/stability acceptance. Earlier v12/v13 timeouts/Launcher/host crash
remain distinct. Both owned diagnostic apps were force-stopped at the end;
the same emulator remained running and its installed data was retained.

Manually inspected all three original SDK captures; no screenshot edits:

- `debug-login.png`: empty original login, `Test server not configured.`.
  URL remains empty; no auth bypass, real login or ordinary-account claim.
- `preview-chat.png`: actual soft keyboard and shared phone controls; only
  `OFFLINE system chat` is visible. `OFFLINE neighbor: hello` is not visible.
  **FAIL for full received-chat visual acceptance**, not a pass from queued2.
- `preview-chat-settings.png`: original FILTER/CHAT BOX frame and source controls
  are visible. The settings modal intentionally hides gameplay chat. No new
  tab/toggle/Apply/scroll/typing/send/close/touch interaction was executed here.

PID logs5356/5483/5562 contain205/211/194 line entries, respectively;0 captured
ERROR/fatal/panic/signal matches. Existing Winit insets/GLES warnings are retained;
this is a bounded capture, not a whole-app crash/performance gate. Preview queued2
through the shared adapter in each scene; duplicate logger entries are not duplicate
packets or proof of two visible messages. Runtime evidence and screenshot hashes
are in `runtime-audit.mjs` /`runtime-v14.json`.

The phone geometry diagnosis is narrow: focused height uses
`ROW*rows +2*TAP +12`, but row placement needs at least `ROW*rows +2*TAP +16`.
The last row exceeds `lines_bottom` by4 logical pixels and is hidden. Next create
a failure-first actual-node regression for the latest row/keyboard bounds, repair
the phone-only layout, then build a **new** exact APK and compare captures. v14 is
not retrospectively marked fixed. Phone HUD weight, complete shared UI/actions/
services/chat pressure/JNI/real HTTPS-WSS/device and full Windows denominator stay
open; the completeness goal remains Active.
