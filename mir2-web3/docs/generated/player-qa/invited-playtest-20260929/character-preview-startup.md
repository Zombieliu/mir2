# Character previews and Windows startup — 2026-09-29

The player's creation screenshot showed a male Wizard's staff hidden behind
dark gold effect shapes. The exported art and offsets are correct: the native
UI was drawing an additive effect as an ordinary alpha image. Editing the
character name also rebuilt the control and restarted its animation.

## Source comparison and change

Crystal `NewCharacterDialog.cs` draws the Wizard's `ChrSel` body plus frame
`+560` with `DrawBlend`; `DXManager.cs` uses SourceAlpha + One for RGB. The
native creation and selection previews now share the existing additive UI
material used by other Crystal effects. The effect has no ordinary ImageNode.
The 32 Wizard materials and their texture handles remain resident; all layers
advance together only when the next frame and its dependencies are loaded.

The preview clock survives name/focus redraws, including the elapsed portion
of a frame. Changing class, gender, or the creation/selection anchor resets it.
Original offsets, dimensions, 250 ms cadence and 16-frame loops are retained.

A read-only comparison of original `ChrSel.Lib`, exported PNGs/metadata and
native frame tables matched all 192 body/effect frames, including every class
and both genders. Source library SHA-256:
`4be898a0dae083802959ee4b1d2c093320fbc0a46ec68b30ac5fc856744a8197`.
The original creation anchor is (338,404), selection (260,420). No artificial
cropping, offset shift or visible fade-in was added.

Two bounded source differences remain: native Timer catches up after a long
stall while the original advances once; selection omits non-Wizard 4x1
placeholder effect frames, a few of which contain one colored pixel. These
are not represented as full 1:1 acceptance. The ordinary creation picker still
offers Warrior, Wizard and Taoist. Assassin and Archer were inspected through
an explicit offline rendering model; this does not enable online creation.

## Windows startup

Release Windows binaries use the GUI subsystem. Debug and test builds retain
the developer console. Opening the release EXE directly therefore does not
request a separate console from Windows. An external batch/terminal launcher
can still create its own terminal independently of the game executable.

Startup/fatal categories, sparse timing milestones and panic source locations
are saved to `%LOCALAPPDATA%/NumeronLegendOfRebirth/logs/startup.jsonl`, with a
TEMP fallback. The file is bounded at 256 KiB, records at 2 KiB, and concurrent
writers never wait for a file lock. This is a structured startup log, not a
complete stderr mirror. Credentials, configuration contents, URLs, packet
bodies and panic payloads are excluded. Existing opt-in diagnostic traces and
redirected developer stderr remain available.

Fatal configuration/assets/initial-map/network-runtime/window failures show
an error dialog and retain nonzero exit codes. A caught main-thread panic
records its source location and exits with code 101. This does not guarantee
recovery from process aborts, GPU-driver crashes or out-of-memory termination.
The current error-dialog copy is Chinese; the three-language migration is a
separate documented work item, not a completed translation claim.

## Verification

- Focused preview checks: 9 passed.
- Shared native-UI regression: 1,108 passed, zero failures, two explicit
  offscreen visual fixtures ignored in the ordinary run.
- New ignored fixture run explicitly: passed. Real packaged assets and the
  production create/select renderers produced 20 cases, 320 frame checks and
  64 PNGs. Every name/focus redraw preserved the current frame and half-frame
  clock; every sequence wrapped to frame zero.
- Sixteen Wizard effect/body-only GPU comparisons found zero darkened pixels
  (tolerance of two values per channel), and 12,828–23,498 brightened pixels each.
  Representative creation PNGs for all ten class/gender combinations and
  additional Wizard selection/body-only PNGs were visually inspected.
- Full native host regression with the complete asset fixture: 768 passed,
  zero failures, three existing explicit environment checks ignored. This
  includes six new bounded logging/privacy/concurrent-lock checks.
- `git diff --check` passes. Focused rustfmt checks pass for the preview module,
  new visual fixture and diagnostic module. Unrelated pre-existing formatting
  in larger native UI modules was preserved.

The first focused compilation failed because `chrono::Local` shadowed Bevy's
system `Local`; qualifying `bevy::prelude::Local` fixed it. The failed log is
retained alongside the passing rerun. No production behavior test was removed.

Evidence root: `C:/mir2-ui-repair-20260921/character-preview-20260929`.
The `gpu-1/preview-report.json` and `preview-frames.jsonl` record actual states;
all screenshots are explicitly marked offline and `liveAcceptance=false`.
This fixture opens no OS window, connects to no server and changes no account,
character or save. The user's active F-drive game was not closed or replaced.

## Delivery and open acceptance

Source fix `4642971f7` and the separate multilingual design were pushed on
`codex/playtest-registration`. Clean attested source
`c3828af30e80600547999eca375b6b86cb700c6b` built successfully with locked Rust
1.95.0 Release, SHA-256
`546BB79CCD867336BE492D5ED72D12C1FE725D14AE2CB4410058FBCD94D54FD3`.
Read-only PE checks confirm AMD64 PE32+ and `IMAGE_SUBSYSTEM_WINDOWS_GUI=2`,
bound to that attestation; no game process was launched for this check.

Candidate `WN-CANDIDATE-20260929-invited-03` passes the strict nonvisual verifier
with `sourceRepoCheck=checked`, valid detached CMS signature and no failures.
The installer input verifier independently checked all 123,029 files and
741,631,686 bytes against the signed package. No runtime logs were included.
Inno Setup compilation exits zero; numeric version is 2026.9.29.3 and the actual
Setup PE machine is AMD64. The compiler and bundled Microsoft runtime have
valid publisher signatures; Setup itself has no public Authenticode signature.

Delivered artifact (594,799,887 bytes):
`C:/mir2-playtest-releases/20260929-preview/Mir2-Invite-20260929-r3-Setup.exe`.
SHA-256: `455B68EF0E22E5BD526295C9277967DFB5B32F97B82446784E95E3061C5AD455`.
Its sibling `preview-release.json` records the completed and open checks;
`installer-build` retains the recipe, exact source allowlist and verification
logs. Previous releases and the user's F-drive game remain unchanged.

Actual installed upgrade, fatal-dialog interaction and player creation/selection
acceptance remain open; automated source/GPU evidence does not close them.
This r3 retains the existing Chinese installer and mixed game text. The requested
`zh-TW` / `en` / `pt-BR` game release is not delivered: its audited implementation
plan is [NATIVE-MULTILINGUAL-DESIGN.md](../../../NATIVE-MULTILINGUAL-DESIGN.md).

Capacity work remains paused at the user's request. There is no server change,
50–100-player stability claim or whole-game parity acceptance in this round.
