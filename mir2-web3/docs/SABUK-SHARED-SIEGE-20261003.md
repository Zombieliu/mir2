# Shared Sabuk siege — 2026-10-03

This delivery implements the selected Crystal **Classic / Request** Sabuk
configuration on one authoritative shared world. It does not claim the other
three conquest modes, whole-game parity, or human gameplay acceptance.

## Ordinary player entry

- Create a Guild through Administrator Edward in Bichon Palace (`0122`, 28,33).
  The native client now handles the ordinary `GuildNameRequest`/`GuildNameReturn`
  naming exchange, including IME, cancellation and stale-dialog boundaries.
  The source level, gold and Wooma Horn requirements still apply.
- The current Guild leader selects **Siege registration** from the same NPC.
  Only one challenger may register. The current owner, a nonleader, an expired
  dialog, or a request during war is rejected without changing funds or state.
- The invited `platinum_176` profile uses Beijing time (UTC+08:00), daily
  **18:00–18:30**. The source helper retains UTC, as Crystal's `Envir.Now` does;
  the profile offset is explicit rather than inferred from client language.
- Enter Sabuk's palace (`0150`) through its public side entrance from map `3`
  at 624,270. Owner-only entrances require current durable Guild membership.
  The castle-management Officer is at 9,13 inside the palace.
- Every ten seconds the server samples actual living occupants of the shared
  palace. A sole eligible challenger takes ownership immediately. An empty
  or contested palace retains its current owner; an unguilded living occupant
  contests it. The previous owner becomes the challenger and can recapture.
  At 18:30 the last owner is retained and the challenger is cleared.
- After war, the owner Guild's current leader can manage taxes, open/close the
  gate, repair walls/gate, rehire archers and withdraw castle tax into the Guild
  bank. Other players can view status, but cannot operate these controls.

## Source and deliberate repairs

Reviewed source: Crystal `Server/MirObjects/ConquestObject.cs`,
`Server/MirObjects/PlayerObject.cs`, NPC actions and Gate/Wall/Archer AI;
`Build/Server/Debug/Server.MirDB` version 117, SHA-256
`829121f4d762ea4427325a767e326a9c04794eb51450ada8a72a0b857705f075`.
Conquest record 1 is Sabuk Wall, map `3`, palace `0150`, Classic(3), Request(0).

The source Administrator advertises a relic and two-day delay but its displayed
target has no executable registration section. The executable ScheduleConquest
action does not charge that fee or impose that delay. This delivery repairs the
ordinary NPC entry using the executable rule, with no invented fee or wait.
The palace Officer and thirteen castle merchant bindings also need repair:
their original NPC records contain Conquest=0 despite their castle scripts.
Bindings are restricted to exact reviewed NPC/script/map combinations.

The source castle region does not encompass its gate and some archers. The
battle region is extended only for those source castle actors; this does not
turn all Mongchon into a war zone. EnemyGuild mode explicitly admits the owner
and registered challenger there. Peace mode remains peaceful. The source PK
exemption uses a war participant or a victim in the active war region.

Sixteen source defenses are shared: one gate, three palace walls and twelve
archers. The source duplicate archer coordinate is retained with distinct IDs.
Gate/wall HP is 5,000, archer HP 9,999; archers retain source damage and cadence.
Living archers regenerate 220 HP per ten seconds; poison delays regeneration.
Dead archers do not revive for free. Repair/re-hire costs and integer partial
repair math follow source, including the possible zero-gold one-HP repair.

Taxes use source 10/15/20/25% choices and f32 price calculation. Owner purchases
are exempt, pearl purchases are excluded, and selling is untaxed. Ordinary
castle purchases persist both bag and belt cells, wallet and castle tax in
one transaction. No bonus siege XP/gold payout is invented. Existing resale
added-stat/durability valuation gaps are outside this change.

## Authority and persistence

