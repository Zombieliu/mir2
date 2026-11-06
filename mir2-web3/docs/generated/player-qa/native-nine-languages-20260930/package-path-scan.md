# Nine-language package path-scan correction

The first full r5 staging run from clean source `2f51bff04bca3854e5131e54326371f33c40610f` failed its existing build-path gate. All other package gates passed. This was a failed staging attempt, not a delivered installer.

The exact native EXE was 110,323,200 bytes, SHA-256 `443E761CA8FE056E23842EEE766EA8AF68F75E02775146446FBD3833FB2AC571`. The rejected `W:\VS_Da` bytes at file offset 70,223,450 belonged to the complete, independently SHA-pinned Noto Sans Regular font. Its embedded font began at 70,041,935; the match was inside its binary glyph-outline table. The pre-existing scanner handled only the pinned TC font's variation table.

The correction pins all nine complete font files independently of their metadata, verifies the SFNT directory and exact table bounds, then clears only the reviewed binary `gvar` or `glyf` span in a private scan copy. Font names, metadata, other tables, neighboring bytes and EXE/package files remain unchanged. A changed, truncated or section-crossing font receives no partial exemption. Root independently verified every whole-file hash and table boundary, including non-overlap with all other tables.

The next exact-EXE scan exposed a separate ASCII-view false positive at offset 76,204,529: `n:\n  Chi`. This is the end of Vietnamese `nhận` followed by a JSON newline in `client.UserNameDescription`; the immediately preceding bytes `E1 BA AD` encode U+1EAD, a letter. The Unicode scan already excludes a drive prefix preceded by a letter, mark or number, but the single-byte scan lost that boundary. The correction strictly decodes only the preceding UTF-8 character inside the same scan range and applies that existing L/M/N rule to an ASCII drive candidate. It restarts inside the rejected match so an actual path later in that candidate cannot be swallowed. Invalid, truncated, overlong and surrogate UTF-8, punctuation, whitespace and independent UNC/PDB checks retain their rejection behavior.

## Confirmed verification

- Windows PowerShell 5.1.26100.9444 complete verifier `-SelfTest`: exit 0.
- PowerShell 7.6.5 complete verifier `-SelfTest`: exit 0.
- The exact EXE above passes path inspection in both complete runs and an independent PowerShell 7 probe.
- New controls cover all eight added fonts, precise masked/unmasked boundaries, source mutations, truncated and cross-range blobs, injected paths and adjacent paths in multiple encodings. UTF-8 controls cover L/M/N word tails, inner and adjacent real paths, invalid encodings, separators and section boundaries.
- Existing asset-closure, source-path, UNC/PDB, ADS and detached CMS controls remain passing. `git diff --check` passes.

Raw passing logs are retained beside the other evidence in `logs/verifier-r5-final-ps51-selftest.log` and `logs/verifier-r5-final-ps7-selftest.log`. The initial package rejection is retained as `r5-initial-package-verification.json`; the subsequent intermediate path failure remains in `logs/verifier-r5-ninefont-ps51-selftest.log`.

This patch changes only the verifier and this evidence. It does not rerun or supersede native gameplay/GPU acceptance, create a public publisher signature, or resolve Windows launch error 4551 on the other laptop. A final installer needs a fresh clean-source attestation and a passing complete package verification after this commit.
