# Windows multilingual r4 delivery — 2026-09-30

The installer is built and its input Candidate passed strict verification.
It has not been installed over the user's active game or on the reported
Alienware laptop. Source/tests/offline rendering and human installation
acceptance remain separate.

## Delivered file

`Numeron-Legend-of-Rebirth-20260930-r4-Setup.exe`

Local output directory: `C:/mir2-playtest-releases/20260930-multilingual`.

| Property | Verified value |
| --- | --- |
| Product / shortcut / uninstall name | Numeron - Legend of Rebirth |
| Installer version | 2026.09.30.4; PE numeric version 2026.9.30.4 |
| Setup size | 601,242,477 bytes |
| Setup SHA256 | `60B0D003C7F366A0B82E465361D930DA8E03341A57D7F77297CE1CECCB48DAFF` |
| Setup architecture / subsystem | AMD64 `0x8664` / GUI `2` |
| Languages | `en`, `zh-TW`, `pt-BR` |
| Native source commit | `97144d6a0c819d608c0e9d1c5034259cac489297` |
| Native EXE SHA256 | `ACE26DEDC4F640757558751293F1EEABB730D86862465B1C47B87A2995A106A0` |
| Candidate | `WN-CANDIDATE-20260929-invited-04` |
| Verified Candidate files / bytes | 123,030 / 756,845,018 |
| Candidate manifest SHA256 | `B4C350C993DA8EEE5C90BC29FFFABD0C080030B9D28CE36936B4B02593AD364B` |
| Final installer recipe SHA256 | `210904E7394EC90B4F44706216F4EDAE4E8241C271B5FCBCC9988BADBA8CEEB2` |

`BUILD-ATTESTATION.json` records a clean, locked Rust 1.95.0 release build.
`package-verification.json` records no failures, the exact native source
commit, valid detached CMS signature and file closure. The independently
generated installer input receipt checks every payload byte and records no
extra files or runtime logs. ISCC returned 0; the output's actual PE headers,
version resource, name, size and SHA256 are recorded in
`installer-build-result.json`. The setup itself has no public Authenticode
signature; the package integrity signature is a separate mechanism.

The compiler and bundled Microsoft runtime both had valid publisher
signatures before use. The fixed AppId and installation identity are retained
for upgrades. The three localized player guides and the font's readable OFL
are included. First-install locale seeding preserves existing preferences;
login and in-game options provide subsequent language selection.

## Bounded corrections during packaging

The first Candidate attempt failed on font/prose path-scan false positives;
the retained failure and passing targeted regressions are in
[the guard follow-up](../packaging-guards.md). The final Candidate passed.

The first Inno compile rejected `GetTempFileName`, which is not a Pascal
support function. Only the installer recipe was corrected to the documented
[GenerateUniqueName function](https://jrsoftware.org/ishelp/topic_isxfunc_generateuniquename.htm)
with its existing safe-path and no-replace preference checks. The successful
recipe hash above identifies this later installer-only correction. The native
EXE, signed Candidate and verified literal input list were unchanged; no
additional native build or gameplay suite was needed.

The previous shared 1,147, Windows 777 and tooling 12 checks plus 47 offline
GPU images remain the applicable native validation. The first compiler
failure is retained. The successful `/Qp` compiler log is empty by design;
the observed exit code and output validation are recorded in the build receipt.

## External acceptance and operational boundary

Still unverified: clean-PC installation/upgrade, actual Alienware DPI
behavior, native human three-language gameplay and the full Wizard/Taoist
0–30 routes. There was no GUI installation, account/save mutation, server
deployment or resumed capacity work during this release.

Public `/playtest/health` returned HTTP 200 at
`2026-09-29T20:01:10.4395649Z`, with HTTP/WS ready and server revision
`9ac3c17ec757fc95c800afd941be5893d8a4bd50`. The bundled `KNOWN-ISSUES.md`
retains an earlier 15-player admission-limit sentence; this health snapshot
reports configured `maxActiveSessions=51`. Treat that old static sentence as
stale, and neither number as a proven gameplay capacity. The server config
was not changed. Capacity optimization remains paused at the user's request.
