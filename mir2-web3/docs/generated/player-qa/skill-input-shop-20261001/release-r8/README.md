# Matched r8 release and actual installed upgrade

This is the release of the bounded skills/input/shop repair described in
[the parent evidence](../README.md), not global Crystal parity or human
three-class gameplay acceptance.

The clean, pushed gameplay source is
`3f5e61533235921369bc13a7760b4a56b0e467e5`. The Windows build, signed
Candidate08, installer inputs, updater build and deployed playtest gateway
are bound to that exact source. Subsequent evidence-only commits do not
change the shipped binaries.

## Installer and package

- File: `C:/mir2-playtest-releases/20261001-native-r8/Numeron-Legend-of-Rebirth-20261001-r8-Setup.exe`.
- Candidate: `WN-CANDIDATE-20261001-invited-08`.
- Setup size: 603,541,478 bytes; SHA256
  `F8E77CB5CDA1AF6438EA16C6F80AECB079B73AE23B10F38120E6CDF24B681012`.
- Game executable SHA256
  `23356513E9F8DCE3B3E12A2FAD9F89BA5F775FBA643FBCE14763F2C7F6D88431`.
- Strict package verification and independent installer input verification
  pass for 123,035 files. The signed updater supplement has 9 files.
- Inno compiler and actual installation exit 0. Public Authenticode remains
  `NotSigned`; detached pinned CMS does not resolve application-control 4551
  on the other laptop.

Exact descriptors, build output and verification records are in this folder.
The full installer log remains in the local release directory; its final
excerpt is explicitly labelled here rather than adding its large raw file
to Git.

## Backend and update distribution

Linux CI run 36803465101, artifact 11137215177, builds the exact source.
The gateway binary SHA256 is
`d216a1a41879a85c3c20056315a37f8e7116cc8d4552c4deefa5032fabea914e`.
Only isolated `mir2-playtest.service` on 7210 was switched. Its previous and
candidate validation stops exit 0 without a shutdown panic. A verified database
and state backup is retained at
`/var/backups/mir2-playtest-before-3f5e61533235921369bc13a7760b4a56b0e467e5`.
Schema, map assets, resource/capacity limits and the original 7110 process and
release remain unchanged. Capacity work stays paused.

Signed sequence 4 is published at
`https://165.154.65.136.sslip.io/client-updates/latest.json`, retaining prior
immutable releases. Publication verifies archive/metadata hashes, pinned CMS
before upload, server idle health and the unchanged Caddy baseline. It
atomically publishes the feed/signature pair without restarting either game
service. Feed SHA256:
`F078D48FE06F5D8058C9BBB53E3A0C367165711B97DC41A88F15C16F96557914`.

The actual signed r7→r8 integration passes 2/2. It downloads only the game
executable, build attestation and start guide: 110,649,477 bytes. It verifies
all new payload hashes and 123,028 unchanged files, retains personal files,
retains the first-launch obligation, and verifies rollback/quarantine and the
highest signed sequence. The existing engine binary is identical and reused;
the new source-bound engine metadata is published under its fresh s4 path.

Real native launcher and HTTPS r7→r8 verification also passes. The first
check downloads the same three files and activates Candidate08; a second
check downloads zero files/bytes. Three native signature/build verification
receipts pass and the personal diagnostic file is retained. This owned QA
installation never starts gameplay; its first-launch acceptance remains
pending by design. See `native-https-update-report.json` and the raw event log.

## Actual user installation

Diagnosis found `F:/mir2/Mir2Invite` running r5 (`c7eacee12`) without a
launcher, even though source and prior r6/r7 packages contained the shop fix.
The real r8 installer now upgrades that exact directory, with the game closed,
and binds its executable and attestation to the clean source above. Installed
native `--verify-only` passes and its signed seed is sequence 4. The legacy
`Mir2 联网试玩版.lnk` is backed up and now targets `Mir2Launcher.exe` in the
same installation. Unrelated shortcuts were left alone.

The first local upgrade helper stopped before executing Setup because its
path comparison mixed slash forms. The corrected comparison and subsequent
successful installer/native verification are recorded; this was not a game
or installer failure. The original stopped-helper log is retained locally.

The actual desktop launcher opens the verified F-drive game at the Traditional
Chinese login screen. An app-owned Bevy screenshot and provenance sidecar
record source3f5e61533 and the exact shipped executable/manifest hashes.
The account and password fields are empty; no human credentials were entered.
The actual fresh installation checks signed sequence4 with zero downloads.
Final service health remains ready with one login-screen socket and no active
character. No OS input or desktop screenshot was used while Computer Use
remains paused. The nine production medicine-shop GPU captures and native
input/cooldown regressions do not substitute for human gameplay acceptance.
