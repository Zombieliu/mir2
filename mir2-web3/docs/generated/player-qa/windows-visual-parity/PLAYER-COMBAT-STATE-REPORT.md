# Windows visual parity player combat-state report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 6606c043300d63380088c59be148eab274061fb9
combat-state implementation revision: 9eaa62283ec453bfa42f8bc3cbddb4c8811abf09
native-audio implementation revision: 144226df3c7a81ae7e7b15866ae4091d610fffb8
branch: codex/windows-visual-parity
playerCombatStateCheckpoint: bounded
struckActionFeedParityComplete: false
nativeGenericStruckDeathAudioAutomatedCheckpoint: bounded
nativeGenericStruckDeathAudioComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes bounded automated player Struck/Die/Dead/Revive state,
PlayerRevive presentation and Native generic player combat-audio checkpoints.
It does not close VIS-01, VIS-02, Windows visual parity or whole-game parity.

## Source-bound behavior implemented

- Web `ObjectHealth` now owns numeric HP only. `Death`/`ObjectDied` own the
  death incarnation, pose and sound. A durable `deathHandled` marker survives
  packet-first snapshot refreshes and is cleared by Revived or entity removal,
  so the Zone's real `ObjectHealth(0) -> ObjectDied` order is consumed once.
- Web self `Revived` immediately returns to Standing. Remote `ObjectRevived`
  always plays the reverse four-frame Revive action; only `effect=true` adds
  the glow and sound. Neither packet invents HP: the next authoritative health
  packet owns that value.
- Native applies the same self/remote action split and effect gate. Player
  death and revive use Crystal's ordinary `384/387/384` frame records even
  when the actor is riding. Web and Native now omit the mount layer during
  Die, Dead and Revive instead of substituting MountStanding.
- The client-owned PlayerRevive effect is exported from `Magic2/1220..1239` as
  20 frames over 2,000 ms, actor anchored, blend enabled and light 6. Exact
  `M79-1.wav` is 484,496 bytes with SHA-256
  `9098F96106FB880720711FB829B9CCDFEB8DB1883132BC680629FCD0360EA83D`.
- Web ordinary player Struck audio resolves attacker weapon family, target
  armour class and gender flinch. A riding target instead uses Crystal's
  tiger/wolf mount-hit family followed by flinch, with no ordinary body-hit.
- Native now resolves the same ordinary body/armour weapon families, mounted
  tiger/wolf families and male/female flinch. The authoritative top-level
  attacker weapon wins over the derived sprite library, so a mounted attacker
  whose weapon layer is intentionally hidden is not misclassified as unarmed.
  `MountUpdate` overlays riding state before the next snapshot and therefore
  before the next `Struck` cue decision.
- Native male/female death cries wait 100 ms. A real frame-sequence regression
  proves the delayed cue survives intervening effect ticks, while revive,
  remove/hide, map change, logout and generation/session reset cancel it. A
  lethal `Struck -> ObjectDied` batch retains body/mount hit plus flinch before
  the delayed death cry. Owner `Revived + ObjectRevived(effect=true)` aliases
  are deduplicated to one revive effect and one M79 cue per actor incarnation.
- The Native audio allowlist now admits the 15 repository-approved player
  combat clips (`70..73`, `80..83`, `138`, `139`, `144`, `145`, two tiger
  clips and one wolf clip) plus `M79-1.wav`. This also fixes the earlier path
  where PlayerRevive queued M79 but the Native allowlist rejected playback.
- Windows package and copied-Candidate verification require all 20 Magic2
  frames, exact M79-1 identity and all 15 exact combat-audio identities. The
  packaging self-test and copied-Candidate verifier self-test remove required
  inputs and fail closed. No Candidate package was built from this revision.

## Automated evidence

| Gate | Result |
|---|---|
| Independent Crystal/source and final P0/P1 review | PASS for the bounded claim |
| Windows native suite | PASS, 367/367 |
| Rustfmt | PASS |
| Web typecheck | PASS |
| Web full frontend logic | PASS |
| Web actor combat-state behavior | PASS |
| Web game-event/audio behavior | PASS, 41 groups |
| Web player-frame behavior | PASS, including mounted death/revive with no mount range |
| Magic exporter and runtime | PASS, 74 spells plus PlayerRevive client effect |
| Sound exporter | PASS, direct Crystal magic-derived identities included |
| Candidate package self-test | PASS |
| Copied-Candidate verifier self-test | PASS |
| Windows vertical-slice gate self-test | PASS, 9 controls and no global percentage |

## Open player-state and final gates

The Web renderer still has one transient Struck slot rather than Crystal's
ActionFeed. A second real hit is no longer discarded, but it restarts the
current three-frame Struck action instead of queuing one pending action. Native
combat-audio automation is bounded, but Crystal's random tiger-cue distribution
is represented by deterministic event-sequence selection for replay stability,
and audible timing/mix still requires the real-machine human gate. Those gaps
are excluded from the completed automated checkpoint.

The complete player classes/equipment/mount/wing matrix, monster special
renderers, all skill/buff/poison/environment effects, HUD/buttons/panels and
the complete legal semantic denominator remain open. Same-EXE authenticated
live WSS, GPU pixels, real 100/125/150% DPI, 30-minute native soak, human
visual/audio/feel acceptance, clean source binding and formal publisher
signing are also still required. `globalParityPercent` remains null.
