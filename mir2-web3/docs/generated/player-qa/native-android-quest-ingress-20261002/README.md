# Android authoritative quest/dialog ingress — bounded source checkpoint

2026-10-02. Exact source: `ccdd51a90542d866a3b08096469c2d2819656ce4`.
Frozen Windows denominator: `3d735745f1117d42a7859e87604a106351dca935`.
Branch: `codex/android-shared-sync`; whole Windows-completeness goal stays **Active**.

This is source/host-schedule evidence, **not** actual JNI, rendered quest, live
login, quest completion, rewards, saving or physical-device acceptance. The last
installed v19 diagnostics bind to `78e7d2309f6a6b92cea1737c76c5eedfd863e46e` and
do **not** contain this checkpoint. No new APK was built for this source leaf.

## Implementation and exact scope

- Extract the existing Windows quest definition/tracker/objective/reward,
  completed-history, NPC dialogue/nearby and reward-tooltip pure projections into
  `client-bevy/src/native_quest_ingress.rs`. Windows calls the same exports.
  Twenty-five normalized bodies, definition fields and transitive tooltip
  dependencies match frozen Windows source. No client quest rules are added.
- Java forwards only public `NewQuestInfo`/`CompleteQuest` during authenticated,
  listed-character STARTING/IN_GAME. The existing UTF-8 byte bound applies.
  Metadata alone is neither login nor world bootstrap. `ChangeQuest` and admin
  packets are not invented/allowed by this leaf.
- Android stages bounded metadata until a validated authoritative self/name/map
  snapshot, then applies the shared tracker, completed history, nearby NPCs and
  dialogue after shared session reset and before UI mutation. Existing shared
  exact request-ID acknowledgements and pending reconciliation are reused.
- Preserve exact NACKs through later same-frame plain snapshots; reject malformed
  owner/map headers, Hero/foreign data, cache/model/receipt overflow and invalid
  snapshots transactionally. Map transitions close scene dialogue while retaining
  personal definitions/history; logout, failed Start and character change clear
  the appropriate personal state. Accepted shared Exit prevents stale reopening.
- Shared-runtime change is eight lines: a headless initializer for the existing
  native queue used by production receive/schedule tests. It does not change its
  queue policy, authentication, renderer, simulation, Zone or saving.

Owned source scope is eleven files, individually hashed before/after every final
gate and checked against the exact committed Git blobs in
[source binding](raw/source-binding.json). Original dirty main and original clean
Android checkouts retain their original heads/status; no reset/stash/clean/switch.

## Actual source gates

| Gate | Result | Boundary |
| --- | --- | --- |
| Android normal | 315 passed, 0 failed | Host source, serial/offline |
| Android ui-preview | 334 passed, 0 failed | Source tests, not an installed preview |
| Shared native-player-ui | 1230 passed, 10 existing ignored | No desktop audio enabled |
| Shared runtime | 292 passed, 1 existing ignored | Existing ignore retained |
| Java Debug / UiPreview | 41 / 41 passed; 0 skipped | Fresh rerun, ten original XML reports; local TLS fixtures, not real credentials |
| API31 arm64 Debug / UiPreview | Both target checks passed | Check mode, not APK assembly |
| Quest focused host/model filter | 24 passed | Included in broader Android totals, not 24 extra passes |
| Windows pure projections on Mac | tooltip 3, completed 7, tracker 4, Gateway quest 2, exact ACK 1 passed | Overlapping filters, not full Windows acceptance |
| Broader Windows bridge on Mac | **98 passed / 1 failed** | Board geometry fixture failure retained |
| Broader Windows quest on Mac | **39 passed / 12 failed** | NPC marker and navigation/supply/map fixture failures retained |
| Frozen pure projection comparison | 25 / 25 bodies/fields/dependencies identical | Normalized function comparison, not gameplay acceptance |

The two broader Windows filters remain **FAILED**, not green or skipped. Their
compiled count/failure-name sets equal the retained same-Mac comparison after
substituting only the two affected Windows files from published parent
`2c2e5e214693859d7d56e414fb3ff9233e58409c`; other current source is unchanged.
The scoped runner restores both owned files byte-for-byte in `finally`.
This comparison is not a checkout-wide historical Windows build or Windows-device
test. See [bridge and final quest binding](raw/windows-bridge-baseline-binding.json).
No simulation/server business rule or backend percentage changes.

## Failure-first and setup evidence

- [Late dialogue after accepted Exit](raw/rust-quest-late-exit-red.log): compiled
  0 passed / 1 failed before shared UI reply-latch guard; repaired regressions pass.
- [Unbound malformed owner/map](raw/rust-quest-owner-guard-red.log): compiled
  0 passed / 1 failed; explicit headers now require typed bound identity/scene.
- [Java original source with new TLS fixtures](raw/java-quest-compiled-red.log):
  two compiled failures before the public-metadata allowlist. Exact source
  substitution/restoration recorded in [binding](raw/java-red-source-binding.json).
- `java-quest-red.log` is an earlier missing-SDK **setup error**, not product red.
  `candidate-gates` includes a test-only comparison type error and incorrect
  Windows `--lib` setup. `candidate-gates-v2/focused-quest.log` has 20 pass/4 fail
  because the fixture omitted the production queue initializer; v3 has private
  queue-constructor compilation errors. These are preserved separately from the
  two actual product regressions. v4 focused production receive/schedule passes.
- All final [gate records/logs](raw/final-source/results.json), original Java XML,
  intermediate failures and projection/baseline scripts retain their bytes.
  [Manifest](manifest.json) records 74 raw artifacts. `raw/** -text` is installed
  before staging; do not strip log CRLF/trailing whitespace to make a diff green.

## Remaining whole-goal gates and next leaf

NI-10 is now **PARTIAL source ingress**, not complete quests/dialogues. Next:
exact-source diagnostics and clearly offline received-model quest/dialog rendering
on API31, then full shared action/route/pending, actual Java-to-JNI and authorized
server receipts/character boundaries. Do not relabel hand-seeded preview as live.

The prior v19 renderer gate remains **FAIL**: 135 GL0x506 entries across nine
distinct captured PIDs; this source leaf has no GPU repair or new capture. Full
phone layout/multitouch/extreme IME/languages/resources/audio, NI-11 complete
service receipts, NI-12–20 missing producers, real HTTPS/WSS/Zone/save and human
physical-device acceptance remain OPEN/PARTIAL. No completion percentage.

Approved test environment/account remains unanswered; credentials must be entered
locally, not retrieved from other workspaces. No production deployment, genuine
store mutation, login bypass, Window-branch push, PR merge or asset/APK/key upload.
