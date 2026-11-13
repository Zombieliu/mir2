# Modern ordinary-protocol comparison

This layer is **game-core-local**: one canonical Gateway, File authority,
in-process session/routing cache, literal-loopback plain WebSocket. It does not
qualify the PostgreSQL/Redis production stack, Internet/TLS, rendered clients,
physical Web/Windows/Android operation, or cloud hardware. No production handlers
or game rules are changed. The first adapter supports `dense-movement-chat`;
`dispersed-melee` fails preflight rather than becoming idle traffic.

Use the common `mir2.comparable.plan.v1` containing the explicit per-actor
60000-ms assessment, generator source SHA, equipment selection, and the actual
raw navigation file SHA. `navigation.v1.json` is the original library's
`row-major-lsb0-bitset`: exactly 80000 decoded bytes/404451 eligible cells with
the recorded door, safe-zone, movement-radius3 and NPC-radius3 exclusions.
The active original Map440 SHA is
`d34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608`.
The repository gzip is a different original `Unused` variant. Only the new
isolated raw `source-client/Map/DogYoHyun.map` copy overrides it.

`prepare.mjs` creates inputs and private credentials; it starts no process and
opens no socket. Use a fresh C-only output directory whose parent already exists:

```powershell
node mir2-web3/scripts/capacity/comparable-reference/modern/prepare.mjs --plan C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/plan-functional2.v3.json --navigation C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/navigation.v1.json --original-map C:/Mir2-Archive/archive-20261024-171610/project-history/mir2-three-class-crystal-20260910/server/Maps/DogYoHyun.map --output C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/modern-audit/fixture-inputs-v3 --endpoint ws://127.0.0.1:19010/ws --source-revision 31e9b6a74661c31d6515dd7322b466285f975d71
```

That particular output was already prepared during offline validation; do not
run the create-new command again against it. Its public `inputs-receipt.json`
contains the exact `seedArguments` array. After freezing a freshly compiled
`seed_comparable_accounts.exe`, pass that array unchanged:

```powershell
$comparisonInputs = Get-Content -Raw -LiteralPath C:/Users/Administrator/AppData/Local/Temp/mir2-comparable-capacity-20261113/modern-audit/fixture-inputs-v3/inputs-receipt.json | ConvertFrom-Json
$comparisonSeedArguments = @($comparisonInputs.seedArguments)
& $comparisonSeedExe @comparisonSeedArguments
```

The executable path is the coordinator's actual frozen seed artifact. The
seed's six options are `--manifest`, `--plan`, `--navigation`, `--account-store`,
`--client-root`, `--offline-receipt`. It requires a new private `accounts.json`
outside the repository and an absent live marker. The offline receipt must
identify either a never-started owned Gateway or a stopped owned PID with actual
exit0; a still-existing PID is rejected. It cannot overwrite an existing store.
Inputs are separately pinned to actual plan/nav/map bytes. Never copy a cloud
pool or ledger into this fixture.

Private input schema:

```json
{"schema":"mir2.comparable.modern-actors.v1","qualificationLayer":"game-core-local","planSha256":"<actual file SHA256>","navigationSha256":"<actual file SHA256>","mapSha256":"d34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608","actors":[{"ordinal":0,"accountId":"<new synthetic account>","password":"<private random password>","name":"<new synthetic name>"}]}
```

The CLI uses ordinary in-memory NewAccount/password hashing, Login,
NewCharacter and StartGame. It prepares L40 male Warriors with full674HP/151MP,
zero experience/gold, capacity46, gear221/317/482/482 at full durability and
nine20-count stacks of potion662. The public server-only checkpoint restore runs
the complete original validator and UID-history floor checks; ordinary LogOut
uses the original save CAS. The validated cohort is published through the
original File authority, then fresh ordinary File Login/StartGame/LogOut is
checked for every actor. Failure never emits a successful ready manifest.

Bag coordinates are intentionally distinct from the source inventory array:
modern `Bag1.slot0..8` becomes native `UserInformation.Inventory[6..14]`
(`runtime/packets.rs::user_inventory_slots`). Belt is empty. The first empty
modern bag cell is9, corresponding to original inventory array15. Modern
`RemoveItem(grid=inventory,to=9)` and original `RemoveItem(to=15)` therefore use
the same physical bag cell. Re-equipping returns the exact UID to Weapon.

