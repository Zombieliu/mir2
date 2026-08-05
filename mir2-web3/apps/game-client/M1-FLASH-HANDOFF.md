# Flash handoff: M1-A deterministic motion clock

Use DeepSeek V4 Flash with high reasoning. Work only in this existing linked
worktree:

    /Users/henryliu/obelisk/ai/numeron/mir2/mir2-web3-cross-platform/mir2-web3

Branch:

    codex/cross-platform-bevy-m0-m1-contract

Do not clone mir2 or chain-poc again. Do not switch to the original dirty
worktree. Do not reset, rebase, clean, amend, push or deploy.

## Read first

1. docs/architecture/ADR-0001-cross-platform-bevy-client.md
2. docs/architecture/M1-CLIENT-MODEL-CONTRACT.md
3. apps/game-client/client-core/src/clock.rs
4. apps/game-client/runtime/src/motion.rs

## Task

Implement only M1-A: inject the client time source used by the Bevy motion table
so deterministic tests do not read a real browser or operating-system clock.

The platform adapter may still acquire the Web-compatible time value, but
motion presentation logic must consume an injected/frame resource. Preserve the
current movementStartedMs and movementDurationMs behavior, wasm-bindgen exports,
Gateway payloads, server corrections and rendering output.

Add focused deterministic tests for:

- a movement step at start, midpoint and expiry;
- future-skew fallback using a frozen clock;
- repeated frames with the same clock value;
- no timing metadata and no position change;
- stale entity removal.

## Hard boundary

Only edit the M1-A allowlist in M1-CLIENT-MODEL-CONTRACT.md. Do not edit
Simulation, Gateway, protocol, React gameplay, content, economy, combat,
progression, inventory, guild, Sabuk or platform-shell code.

The public client-core primitives are frozen. If a signature or boundary appears
wrong, stop and report the exact conflict instead of widening the task.

Do not add Tauri or native platform scaffolding in this task. Do not change
dependencies unless the existing allowlist cannot compile; if that happens,
stop and report before editing.

## Verification

Run every M1-A acceptance command from M1-CLIENT-MODEL-CONTRACT.md. Inspect git
status before and after builds because asset/WASM build commands may regenerate
tracked artifacts. Do not stage or delete unrelated generated output.

Return:

- concise implementation summary;
- exact changed-file list;
- exact validation commands and test counts;
- cargo tree output for mir2-client-core;
- whether any generated files changed;
- blockers or assumptions.

Do not commit. Leave the reviewed diff for Sol/Codex to inspect and commit.
