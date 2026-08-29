# VIS-03 Windows quest marker report

Date: 2026-08-29

Status: authoritative automated quest-marker transition pass on the branch;
same-EXE capture and human acceptance remain open.

## Claim state

```text
branch: codex/windows-visual-parity
workingTreeBase: 182485d24a0645a9034a5eaefdcccaa180b9d4b2
authoritativeQuestIconBound: true
crystalQuestSelectionOrderBound: true
questTypeColourDiscriminantsBound: true
questMarkerTwoFrameCadenceBound: true
q1AcceptFinishTransitionPassed: true
incrementalNpcPacketRetentionPassed: true
candidateResidentMarkerAssetsBound: true
dailyBlueDrawableSourceFramesPresent: false
exactBodyFrameAnchorAccepted: false
sameExeCaptureProduced: false
fullLiveStateTransitionCoverage: false
occlusionAndZOrderAccepted: false
humanVisualAccepted: false
accepted: false
```

## Closed bounded leaf

- Simulation now selects the NPC marker per character and emits the exact
  Crystal `QuestIcon` discriminant as `questIcon`; the shared Zone map layer
  deliberately clears this character-specific field and Gateway reapplies the
  requesting session's personal value after shared-world composition.
- Selection follows `NPCObject.GetAvailableQuests(true).FirstOrDefault()`:
  current quests in insertion order targeting the finish NPC are considered
  before the NPC's available list. The former invented
  `ready > available > in-progress` priority has been removed.
- Available quests are gated by authoritative level, class, prerequisite,
  stage, start-NPC identity, and loaded-object-id aliases before a marker is
  emitted.
- Exact Crystal icon discriminants and frame formula are represented:
  - in progress: white question `1` -> `Prguse 983/984`
  - available general/repeatable: yellow exclamation `2` -> `985/986`
  - ready general/repeatable: yellow question `3` -> `987/988`
  - available daily: blue exclamation `5` -> `991/992`
  - ready daily: blue question `6` -> `993/994`
  - available story: green exclamation `52` -> `1085/1086`
  - ready story: green question `53` -> `1087/1088`
- Native animation keeps Crystal's two-frame 500 ms cadence. Legacy snapshots
  can recover general/repeatable markers from tracker accept/finish NPC ids,
  but never guess daily/story colour.
- Candidate packaging and verification now fail closed if any resident
  `983..988` or `1085..1088` marker frame is missing.

## Automated evidence

| Gate | Result |
| --- | --- |
| `quest_icons::authoritative_npc_quest_icon_tracks_original_q1_accept_and_finish_roles` | PASS |
| fresh Jane `!` -> ready Jude `?` -> post-turn-in Jude next `!` | PASS |
| `entity_overlays::tests::npc_quest_markers_follow_authoritative_quest_status` | PASS |
| authoritative marker works without client `QuestTracker` | PASS |
| missing `questIds` fallback uses accept/finish NPC roles | PASS |
| wrong-role NPC does not receive a marker | PASS |
| `incremental_npc_packet_preserves_authoritative_quest_marker_fields` | PASS |
| `routing::tests::shared_in_process_registry_keeps_npc_quest_icons_personal_per_session` | PASS |
| full Windows native tests | PASS, 482/482 |
| Candidate verification self-test, including missing story-frame rejection | PASS |
| Rust formatting and `git diff --check` | PASS |

## Source-asset limitation discovered

The supplied Crystal `Prguse.Lib` export has drawable files for `983..988` and
`1085..1088`, but no exported drawable frames at `991..994`. The code retains
Crystal's exact daily icon values and frame formula; it does not substitute a
yellow or green icon and call that parity. Daily blue marker pixels therefore
remain an explicit source-asset gate until the original library is confirmed
to contain drawable data or a legally valid source provides those frames.

## Explicitly open gates

- Same-EXE timed captures for available, in-progress, ready, cleared, relog,
  reconnect, and map-transfer transitions.
- Exact `BodyLibrary.GetOffSet(BaseIndex) + GetSize(BaseIndex)/2 - 28, -40`
  placement instead of the current bounded tile-anchor fallback.
- Occlusion and z-order review against roofs, trees, foreground, and labels.
- Daily blue drawable-source closure, broader quest-chain denominator, quest
  diary/map guidance, failure/abandon/repeatable rules, and persistence audit.
- Human visual acceptance, authenticated live WSS, real DPI, native soak,
  production installer/updater, legal asset closure, and publisher signing.

This report closes an automated NPC-marker authority and Q1 transition
numerator. It does not claim full quest-system completion, full native visual
parity, a complete game, or overall Crystal 1:1 acceptance.