The seed produces private `actors.ready.json` (`modern-cohort.v1`, including
the actual character indices) and `seed-receipt.json`. Root-owned lifecycle
control must stop the Gateway before later offline File inspection. Seeding is
trusted fixture preparation, not progression evidence or an auth-rate benchmark.

`private/environment.json` is a child-process environment projection, not a shell
script. Scrub inherited `MIR2_*`/`CRYSTAL_*` variables before applying it. Null
means unset. Supply a different explicit loopback `MIR2_GATEWAY_TCP_ADDR` and a
pinned C-only `MIR2_CRYSTAL_MAP_PACK`. Important entries are File backend,
commercial identity rules, `crystal_full`, unsafe-player-commands0, Redis/channel
PG unset, exact Web listener/Origin and a new private recovery directory/key.
The controller owns the source-pinned binary, process identity and the common
4CPU/8GiB Windows Job (including all declared game-core descendants).

Graceful server shutdown belongs to the owned process controller:
`MIR2_GATEWAY_STDIN_SHUTDOWN=1`; write `shutdown\n` to that exact process's
stdin after all ordinary client exits, then wait for actual exit0. The adapter
does not start, stop, restart or install any service. Full-stack comparison later
must count PostgreSQL/Redis inside the same total resource budget.

The controller supplies `mir2.comparable.modern-runtime.v1` with
`qualificationLayer=game-core-local`, `backend=file`, `sessionCache=in-memory`,
the exact `sourceRevision`, `binarySha256`, `endpoint`, `accountStore`,
`planSha256` and its independent process/quota evidence. A File-ready cohort is
bound to that same account-store path and plan. Run the new adapter with explicit
absolute inputs and an unused report prefix:

```text
node driver.mjs --plan <plan.json> --cohort <private/actors.ready.json> --runtime-receipt <actual-runtime.json> --ledger <private/login-ledger.json> --output <fresh-C-report-prefix> --ws-module <pinned-next/dist/compiled/ws/index.js>
```

The report is `<prefix>.report.json`, raw evidence `<prefix>.events.jsonl`.
Predeclare both in the controller. Top-level `processId` is the actual Node PID.
The adapter has no public HTTP auth, raw passkey, QA, Stage5, MoveTo or demo
fallback. It does not retry auth/transport/game failures. Every actual ordinary
WS Login reservation is atomically persisted before connecting. The local ledger
retains all historical attempts and enforces30/901s, conservatively preserving
the server's normal30/900s peer bucket. The coordinator must inherit earlier
attempts unchanged when binding the next plan, keep the same endpoint/peer, and
merge new attempts back afterward. No stage may reset counters/ledger or change
ports to gain admissions. The client waits for a whole equipment phase's login
headroom before the first mutation; it does not wait for another window after an
equipment ACK while pretending to meet the15s state deadline.

Gear runs outside measured actions: selected actors, two rounds each. Every
Remove/Equip requires the exact successful UID/grid/destination ACK<=15s, then
normal Logout<=5s, independent valid WS Close<=5s and a fresh ordinary
Login/StartGame/complete owned snapshot, all within ACK+15s. Inner deadlines are
capped by the outer deadline. Connection numbers and peer shared IDs are rebound;
old-socket state cannot supply fresh proof. Every original and fresh complete
inventory/equipment/belt/gold/capacity roster is retained. After the steady period,
all actors normally exit; final fresh saved-state samples are ordinals0 and
population-1. Stopped-server all-actor disk inspection is a separate required
coordinator proof; the driver never claims that final two fresh logins establish
it for everyone.

All measured actions have immutable plan `dueMs`/`deadlineMs` offsets. Only one
unique action per actor may be unresolved. Timeout stays failed; a late receipt
can retire that pending action but cannot fill completion. NotSent/rejected are
separate denominator outcomes and do not count as timeout. All complete actor
minutes independently require completion>=.95, timeout<.01 and the common
move/chat/sent-schedule P95 limits. All actor-minute results are ANDed, with no
five-minute averaging or later rewriting a failed window. Identity/capture/
causal ambiguity is a separate hard failure. Observation drains only the last
already-sent absolute deadlines (at most2500ms); no extra actions or budgets are
added. `observationEndedAtMs` is an actual clock sample; the planned UTC end is
labelled separately.

