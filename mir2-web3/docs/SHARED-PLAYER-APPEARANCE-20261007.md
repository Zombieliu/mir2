# Shared player appearance — 2026-10-07

The user reports that father and PubMinerB07 see each other's names but not
their bodies. Both ordinary clients render their own character normally.
Root is the sole writer in `codex/remote-player-appearance-20261007`; one
read-only explorer reviews Crystal/Zone/client boundaries.

## Cause and server-only correction

`ZonePlayer::from_join` incorrectly initializes `transform_type=0`. The protocol
uses `-1` for an ordinary player. `world_entity_sprite_from_object_player` selects
`Transform/00` for zero, which has no usable body in the released native assets.
The native packet-first cache then replaces the correct periodic remote sprite
with that invalid ObjectPlayer sprite. Class/gender and position are present;
this is not a missing-name/AOI or multiplayer-session implementation.

Changing the sentinel alone would expose a second gap: the shared actor had
default armour/weapon/hair because normal UserInformation/EquipItem/RemoveItem
do not emit a complete PlayerUpdate to Zone. The fix therefore:

- Initializes new ordinary shared players with transformation `-1`.
- Extracts authenticated personal equipment shapes, hair, light, wing and
  mount/fishing looks with the same shape-selection rules as SelfPlayer.
- Calibrates the actor and its unsent first ObjectPlayer during a real Join.
  An observer does not receive an initial naked/default actor followed by a fix.
- Sends a complete current ObjectPlayer to existing AOI observers only when
  looks change. Repeated unchanged synchronization produces no extra actor.
  Removing the weapon clears it to `-1` rather than inventing CWeapon/00.
- Keeps Zone position, direction, HP/life, poison, hidden state and legitimate
  transformations authoritative. The appearance DTO cannot set those fields.
- Defaults/omits zero hair in snapshots, preserving old zero-hair serialization
  and explicit valid transformation zero. Nonzero hair survives strict restore.

The released R20 client and signed update feed16 are sufficient. No Windows
installer or native resource update is required for this server correction.

## Actual checks and retained limits

| Scope | Actual result |
| --- | --- |
| Baseline new ordinary appearance tests | RED: 0/2; both show transformation zero instead of -1 |
| Candidate Zone appearance, first Join, AOI/status and snapshot tests | 5/5 pass; ordinary three classes × both genders included |
| Candidate ordinary Gateway creation/bootstrap, equip/remove and transfer/re-entry | 3/3 pass; real protocol equipment ingress and comparison to complete SelfPlayer sprite |
| Atomic Gateway world checkpoint regressions | 2/2 pass |
| Adjacent full shared Zone regression | 208 pass / 1 fail |
| Same failed drop-award assertion on unchanged released source8fa | 0/1; identical assertion at shared_zone.rs4746 |
| Read-only independent review | Main path passes; restore/replay limits below remain explicit |
| Actual isolated playtest rollout | Drained private backup, old/Candidate normal stop0 and cold restart pass; only ExecStart changes |
| Actual ordinary public WSS | 44/44 over six class/gender variants in five owned accounts; all8 normal logouts confirmed |
| Final actual public/private health | Pinned source/healthy; all six admission/session counts zero |
| Native human two-client appearance | Open; no new screenshot/GPU human acceptance is claimed |

Raw logs are retained under `C:/mir2-build/remote-player-*`; no failed run is
rewritten as a pass. The unrelated original drop-award expectation is not
changed as part of appearance repair. Fixture item/progression preparation
is test-only; no QA/debug grant is exposed to ordinary clients.

Old event-sourced ZoneReplay checkpoints re-run historical Join inputs and
may have different commitments under the corrected default. This change
does not claim old replay compatibility. Direct committed Zone snapshots
retain their explicit transformation. An already-restored erroneous actor
must leave/rejoin; do not erase all transformation zeros. Production cold
world recovery removes online session actors before new authenticated joins.

