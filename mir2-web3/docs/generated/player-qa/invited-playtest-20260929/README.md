# Invited playtest release — 2026-09-29

User-authorized delivery: a standalone Windows ZIP and an isolated public test
realm, preserving the original service and Warrior save. Candidate only; no
whole-game, capacity or three-class acceptance is implied.

## Delivered components

| Component | Exact identity |
| --- | --- |
| Native source | `d33708e1f5d19be5881b8d658a6cdfa3d23d8a15` |
| Native EXE SHA-256 | `E6EFD6887742342532114DE313161F209070EE828C3137F3E58F911784AB00DD` |
| Gateway source | `20aeb345f8bf584582acddef5c4ac3f4874458a1` |
| Gateway ELF SHA-256 | `045b60a876573e1608bc7035151b4aab99445c9bb4b3416ac8872013077afc50` |
| Candidate | `WN-CANDIDATE-20260929-invited-01` |
| Public endpoint | `wss://165.154.65.136.sslip.io/playtest/ws` |
| Client archive | `C:/mir2-playtest-releases/20260929/Mir2-Invite-20260929.zip` |
| Archive bytes | 632,909,156 |
| Archive SHA-256 | `53A10D0337A6A7CDD64D87DD1EEF93E145F41FB9099FEDA4EB6B57BDD9CC88B7` |
| Manifest SHA-256 | `C039E7E404505FEA256C422AE8C82D4790A1D528E8DA3D37C2923ED973B8A642` |

The source pair is deliberate: later Gateway changes did not change the native
runtime. Test-only corrections and these records were committed afterwards;
they do not relabel either deployed binary. Native build: clean source, locked
Rust 1.95.0 release and prescribed path remapping.

