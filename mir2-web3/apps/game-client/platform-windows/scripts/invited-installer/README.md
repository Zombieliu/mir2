# Numeron - Legend of Rebirth installer recipe

This is the source of the three-language r4 Inno Setup recipe. It consumes an
already built, attested and strictly CMS-verified client-only Candidate;
it does not build the game, change saves or deploy a server.

The player-facing product name is `Numeron - Legend of Rebirth`; the current
output is `Numeron-Legend-of-Rebirth-20260930-r4-Setup.exe`, version
`2026.09.30.4`. The internal recipe filename, Candidate family, AppId and
default installation directory remain stable so existing installs can upgrade.

Use a fresh build directory outside the checkout. Copy `Mir2-Invite.iss` and
`prepare-installer-input.py` there, together with the three `README.*.txt`
files from `../player-readme`. Supply `tools/vc_redist.x64.exe` from Microsoft
and Inno Setup 7, including its Traditional Chinese and Brazilian Portuguese
language files. Verify the compiler/runtime publisher signatures before use.
The runtime used for r4 is 14.51.36247.0, SHA256
`843068991DAAA1F73AD9F6239BCE4D0F6A07A51F18C37EA2A867E9BECA71295C`.

After `verify-windows-candidate.ps1` succeeds on the untouched signed package,
run with Python 3.13 (or a Python version supporting `Path.is_junction`):

```text
python prepare-installer-input.py <verified-package-root> <exact-40-hex-source-commit>
ISCC.exe /Qp Mir2-Invite.iss
```

The preparation step requires `WN-CANDIDATE-20260929-invited-04`, a clean source
attestation, exact recursive file closure, hashes and no runtime logs/links.
It creates a literal file list and verification receipt, refusing overwrite.
Its byte verification supplements, and never replaces, CMS verification.

The installer is x64, per user, Windows 10 or later. Keep its AppId unchanged
for upgrades. Missing Microsoft runtime installation can request elevation;
the game itself does not. It does not close running games. First installation
seeds `en`, `zh-TW` or `pt-BR` only when neither user preference nor previous
seed exists. Player preferences and server characters survive uninstall.

Compiling the installer is not proof of installation on a clean PC, DPI
behavior on another laptop, or human gameplay acceptance. Do not run it over
an active player's installation as part of build verification.
