# Native Android received chat source leaf — 2026-10-01

NI-09 bounded progress, not complete Windows chat/UI/player acceptance.
Frozen Windows: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
Parent: `c418c5ab12a887cb0aac8d8237360526657e1d67`.
v14 / `0.1.11-received-chat` is prepared but **not built or installed at this
source checkpoint**. The existing v13 APKs do not contain this chat change.

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