Linux CI37617054260 on source `c6c32381a646dae1067dafdeec57691f606b1779`
passes both build/security and reused siege acceptance jobs. The appearance
gate passes Zone5, Gateway3 and cold-world session cleanup1; these overlap
the Windows checks rather than increasing distinct case coverage. The actual
artifact11480868639 ZIP digest and package/binary/source metadata are verified.
The first verifier used the wrong expected archive member names; its failure
is retained, then corrected against the pinned packager's exact four entries
(`mir2-gateway`, `zone_host`, `RELEASE.json`, `README.txt`).

The current developer Windows linker could not reopen the already-built test
executable (LNK1104); no user process was stopped. That previous Candidate test
executable passes the unchanged cold-world test directly, and clean Linux CI
independently compiles/tests the final committed source. Do not call the failed
Windows relink a pass.

The exact verified Linux Gateway is now activated in the isolated invited
playtest after the human replies "已退出，可以切换". Read-only preflight confirms
all six admission/session counts and ordinary TCP7200 are drained. The sealed
operator creates private PostgreSQL/state/config backups, changes only that
service's ExecStart and preserves capacity51/66/51, environment/base unit/resource
limits. Old and Candidate stop exit codes are zero; the Candidate then cold
starts without a shutdown panic. Live PID2984990 runs the pinned binary/source.
Original production PID3855184/release stays unchanged. No Windows installer,
resource rebuild, native feed16 promotion or database restore is performed.

Ordinary public HTTPS returns200/healthy on the exact source. WSS with the
required Origin and normal TLS passes44 checks over Warrior/Wizard/Taoist and
both genders:20 initial appearance relations,8 gender-switch relations,
8 movement broadcasts and8 re-login appearance relations. Complete remote
sprites equal the corresponding owner's ordinary SelfPlayer descriptor and
use transformation-1/CArmour. Both walked positions persist across ordinary
logout/re-login, AOI leave removes the prior actor, and all8 owned client
lifecycles receive LogOutSuccess. Final private/public health confirms all
owned connections, admissions and reconnect leases drain.

The first public harness fails because Node's global WebSocket omits the
required Origin. The second, corrected transport succeeds for five ordinary
accounts then hits the unchanged five-registrations-per-peer/hour rule. Both
failed reports and cleanup evidence are retained. The passing run reuses those
five owned accounts and normally creates a female Taoist character in the
Taoist account, keeping maximum concurrent actors at five. It makes no further
registration or privileged state preparation, uses no operator rate-limit
reset, and restarts no service to get a pass. This is functional/public-protocol
acceptance rather than new load-capacity or native GPU/human acceptance.

No Windows reinstall is necessary. Real native two-client screenshots/human
appearance acceptance and full P1–P8 parity remain separate, unfinished gates.
[Actual operator results, all public runs and redacted traffic](generated/player-qa/shared-player-appearance-20261007/rollout-01/README.md)
are archived separately from the historical preparation snapshot.

The verified binary is 80,703,496 bytes, SHA256
`7ffbea9de2998ff5265b9bb212caf9c56c9d8c61de5dbc3fa3235442217818d9`.
CI, raw failures and operator preparation are retained in
[the prepared-delivery evidence](generated/player-qa/shared-player-appearance-20261007/prepared-01/README.md).

## Requested father testing funds

The user authorizes **50,000 gold to father**. The exact playtest account and
character index are confirmed in the private operator receipt; the unrelated
production Admin API character named father is not used. Trusted SSH calls
the existing private loopback system-mail endpoint;
the public proxy does not expose it. Delivery200 / deliveredCount1 / mailIds[1]
is recorded once under case `father-gold-20261007-01`. No direct wallet/save
edit, restart or repeated grant occurs. The user must claim the in-game mail
attachment before the wallet increases. Private operator request/receipt:
`/var/backups/mir2-playtest/support/father-gold-20261007-01/`.
