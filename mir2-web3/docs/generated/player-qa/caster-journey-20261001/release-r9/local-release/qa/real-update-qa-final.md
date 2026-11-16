# Exact r8 to r9 updater QA

The two actual signed-package tests passed with exit 0 in 309.15 seconds on October 1, 2026. Source `180b01d46f35a5e97f4c60ee2ae033c7b53717e5`, Candidate `WN-CANDIDATE-20261001-invited-09`, invited sequence 5. This task did not publish, launch the game, or touch the user's installed client or saves.

## Observed result

- Activated the exact attested r9 game and updater engine from the verified CMS bundle.
- Downloaded six changed game payload files plus the engine: 112,374,808 bytes (107.17 MiB); the game payload is 110,649,368 bytes.
- Verified all 123,031 new payload hashes; 123,025 payloads were unchanged, with no downloads under `mir2-assets/` and no removed payload paths.
- Rechecking pending first launch downloaded zero files and retained the first-launch health obligation.
- Simulated failed first launch: quarantined r9, rolled both game and updater engine back to r8, and rejected another attempt to apply that quarantined release.
- Preserved both synthetic personal fixtures through update and rollback: `game/logs/player.log` and `game/personal-file.txt`.
- Native pinned-key CMS verification accepted the actual signed r8 metadata and rejected signature/metadata tampering. The test retains its historical `real_r6` name; the fixture is the exact r8 source `3f5e61533235921369bc13a7760b4a56b0e467e5`.

Changed game payloads: `BUILD-ATTESTATION.json`, `KNOWN-ISSUES.md`, `MULTILINGUAL-SOURCES.json`, `NotoSansTC-OFL.txt`, `README-START.txt`, `mir2-platform-windows.exe`.

## Evidence and limits

Final receipt: `C:\mir2-playtest-releases\20261001-native-r9\real-integration-final-process.json`.
Exact fixture configuration: `C:\mir2-playtest-releases\20261001-native-r9\real-r8-r9-final-fixtures.json`.
Actual Rust test log: `C:\mir2-playtest-releases\20261001-native-r9\real-package-integration-final.log`.
Actual update report: `C:\mir2-updater-qa\real-r8-r9-final-v1\report.json`.
File hashes: `C:\mir2-playtest-releases\20261001-native-r9\real-update-qa-evidence.json`.

Tests used a file-backed source and a fresh owned installation tree. Public HTTPS transfer, the user's installed F-drive client, native GUI behavior and human gameplay acceptance are outside this task's scope. Package/CMS checks are internal Candidate checks; these results do not establish public Authenticode or whole-game acceptance.

Earlier failed release/QA preflights remain intact. The final helper's initial self-preflight wrongly expected `clientOnly` on `RELEASE-STATEMENT.json`; the schema records it on `VERSION.json`. That helper-only failure was preserved in `real-final-helper-preflight-attempt1.json`, corrected before the final watcher, and no tests ran during it. Prior canonical pipeline attempts were not reused as successful r9 build attestations.
