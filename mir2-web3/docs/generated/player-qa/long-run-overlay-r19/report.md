# Windows sustained-run overlay reconciliation — R19

Date: 2026-09-15 (Asia/Shanghai)

## Failure signature

During a long right-button run, the retained actor body could already use the
next reconciled movement pose while the native name and self-health overlay
still read the raw gameplay snapshot. A target-centred snapshot containing the
self entity on the previous source tile therefore separated `Scout` and its
health bar from the body by whole-cell increments. The next settled snapshot
made the layers meet again, which is why the defect appeared transient.

User evidence:
`C:/Users/Administrator/AppData/Local/Temp/codex-clipboard-1e0bbd15-089f-4432-ac24-f4196f5549cd.png`

## Change

- Native entity overlays now consume `NativeEntityPresentation`'s reconciled
  payload, the same coordinate source that produced the retained sprite scene.
- The retained self overlay root now tracks the renderer-owned self entity and
  camera composition each frame. The name and health bar cannot preserve a
  separate movement offset across successive run windows.
- The stale-self-source regression now covers the observed mixed snapshot:
  scene centre on the destination tile, self entity still on the source tile.

## Automated verification

- `cargo +1.95.0 test stale_self_source_echo_keeps_the_active_target_and_motion_window` — pass
- `cargo +1.95.0 test entity_overlays::tests` — 15 pass
- `cargo +1.95.0 fmt --check` — pass
- Windows client full suite with the packaged asset root — 647 pass, 0 fail
- Release build — pass

## Candidate package

- Executable:
  `C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-platform-windows-long-run-overlay-r19.exe`
- SHA-256:
  `4934CB976ED2EA3BC252D590936FA6B2A5E6FC8C79552A96EA6AB0CB8DB767AC`
- Historical process: PID `42748`, gateway `ws://127.0.0.1:17810/ws`.
  The initial before-run capture was character selection with an invalid
  character index; it is not evidence of a successful in-game run.
- Renderer soak counters stayed bounded at 32 retained entity layers, 52
  effects, and 1,437 map tiles during the observed idle interval.

## Remaining visual check

The R19 process exited after approximately 48 minutes with
`memory allocation of 8388608 bytes failed`. Bounded renderer registry counters
do not prove bounded total process memory. The captured input contains left
clicks, without proof of a continuous held right button. R19 is not left running
and this visual/soak acceptance remains open.

R20 adds image, inbound-queue, atlas and frame-cache memory counters. Host
investigation also found and stopped an orphan OneDrive helper reserving about
126 GiB of private memory; no evidence establishes that as the R19 failure's
sole cause. R20 visual investigation found missing equipped body/weapon
resources, recorded separately in the 2026-09-15 diagnostics evidence.

Acceptance is
that the character body, `JWarrior` name, and green self-health bar remain one
rigid visual group throughout the hold and after release.
