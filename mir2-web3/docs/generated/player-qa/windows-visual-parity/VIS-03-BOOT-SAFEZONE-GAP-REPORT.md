# Windows native boot announcement and safe-zone gap report

Date: 2026-08-29

## Scope

This report records two high-visibility user-reported Windows gaps on
`codex/windows-visual-parity`. The safe-zone code leaf is now bounded at
implementation revision `aae9c2c7e06dbceb6f6539c7b29eba63ece293c4`; the
login announcement implementation now exists but remains unaccepted:

- Crystal-style post-login announcement popup
- Crystal safe-zone presentation

It is a scoping and evidence note, not a closure claim.

## Findings

### 1. Login announcement popup

Current branch evidence now shows the required bounded in-game notice chain:

- simulation `StartGame` already emits one authoritative `UpdateNotice` per
  gameplay session and tests that it never contains third-party `LOMCN` /
  `Supercode` copy;
- the Windows native protocol parses `UpdateNotice`, the gameplay bridge
  retains one monotonic `(generation, sequence)` notice update, and resets it
  on logout/return/disconnect/session-generation changes;
- native Bevy now mounts the Crystal notice plugin and renders the in-game
  panel using the original `Prguse/961`, `Prguse2 470..475`, and
  `Title 193..195` assets with bounded scrolling and close behavior;
- focused simulation and native notice tests cover source-bound trigger,
  close/re-login, empty-notice fail-closed behavior, and session reset.

Conclusion: the code leaf now exists on this branch. Exact same-EXE visual
verification, clickable/color span and scrollbar fidelity review, persistent
Crystal `LastUpdate > LastLogoutDate` delivery semantics, and human acceptance
remain open, so the item stays open in the denominator.

### 2. Safe-zone presentation

Current branch evidence shows the full bounded source-to-render chain:

- simulation/session snapshot exports `in_safe_zone`;
- the imported Crystal `Setup.ini` enables `SafeZoneBorder`, and the generated
  map manifest therefore materializes persistent `TrapHexagon` boundary
  objects;
- the prior runtime default hard-coded the optional switch off, preventing
  those objects from reaching the client despite the imported source setting;
- revision `aae9c2c7e06dbceb6f6539c7b29eba63ece293c4` derives the default from
  imported boundary-object evidence while retaining an explicit opt-out;
- Windows renders the persistent objects from exact `Magic 1390..1399` frames
  at 100 ms and removes them only on the authoritative object/scene lifecycle;
- `inSafeZone` also reaches the shared `UiReadModel` for deterministic state
  assertions.

Evidence: focused source-setting projection, persistent native effect,
read-model and cursor tests pass; full simulation passes 1482/1482.

Conclusion: the code leaf is bounded. Exact same-EXE capture and human visual
acceptance remain open, so this is not global safe-zone or visual acceptance.

## Open acceptance gates

The overall user-observed items are not accepted until all of the following
exist:

- deterministic trigger coverage;
- same-EXE screenshots or video bound to the exact Candidate;
- no duplicate or stale presentation on relog/re-entry;
- correct clear behavior when the state ends;
- human visual acceptance against Crystal.

## Recommended next implementation order

1. Package the exact safe-zone implementation revision and capture the
   persistent boundary animation plus entry/exit state transitions.
2. Capture the native post-login announcement against the exact Candidate and
   verify no duplicate popup within one gameplay session.
3. Only after both are evidenced, bind them into the backlog denominator with
   exact-Candidate captures.
