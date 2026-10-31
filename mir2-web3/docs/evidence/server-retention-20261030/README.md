# Server retention evidence — 2026-10-30

Fresh host operations on 165.154.65.136. This is storage maintenance evidence, not a combat capacity qualification.

- [inspected-candidates.json](inspected-candidates.json): all 32 old allowlisted candidates inspected; 3 hardlink refusals and the separately retained source-preparation directory are excluded from deletion.
- [reference-audit.py](reference-audit.py): frozen, host-specific read-only reference collector used for this operation; not an automatic deletion scheduler.
- [reference-before-execution.json](reference-before-execution.json): fresh reference scan, zero unresolved errors; file paths/hashes and service identities only, no config/env contents.
- [execution-plan.json](execution-plan.json), [execution-check.json](execution-check.json): exact raw plan and whole-batch precheck. Preserve original bytes and integer precision when verifying SHA or metadata timestamps.
- [cleanup-result.json](cleanup-result.json): 28 completed directory deletions, 1,000 entries removed, partial=false; actual filesystem free-space delta recorded separately from logical bytes.
- [journal-deployment.json](journal-deployment.json): effective settings, journal-only restart/rotate and archived-log retention; original rsyslog policy unchanged.
- [public-channels-post.json](public-channels-post.json): six HTTP GETs and installer HEAD; hash equality to pre-maintenance bytes is checked in final-verification.json. This is not a new cryptographic CMS validation or a full 1 GB installer download.
- [linux-cleanup-tests.json](linux-cleanup-tests.json): 31 passing tests on real Linux, including seven filesystem fixtures; tests use their own generated directories.

Private full records and small provenance archive remain under /var/backups/mir2-retention-20261030 and C:/mir2-build/server-retention-20261030. Raw initial gate failures are retained there: a Python data-encoding error occurred before any mutation; an overly strict concatenated-config check stopped after writing the intended journal drop-in and before activation. Final effective-key verification and execution succeeded; no failed cleanup plan was blindly retried.

No Docker operations, game restarts, account changes, real funds, release changes or CCU tests took place. Public releases, original backups and all uncertain candidates remain protected.

- [audit-installation.json](audit-installation.json): all 31 daily-audit tests passed on Linux (including 15 filesystem fixtures), unit verification passed, actual initial oneshot succeeded and timer is active/enabled. It includes exact installed hashes and initial report.
- [final-verification.json](final-verification.json): 28 deletion paths absent, all 13 remaining candidates present; four live PIDs unchanged, health/Redis OK, feeds unchanged, deployed pins match, private report permissions and active timer verified. Warning is expected because recovered free disk is 17.9%, below the configured 20% threshold; there are no collection errors.

- [runbook-update.json](runbook-update.json): installed-status prose correction only, with old/new runbook pins; final verification uses the updated runbook hash. Runtime settings did not change.

- [protected-baseline.json](protected-baseline.json): original 2026-10-30T05:06:20Z read-only baseline (844,476,416 B available), original service PIDs, feed hashes/resolved paths, health and pre-maintenance journal settings. Later pre-execution free-space readings differ because services remained live.