Raw `received` records carry `actorOrdinal` (observer), `connection`,
`receiveSeq`, `atMs`, `atUtc`, packet and payload, including actual chat text.
`actionSent` carries exact target, nonceHash, expected peer/shared IDs and fresh
sequence floors with connection. Terminal receipts join that raw namespace.
Catalogs are hashed; repeated snapshots are compact, while gear and sampled-save
proofs include full rosters. Summary booleans are conveniences for independent
recomputation, not substitutes for raw events or controller resource evidence.

Adapter v2 keeps only the latest complete snapshot and a separate live transform
view sharing that current roster. Ordinary snapshot history is capped at16
immutable entity identity/position/hash projections; the raw file retains every
receive sequence/time. Authenticated bootstrap emits `ownedSnapshotEvidence`
with full carried state and its exact `sourceReceipt`. Equipment originals carry
that receipt and the admission snapshot ref. A pre-login public shell is never
an authenticated initial-state anchor. Full gear/save rosters remain strong
evidence rather than being replaced by memory-only hashes.

Adapter v3 requires the corrected `workload.v2.json` definition and explicit
`scheduling.version=2`. Every actor's chat uses its own walk phase plus the
1000ms midpoint offset, preserving all counts, deadlines and the one-pending
gate. The old actor99 walk/chat 10ms separation is rejected. Frozen v1/v2
artifacts and their actual results remain historical evidence. The separate
`combat.mjs` work in progress is not part of this dense adapter release.

`steadyApplicationTraffic` samples current connections before the first measured
input and at epoch+duration, before observing the already-issued deadline tail.
Each actor exposes start/end total counters and their actual sampling times and
deviations. Exact steady `sentBytes`/`receivedBytes` additionally count each frame
only when epoch<=its recorded time<epoch+duration, so sample jitter and the
tail cannot mix bootstrap, gear, relogin or tail bytes into steady throughput.
Raw sent/received records expose `wireBytes` for independent recomputation. The
scope is JSON WS payload bytes, without WS framing/control, TCP/IP or deflate;
the client explicitly disables per-message deflate. Existing campaign totals
remain separate. These counters establish neither NIC wire bytes nor peak link
rate.

Offline checks: `node --test driver.test.mjs`; Rust bin tests are compiled/run
only in the coordinator's clean source snapshot. The SDK mock never binds a
socket. Optional `MIR2_COMPARABLE_SYNTHETIC_PLAN` plus
`MIR2_COMPARABLE_SYNTHETIC_OUTPUT` exports an explicitly synthetic, credential-free
schema example. Such a report is never live performance evidence.

Adapter v6 fixes injector fairness and receipt lookup only. The shared dense/melee
scheduler yields with `setImmediate` after every planned offer, including an
already-overdue backlog. The original absolute due/deadline clock still owns
sent scheduling and receipt verdicts; yield delay is charged normally. It adds
no gameplay retries, offers, auth attempts, deadline extensions or rate changes.

Receipt readers seek by monotonic `receiveSeq` and inspect only newly retained
events. Pending/witness and combat-context caches live in external WeakMaps, so
`cleanGate`/JSON cannot serialize client cycles or cursor state. They cache all
matching retained references in original order, not just the first: if match A
is trimmed after match B arrives, B becomes the same first match that the old
`events.find` would select. Trimming removes cached references too. Independent
witnesses can use the same receive without consuming it. Owner rejection remains
a fallback only when no exact owner receipt exists in the current batch.
The 6000-event history/capture, connection/sequence/namespace/nonce predicates,
late failure fence and every scoring/auth rule remain unchanged.

The old 100-person failed run remains failed. V6's bounded offline regression
uses100 histories of6000 events plus overdue offers and actual timer/file-I/O
callbacks, without replaying the old billions-of-scans load. The counterexample
fails before the yield and passes afterwards; original SDK/source-closure tests
and trim/ambiguity/late-failure cases also pass. These are tooling tests, not a
server build, deployment, game test or capacity qualification. Run all related
offline checks with `node --test driver.test.mjs combat.test.mjs melee-source.test.mjs`.
