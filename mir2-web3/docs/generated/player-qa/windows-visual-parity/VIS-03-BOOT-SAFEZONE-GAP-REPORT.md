# Windows native boot announcement and safe-zone gap report

Date: 2026-08-29

## Scope

This report records two high-visibility user-reported Windows gaps on
`codex/windows-visual-parity`. The safe-zone code leaf is now bounded at
implementation revision `aae9c2c7e06dbceb6f6539c7b29eba63ece293c4`; the
login announcement remains open:

- Crystal-style post-login announcement popup
- Crystal safe-zone presentation

It is a scoping and evidence note, not a closure claim.

## Findings

### 1. Login announcement popup

Current branch evidence shows three adjacent pieces, but not the required
Crystal in-game notice dialog:

- native shell/login notices exist for login and error flows;
- chat already understands the `announcement` channel color family;
- no reviewed in-game Windows presentation/trigger path was found on this
  branch for the Crystal notice window that appears after entering the world.

Conclusion: this gap is not a hidden toggle on the current branch head. It
still needs a bounded in-game trigger + presentation implementation and exact
same-EXE verification.

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
2. Add the Crystal-style post-login announcement presentation on the native
   in-game path rather than treating chat or shell notices as equivalent.
3. Only after both are evidenced, bind them into the backlog denominator with
   exact-Candidate captures.
