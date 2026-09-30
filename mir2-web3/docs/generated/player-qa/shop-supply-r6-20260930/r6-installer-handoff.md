# r6 internal Windows installer

Built from clean `f8800b9a078fa3591b4b02bea9f70d642f08b709`, pushed on
`codex/playtest-registration`. This contains the two verified shop/supply UI
corrections in [the evidence](README.md), with the same nine languages.

- Setup: `Numeron-Legend-of-Rebirth-20260930-r6-Setup.exe`
- Directory: `C:/mir2-playtest-releases/20260930-ui-r6/`
- Version: `2026.9.30.6`; **602,699,175 bytes**.
- Setup SHA-256: `C02BB6E61116E10C09C1CBDD40448571CABB7FA33EB828CFA7E4D6C430998E6B`.
- Game EXE: **110,327,808 bytes**, SHA-256
  `DFA0C5ADF521FCF62A1A58CB8F21D55981795F038C05C6F1F53C145BCD897BD0`.
- Candidate: `WN-CANDIDATE-20260930-invited-06`, **123,035 files /
  764,344,934 bytes**. [Receipt](r6-installer-receipt.json).

Clean attested Release compilation passed in 3m31s. Strict package verification
passed with source checked, exact file hashes, a valid internal detached CMS
signature and no failures. Independent installer-input verification checked all
123,035 files. Inno Setup exited 0. Compiler and Microsoft runtime publisher
signatures and pinned hashes were verified. Setup and game remain `NotSigned`;
this does not resolve the other laptop's application-control error 4551.

Packaging and its unchanged child verifier ran under PowerShell 7.6.5; the
existing complete verifier SelfTest passed first. A caller-scoped `powershell`
alias selected the available `pwsh` for the child invocation. No guard, file
scan, source check or signature check was skipped or changed. The package
manifest is smaller than r5 because PowerShell 7 uses compact indentation.

The installer was compiled, not installed or launched. No active player was
logged out, no client replaced, no server deployed and no account/save changed.
The r5 artifact remains preserved. Human installation and gameplay acceptance
remain open; this is an internal playtest package, not a signed public release.

## Updating instead of reinstalling

The manifest comparison found **123,028 unchanged payload files**. Only the game
EXE, `BUILD-ATTESTATION.json` and `README-START.txt` changed, plus the four release
metadata files outside the recursive manifest. See [the comparison](r5-r6-file-change-analysis.json).
This proves that ordinary UI fixes do not require redownloading all assets.

The current native installer still has no automatic updater. The comparison is
not an apply script or an authenticated update feed. A launcher should consume
a verified version manifest, fetch only changed files, stage/check downloads,
wait for the game to close and activate with rollback. Native code and embedded
catalogues still require rebuilding; users need only the resulting changed
files once that distribution path exists. No binary-delta size is claimed.

Git's automatic maintenance again reported the known historical object
`1ae7f886fa87573bdbb59caf0772103929fac94c` as unreadable. The current commit and
push succeeded, and source/build/package checks passed. No unrelated branch or
historical object was deleted as part of this fix.
