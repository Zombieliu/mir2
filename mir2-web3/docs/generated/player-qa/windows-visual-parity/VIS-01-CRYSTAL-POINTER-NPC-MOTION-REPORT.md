# VIS-01 Crystal pointer, NPC interaction and self-motion report

Date: 2026-08-29

Status: automated Candidate pass; fresh human visual/feel acceptance pending.

## Closed bounded leaf

- Windows native self movement now has one presentation owner from the
  command frame through the authoritative acknowledgement. The existing
  diagnostic local-command shadow no longer takes over the camera or player
  sprite at the ACK boundary, while correction/session reset clears the
  retained self-camera window immediately. Zone position, collision and
  movement cadence remain authoritative.
- The native window uses four PNG conversions of the repository's exact
  Crystal cursor resources: Default, Normal Attack, Compulsion Attack and NPC.
  Alpha-tested hover identity maps Monster to Attack, NPC to NPC talk, and
  Shift+remote Player to Compulsion Attack. UI capture, dead state,
  non-InGame state, focus loss, self hover and unknown identity fail closed to
  Default.
- Crystal `Settings.NewMove` right-click feedback is exported from
  `Magic3.Lib` frames 500 through 509 and plays for 600 ms at the clicked
  empty-world tile. It is presentation-only and cannot move the player or
  bypass Zone validation.
- A distant native NPC left click now enters a bounded authoritative approach
  state. It sends at most one movement intent at a time, waits for the
  authoritative ACK/snapshot, and emits exactly one existing `InteractNpc`
  intent after reaching an adjacent tile. Dialog open, focus/session loss,
  timeout or NPC disappearance cancels the pending action.

## Source boundary and known semantic difference

- Crystal `GameScene.UpdateMouseCursor` owns the four implemented cursor
  states, and `MapControl.OnMouseClick` owns the `Magic3, 500, 10, 600`
  NewMove marker for a right click on empty ground.
- Crystal does not define a generic empty-ground left-click marker in the
  audited click branch. Windows therefore does not invent one. Left click
  retains its source-aligned priorities: Monster attack, NPC interaction,
  same-tile pickup, then empty-ground Walk.
- Crystal sends `CallNPC [@Main]` and its server accepts the call within
  `Globals.DataRange`. The current Rust interaction path still requires
  adjacency, so this bounded Windows fix matches the already playable Web
  approach-and-interact bridge instead of claiming exact server-range parity.
  Replacing that compatibility bridge with source-exact shared-Zone NPC range
  semantics remains open.

## Automated evidence

| Gate | Result |
|---|---|
| Windows native host suite, Rust 1.95 | PASS, 463/463 |
| Shared Bevy runtime suite, Rust 1.95 | PASS, 199/199 |
| Distant NPC approach then single interaction | PASS |
| NPC disappearance cancels without a fake dialog request | PASS |
| Hover cursor Monster/NPC/Shift-Player/default matrix | PASS |
| NewMove Magic3 500..509 frame and 600 ms lifetime | PASS |
| Magic exporter deterministic end-to-end suite | PASS, 74 spells |
| Candidate package self-test | PASS |
| Candidate verifier self-test | PASS |
| Rust formatting and diff checks | PASS |
| Clean attested Release build | PASS |
| Package-time verifier | PASS, `sourceRepoCheck=checked`, nonvisual |
| Independent final-directory verifier | PASS, `sourceRepoCheck=checked`, nonvisual |
| Four packaged Crystal cursor PNGs | PASS, 4/4 |
| Packaged NewMove effect frames | PASS, 10/10 |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-POINTER-NPC-MOTION-20260829` |
| Revision | `4d035489a966d827ef5aa49567d4b53bf344d2a7` |
| Release EXE bytes | 67,901,952 |
| Release EXE SHA-256 | `6782C69AF21BBC0DD72965154AD81CFF15CCFB8FB2F80FCEDF208B06721C6D03` |
| Build completed UTC | `2026-08-28T21:27:44.7247873+00:00` |
| Build attestation SHA-256 | `13AE822FEF7FC758AF49C7E03C527016699981478F4D1DCE9FEF9363C8C765AC` |
| Package payload files | 32,965 |
| Candidate total files | 32,969 |
| Package manifest SHA-256 | `CB581BC41CF38FD196E6341A9BFB9884062060D36265EA32200DC809F0B7A395` |
| Package aggregate SHA-256 | `306C449FF9F296F54C88B7A2ACB51CED60700485F5A592159FD2E5F98CE13633` |

The exact EXE was launched as PID 263988 with a process-local
`ws://127.0.0.1:7210/ws` override. Gateway PID 237188 was listening on
127.0.0.1:7210 and `/health` returned 200 after launch. This proves package
identity and local transport readiness only; it is not authenticated live WSS
or human visual acceptance.

## Explicitly open gates

- The current user retest must determine whether command-to-ACK self motion
  actually removes the reported running flash on the real window and GPU.
- Human verification is still required for attack/NPC cursor visibility,
  right-click marker placement, sustained right-click run feel, collision
  correction and distant NPC dialog opening.
- Source-exact NPC `DataRange` semantics, full mouse combat/pathfinding,
  remaining UI/chat, all player actions/classes, skills/VFX, monsters/maps and
  semantic denominators remain open.
- Authenticated same-EXE live WSS, real 100/125/150% DPI, native 30-minute
  soak, formal publisher Authenticode and human visual/audio/feel remain
  mandatory.

This report closes one automated and packaged Windows interaction numerator
leaf. It does not claim visual 100%, a fully accepted playable vertical slice,
whole-game 90%, or Crystal 1:1 completion.
