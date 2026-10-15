# V2 native clean-source build and startup diagnostic

The attested release build completed successfully at
2026-09-17T21:04:19.6849784Z from clean source
`6e1778957299367b3fbd39961f827c1bcb2d628a`, using Rust 1.95, locked offline
dependencies, the standard attested builder and one compilation job.
The earlier wrapper-only empty-RUSTC failure is preserved separately.

The 78,138,368-byte EXE has SHA256
`A95B005550BA897B5D84F45AA48E9F777D6821EAF4F70EF98110F851CD2711A0`.
Its copied immutable development artifact and matching BUILD-ATTESTATION.json
are in `C:/mir2-newcomer-v2-clean-20260918/native-client`; the copy hash was
verified. Gateway and native Rust source revisions match. Later Node controller
repairs are separate scripts and are not claimed as part of this EXE.

The first computer-use launch returned a native window, but its screenshot
request failed with `no screenshot targets found`. That observation does not
establish an application crash or a resolved visual defect. No screenshot was
accepted and no login or gameplay acceptance is claimed.

A separate hidden startup diagnostic, without game login or input automation,
remained alive during its initial check and produced actual startup logs:

| Stage | Milliseconds since launch |
| --- | ---: |
| Assets validated and map decoded | 90.843 |
| Runtime configured | 991.662 |
| WebSocket connected | 996.040 |
| First main update | 1366.413 |
| First world snapshot | 1399.260 |

All required manifest/map/cursor/wing checks report complete and all seven
starter atlas pages were pushed. The logs contain no panic. This is startup
evidence, not evidence of authenticated gameplay or UI correctness. Raw local
evidence is `native-build/verified-build.json`,
`native-client/BUILD-ATTESTATION.json` and
`native-startup-diagnostic/{launch.json,stderr.log}` under the fresh run root.
The current process check finds no native client; its eventual stop is not
attributed to a crash without supporting evidence.

The development artifact uses a junction to the current repository assets and
localhost WS19810, at 1024x768. It is not a portable signed Candidate package.
Formal signing material, package verification and native/original visual
acceptance remain open. `signedPackage=false`, `visualAccepted=false`.

## 2026-09-20 manual QA launch correction

An interactive QA launch connected the V2 Gateway on port 19910 but omitted
`MIR2_QUEST_GUIDANCE=newcomer-v2`. The server correctly completed character
`a1`'s first Jane quest and persisted level 2, but the client did not enable
the V2 Diary entry for quest 2110002 or the compact journey HUD. This was a
launch configuration error, not a deleted second quest. The client was closed
through its window and reopened with the matching guidance profile; the saved
character remained level 2 with quest 2110001 completed. The new window is
connected, while post-login visual confirmation of the Diary remains pending.

Use `apps/game-client/platform-windows/scripts/start-newcomer-v2-qa.ps1` for
future isolated V2 manual runs, passing the development client directory and
local WebSocket URL. It sets the presentation profile and leaves account and
password entry to the visible login screen. Quest 2110002 starts and finishes
in the Quest Diary (`Q`); its first objective is to equip the WoodenSword
awarded by Jane. It is not offered by another NPC.
