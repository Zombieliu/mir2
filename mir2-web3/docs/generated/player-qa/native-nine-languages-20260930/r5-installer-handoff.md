# Nine-language r5 internal installer

Built on 2026-09-30 from clean source `c7eacee12599a535065ddfe81c0539450f65a5bb`, pushed on `codex/playtest-registration`. This is an internal invited-playtest artifact, not an Authenticode-signed public release. The separate application-control launch failure on the user's other laptop remains unresolved.

## Delivered artifact

- File: `Numeron-Legend-of-Rebirth-20260930-r5-Setup.exe`
- Local release folder: `C:/mir2-playtest-releases/20260930-multilingual9/`
- Installer version: `2026.9.30.5`; size: **602,942,741 bytes**.
- Setup SHA-256: `BC48A3952F38A345653C67C90CACA86C585B8C5756F476B3C3F6A8D4F581B988`.
- Game EXE: **110,323,200 bytes**, SHA-256 `37DDEC37222B29DF588A3903D30D9155869E7BB878D4ECB316126656FA40BFEE`.
- Candidate: `WN-CANDIDATE-20260930-invited-05`, **123,035 files / 774,061,001 bytes** including four manifest/signature metadata files.
- Languages: English, Traditional Chinese, Brazilian Portuguese, Russian, Hindi, Indonesian, Vietnamese, Thai and Arabic.

The prior r4 artifact remains unchanged. This build did not install the client, replace a running game, log in, edit player saves, or deploy a server.

## Confirmed release checks

The clean-source attested release build passed in 3m 34s. The complete package verifier passed with `sourceRepoCheck=checked`, no failures, exact file hashes and a valid internal detached CMS signature. The independent installer-input verifier then checked the same 123,035 files, with no extra files or runtime logs. The original public WSS endpoint is retained.

Inno Setup 7 completed with exit 0 using the literal verified input list and the nine-language recipe. Its compiler's Pyrsys signature and the Microsoft Visual C++ runtime's publisher signature were verified. The final setup has the expected version/name; both setup and game EXE report **`NotSigned`**. Their bytes and SHA-256 values are captured in [the release receipt](r5-installer-receipt.json).

The language implementation's already-passing shared/native/Node/GPU checks are in [the main evidence](README.md). No additional gameplay test was inferred from successful compression. The two initial build-path false positives, narrowed fixes and passing verifier controls are documented in [package-path-scan.md](package-path-scan.md); the failed attempt remains recorded.

## Limits and 4551 follow-up

This installer includes localized launch-error handling and bounded diagnostics, not a way to disable or bypass Windows policy. Internal CMS integrity signing is separate from an Authenticode publisher signature. A public release still needs an approved public signing identity, signing of game/setup/uninstaller, and acceptance under the affected laptop's actual policy. A valid signature alone does not guarantee that a managed policy will allow it.

The read-only `Mir2-Launch-4551-Diagnostics-20260930.zip` is retained in the release folder and at `C:/mir2-playtest-releases/`. It must be used on the affected laptop; this machine's status cannot identify that laptop's policy. Its instructions include built-in read-only commands if PowerShell refuses the diagnostic script. No affected-laptop report has been received yet.

Clean-PC installation/launch, the affected laptop, human nine-language gameplay and native-speaker review remain unaccepted. Large parts of the six new content catalogues are offline translation drafts. Arabic shaping/bidi is supported without mirroring the complete Crystal geometry. This delivery does not claim complete caster 0–30 acceptance, 50–100-player capacity or global Crystal parity.