Linux CI [36457674454](https://github.com/Zombieliu/mir2/actions/runs/36457674454)
completed successfully at the exact Gateway source. Authenticated repository,
workflow, head, artifact ID 10986333003 and artifact digest were checked before
deployment. Artifact ZIP SHA-256 is
`4fc943f0e47ce6488b89db8625b4fb3ef5489042bd3f8bc0652aa482eda26d83`;
release tar SHA-256 is
`a8b5d3e543c3c42129196622ed46c58c2bd9fc6ef4fba2ddd815e86d0831efe3`.
Only the verified Gateway was installed, not the supplied zone_host binary.

## Package and native evidence

- Strict PowerShell package verification passed with source checks and valid
  internal CMS. Independently created ZIP, decoded every member, checked CRC,
  reread extracted disk files and reverified .NET CMS plus actual metadata binding.
- 123,029 game files / 751,311,796 bytes, plus one outer Chinese first-run guide;
  every file path, size and SHA-256 matches. No extra entries, links or ADS.
- 127 actor libraries / 25,373 frames. Fifteen journey/supply maps: 24,780 original
  references and zero missing drawable resources after adding 1,509 floor frames.
  Original 842 out-of-range and 337 empty references remain explicitly classified.
  Atlas borders/page budgets and all resource allowlists remain enforced.
- Native tests: 753 passed, three existing explicit ignores. Four new tests cover
  actual TLS ClientHello and required Origin for initial and reconnect handshakes.
- Exact independently extracted EXE opened on this Windows host and authenticated
  an ordinary public account through the signed TOML endpoint, with no endpoint,
  asset-root, admin or QA gameplay override. Only explicit test credentials and
  character selection were supplied in the child process environment, never ZIP.
- Actual StartGame, correctly rendered Bichon scene/minimap, ordinary movement,
  quest list/detail, bag and Big Map were observed. Asset logs report Remote 0
  for this observed scene; three preexisting cache entries prevent treating this
  host as a clean-PC or cache-empty acceptance. Screenshots:
  [quest](native-public-quest.png), [map](native-public-map.png).
- The game was left open when manual input was detected; further UI actions were
  paused to avoid competing with the user. Native normal-exit verification was
  not claimed at this checkpoint; ordinary protocol logout/save is tested below.

## Ordinary multiplayer and recovery evidence

Local report `1790616199270-ff0bb4fa`: **11/11**. Idle-observer chat **1 ms**;
previous **11,445 ms** failure retained. Both actors share monster HP, traverse
normal `0 → 0141 → 0` doors and immediately relog after successful LogOut with
matching position, direction, gold, inventory/equipment IDs and quantities.

Fresh public report `1790617673458-79ff1564`: **11/11**, 112.116 seconds, chat
**136 ms**. Distinct ordinary accounts registered on the new PostgreSQL realm.
Unauthenticated gameplay rejection, mutual visibility, movement/turn, shared
combat, both doorway round trips, normal logout and fresh-connection persistence
passed. No admin teleport, grants, save edits or demo fallback.

Public native-resume report `1790618755549-5288c645`: **9/9**, 21.487 seconds.
Account B walked one legal step, lost its socket without LogOut, and resumed on a
new TLS connection without Login/StartGame. Same actor/state, generation and
credential rotation, used-ticket replay rejection, resumed-owner turn and normal
LogOutSuccess all passed. This tests nativeResumeV1 wire semantics; it does not
claim that the GUI reconnect loop or server-process-restart recovery was tested.
A prior harness failure misread flattened UserLocation x/y; its report is retained,
normal logout occurred, and a regression for the actual wire shape was added.

## Automated regression accounting

- Focused Gateway: live chat 5, explicit leave 6, peer-scoped auth 4 passed.
- Full Gateway after restoring three test-module mounts: **805 passed / 1 failed /
  15 ignored**. The failure expected old profileVersion25; committed content was
  already26. Test-only correction then passed. Seven V2 tests ran separately with
  their required environment and passed. **813 unique tests passed; eight existing
  PostgreSQL environment tests remain unexecuted.** No second full-suite pass claim.
- The earlier TownRevive fixture expected spawn instead of latest safe-area bind;
  corrected fixture retains real AOI and authoritative-state assertions and passes.
  These test changes do not alter shipped runtime behavior.
- Node ordinary smoke21 and native-resume8 passed. Real isolated Redis: nine
  groups, including 16 stale-refresh/release and 16 new-owner/cleanup races.
- Ubuntu non-root release/save-recovery security gate and actual Caddy network
  fixture passed. Origin, prefix/query forwarding, real-peer header overwrite,
  six forbidden-route checks and a real WebSocket101 were verified.

## Deployment and operational limits

Original service remains `79ba815c0-checkpoint-base64`; its Gateway, Zone,
validators, database and user save were not replaced. Original database/config
backup is root-only `/var/backups/mir2-before-playtest-20260929`; dump restore index
was checked. Independent system user, PostgreSQL database/role, secrets, recovery
MAC/journal and Redis6381 isolate the new realm. New DB role can access zero old
production tables. New Gateway7210/TCP7200 and Redis6381 listen only on loopback;
Caddy exposes only `/playtest/ws` and `/playtest/health` under existing TLS.

Both health endpoints remain ready. New service restart count and recent
panic/OOM/save/recovery fault counts were zero after protocol/native smoke. With
one native actor online, Gateway cgroup memory was about638MiB and host available
memory about1.65GiB. These are point observations, not capacity or soak results.

Active-player cap15, WS cap30 and resume cap15 are admission limits only. The new
realm is empty of the user's old a1 save. Small-group invitation describes package
distribution; no invite-code registration restriction exists on the endpoint.

Players need Windows10/11 x64 plus the Microsoft Visual C++ v14 x64 runtime
(`VCRUNTIME140.dll` import). The Chinese guide links the
[official runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist).
Internal CMS is not public Windows Authenticode signing. Installer/auto-update,
clean-PC and cache-empty use, 125%/150% DPI, long native soak, load testing, Wizard/
Taoist 0–30 human routes and full Chinese text remain open. Observed UI can move
one tile when closing/opening a panel; click consumption needs a focused follow-up
before calling the entire UI accepted.

Detailed operator evidence lives outside the player ZIP at
`C:/mir2-ui-repair-20260921/playtest-release-20260928`. The checked compact results
are [package](zip-delivery-result.json) and [protocol](protocol-verification-summary.json).
Private passwords and resume credentials are absent from committed evidence.
