# Shared MP DPR2 compact stability repair

2026-10-01. The whole Windows/Web/Android/iOS goal remains active. This is a
bounded regression repair inside the current MP adoption work, not a new HUD
feature or acceptance of the current 839b8d build.

## Evidence and failure

The MP source boundary is frozen at365 files before this lease. Canonical839b8d,
Next, unchanged runtime limits and the376,235,325-byte clean package pass. Normal
MP browser paths are recorded separately. At600×320 process-DPR2 the new shared
GL2 compact Diary alternates ready/current and textUnlaidOut, renders empty/dark,
and cannot meet the unchanged stable compact input/readability gate. An ordinary
close can work but is not acceptance.

The same current Web adapter with the archived5a runtime has two browser body
hashes and stable compact Diary, both directly and after844Diary/Bag→600 history.
Do not label this only a prior device gap. Source diagnosis v2 has a strong
candidate: idle mutable QuestUiState access triggers render rebuilds and a
host visibility/readiness feedback loop; it has not proven a unique cause.

## Lease and implementation boundary

Root owns architecture, contracts/global docs, source/evidence closure, release/
package integration, actual clients and any account/store/service use. The only
product writer is Sol/high /root/touch_bag_web, and only this file is leased:

- apps/game-client/client-bevy/src/quest_ui.rs

Read all related host, portable Quest, turn-in and MP source. Add meaningful
regression tests in this file. First establish that an idle open Diary with no
pending turn-in does not require a semantic state transition. Avoid taking
mutable QuestUiState or assigning an already-None pending field merely for idle
advancement. Preserve real pending-turn-in advancement, cancellation, feedback,
stale measured-action rejection, authoritative intents and normal close/input.

Do not relax readiness/text/layout/dimension/class/asset/ownership gates or grant
ready from stale/unlaid text. Do not rewrite the renderer, change assets/fonts/
ABI/Web/store/server/gameplay rules, bypass a real state change, or mask the
failure by switching600 to DOM. No code outside this lease unless a concrete
compile or behavioral blocker is reported to root and the lease is extended.
No source edits by reviewers and no overlapping writer.

## Meaningful checks

Run focused idle/no-pending change-detection and renderer/compact regressions,
including a scale-factor2 multi-frame fixture where feasible. A single current
stamp is insufficient. Demonstrate unchanged actual updates/close/stale actions.
Use locked existing Cargo and an isolated target; save commands/logs/results,
before/after hashes and patch into the new QA directory. Keep failed attempts.

After implementation freeze, a separate Sol/high read-only reviewer and root
verify it. Root then builds current canonical native/three-WASM and Next/package
as appropriate, preserving the unchanged JS/raw/gzip/360MiB package limits.
Actual source-matching600×320 DPR2 must keep readable text, stable current/
generation/revision and ready/capturesPointer across distinct frames, consume
ordinary open/close and recover MP. Regress640/844 Diary/Bag/MP, failure/normal
fallback, GPU/lean/legacy where the changed scope warrants. Normal Logout/public
save/relogin, source/body/copy hashes, service cleanup and exact store checks
remain separate evidence. No runtime/model/status/packet/stat/admin injection.

## Preserved boundaries

839b8d evidence, the rejected interrupted case05, corrected-case06 failure, old
runtime comparison, all prior checkpoints and accepted saves remain intact.
Initial StartGame no-ops and low-Warrior initial4 image/fetch requests remain
explicit unknowns, not erased by this repair. Partial/zero MP gameplay and MP
persistence are not proven by the11 public save fields. Desktop process-DPR2/
touch emulation is not a phone/native OS input test. Whole HUD, locale, native
Welcome/physical/trusted capture/normal Logout, devices, performance/public
delivery and human acceptance remain open. Do not complete the overall goal.
