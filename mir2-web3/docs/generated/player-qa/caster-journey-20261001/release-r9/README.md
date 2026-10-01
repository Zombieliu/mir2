# r9 medicine-shop delivery

The r9 Windows installer is available at
`C:/mir2-playtest-releases/20261001-native-r9/Numeron-Legend-of-Rebirth-20261001-r9-Setup.exe`.
It contains Candidate `WN-CANDIDATE-20261001-invited-09`, clean client-only
source `180b01d46f35a5e97f4c60ee2ae033c7b53717e5`, based on the public r8
`3f5e61533235921369bc13a7760b4a56b0e467e5`. This release changes the client
shop lifecycle; it does not deploy the separate caster simulation/controller work.

The delayed passive `activeNpcDialog: null` snapshot no longer closes the
purchase service and restores the bag over it. Ordered exit/retry, late goods,
NPC changes, map/logout and actual empty/nonempty `NPCResponse` boundaries
retain their documented closing behavior. See the
[dynamic reproduction and fix](../shop-lifecycle-20261001.md) and
[actual response boundary evidence](../npc-response-service-boundary.json).

## Artifact identity

| Artifact | Result |
| --- | --- |
| Installer | 603,527,723 bytes; SHA256 `951EF073484C2FAF281679BADB389BFEA1A4985FEEA04955532B4ABB8B927FB8` |
| Native executable | SHA256 `09E186B9F13F7F1E10D4857EFAE52EF940B4F43E2D7CE955F4FE1627C433A488` |
| Clean-source attestation | SHA256 `1FD9D1D881054D7BAFAA1B71A9E80816C8F54FD5BC62E77F06433CF24A92BA10` |
| Complete package | 123,035 files, including 123,031 payloads |
| Signed discovery | Invited sequence 5 |

The [local release receipt](local-release/r9-release-receipt.json) binds the
exact source, signer, package, installer and updater identities. The original
[canonical verifier](local-release/WN-CANDIDATE-20261001-invited-09-verification.json)
retains its staging path and original bytes. Its reuse is documented separately:
[unchanged source, critical files, completion and signature guards](local-release/candidate-verification-reuse.json).
Installer-input verification separately checks all package files. Earlier
four failed build/package preflights remain in the local evidence inventory;
they are not counted as successful builds.

## Signed-package updater and publication

The actual Rust r8-to-r9 signed-package integration tests pass **2/2** in
309.15 seconds. They verify all new payload hashes, unchanged assets, retained
personal fixtures, signature tampering rejection, first-launch quarantine and
rollback of both game and updater engine. These use an owned fixture tree;
they do not launch the game. See the
[QA scope](local-release/qa/real-update-qa-final.md) and
[actual report](local-release/qa/report.json).

The measured delta is six changed game payloads and one updater engine:
112,374,808 bytes (**107.17 MiB**). All 123,025 remaining payloads are unchanged;
no `mir2-assets/` payload is downloaded. Version, manifest and signature
metadata transfers are separate from the payload counters.

Publication completed on October 1, 2026 at 18:57 UTC. The update endpoint is
[invited discovery](https://165.154.65.136.sslip.io/client-updates/latest.json).
The independent [public HTTPS check](publication/public-https-publication-report.json)
uses normal certificate verification and confirms byte-for-byte equality with
the locally verified sequence-5 feed and detached signature, both `no-store`.
The [server receipt](publication/deployment/apply.log) records immutable release
promotion and the atomic discovery switch. Caddy's hash and both Gateway
health/revisions are unchanged; port 7210 remains on r8 `3f5e61533`. No game
service restart, save migration or account-store editing is part of this rollout.
Configured admission ceilings are not capacity acceptance. Capacity remains paused.

## Actual installation and acceptance scope

The closed F-drive installation check **timed out after 900 seconds** before
any native activation receipt. At that failed checkpoint, installed metadata
still reported Candidate08.
That actual installation failure is retained, distinct from the passing
signed-package fixture tests and public discovery checks. Its
[original helper and failure identities](installed-check-attempt1/report.json)
are not overwritten. The pre-payload interval includes remote metadata retrieval
and local verification; it is not measured as purely local hashing.

A fresh [native HTTPS retry](installed-check-attempt2/actual-native-https-update-report-attempt2.json)
**passes** and activates exact Candidate09 and the verified r9 engine. It reuses
five independently hashed complete caches and downloads two remaining payloads
(112,308,736 bytes); the partial EXE is discarded by the existing safe cache path.
All six changed game files and all four metadata files match the verified package.
Eight language/display/control preference files remain identical. A repeated
check downloads zero payload bytes/files. Native launcher signature/engine
verification passes before and after. The helper does not launch the game or
edit saves. A subsequent normal launcher boot opens the blank login screen;
the updater transaction becomes `accepted` through its actual first-launch
health path. The obsolete Start Menu shortcut is backed up and redirected to
the launcher, matching both desktop entries. The
[boot receipt](installed-first-launch/native-launcher-boot-receipt.json) records
the exact installed EXE, engine and zero-download native recheck.
Authenticated mouse/GUI acceptance remains separate. Recovery identities are preserved in their
[own inventory](installed-check-attempt2/inventory.json).

The root client checks pass shared 1197 / 10 ignored, Windows 809 / 5 ignored
and runtime 276 / 1 ignored. The earlier `715078944` backport checks remain
historical 803 / 5 ignored results and must not be relabelled as an exact
full-suite run of changed `180b01d46`. Clean attested build, complete package
verification and actual signed-package tests cover the final release identity.
Six delayed-hide offscreen GPU captures are synthetic production UI/ECS
fixtures, not authenticated native mouse purchases or human gameplay acceptance.

Authenticated native login, real Samuel Buy/Sell clicks and affected-laptop acceptance remain
open. Internal pinned-key CMS signatures are separate from public Authenticode
publisher signing and the other laptop's Windows application-control issue.
No whole-game 1:1, fresh caster timed-route or 50–100-player acceptance follows.

[Local evidence inventory](local-release/inventory.json) preserves 55 original
small artifacts with exact-byte hashes; the publication evidence has its own
[inventory](publication/inventory.json). No large binaries, manifests, account
stores, raw account traces or private credential-helper contents are committed.
