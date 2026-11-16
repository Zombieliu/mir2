# Real 43-save offline profile audit

All 43 original rows loaded through the real canonical SimulationSession three
times: original→`platinum_176`, original→`platinum_fiveclass`, and A's actual durable
checkpoint→independent B. Both use `newcomer-v2`. The 129 StartGame operations all
succeeded with complete original account/shared context; no row failed.

The directly loaded A and B results are identical: complete durable checkpoint,
complete WorldSnapshot, and the in-memory stored save all have **zero** exact or
embedded-JSON semantic differences. Raw roster metadata and raw→typed saves also
have zero differences.

The strict overall result remains **false**. No fields were excluded:

| Comparison | Exact differences | Embedded JSON semantic differences |
| --- | ---: | ---: |
| Original→A | 132 | 136 |
| Original→B | 132 | 136 |
| Direct A versus direct B | 0 | 0 |
| A→B save | 129 | 129 |
| Direct B versus A→B snapshot | 0 | 0 |
| Runtime versus durable checkpoint, each profile | 19 | 19 |

The original→A and original→B changes match exactly for every ordinal. They are
confined to `/revision`, `/default_npc_events` and, for ordinal 5 only, four
`/skill_states_json` entries. The skill semantic differences are the two timing
fields `cast_time_ms` and `cooldown_ends_at` in slots 0, 2, 3 and 4. Both profiles
apply the same changes. No other save field differs, including currency, complete
item strings/UID carriers, equipment, quests, character metadata, map/position,
vitals and experience.

The A→B changes are exactly three paths for every ordinal:

- `/default_npc_events/committedSequence`
- `/default_npc_events/nextSequence`
- `/revision`

These are retained as differences. The implementation explains the common
normalization: ordinary StartGame enqueues Login and checks the daily event in
`apps/simulation/src/runtime/packets.rs:9756`; the source event queue has the
fixed fields in `default_npc_events.rs:137`, enqueues at `:481`, checks the daily
date at `:415` and commits its sequence at `:838`. Save revision advancement is
in `save.rs:1735`. Ordinary new-session skill timing restoration is in
`save.rs:3363`; durable skill-clock encoding is in `save.rs:175`. Runtime versus
durable checkpoint differences concern `_runtimeCooldownClock`, 19 entries
across six ordinals, and are separately reported. None of this reasoning turns
the strict exact-preservation result into a pass.

Input was 1,880,632 bytes. Its SHA256 before and after execution remained:

`10b2afa37836062c702639d643f861668c5ddd4100cd8788974cda22cefd1be0`

The prepared executable's SHA remained unchanged. Public report path inspection
and a Node-only identity/credential/long-UID scan found zero leaks or schema
violations; all 1,922 reported paths were retained, with zero omitted paths.
Dynamic private object keys remain ordinal tokens in the original full report;
the summary resolves only source-verified fixed schema field names. No identity,
password, storage password or UID value is present in the public evidence.

This is an offline simulation readback, not browser, network authentication,
File/PostgreSQL persistence or ordinary gameplay acceptance. No external service,
network operation, File/PG account writer, explicit tick or command beyond
StartGame was used. The source input and production repository were not modified.

Evidence in this directory:

- `clone43-execution-receipt-r1.json`: execution, source/binary and input preservation.
- `clone43-report-r1.json`: complete per-ordinal exact/semantic diff paths; strict false.
- `clone43-summary-r1.json`: common versus direct versus chained results, paths and counts.
- `clone43-privacy-r1.json`: public path and secret scan results.
- `clone43-input-preservation-r1.json`: input SHA before/after and finite wrapper outcome.
- `build-test-final.log`: five focused tests passed against the actual simulation/protocol.
