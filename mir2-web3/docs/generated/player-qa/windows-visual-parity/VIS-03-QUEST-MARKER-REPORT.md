# VIS-03 Windows quest marker report

Date: 2026-08-29

Status: bounded automated overlay pass on the branch; same-EXE capture and human acceptance remain open.

## Claim state

```text
implementationRevision: 71ff4311941467f34554fe1ab6401948d122eb7a
branch: codex/windows-visual-parity
prguseQuestMarkerAssetsBound: true
questMarkerTwoFrameCadenceBound: true
questMarkerPriorityBound: true
questTrackerDrivenNpcBindingBound: true
focusedWindowsOverlayTestsPassed: true
exactBodyFrameAnchorAccepted: false
sameExeCaptureProduced: false
fullLiveStateTransitionCoverage: false
occlusionAndZOrderAccepted: false
humanVisualAccepted: false
accepted: false
```

## Closed bounded leaf

- Native Windows NPC quest markers no longer use text placeholders.
- The current branch head binds the same Crystal `Prguse` marker families the
  user expects:
  - in-progress: `983/984`
  - available: `985/986`
  - ready to turn in: `987/988`
- Marker animation follows the original two-frame 500 ms cadence.
- Marker choice is driven by authoritative `questIds` on NPC entities plus the
  current `QuestTracker`.
- Priority is explicit and source-shaped:
  - ready to turn in wins over available
  - available wins over in-progress
- Markers are rendered as bitmap overlay images, not substituted text glyphs.

## Automated evidence on the current head

| Gate | Result |
| --- | --- |
| `entity_overlays::tests::npc_quest_markers_follow_authoritative_quest_status` | PASS |
| in-progress marker paths `983/984` | PASS |
| available marker paths `985/986` | PASS |
| ready-to-turn-in marker paths `987/988` | PASS |
| repository asset presence for `Prguse/983..988` | PASS |
| animation phase at `0/499/500/1000 ms` | PASS |
| ready-to-turn-in priority over mixed quest list | PASS |

Command run on `2026-08-29` against revision `71ff4311941467f34554fe1ab6401948d122eb7a`:

- `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml npc_quest_markers_follow_authoritative_quest_status -- --nocapture`

## Why this remains bounded

- This proves the native overlay now selects the right Crystal marker asset
  family and cadence under focused test conditions.
- It does not yet prove exact placement against every real NPC body frame,
  every map/background combination, or every live quest transition sequence.
- It does not yet prove the same exact EXE the user plays visibly shows the
  correct marker on screen after login, map transfer, accept, progress, turn-in
  and relog/reconnect edges.

## Explicitly open gates

- Same-EXE screenshots or timed capture for all three visible states.
- Exact body-frame anchoring review against Crystal.
- Occlusion and z-order review when markers overlap roofs, trees, foreground,
  or nearby labels.
- Full live state-transition retest: available -> in progress -> ready to turn
  in -> completed/cleared.
- Human visual acceptance and the broader end-to-end quest denominator.

This report closes one bounded Windows quest-marker numerator. It does not
claim full quest-system completion, full native visual parity, or overall
Crystal 1:1 acceptance.
