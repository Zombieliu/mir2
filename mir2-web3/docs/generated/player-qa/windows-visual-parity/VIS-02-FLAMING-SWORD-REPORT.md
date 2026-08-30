# Windows visual parity VIS-02 FlamingSword report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 974263f5615904b36c8735404723e6568a2c22a9
FlamingSword implementation revision: 160e8d3ccc0eb17f8e49b6505c5a58666a35029f
branch: codex/windows-visual-parity
vis02Status: in_progress
firstFiveAutomatedPresentationCheckpoints: complete
flamingSwordAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes the bounded automated FlamingSword presentation checkpoint
inside VIS-02. Lightning, FireBall, SoulFireBall, FireWall and FlamingSword now
all have source-bound automation checkpoints. That is not VIS-02 acceptance:
the wider Struck/Die/Dead/Revive chain, authenticated same-EXE delivery, GPU
raster pixels and human animation/audio/feel review remain open. It is also not
Windows visual 100% or whole-game 90%; the semantic inventory is incomplete.

## Source-bound behavior implemented

- `SpellToggle(FlamingSword)` only arms state. It creates no bitmap, light or
  audio. The presentation begins only from a typed `ObjectAttack` carrying
  `spell=8`, `level` and the ordinary `Attack1` action. An ordinary
  `ObjectAttack(spell=0)` creates no FlamingSword overlay or dedicated audio.
- Native Windows and Web both resolve an attacker-bound overlay from
  `Magic/3480 + direction*10 + frame`, with eight directions, six visible
  frames, 100 ms per frame and an exact 600 ms lifetime. The last four slots
  in each ten-frame source stride are not rendered.
- The overlay follows the live attacker, uses additive blend with Crystal's
  0.7 rate/opacity, has no generated shadow and contributes no light. A new
  attack from the same attacker restarts one stable overlay; different
  attackers coexist.
- Exact `M8-1.wav` starts with `Attack1` at time zero. The ordinary weapon swing
  remains at attack frame 1, 100 ms later. Pending delayed swings are cancelled
  on actor removal/hide, map change, logout, socket close, client reset and
  subscriber teardown.
- `M8-1.wav` is 132,720 bytes with SHA-256
  `6A4A29C45E6D9882DD63D67FD4825C9401481DF52383BB74C5FF0644A8EC1B0B`.
  Source packaging and copied-Candidate verification require that identity and
  all 48 visible direction/frame PNGs; self-tests remove one required sound or
  frame and fail closed. No Candidate package was built from this exact head.

## Packet-evidence scope

The fixture is a typed `ServerPacket -> client event` projection contract, not
an authenticated production transcript. It proves that Gateway serialization
preserves `ObjectAttack.spell=8`, level, direction and Attack1-compatible attack
type for all eight direction cases. `productionReachability=not-asserted` is
intentional: this checkpoint did not replay the simulation toggle/next-valid-
melee path over live WSS.

The ordinary-attack record is explicitly a synthetic compatibility case. It
proves that the presentation consumers do not infer FlamingSword from a normal
attack; it is not a second production attack claim.

## Automated evidence

| Gate | Result |
|---|---|
| Independent Crystal/source audit | PASS; Attack1 overlay/audio contract identified |
| Independent final P0/P1 review | PASS; no remaining P0/P1 |
| Gateway packet-event projection fixture | PASS, 1/1 |
| FlamingSword focused native effects | PASS, 5/5 |
| Full Windows native suite | PASS, 357/357 |
| Full client runtime suite | PASS, 191/191 |
| Full `mir2-client-bevy` native-ui suite | PASS, 393/393 |
| Web ObjectAttack reducer and audio lifecycle | PASS; game-events 37 groups |
| Web typecheck and full frontend logic | PASS |
| Magic-effect exporter/validator | PASS, 74 spells |
| Full offline resource/audio gate | PASS; 40,763 manifest assets, 330 present sounds |
| Candidate package script self-test | PASS; FlamingSword asset closure fails closed |
| Candidate verifier self-test | PASS; FlamingSword asset closure fails closed |
| Visual ledger integrity | PASS, 410 UI leaves; global percentage remains null |
| Rustfmt and diff checks | PASS |

The native suites ran against source assets in the isolated visual-parity
worktree. The frozen playable Candidate processes were not stopped, replaced,
launched or used as evidence for this revision. The final Web run used the
already matching ignored prebuilt Bevy runtime solely to satisfy its byte/hash
budget test; it is not a newly built Candidate.

## Existing server support and unclosed boundaries

This revision adds no simulation or shared-Zone gameplay authority. Existing
personal/shared FlamingSword toggle and melee code was not changed or
re-certified by this presentation commit. A live same-EXE test must still prove
that the toggle is silent, the next valid targeted ordinary melee consumes it
exactly once, invalid attempts retain the arm state as Crystal requires, and
the emitted `ObjectAttack` reaches all observers with stable ordering.

Completing the first five automated presentation checkpoints does not close
all 129 non-None spells, the combat-state chain, buffs/poisons, monster special
renderers, weather/environment effects, HUD/buttons/panels or the complete
legal asset denominator. Same-EXE authenticated live WSS, real GPU additive and
alpha pixels, 100/125/150% DPI, 30-minute native soak, human visual/audio/feel,
clean source binding, complete legal assets and formal publisher signing remain
acceptance gates.
