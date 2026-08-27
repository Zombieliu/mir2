# Windows visual parity player combat-state report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 6606c043300d63380088c59be148eab274061fb9
implementation revision: 9eaa62283ec453bfa42f8bc3cbddb4c8811abf09
branch: codex/windows-visual-parity
playerCombatStateCheckpoint: bounded
struckActionFeedParityComplete: false
nativeGenericStruckDeathAudioComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes a bounded automated player Struck/Die/Dead/Revive state and
PlayerRevive presentation checkpoint. It does not close VIS-01, VIS-02,
Windows visual parity or whole-game parity.

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
- Windows package and copied-Candidate verification require all 20 Magic2
  frames and the exact M79-1 identity. Their self-tests remove the final frame
  or sound and fail closed. No Candidate package was built from this revision.

## Automated evidence

| Gate | Result |
|---|---|
| Independent Crystal/source and final P0/P1 review | PASS for the bounded claim |
| Windows native suite | PASS, 360/360 |
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
still lacks the generic player body/mount hit plus flinch chain and the 100 ms
male/female death cry, including delayed-cue cancellation on remove/revive/map
change/logout. Those gaps are excluded from this checkpoint.

The complete player classes/equipment/mount/wing matrix, monster special
renderers, all skill/buff/poison/environment effects, HUD/buttons/panels and
the complete legal semantic denominator remain open. Same-EXE authenticated
live WSS, GPU pixels, real 100/125/150% DPI, 30-minute native soak, human
visual/audio/feel acceptance, clean source binding and formal publisher
signing are also still required. `globalParityPercent` remains null.
