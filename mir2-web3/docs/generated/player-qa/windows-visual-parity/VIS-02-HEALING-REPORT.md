# Windows visual parity VIS-02 Healing report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 7157875c7052156537cfc1343e9712472f6bdb12
Healing implementation revision: 24d9b73a30fc18edf0649283d14495c6f4900aff
branch: codex/windows-visual-parity
vis02Status: in_progress
healingAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded automated Healing presentation checkpoint
inside VIS-02. It is additional to the first five-skill slice and therefore
does not alter the declared spell or effect denominator. Wider cast, struck,
die, dead, revive, monster and UI inventories remain incomplete. No exact-head
package, live-WSS playback, GPU raster capture or human animation/audio
acceptance was produced, so this is not full VIS-02, Windows visual parity or
whole-game parity.

## Source-bound behavior implemented

- Typed `ObjectMagic(Healing)` starts the caster-owned `Magic/200..209`
  sequence: ten 60 ms frames, 600 ms total, light 6 and exact `M61-0.wav`.
  Audio requires an active resolved cast and native sequence replay does not
  duplicate it.
- Raw typed `ObjectEffect` value 3 starts the target-owned
  `Magic/370..379` sequence: ten 80 ms frames, 800 ms total, light 6 and
  exact `M61-1.wav`. Crystal handles Healing outside the generic delayed-
  effect path, so this effect intentionally ignores packet delay.
- The target effect follows the live object, disappears on Hide/Remove and
  does not resolve or play audio for a missing target. Map, generation and
  session boundaries clear active presentation state.
- Web projection accepts both string `Healing` and numeric spell 61, maps them
  to the exact cast/target sound IDs and attaches the target effect to the
  live actor.
- Exact audio identities are:
  - `M61-0.wav`: 194,008 bytes, SHA-256
    `AADE9DB9A46762B8C319A2FD3611FBB4CDC86D444B5C3FD14DC92AEC812F94A1`;
  - `M61-1.wav`: 308,496 bytes, SHA-256
    `9E3942A729F886197B30D1CA0084AA020179F62BCA64C6044E36D6E080D74ED5`.
- Source export, the Web present manifest, Bevy allowlist, Candidate package
  rules and copied-Candidate verifier require both exact sounds and all
  `Magic/200..209` plus `Magic/370..379` frames. Self-tests remove each sound
  and both range endpoints and prove fail-closed behavior. No package was
  built from this exact head.

## Packet-evidence scope

The fixture is a typed packet/event projection contract, not an authenticated
production transcript. It proves the client interpretation of one
`ObjectMagic(Healing)` and one `ObjectEffect(effect=3)` packet, including
lifecycle and negative-target behavior. It does not prove Gateway delivery,
server healing authority, heal amount, mana/cooldown, target eligibility or
shared-Zone scheduling.

Native projection carries sequence identity and has explicit one-shot replay
coverage. The Web game event currently has no sequence/generation identity;
an upstream retransmission can therefore recreate the target effect and replay
its sound. Independent review classifies that as a non-blocking P2 for this
ordinary packet-to-event checkpoint. Web retransmit/reconnect robustness is
not certified here.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source behavior and asset identity audit | PASS |
| Independent final review | P0=0, P1=0; one retained Web replay P2 |
| Healing focused native effects | PASS, 4/4 |
| Full Windows native suite | PASS, 398/398 |
| Full `mir2-client-bevy` native-ui suite | PASS, 416/416 |
| Web game-event and audio routing tests | PASS, 45 groups |
| Sound exporter end-to-end test | PASS |
| Magic-effect exporter/validator | PASS, 74 spells |
| Web scene-effect runtime and typecheck | PASS |
| Exact asset byte/hash tests | PASS |
| Candidate package script self-test | PASS; missing Healing assets fail closed |
| Candidate verifier self-test | PASS; missing Healing assets fail closed |
| Rustfmt and diff checks | PASS |

The tests used source assets in the isolated visual-parity worktree. The
frozen playable Candidate processes were not stopped, replaced, launched or
used as evidence for this revision.

## Existing authority and unclosed boundaries

This client checkpoint changes no personal simulation, shared-Zone, Gateway,
protocol, persistence or Healing gameplay authority. Any existing server
Healing implementation was not revalidated against a live authenticated
same-EXE path in this leaf and must not be inferred from these tests.

Still required are Web sequence/generation deduplication if retransmit parity
is promoted into the contract; exact-head Candidate construction and copied-
package verification; authenticated live-WSS delivery; GPU additive/light and
anchor pixels; audible two-stage timing; real 100/125/150% DPI; a 30-minute
native soak; human animation/audio/feel acceptance; the complete semantic
denominator and legal asset pack; clean-source binding; formal publisher
signing; and final human acceptance.
