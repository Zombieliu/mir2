# V11 execute / wait audit v1

Offline audit of completed episode-50 public stage captures. Original AOI missing fatal at 1793029007019 and false acceptance verdicts remain unchanged. No source edits, tests, builds, load, private stderr or sample reads.

Each raw public file was streamed exactly once with SHA256 computed in the same byte stream. Later analysis uses strict scalar public caches. See stream-provenance-v1.json, findings-v1.json and selected-strict-public.jsonl.

## Current evidence

| Scope | Stage | Rows | Aggregate elapsed | Maximum |
| --- | --- | ---: | ---: | ---: |
| Global | shared_session.predrain | 14934 | 3092.091 s | 1022 ms |
| Global | shared_session.inner_execute | 3777 | 437.493 s | 1621 ms |
| Fatal observed +/-5 s | shared_session.predrain | 604 | 172.564 s | 1022 ms |
| Fatal observed +/-5 s | shared_session.inner_execute | 107 | 13.047 s | 455 ms |
| Fatal observed +/-5 s | shared_session.reconcile | 198 | 53.069 s | 1235 ms |
| Global | AccountStore store_wait | 10231 | 929.516577 s | 364 ms |
| Fatal observed +/-5 s | AccountStore store_wait | 548 | 51.114077 s | 315 ms |

The fatal-window StoreWait is 269 experience_admission records / 27.209603 s, 278 guild_lookup / 23.848113 s, and 1 transaction / 56.361 ms. StoreWait measures AccountStore mutex acquisition. Frozen/write-state mutex has a separate WritableWait stage and no >=50 ms completed rows. Global store_held has one 157.946 ms transaction row with nested 137.486 ms marker_begin_sync; none are in the fatal window. DriverHeld has seven rows / 581.165 ms globally, none in the fatal window.

A unique public time/duration signature associates public connection 46 serial_gate.zone_outbound_wait=1836 ms at observed1793029007730 with coordinator-supplied registration46 batch524 read_gate=1836.063 ms at recorded1793029007730. This is only an association; the counters are independent namespaces. Public connection46 has socket_turn.input=1881 ms, execute=831 ms then flush=1032 ms (54.86% of turn). The flush cannot be attributed uniquely to a child stage from these captures.

All totals are cumulative elapsed wall durations across concurrent and nested spans; they are not CPU time and must not be summed into wall-clock runtime. Gateway durations are floored milliseconds and observedAtMs is launcher receive time. Store totalUs aggregates completed samples; maximum occurrence endpoint timestamps are reconstructed separately. The captures are left-censored at 50 ms. Missing rows do not prove absence of a holder, an unfinished span, an uninstrumented lock, or an excluded public stage.

## Bounded source target

refresh_personal_experience_context first refreshes shared profiles and then enters AccountStore admission, holds the store while waiting for Zone, and rebases all registered recipients. The source therefore supports investigating redundant authority acquisition and repeated full-recipient refresh within the same private command. Reuse a consistent immutable parsed authority view only while retaining frozen checks, current trusted owner rates, exact publication linearization, and AccountStore-before-Zone lock order. Input write-gate residency across flush is a separate evidenced target; preserve bootstrap/ACK/FIFO, save and route semantics.

The data does not identify a specific long Zone holder or prove CPU saturation. pending_lock_wait is only one Zone-lock site (5 fatal-window rows /338 ms/max106 ms). No extra timing round, source modification or acceptance change was performed by this audit.
