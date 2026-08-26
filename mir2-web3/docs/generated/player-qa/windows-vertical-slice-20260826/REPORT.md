# Windows Vertical Slice Evidence Report

- Date: 2026-08-26
- Branch: `codex/wn-candidate-recovery`
- Packaged source revision: `f0ab4936c44df304e60e66e08529913201636b51`

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
| `platinum_176_combat_milestones` | 15/15, repeated twice | The current combat certificate is reproducible; Warrior D504 now records real `FlamingSword` damage against an auto-revive Zuma Guardian. |
| Focused melee packet regressions | 2/2 | `FlamingSword` and `Thrusting` retain their declared Crystal defence types through delayed packet damage. |

## Exact release artifact

| Field | Recorded value |
| --- | --- |
| Candidate | `WN-CANDIDATE-02-20260826` |
| Repository-relative artifact | `dist/mir2-windows-candidate/WN-CANDIDATE-02-20260826` (ignored build output, not committed) |
| Source revision | `f0ab4936c44df304e60e66e08529913201636b51` |
| Source worktree at build | clean; source-status digest `2C6A5536992A07D01499AA8736926B357808E699DA1A978C3D873C6A8D2EB1B1`; 0 status lines |
| EXE SHA-256 | `F516F3C9B9122D719F809F8F903147FCB4E999822DF3183AC7CC1F17C8172CA6` |
| EXE size | 66,665,472 bytes |
| Build completed | `2026-08-26T02:00:54.7847597+00:00` |
| Build attestation SHA-256 | `387D922E8EB4EB04FD72A895986F5F0ECC0F701A324C740DDB21A94B0B2EB979` |
| Package manifest SHA-256 | `FEEEEED6E78096171285A011181E9EAED11F0073367224EA8F1DC8C3E49FF467` |
| Manifest payload aggregate | `B3A83FB815555494AC9DDC18A1FA76975E31B5D1CD68A74BE8A4049ED74FEF5B` |
| Manifest coverage | 10,254 payload files / 322,285,374 payload bytes |
| Complete package | 10,258 files / 325,281,417 bytes |
| VERSION.json SHA-256 | `590E42F9D535E49E7712878E6FB4E6CA654CB8F3DF0812ED25E20C470D5D8E79` |
| RELEASE-STATEMENT.json SHA-256 | `A11FF0BBF6C6BCF8B208AE463F00160780955465959050FA5F0D571FAD313B8B` |
| Detached CMS SHA-256 | `BB12437B56ECEB4B79282D5D2330666A1D663B52AF48CD0133AEDA8C79DC89D5` |
| Truth fields | `staged=true`, `builtByPackagingScript=false`, `accepted=false`, `visual=false` |

The four files excluded from the payload manifest are the manifest itself,
`VERSION.json`, the release statement, and its detached signature. Those files
are cross-bound by `VERSION.json` and `RELEASE-STATEMENT.json`; the exclusion is
therefore explicit rather than an uncovered payload gap.

## Nonvisual package and verification evidence

No command in this round used `-Launch`, and no client executable was started.
The package driver and verifier ran directly from the attested clean source
revision; dependency and generated-asset outputs remained ignored, and tracked
source stayed clean. Their exact SHA-256 values were
`3688B47E499C4B920DAE1E46297663DDB410555C9AEDE2DFE8EA2AE7A3119640`
and `70AEEE83F4CAAF5126B1460EFB6DFBC3E653A74901C22D7FC2FB60190AB21170`.
Both tools are part of the packaged source revision.

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

- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-02-20260826-package-summary.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-02-20260826-verification.json`
- `docs/generated/player-qa/windows-package-preflight/WN-CANDIDATE-02-20260826-verification-copied-artifact.json`

## Predecessor recovery record

The superseded `WN-CANDIDATE-01-20260826` package attempt safely failed before
publication because the old
scanner decoded arbitrary `.text` machine bytes through replacement ASCII and
misread the bytes at EXE offset 4,044,970 as `B:\...`. The replacement scanner
now bounds-checks PE sections, scans only non-executable section data, uses a
one-byte-preserving printable ASCII view, UTF-8, and both UTF-16LE alignments, and
retains fail-closed checks for machine/CI absolute paths, extended Windows
paths, UNC paths, Unix build roots, and non-basename PDB references.

Its first hardened rerun then exposed a boundary bug in the exact standard Rust
virtual-root allowance: the real `//rustc/<40-hex>/library` value is NUL
terminated. That predecessor failure remains preserved in
`WN-CANDIDATE-01-20260826-verification-initial-failure.json`. The boundary was
corrected and covered by a NUL-terminated regression. The final
scanner permits only the canonical Rust virtual root and a basename-only RSDS
PDB name; it still rejects near-miss Rust roots and directory-, drive-, URI-, or
ADS-qualified PDB references. Those fixes remain in the current tools. The
`WN-CANDIDATE-02-20260826` dry-run, formal staging verification, independent
final-directory verification, and copied-artifact verification all passed
without a new packaging failure.

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