Personal Session retains login, inventory and saves. Shared Zone retains actual
positions, life, combat and defenses; shared AccountStore conquest rows retain
ownership, registration, bank/tax, HP, event sequence and clock grants. Neither
a client packet nor a personal Stage5 castle snapshot can import authority.
Ordinary source-script owner reads use the shared record; legacy personal
castle mutation helpers are inert while shared conquest is enabled.

Production TCP and WebSocket listeners share a Zone registry and one independent
clock, including when no players are online. Only primary map/palace Zones
contribute samples. Stable account/character and Guild identities are rechecked;
packet-supplied names and positions do not decide capture.

PostgreSQL locks follow Conquest → Guild → Hero → account/save order. Mirror
castle transactions require exact original account and complete save images,
then execute strict source CAS. Thus a newer ordinary wallet or inventory write
cannot be overwritten even when a legacy Mirror version did not advance.
Failed transactions do not publish capture notices, charge bank funds or keep
local inventory changes. Unknown COMMIT outcomes freeze writes instead of replay.
Restores invalidate process clock grants and reject orphan Guild ownership.

Zone checkpoints authenticate their source collision data and restore castle
actors inert until a committed projection is rehydrated. Delayed damage is
fenced by battle/generation/end time. Final pre-cutoff damage is saved in the
same transaction as settlement; the shutdown clock flushes before relinquishing
its lease. SIGTERM and Ctrl+C both enter normal shutdown. A crash instead relies
on the expiring lease and last committed ten-second sample.

## Resource and validation scope

Candidate map defaults expand from 25 to 32 with `0122`, `0150`–`0155`.
The reviewed seven-map expansion has zero omitted maps or blocking drawable
references. The existing source's invalid/no-draw frames are reported separately;
this is not a claim that every world map has been packaged.

Local focused checks: simulation conquest 95 passed/1 dedicated-DB ignored;
domain integration 29 passed; Gateway clock/routing 13 passed; real TCP+WebSocket
ordinary palace/capture/NPC flow 1 passed. Account-store regressions 22 passed,
shared-Guild regressions 24 passed/2 dedicated-DB ignored. Windows native host
845 passed/5 existing ignored using the signed R15 resource input; shared native
UI 1,217 passed/12 existing ignored after the input-indicator repair, including
19 Guild-dialog cases. First clean-tree host run lacked generated map cache and
failed 158 asset-dependent cases; that environmental failure is retained.

The broader Gateway run passed 875 cases, ignored 18 existing cases and failed
one unrelated QA position fixture. That fixture now uses an available legal
cell and passed its focused rerun; production movement validation was unchanged.
Nine-language production-widget GPU fixtures passed IME/committed name fields
and all paginated castle pages. They caught an Arabic line-height overflow:
caret and composition indicators now fit the source skin's content viewport.
The retained [local receipt](generated/player-qa/sabuk-20261003/local-validation.json)
records log hashes and representative screenshots. These controlled NPC-page
fixtures do not constitute live gameplay or OS-specific IME-window acceptance.

Dedicated PostgreSQL concurrency and the paired R16 release are separate
remaining delivery gates at this checkpoint.
The reusable `sabuk-acceptance.yml` runs the isolated PostgreSQL gate as part of
the Gateway release workflow. Human mouse/gameplay acceptance remains separate.

## Human acceptance route

Use two ordinary accounts with distinct Guilds. Register as the nonowner leader,
join the palace during the configured window, capture, contest it with the other
living character, remove the contest and recapture. Check both clients' notices
and the Officer's owner name. Damage the source gate/wall, then log out normally
and restart the drained server; check HP, ownership, bank and tax persist.
After war, verify an owner leader repair/withdraw succeeds once, a stale or
nonleader operation fails, and both gate states agree with ordinary movement.
Check the management page through all twelve archers in the chosen language.

The user's installed `F:/mir2/Mir2Invite` client remains deferred while another
game holds its files. Do not alter its update journal, overwrite its files, or
stop that game. Fresh package/isolated tests do not count as installed acceptance.
