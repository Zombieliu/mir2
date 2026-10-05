# Current-source updater release preview, 2026-10-05

The updater/launcher was built from pushed root source
`48454bba7623694ca7432613af52bee28e83970e`. The game remains the original
`WN-CANDIDATE-20261004-invited-17`, source
`6032ef8b3e27dd97bad0b20c8676ef9f185db64b`. This is a local preview, not a
public release. Preview feed sequence 13 has not been published; the known
public sequence remains 12. Do not distribute the preview Bootstrap as a
working public installer before publishing a coherent authenticated feed.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Mir2Updater.exe | 1,859,072 | `50F99917916F10E8E8F7E906DE1D6E1FA7EDB9A735F0CE95873BF5D196D426CF` |
| Mir2Launcher.exe | 389,120 | `E478321DCDCD107669D544DEFD9FC3318E2B50BD26DCA4FCCC042A84EF69635D` |
| Numeron-Legend-of-Rebirth-20261005-r17-Bootstrap.exe | 27,496,643 | `6175BD72F273C2F00D96117183FC1BF4D1F987C15455A1064E4F0E975CA70AF5` |

The local installer is at
`C:/mir2-playtest-releases/20261004-native-r18/updater-resume-release-root-01/bootstrap/artifacts/`.
The engine bundle verifies its pinned detached CMS, executable hashes and
source identity. The Bootstrap build validates the original R17 metadata,
updater bundle, nine-language inputs, pinned VC runtime and compiler. It
embeds the new engine but no game payload, assets or game build attestation.
It was compiled, not installed or launched. Installer Authenticode is still
NotSigned; detached updater trust is a separate mechanism.

The Bootstrap uses three unchanged issued helper scripts from exact source
`4c60c323aed11829abae6ee5ce6e13c675a3c1c3`; their bytes are archived. They
verify the historical immutable R17 package. This does not replace current
root's broader Candidate map-scope guard or count as its acceptance. The
historical external-package sourceRepoCheck is unavailable. Root production
sources and all 20 source paths recorded by each runner receipt stayed exact
before, after and at evidence collection. The external repair harness's 85
registry package versions/checksums match the root lockfile.

Two previously ignored library fixtures now actually ran against this source:
the CMS trust/negative-fixture case and signed sequence 12/13 discovery race
and tamper case. Each passed one test with zero failures/ignores and 68
filtered tests. Two other existing signed library fixtures were not rerun.
The separate 39 Python Bootstrap checks passed; those are structural checks,
not substitutes for signed integration tests.

The isolated prepared repair uses the real R17 full manifest/CMS and actual
release-profile library. It pre-populates 123,917 verified files and a verified
warm content cache, then repairs 2,048 selected files (143,571,478 target bytes,
1,996 unique content objects) and rolls the old engine to the new engine. The
sample's exact definition, forced paths, size-bin allocations, TSV and hashes
are archived. File-mapped transport is not HTTPS or interrupted resume.

| Actual measured stage | Seconds | Relationship |
| --- | ---: | --- |
| Current-source check_update | 81.170 | Overall update API call |
| Checking 125,965 installed entries | 47.593 | Inside check_update |
| Staging the 2,048 target files | 6.854 | Inside check_update |
| Preparing 2,055 transaction records | 1.422 | Inside check_update |
| Installing the 2,055 transaction records | 22.692 | Inside check_update |
| Independent full target/metadata/engine/launch verification | 35.740 | After check_update; no game execution |
| Pending-first-launch recheck | 0.826 | Separate call, no changes/downloads |
| Real quarantine and rollback | 13.993 | Separate call |

The four internal phase times are contained in 81.170 seconds, not added to
it. Pre-population and cache setup take additional time and are excluded from
that API measurement. These measurements do not establish fresh-install time
or public download speed and cannot be compared as an A/B speed result with
the earlier 41-minute dev-profile full fresh retry.

**The original prepared-repair command exited 1 and remains failed.** It had
already verified the full new target, pending state, executed rollback and
verified the old baseline/metadata/engine and six synthetic game-directory
personal-file witnesses. Its harness then incorrectly compared highest.txt
to numeric `13`; the file actually stores an authenticated feed SHA digest.
The original source, stdout, stderr and receipt are preserved unchanged. Its
later branches did not execute and its intended result file does not exist.

An additive validator subsequently authenticated the digest's sequence-13
CMS receipt, quarantine, original sequence-12 seed, restored active engine,
inactive new engine, rolled-back journal and actual backups. It verified all
123,917 baseline files, the 2,048 absent repair targets and six synthetic
witnesses. It also called real check_update twice: same signed sequence 13
was rejected as `release quarantined after unsuccessful first launch`; lower
signed sequence 12 was rejected as `update downgrade rejected`. Both fetched
only the feed pair. Fingerprints of 2,066 explicitly selected state paths
were unchanged. This is not a whole-installation, all-cache or AppData audit;
the validator does not turn the original command into a passing run.

The old Bootstrap installs all game files before upgrading its engine, so an
engine-only feed cannot accelerate that old executable's first empty install.
The new Bootstrap starts with the new engine. Existing completed installations
can receive an engine-only update without recompiling the game.

Actual local Wrangler whoami exited 1: not logged in; its OAuth token expired
and could not refresh in this non-interactive environment. This local CLI
result is distinct from the earlier opaque CI user/account token 401s, whose
cause is not established. No effective Cloudflare authorization exists yet.
No remote upload, Worker deploy, CAS promotion, public feed update, public
HTTPS resume, CDN cache/range/speed acceptance, fresh installation, other
laptop run, game launch or human acceptance occurred. Gateway/Caddy, live
game, F installation and real saves were not changed.

[Proof inventory](PROOF-INVENTORY.json) binds 130 regular archive members;
[byte-exact archive](BYTE-EXACT-PROOF.tar.gz) is 2,563,824 bytes, SHA-256
`19a0a57e3fc3ebc4363bbb410f1321a489381aac759b982eb7d370db6c73c72b`.
All members were read back and hashed without extraction or execution. It
contains original command results (including both nonzero commands), source
snapshots, external harness, signed fixtures, build controls and selected
post-rollback state. It excludes the full game/preset/cache/backups, installer
EXE, private keys/configs, fonts, VC/Inno binaries and build targets. The
original prepared-repair runner is reconstructed byte-exactly against its
recorded pre-run SHA; the additive audit source and later runner are separate.

[Root review](ROOT-RELEASE-REVIEW.json),
[additive audit](PREPARED-REPAIR-ADDITIVE-AUDIT.json),
[Bootstrap build](BUILD-BOOTSTRAP.json), [updater build](BUILD-UPDATER.json).
