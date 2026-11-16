This is an explicit ordinary-protocol extension of the local comparison tool.
It measures the File authority, in-process session cache and loopback Gateway
inside the declared game-core budget. It establishes no PG/Redis production,
public-network, full-combat or physical-three-client capacity claim.

Use `melee-driver.mjs` to opt in. `driver.mjs` continues to require the corrected
dense plan and rejects a combat option. Both `prepare.mjs` and the melee driver
require an external absolute `--combat-contract` path whose raw SHA256 is
`8b373b5d03a71c468934d3591c6d06ad65ce6fe2eeb3208acca39ee0ae25a25b`.
The embedded contract must be equal; missing, changed or inferred contracts fail.
The corrected common plan is `plan-melee2.v4.json`, SHA256
`8f4d4cab28b5d27a3bb1a177fdbf9b717dce63e57e0eee115243e8c98d4200bd`.

Preparation is offline and creates a new C-only fixture. It does not start a
Gateway, open a socket or compile anything:

```powershell
node mir2-web3/scripts/capacity/comparable-reference/modern/prepare.mjs --plan C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/plan-melee2.v4.json --navigation C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/navigation.v1.json --original-map C:/Mir2-Archive/archive-20261024-171610/project-history/mir2-three-class-crystal-20260910/server/Maps/DogYoHyun.map --output C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/modern-audit/melee-fixture-inputs-v1 --endpoint ws://127.0.0.1:19010/ws --source-revision 31e9b6a74661c31d6515dd7322b466285f975d71 --combat-contract C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/combat-contract.v1.json
```

That example path was prepared once; use its existing public `inputs-receipt.json`
or a fresh output name. Pass its exact `seedArguments` array to the separately
built, source-pinned new seed executable, while the owned Gateway is stopped.
The extra seed option is `--combat-contract <copied fixture/combat-contract.json>`.
The original complete character validator, File writer fencing, UID floors and
ordinary logout CAS still govern publication. Dense lanes keep two unique
horizontal cells; melee lanes declare the same stationary cell twice and count
one occupancy per actor. All actor start cells must be unique and eligible in
the original navigation bitset. The tool rejects skills, buffs, saved pets,
heroes or intelligent creatures; it does not clear them during a run.

The coordinator must carry every prior attempt in the same endpoint/peer's
local auth ledger into any new stage. Preparation's fresh ledger is not evidence
that the endpoint has unused auth budget. The driver preserves all inherited
attempts and reserves each normal Login before connecting. No public wallet
pool, auth purpose, IP or existing ledger is reset.

Startup uses the same private environment and owned controller as the dense
tool, including `MIR2_GATEWAY_STDIN_SHUTDOWN=1`, explicit distinct loopback
ports, pinned C map pack and the total 4-CPU/8-GiB server-process job. Only the
coordinator starts the source-pinned server and supplies a runtime receipt.
For an already prepared fixture, its controlled injector command is:

```powershell
node <frozen-modern-bundle>/melee-driver.mjs --plan <fixture>/plan.json --cohort <fixture>/private/actors.ready.json --runtime-receipt <owned-runtime-receipt.json> --ledger <fixture>/private/login-ledger.json --output <fresh-run-prefix> --ws-module <pinned-C-ws-module> --combat-contract <fixture>/combat-contract.json
```

Every path in that command is absolute. The controller predeclares the report
and events paths and binds the report's actual `processId`. Shutdown remains a
normal controller `shutdown\n` on owned Gateway stdin, after all client exits;
this adapter neither stops services nor relaxes the resource controller.

The active plan has 50 attacks and two nonce chats per actor per minute.
Attack period is 1200ms, and chat uses that actor's own phase plus the 600ms
midpoint. Tickets are pre-materialized. Deadlines are absolute offsets from the
same measurement epoch. The owner and every declared peer use their current
connection and receive-sequence floors. The ordinary command is
`attackDirection` with direction and `spell:0`; mode `5` is an ordinary
`ChangeAMode` request/ACK. Raw target attacks, QA, debug movement and passkey
account injection are outside the command whitelist.

