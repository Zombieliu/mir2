# Unexpected native exit — 2026-09-21

User reported the window disappeared directly while playing the profile-assets
build. Its stderr ended with `gateway session ended`, without a Rust panic.
Application event IDs 1000/1001 had no matching Mir2 entry. The Gateway stayed
alive and recorded successful web-teardown checkpoint persistence for demo/7.
This does not identify the exit cause or prove the last unsaved action survived.
The old launcher did not retain a process exit code.

Two exit-path problems were reviewed:

- A raw Enter message left unread by another UI branch could confirm a quit
  prompt opened later in the same frame. The test injects the event in PreUpdate
  to reproduce real Winit ordering; original code failed with no new key on the
  next frame. Requiring a fresh just_pressed edge in the leave modal fixes it.
  A subsequent fresh Enter still confirms. Focused leave-game tests: 4 passed.
- Native WindowPlugin previously used close_when_requested=true, allowing OS
  close to bypass the game confirmation. A host-specific setting disables that
  automatic close; Windows routes in-game close requests through LeaveGameDialog
  while login close remains available. Headless lifecycle regression: 1 passed.
  Web/other hosts retain the existing default.

Exit requests/answers/effects, shell exit buttons, WindowCloseRequested,
WindowClosed, AppExit and app.run return are now logged without credentials or
chat. `scripts/launch-native-diagnostics.ps1` preserves stdout/stderr and records
process start, 10-second memory/CPU samples and exit code in timestamped files.
A where.exe smoke run verified capture of its real exit code 2.

Neither reproduced defect has been proven to cause this particular incident.
Release package and subsequent logged play must be kept distinct from a claimed
crash fix or soak-test pass. Existing source-asset and ordinary-save evidence is
unchanged; no role or server save was edited during this investigation.
