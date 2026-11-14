# Prepared-v2 socket diagnostics source/build binding (41 inputs)

These scripts are **not executed** by preparation. Run only after the final reviewed `web.rs` diagnostics have passed their checks and root has committed the complete tracked tree. Choose that exact new 40-character commit as `--expected-head`; no provisional HEAD or old Release constant is accepted.

Run from any directory, substituting the actual current repository/worktree path and the final commit:

```powershell
node C:/mir2-build/p01-completion-20261024/socket-diagnostics-v2/pin-socket-diagnostics-build-v2.mjs --repository <checkout-or-mir2-web3-directory> --expected-head <new-40-character-commit>
node C:/mir2-build/p01-completion-20261024/socket-diagnostics-v2/build-socket-diagnostics-v2.mjs --repository <same-checkout> --expected-head <same-commit>
node C:/mir2-build/p01-completion-20261024/socket-diagnostics-v2/capture-socket-diagnostics-release-v2.mjs --repository <same-checkout> --expected-head <same-commit>
```

Execution order: complete the corrected focused/adjacent checks; commit and obtain the full HEAD; stop other compilation/tests/realms; pin source; run the owned locked/offline Release build; capture Release binding. The build writes a complete unique log and success/failure receipt, checks the exact source before and after compilation, and copies a successful fresh executable to an exclusive external binary path. Capture never builds or promotes a failed receipt.

Prepared-v1 remains byte-for-byte preserved in `socket-diagnostics-v1/`. The original focused compile-v1 failed with four E0061 constructor-arity errors; it did not execute the seven diagnostics tests. The log `C:/mir2-build/cell-preproduction-p0-p1-20261024/p0-socket-diagnostics-focused-v1.log` is preserved (62,567 bytes; SHA-256 `d9191d6a310a2e6c7e008046fea14aee62afd08149c38d0f293051051b15972c`). Fixing the real callers adds two previously unchanged backend fixture paths to the base-to-HEAD closure, which is now **41** (36 Rust files and five Cargo/.cargo inputs). No wrapper is added to conceal those dependencies.

Fixed constants are the preserved Git base `68f810b442321ae66360a2be18e31dc309efdaf7`, previous backend commit `461f1d85eed6309a55a88bd79a0022f7b97febe4`, the previous manifest's raw SHA, the old `855b…` executable SHA (for rejection only), fixture baseline `0998105c30f35502b8b569b82fb7942a5d356c89`, the installed 1.95.0 toolchain and existing external target cache. The caller must provide the new committed HEAD and repository path. Closure must be the original 39 paths plus `web_live_overflow_tests.rs` and `web_transport_close_tests.rs`. Only `web.rs` may change among the original39; the other38 retain exact raw and Git-LF SHA. In the two new paths, removing the approved three-plus-one `None` constructor arguments must reconstruct the exact099 Git-LF blobs; no old assertion or other fixture byte may change. Ignored-inclusive untracked Rust/Cargo/.cargo files remain rejected.

All new outputs use exclusive creation. They are `backend-source-socket-diagnostics-v1.json`, `socket-diagnostics-v1-build-receipt.json`, `release-socket-diagnostics-v1.json`, `socket-diagnostics-v1/bin/mir2-gateway.exe`, and `C:/mir2-build/cell-preproduction-p0-p1-20261024/p0-socket-diagnostics-v1-release-build.log`. No older manifest, log or binary evidence is replaced. A failed attempt remains preserved; a reviewed retry needs fresh versioned output names rather than deleting or overwriting the failed attempt.

The output API names remain `socket-diagnostics-v1` because no actual source/build/release artifact has been captured yet; prepared-v2 is a corrected tool template, not a second build. The new Release JSON retains the existing consumer fields (`sourceCommit`, `baseCommit`, `backendSourceManifest`, `backendInputCount` now41, both source hashes, `binary`, `buildLog`) and adds the actual build receipt and toolchain binding. It is a build certificate only. Full lifecycle checks and every prospective capacity gate remain separate.