Modern target outcomes use its local victim ID, attacker-bound `ObjectStruck`
(or ordinary `Struck`), victim-bound `DamageIndicator` and one exact HP-operation
delta. Typed damage payloads retain `object_id`/`damage_type`; ordinary positive
damage magnitude differs from the original server's negative Hit wire sign.
Modern shared PvP publishes its HP operation before the public struck/damage
packets. Positive healing operations stay separate, so a net HP increase can
still contain a provable exact damage operation. Competing damage or inconsistent
HP evidence is a hard failure. Explicit uncontested Miss and attacker-bound
zero block are separately labelled; an animation or an arbitrary HP change is
insufficient.

Reactive potions poll every 25ms, only when that actor has no unresolved attack,
chat or potion, at HP<=70% and >=2000ms since its actual prior potion send.
Ordinary `UseItem` names the real inventory UID. Modern proof requires its
successful UID/grid ACK and an independently received full owned snapshot whose
potion stock falls by exactly one. Full before/after carried evidence is logged
only at those strong observations; a later HP rise is not potion proof. The
original native protocol has ACK-derived immediate stock; its final fresh UI
and stopped File readback close that separate evidence boundary. Neither adapter
restocks, retries a timed-out dose or adds potions to the 52-primary denominator.
A primary due during a pending potion is `notSent`.

Timeouts fence new primary sends until unique late evidence retires the pending
action, and remain failed. Every complete actor-minute separately applies the
common completion, timeout and attack/chat/sent-schedule P95 gates. NotSent and
rejection remain distinct; an isolated NotSent does not introduce a new veto
beyond those common thresholds. Death, identity drift, capture failure and
causal ambiguity are hard failures. Steady application payload counters retain
the exact epoch-to-duration window, excluding bootstrap, gear and deadline tail.

Equipment still uses the same Weapon UID for both remove/equip rounds, with
15s ACK then 15s whole normal logout/close/fresh-login state budgets. All actors
finally Logout and Close under independent 5s budgets. The final save anchor is
frozen synchronously when `LogOutSuccess` is observed, after all preceding
ordinary HP, durability and stock notifications on that connection. Its last
full owned snapshot and exact packet patches are recorded in `logoutOwnedState`.
Selected ordinals `[0,last]` use new sockets and compare their initial ordinary
UserInformation pools plus full fresh owned rosters exactly. No HP tolerance or
send-time stale baseline is substituted. The coordinator independently reads all
actors' File saves after shutdown.

Offline tests use built-in SDK mocks and never bind a socket:

```powershell
node --test scripts/capacity/comparable-reference/modern/driver.test.mjs scripts/capacity/comparable-reference/modern/combat.test.mjs
```

`MIR2_COMPARABLE_MELEE_PLAN` and `MIR2_COMPARABLE_MELEE_OUTPUT` optionally export
the explicitly synthetic full-flow sample. Its 104 primaries, gear, potions and
save rows are schema/reducer evidence, never live capacity results. Seed Rust
tests and builds run only in the coordinator's clean source snapshot.

SDK v6 uses the same fair scheduled-offer loop as dense mode, with a real
`setImmediate` yield even while already overdue. It keeps the fixed attack/chat
schedule, original deadlines, single pending fence and reactive potion policy.
Attack owner/peer animation, target attacker/victim/damage and HP-operation
readers, plus ordinary potion ACK lookup, now incrementally seek receive sequences
instead of repeatedly scanning the old history prefix. These context caches are
external WeakMaps over the unchanged retained histories; newly arriving competing
damage/attacker evidence is still checked even after an earlier hit was cached.
Front-trimmed owner/ACK evidence is not kept alive by the cache, and first retained
duplicates follow the original predicate order. No HP, stock, save, timeout,
causality or safety gate was weakened. The full mock and source-closure checks
cover the shared entry; no v6 PvP live result is implied by the offline tests.
