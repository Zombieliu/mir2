# Windows Vertical Slice Evidence Report

- Date: 2026-08-26
- Branch: `codex/wn-candidate-recovery`
- Packaged source revision: `4c7e60baa5d85e63858a6fd1717af01c5f893f3d`

## Scope

This report binds the bounded functional Windows/server vertical slice to one
exact attested Release EXE and one exact nonvisual Candidate package. It does
not certify global Crystal parity, strict Windows Candidate 100%, or a
human-accepted release.

## Passed evidence

| Gate | Result | Meaning |
| --- | ---: | --- |
| `ordinary_candidate_loop` | 2/2 | Fresh ordinary-player backend loop through Bichon movement, combat, quest drop/pickup, delivery/reward, save, and new-session relog restoration. |
| Gateway fresh-account persistence | 1/1 | Normal account/login/character/start-game path, authoritative transform persistence, logout/save, and restore in a new Gateway session. |
| Original Zone checkpoint incident | Reproduced | The historical active/standby mismatch was observed; it is retained as failure evidence, not hidden. |
| Ordered Zone restore regression | PASS | Restore is replayed in order and Zone bindings are refreshed without reintroducing the removed temporary entity. |
| `zone_rpc` library checkpoint group | 21/21 | Focused checkpoint/replay library coverage passed. |
| Clean-HEAD asset generation | 312/312 | Asset generation/checks from a clean HEAD completed successfully. |
| Web typecheck | PASS | The Web typecheck gate remained green for this closeout. |

## Exact release artifact

| Field | Recorded value |
| --- | --- |
| Candidate | `WN-CANDIDATE-01-20260826` |
| Repository-relative artifact | `dist/mir2-windows-candidate/WN-CANDIDATE-01-20260826` (ignored build output, not committed) |
| Source revision | `4c7e60baa5d85e63858a6fd1717af01c5f893f3d` |
| Source worktree at build | clean; source-status digest `8416164439BB1BAACFAEAB827D65844FA92C2669F2F6AF2F69745ABF8986CC4A` |
| EXE SHA-256 | `822C718721EE6F1AB20C137AF00B86F4D887D4828776BD2D36EB231AA1216972` |
| EXE size | 66,664,960 bytes |
| Build completed | `2026-08-25T21:14:50.8743267+00:00` |
| Build attestation SHA-256 | `60AB3279F50CA85357F535430D5F80708594D1BC10B8831AFBD36971CDEDCCAA` |
| Package manifest SHA-256 | `6E6042BEA21F06D8A75612F1EF0AE49EE3AC2ABC4F98601509BEA2D1A4381DC2` |
| Manifest payload aggregate | `22E1391C434F665A0E4071721085FBA4D721A92D7C5D0F733C346408092A16F5` |
| Manifest coverage | 10,254 payload files / 322,284,094 payload bytes |
| Complete package | 10,258 files / 325,280,137 bytes |
| VERSION.json SHA-256 | `73F2B3E949622337D74B871D1FE2441408F9EF0A722EA0AE57DACB46CB3DD998` |
| RELEASE-STATEMENT.json SHA-256 | `E3330D08E1AD8CB1DA63B2576A33C634061DE4E417932B22E352B9A91B078646` |
| Detached CMS SHA-256 | `5A700EF488E24FD105B77E95A79878BE966614A1EB8F3C811FAD597A0BC93702` |
| Truth fields | `staged=true`, `builtByPackagingScript=false`, `accepted=false`, `visual=false` |

The four files excluded from the payload manifest are the manifest itself,
`VERSION.json`, the release statement, and its detached signature. Those files
are cross-bound by `VERSION.json` and `RELEASE-STATEMENT.json`; the exclusion is
therefore explicit rather than an uncovered payload gap.

## Nonvisual package and verification evidence

No command in this round used `-Launch`, and no client executable was started.
The post-build package driver and verifier were copied into the clean
worktree's ignored `dist/.candidate-tools` directory, leaving tracked source at
the attested revision. Their exact SHA-256 values were
`3688B47E499C4B920DAE1E46297663DDB410555C9AEDE2DFE8EA2AE7A3119640`
and `70AEEE83F4CAAF5126B1460EFB6DFBC3E653A74901C22D7FC2FB60190AB21170`.
Those tool changes are part of the follow-up documentation/tooling commit, not
the already-built runtime revision.

