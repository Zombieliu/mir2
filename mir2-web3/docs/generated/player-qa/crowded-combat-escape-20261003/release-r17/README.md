# R17 paired crowded-combat release

Status: packaged and published, **not** complete P1 or human acceptance.
The Goal continues with dense-hit/native acceptance and classic map closure.

Game source is `6032ef8b3e27dd97bad0b20c8676ef9f185db64b`.
Windows Candidate is `WN-CANDIDATE-20261004-invited-17`; signed discovery
sequence is 12. The invited Linux Gateway uses the same game source. The
updater/bootstrap recipe remains the frozen delivery source
`4c60c323aed11829abae6ee5ce6e13c675a3c1c3`.

The [release state](release-state.json) indexes the source evidence and hashes.
Build-stage `published=false` or `deployed=false` fields in copied input
receipts retain their original meaning; publication is proved separately by
the live and public receipts, not by rewriting those historical inputs.

## Delivery evidence

- [Independent strict Candidate verification](strict-candidate-verification.json)
  checks clean source, pinned detached CMS, PE, package/resource closure and
  the exact 111,013,376-byte EXE. EXE SHA is
  `d0780475efa9f9c1ecd730dfcfc11e38b9b0659f05b3b1f7d93b4f80c3d734e0`.
  Canonical manifest SHA is
  `ceae366bebca1c19c3f529eb22f016ccc80dff5b8655694e76682e31ab12cb18`.
- [Exact Linux CI input](linux-ci-inputs.json) records
  [run 37165934500](https://github.com/Zombieliu/mir2/actions/runs/37165934500).
  Gateway SHA is
  `1ed52738885292b734c4055a9fd7d585b79d519779084bca2a617d0a9f5eb9ab`.
  Rust 1.89/save recovery and isolated Sabuk domain/conquest/PostgreSQL/transport
  gates passed within their stated scopes.
- [Gateway apply](gateway-apply.json) records zero sessions/leases/in-flight
  operations before shutdown, retained database/private-state backup, normal
  old and new shutdowns with exit 0, restart and unchanged environment/resource
  limits. Rollback is configuration-only; it does not overwrite player progress.
- [Final read-only live verification](final-live-verification.json) checks the
  exact executable, source, healthy endpoints, unchanged original realm PID
  and executable, environment, base service unit and Caddy hashes.
- [Feed publication](feed-publication.json),
  [public full-byte checks](public-update-verification.json),
  [updater identity](updater-bundle-verification.json) and
  [signed resource inputs](signed-resource-inputs.json) bind the public feed,
  signatures, full manifest/EXE/updater and seven Sabuk layout files. TLS stays
  enabled and redirects are rejected. No separate Python CMS claim is made:
  downloaded bodies match the inputs verified by the root's pinned CMS gate.

## Actual update and installer

The real signed R16 to R17 file-source integration test passed **1/1**; the
separate historical hardcoded r6 test was filtered. This is an isolated copied
installation, not the user's installed client or a public-HTTPS installation.

[Update/rollback receipt](signed-package-update-rollback.json) and
[exact changed payloads](signed-resource-delta.json) record three files:
`BUILD-ATTESTATION.json`, `README-START.txt` and `mir2-platform-windows.exe`.
Their payload is 111,064,197 bytes; including metadata, wire size is 135,432,721
bytes. 125,962 game payloads and the updater engine are unchanged. No
`mir2-assets` files are downloaded and no binary-difference patch is claimed.
New hashes, personal-file preservation, pending-first-launch state, quarantine
and rollback to R16 are checked. The game was not launched by this test.

The runtime-inclusive nine-language online installer is 27,482,388 bytes.
[Build proof](bootstrap-build.json), [immutable publication](installer-publication.json)
and [public HEAD/range/full-body SHA](public-bootstrap-verification.json) all
match SHA `aced5e53a57c0e98081f8415c5ebfb95942be7b3cbb130490db24e5f8dc15621`.

[Download R17 online installer](https://165.154.65.136.sslip.io/client-updates/releases/bootstrap-WN-CANDIDATE-20261004-invited-17/Numeron-Legend-of-Rebirth-20261004-r17-Bootstrap.exe).
This is hosted on the existing static download server; R2/CDN is not promoted.
The installer and EXE remain publisher-Authenticode `NotSigned`. Pinned CMS
integrity does not resolve the other laptop's Windows 4551 application-control
block, and clean-PC/laptop installation is still unaccepted.

## Protocol scope and remaining gates

[Independent ordinary-network audit](ordinary-network-analysis.json) uses the
exact R17 Linux ELF in a new isolated File realm with unsafe QA disabled.
Three fresh classes register, create, equip, walk/run, **attack Deer then move**,
stop, normally log out and relog with matching items/transforms. Owner attacks
deal positive damage 2/4/3 before their escape ACKs of 529/527/526 ms. At least
1.5 seconds of no-input checks are stable; owned SIGTERM exit is 0.

**No monster hit the owner** in those trials. Fresh characters had no learned
spells. This is not seven-monster continuous-damage, full-surround, learned
cast/death-barrier, native mouse/render, five-second drift, load or human-feel
acceptance. Source-level dense Zone tests remain distinct. The next exact-ELF
plan naturally lures original hostile monsters and records prepared-character
state separately from an ordinary leveling journey; failed geometric
preconditions are inconclusive, not passes.

This release retains the existing 32-map native image closure and 1,620 raw
layout gzip files. The newly generated 164-map resources are not in R17.
The user's F-drive installation/update journal, other games, player saves and
original realm were preserved. Capacity work remains paused; configured
51-session limits are not a capacity acceptance result.

Missing cached inputs, two long-Windows-path packaging attempts, the first
installer verifier's stale R16 URL and the first final-live verifier's wrong
feed path remain recorded in `release-state.json` with original external logs.
The strict gates were retained; the successful package uses a clean short
managed worktree and corrected immutable verifier recipes.
