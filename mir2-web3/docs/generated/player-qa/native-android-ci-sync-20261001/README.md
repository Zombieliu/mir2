# Android integration CI repair — 2026-10-01

This is a dependency-lock and mechanical-formatting checkpoint on independent
`codex/android-shared-sync`, not a new Android visual, online or device gate.
The original dirty checkout and `codex/android-player-journey` are preserved.
PR [#253](https://github.com/Zombieliu/mir2/pull/253) remains Draft; neither it
nor the Windows source PR is merged by this round.

## Source and exact repair

- Pre-repair parent: `7233ed261c54d77476f6daa44c79714c07da46e4`.
- Lock repair: `e98a640c5`, synchronizing Windows standalone `Cargo.lock` with
  the Android-target GLES dependency already declared by shared runtime.
- Formatting repair: `e95cef74c42949a3bf4917cbe8185fb93356721a`.

The Windows lock adds the runtime's `bevy_render` edge and wgpu's
`wgpu-core-deps-emscripten` edge/package at **29.0.4**. No existing locked package
version is upgraded. The Windows, Android, shared-client and runtime metadata
still resolve **Bevy 0.19.0**. The root Windows host does not gain the Android
local-renderer patch. `--locked`, format checks and audit gates remain enabled;
no workflow, authentication, protocol, gameplay, save or production setting
is changed.

CI-version formatting is applied to exactly 200 Rust files:

| Scope | Files | Formatter |
| --- | ---: | --- |
| Shared `client-bevy` | 63 | Rust 1.95.0 |
| Shared `runtime` | 6 | Rust 1.95.0 |
| Windows host | 20 | Rust 1.95.0 |
| Gateway | 19 | Rust 1.89.0 |
| Simulation | 92 | Rust 1.89.0 |

`client-core` and Admin API already satisfy the respective checks and have no
source edits. A separate comparison formatted every changed file's original
parent contents with the exact CI version, then compared to the final file:
**200 checked, zero mismatches**. This is formatter provenance, not a claim
that new gameplay behaviour was implemented or visually accepted.

## Failure evidence before repair

- [Windows native host](https://github.com/Zombieliu/mir2/actions/runs/36767795081/job/110066238307)
  stops because the Windows lock needs an update with `--locked` enabled.
- [Shared Rust/WASM](https://github.com/Zombieliu/mir2/actions/runs/36767795081/job/110066238382)
  stops at formatting, before unit tests or WASM compilation.
- [Local Candidate](https://github.com/Zombieliu/mir2/actions/runs/36767795220/job/110066238086)
  stops at Gateway/Admin/Simulation formatting.
- The same pre-repair run's Android Capacitor/native Bevy lane passes. That
  does not establish shared/Windows or phone-layout acceptance.

## Local validation

| Check | Result and boundary |
| --- | --- |
| Exact shared/core/runtime/Windows format commands | Pass with Rust 1.95.0 |
| Gateway/Admin/Simulation format command | Pass with Rust 1.89.0 |
| Changed Rust files vs original formatter output | 200/200 exact matches |
| Dependency-lock metadata | Pass: Windows MSVC, Android ARM64, shared macOS, runtime WASM, backend Linux graphs; no lock rewrites |
| Windows target `fetch --locked` | Pass; dependency resolution, not an MSVC build or Windows execution |
| Shared core locked tests | 12 passed / 0 failed |
| Shared client default locked tests | 171 passed / 0 failed |
| Runtime default locked tests | 288 passed / 0 failed / 1 existing ignored |
| WASM locked compile | Pass: `wasm32-unknown-unknown`, Rust 1.95.0 |
| Shared `native-player-ui` locked tests | 1164 passed / 0 failed / 7 existing ignored; overlaps default tests, not added to a unique denominator |
| Android host locked tests | 202 passed / 0 failed; macOS-hosted Rust tests, not Android device execution |

Remote CI results are recorded only when they actually finish. Local logs and
metadata are under the external
`/private/tmp/mir2-ci-sync.jCvUMv/` directory; they are not shipped as artifacts.

## APK and UI boundaries

Existing Debug and uiPreview APKs remain bound to source
`8d50cb8a369b399b957c4188c204ac4b48739b7b`, **not this formatting commit**.
They are not rebuilt or overwritten here. Their exact identity and offline
screenshots remain in the
[APK/evidence record](../native-android-shared-sync-20261001/README.md).
Both APK hashes were rechecked unchanged: Debug
`1f1acd5e03a790554185012423936f45ddf7d447c9e193fe44b85cdba00111aa`,
uiPreview `8fe46d14877c855d7bff33168dae15e10f38049d7b76beb36ad896556ce922ad`.

The user's `world-render` screenshot correctly exposes unfinished phone UI:
the bottom HUD is still desktop-scaled, the chat is a large white field, and
Android action buttons still use preliminary presentation. Inspection confirms
the white field is the bundled `Prguse/2221.png`, not proof of a missing entire
UI texture. The asset inside uiPreview matches the inspected source pack with
SHA-256 `4a4431187a71592dbc4ea55407c06ee6e8c60cc7b53132ce324e68578f7ffbd2`.
Using the existing transparent frame does not complete phone layout. No UI
redesign or new after-fix screenshot is delivered by this CI-only change.

## Still open

- Remote rerun results must not be inferred from local formatting/metadata.
- Web dependency security audit is a separate follow-up task, **not fixed
  or weakened here**. Other unrelated CI lanes are not repaired by this slice.
- Phone-first HUD/chat/actions and complete window/touch/IME layout.
- Approved real Gateway login, authoritative player loop and physical-device
  acceptance. Offline fixtures and successful builds do not close these gates.