| Gate | Result |
| --- | --- |
| Package-script PowerShell 5.1 AST | PASS |
| Verifier PowerShell 5.1 AST | PASS |
| Package self-test | PASS; `ADS_SELFTEST=passed`, `REPARSE_SELFTEST=passed` |
| Verifier self-test | PASS; `VERIFY_ADS_SELFTEST=passed` |
| Attested-build self-test | PASS |
| Supply-chain static test | PASS; 15 immutable actions |
| Publisher-contract self-test | PASS |
| Windows `npm.cmd` package invocation | PASS in dry-run and formal branches; avoids PowerShell resolving `npm` as `pm` |
| Native keyed map generation | PASS; 4,650 emitted, 4,648 keyed, 2 additive, 50,595,486 image bytes; manifest `86478e87d93597432521f3b0345c07eb9bfe9ec489eacbc26774a9983fadfe7e` |
| Formal staging verification | PASS; `sourceRepoCheck=checked`, `nonvisual=true`, `launchRequested=false`, 0 failures |
| Independent clean-worktree final-directory verification | PASS; `sourceRepoCheck=checked`, `nonvisual=true`, 0 failures |
| Copied-artifact verification | PASS; `sourceRepoCheck=unavailable`, `nonvisual=true`, `launchRequested=false`, detached CMS valid, 0 failures |

The copied-artifact run reports source checking as unavailable because its
package root is outside the clean verification repository. It still recomputed
all package hashes and verified the detached signature. Source identity was
already checked in both the staging and clean final-directory runs; the copied
artifact's EXE, attestation, manifest, version, statement, and signature hashes
were also compared with the clean verified package before that final run.

Durable generated evidence:

- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-01-20260826-package-summary.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-01-20260826-verification.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-01-20260826-verification-copied-artifact.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-01-20260826-verification-initial-failure.json`

## Verification failure and recovery record

The first package attempt safely failed before publication because the old
scanner decoded arbitrary `.text` machine bytes through replacement ASCII and
misread the bytes at EXE offset 4,044,970 as `B:\...`. The replacement scanner
now bounds-checks PE sections, scans only non-executable section data, uses a
one-byte-preserving printable ASCII view, UTF-8, and both UTF-16LE alignments, and
retains fail-closed checks for machine/CI absolute paths, extended Windows
paths, UNC paths, Unix build roots, and non-basename PDB references.

The first hardened rerun then exposed a boundary bug in the exact standard Rust
virtual-root allowance: the real `//rustc/<40-hex>/library` value is NUL
terminated. That failure is preserved in the initial-failure JSON above. The
boundary was corrected and covered by a NUL-terminated regression. The final
scanner permits only the canonical Rust virtual root and a basename-only RSDS
PDB name; it still rejects near-miss Rust roots and directory-, drive-, URI-, or
ADS-qualified PDB references. The real EXE path scan and both full package
verifications then passed.

This scanner is deliberately a machine/CI path-leak heuristic over bounded
non-executable PE section data, not a claim to classify every arbitrary byte in
headers, overlay data, or executable machine-code sections. That scope avoids
the reproduced `.text` false positive while covering the compiler/debug string
locations relevant to this release check. Every regex view has a 30-second
timeout that fails closed.

## Signing boundary

`RELEASE-STATEMENT.p7s` is a valid detached CMS signature trusted by thumbprint
`B179E9D6222332C9DB5E960BAECF9990252CFBC7`. The certificate is self-issued as
`CN=Mir2 Internal Candidate 2026-08-26` and is valid from
`2026-08-25T20:45:13Z` through `2026-09-24T20:55:13Z`. This is an internal
Candidate signer only. The EXE's Authenticode status is `NotSigned`; neither an
official publisher certificate nor a formal release-signing ceremony is
claimed.

## Not closed

The following gates remain open and are not inferred from the passed backend
tests:

- Windows pure-UI execution from account creation through the quest flow;
- one continuous authenticated live WebSocket run using this exact final EXE;
- real Windows 125% and 150% OS-DPI behavior;
- a real 30-minute native-client soak with Gateway, client-log, crash-event,
  process-liveness, and memory evidence;
- original-client screenshot comparison and human visual/gameplay-feel
  acceptance;
- an official release certificate and formal Authenticode/release-signing path.

## Verdict

`Windows vertical slice = functionally exercised and artifact-verifiable
internal Candidate evidence; not global or strict Candidate 100%.` The exact
package is staged and cryptographically bound, but its truth fields remain
`accepted=false` and `visualAccepted=false`. The open same-EXE UI, live,
DPI, soak, human, and formal-certificate gates must be completed before an
Accepted claim is made.
