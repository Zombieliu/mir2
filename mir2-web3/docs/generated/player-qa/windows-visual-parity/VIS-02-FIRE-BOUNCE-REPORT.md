# Windows visual parity VIS-02 FireBounce report

Date: 2026-08-28

## Claim state

```text
implementation revision: 90f861a9d
observed implementation bundle: 17b234911a44dd4df47d2e6d11270a5b7ca2370d
branch: codex/windows-visual-parity
fireBounceTimingCheckpoint: complete
fireBounceWholeSemanticLeafAccepted: false
skillEffectDenominatorComplete: false
authenticatedLiveWssTranscriptProduced: false
sameSceneVisualCaptureProduced: false
humanVisualAudioFeelAccepted: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

This is a bounded shared-Zone timing and Windows presentation checkpoint. It
does not declare FireBounce, VIS-02 or the spell denominator complete.

## Crystal source binding

Crystal `HumanObject.cs` makes the first FireBounce target different from
later hops:

- `ObjectMagic` owns the first leg; an extra initial ObjectProjectile must not
  produce a duplicate missile.
- the first authoritative hit is delayed by 500ms plus 50ms for each tile of
  travel;
- the next target is selected only after the previous hit resolves;
- later hops are limited to live hostile targets within radius three with a
  clear projectile path;
- each later hit is delayed by 50ms per hop tile;
- a moved target outside the audited location tolerance cancels the pending
  first hit instead of applying damage at a stale location.

Crystal's runtime chooses among eligible hop targets randomly. The current
single-writer Zone uses its deterministic roll so tests and replay remain
stable; that entropy difference is recorded as open rather than hidden.

## Implemented behavior

- Shared Zone schedules the first hit at `500 + 50 * tileDistance` and stores
  its target location, remaining hops and due time in checkpointable pending
  hit state.
- A resolved hit selects no more than one eligible next monster, emits the
  authoritative monster-to-monster FireBounce projectile immediately, then
  schedules damage at `50 * hopDistance`.
- Checkpoint/recovery preserves the pending chain instead of restarting or
  applying all hops at once.
- Windows presentation delays the client-owned first projectile until the
  Crystal Spell action boundary, deduplicates the legacy first supplement, and
  continues to consume authoritative later-hop packets.
- Projectile direction and clock are source/target bound. Completion
  suppression uses the existing terminal-`Dead` rule.
- Cast/projectile/impact audio remains typed as `M34-0.wav`,
  `M34-1.wav` and `M34-2.wav`, with Candidate asset identity coverage.
- The Web magic-effect export contains the corrected FireBounce definition.

## Automated evidence

| Gate | Result |
|---|---|
| Shared-Zone FireBounce timing, movement cancellation and recovery regressions | PASS |
| Full shared-Zone suite | PASS, 204/204 |
| Native first-leg, hop, dedupe, lifecycle and asset regressions | PASS |
| Full Windows suite at the combined code head | PASS, 436/436 |
| Magic-effect export regression | PASS |
| Candidate asset checks exercised by the focused suite | PASS |

## Native boot boundary

The combined implementation bundle containing this revision booted as the
138,914,304-byte EXE with SHA-256
`ED6C1BB4F9D5EB4F501201C361EE3437DF7CB8EB2B192B3F2F55AA63A7871037`
and connected to `ws://127.0.0.1:7110/ws`. No FireBounce same-scene capture,
physical audio evidence or authenticated live-WSS transcript was produced.

## Explicitly open gates

Crystal runtime-random versus Zone-deterministic hop selection remains an
explicit semantic difference. Multi-target live capture, Web/runtime
equivalence beyond export, complete damage and skill denominators, same-EXE
authenticated WSS continuity, real DPI, 30-minute native soak, human
visual/audio/feel acceptance and formal publisher signing remain open.
`globalParityPercent=null` remains mandatory.
