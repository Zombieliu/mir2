# NPC population: Source selection and admitted Player census

This bounded foundation is based on pushed source
`ecd81f6a1528e0a1a85bfd920356f607bc37a2a9`. It is implemented and locally tested.
It is not yet called by the ordinary NPC interpreter. Its new Linux/publication
and native/human gates remain open; public Gatewayc704/R23/feed18 is unchanged.
The expired12-hour heartbeat stays paused and the broader Goal is unfinished.

## Behavior and original Source

The pure selection module preserves successful MapInfo order and original
ordinal/index/FileName. It does not sort by index, normalize filenames, equate
map titles or manufacture successful loads. Crystal appends a map only after
`Map.Load()` succeeds (MapInfo.cs:206–210). Its instance selection is among
successful matching entries, with negative/zero/one selecting the first
(Envir.cs:4613–4619). Duplicate indices do not reorder those instances.

CHECKHUM parsing preserves the original three required arguments, optional
fourth instance, default1 and ignored extra tokens (NPCSegment.cs:255–259).
Numeric parse failure and absent Source map return false in original order.
Known count, absent Source map and unavailable authority remain different.
The six original comparisons use a checked nonnegative Int32 population;
invalid operators raise an error only after a real population read. An
unavailable read cannot become false and enter ELSEACT.

FileName matching is bounded to the imported ASCII domain: all464 metadata
entries are ASCII, including one empty name. Non-ASCII data/query is unavailable
instead of claiming complete CurrentCultureIgnoreCase parity. Imported metadata
and constructor names are not proof of successful loading or map ownership.

The Manager capture checks current complete OnlineOwner, actual Player identity,
ZoneKey and `Some(life)`, including legitimate life0. Every counted Player must
match its Book and reverse session route; equal route/Player cardinalities prove
the reverse direction as well. Orphan Book owners without routes reject capture.
Dead and out-of-AOI Player Nodes remain counted until actual Leave. Pets/Hero
and rankings are not the Players collection. Absent or inconsistent Zones
return unavailable, not zero. A present empty Zone still needs independent
successful Source-load and ownership proof before it may supply a known count.

The read set has private fields and no serde/save/checkpoint installation path.
Cold Manager restore creates a new identity Book; old reads do not authorize
restored Players. Actor-binding matching is explicitly not freshness. A retained
A→B→A transfer can match the old actor binding while the old count remains stale;
a new invocation must capture again and bind the Gateway presence epoch.

## Actual local evidence

| Gate | Result | Limit |
| --- | --- | --- |
| Pure Source selection/comparison plus real Manager census |28 passed|20 pure cases and8 isolated ordinary Manager lifecycle cases; synthetic records are not production Source load proof.|
| All adjacent Manager tests |11 passed|Includes the same8 census cases plus3 existing parallel tick/Leave regressions.|
| Online identity epoch exhaustion |1 passed|Actual existing Book refuses epoch exhaustion rather than recycling authority.|
| Distinct final tests |32 passed,0 failed,0 ignored|40 executions include8 duplicates; not40 independent checks or a full suite.|
| New exact Linux/Gateway/Source/native/human |Open|The three new selectors are in the existing release workflow; local results do not attest a Linux run.|

Before/after16 selected input hashes match final bytes. This is not whole Cargo,
toolchain or workspace closure. Initial22pass/1fail used the wrong AOI helper;
the corrected fixture inspects ordinary ObjectPlayer outbounds. An initial
adjacent filter selected zero tests and is excluded, then corrected to
`zone::manager::`. Archive preparation metadata/path failures are retained.
No actual account/save or existing test assertion was changed to obtain these
passes. A read-only reviewer checked the bidirectional census and retained-map
semantics but did not run tests or claim additional acceptance.

The [61-entry archive](generated/player-qa/npc-population-20261010/candidate-01/original-evidence.zip)
and [byte-verified receipt](generated/player-qa/npc-population-20261010/candidate-01/EVIDENCE.json)
retain original Source files, all logs including failures/zero selection, runners
and final source. Archive306663 bytes, SHA256
`9f53410692bb0aa24a73bc73c0406d50295704b46876229a4d612932753456f4`.

## Next integration and remaining limits

Ordinary `crystal_npc_check_hum` still has its prior fixed-one/three-parameter
adapter. This foundation must not be described as its production fix.

The next producer must prove complete successful Source loading and map each
original instance to a complete ZoneKey. Lazy Zones, metadata, collision
fallback, map titles and a guessed default instance are insufficient. The
current per-map read also is not a proof of complete global Book structure:
an orphan route to an absent Zone stays unavailable for that key.

Gateway capture must bind full owner lease including owner_id, real source
sequence, admitted Node/life, presence epoch, no teardown fence and actual
server-resolved NPC/script/page/body. Existing economy context omits owner_id
and cannot stand in for that binding. All return paths must clear ephemeral
context. Login's pre-Join and MapEnter phases need explicit sequencing, and
MOVE followed by another read must not use old-map data.

Verified standby replay needs a server-produced population transcript bound
to that same Source command/digest chain. It cannot mint today's live authority
from cold bytes or commit externally. Capacity reservations and all seven
shared-world commands still need the same Source-CAS durable outbox and real
application/recovery; two independent `< limit` reads do not reserve a slot.
P1–P7/Mentor natural/native/human and full100% Candidate acceptance remain open.

Root is the sole common code/Git/rollout writer. The bounded pure-module worker
and read-only reviewers have returned ownership.
