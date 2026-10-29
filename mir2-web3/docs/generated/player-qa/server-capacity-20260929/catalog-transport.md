# Catalog transport and public-network investigation

Status: candidate under validation; not deployed, no 50/100-player acceptance.
The approximately twelve-hour requested window ended at 2026-09-29 07:20 UTC.
The stable mixed-play goal was not achieved in that window. The earlier
00:46–04:22 UTC execution/monitoring gap remains uncredited.

## Observed public-network delay

The movement-only run `1790663876703-ab6f6d9c` admitted fourteen ordinary actors
and stopped at 06:42:29.989 UTC. Its first ten-actor stage retained failed
activity/collision verdicts. The diagnostic profile never grants playability
acceptance. Cleanup completed without recorded errors.

The `network-9ac-live3` trace covers 06:39:46.898–06:44:43.898 UTC at three-second
intervals. Matching newly appearing anonymous TCP tuples to admission times
suggests the following correlation; this is not a server-provided actor/socket
identity or a packet-level synchronized trace.

| Actor suffix | StartGame ACK to authoritative snapshot | Post-ACK bytes | Public TCP observation |
| --- | ---: | ---: | --- |
| 509 | 178 ms | 685,805 | no sampled send backlog/retransmission |
| 510 | 5,036 ms | 699,373 | send queue 550,290 bytes; congestion window 114→16 |
| 511 | 5,569 ms | 699,564 | send queue 543,594 bytes; congestion window 115→11 |
| 512 | 7,784 ms | 691,994 | send queue 563,208 bytes; congestion window 107→4 |

The Gateway/Caddy loopback receive/send queues were zero at all one hundred
samples. RTT was approximately 55–70 ms. Public TCP retransmissions and
congestion-window collapse accompany slow bootstraps, but the trace cannot
identify the losing network hop. It does not establish a fixed bandwidth cap.

A separate bootstrap contains 509 static item/recipe/shop/quest envelopes
totaling 603,847 bytes, 91.4% of its 660,574-byte post-ACK batch. Its largest
single snapshot is 51,019 bytes. Native receive/observation CPU work does not
account for the multi-second receipt gaps.

## AOI failure interpretation

All 272 recorded AOI denominator/count samples can be reproduced from the
retained owner transforms and lifecycle events without a mismatch. This does
not make the cross-connection positions a synchronized server state.

At 06:42:29.989 the observer 505 still used actor 509's old owner-confirmed
position `(259,589)`, within sixteen cells. Another observer had already received
509 at `(261,589)` at 29.257, outside 505's range. Actor 509 received its own
confirmation only at 30.490. The reverse visibility of actor 100 arrived at
509 at 30.491; the projection then inserted it normally. Its missing timer
also began while the position used for the expectation was stale.

Consequently the hard stop is retained, but neither pair proves a persistent
server AOI omission. Actor 509's TCP connection accumulated 25 retransmissions
across the corresponding three-second sample interval. Distinguishing network
stall from sender scheduling would require bounded server enqueue/send timing.
No acceptance threshold or failed denominator was relaxed.

## Candidate behavior

The explicit `serverCatalogGzipV1` capability enables compression of contiguous
`NewItemInfo`, `NewRecipeInfo`, `GameShopInfo` and `NewQuestInfo` envelopes only
inside a successful game/map bootstrap. Legacy clients keep the original Text
envelopes. Authentication, private state, resume credentials, game-shop receipts,
world snapshots and live movement remain on their existing paths.

The shared optional protocol transport uses a sixteen-byte `M2CATGZ1` header
and one gzip member. The decoded body retains exact UTF-8 Text envelopes with
length prefixes: at most 64 entries, 128 KiB decoded and 132 KiB on the wire.
No catalog fields are dropped. Both decoders reject malformed lengths, CRC,
UTF-8, extra members and trailing bytes; the native decoder validates all real
catalog types before applying the first envelope. Existing resume quarantine
and written purchase cleanup remain in force.

The Node capacity client advertises the same capability, applies messages in
order, and measures physical WebSocket payload bytes once, including the new
header/gzip data but excluding WebSocket/TLS framing. Invalid data latches a
failure and closes the connection; subsequent buffered frames are ignored.

## Validation so far

- Shared codec: 5 focused tests, including Node→Rust fixture decoding.
- Node transport: 13 tests, including Rust→Node fixture decoding, actual socket
  negotiation by `CapacityClient`, exact received-byte accounting, and atomic
  rejection of malformed batches. Three integration regressions first failed
  before the capability-state/error-latching fixes.
- Node multiplayer/resume adjacent checks: 29 passing tests. Complete final
  transport/multiplayer/resume/capacity suite: 115/115 in 257.64 seconds,
  including 73 capacity tests after the chase-strategy change. The earlier
  capacity-only 65/65 run remains separate evidence.
- Gateway: 7 focused tests, including certificate-verified TLS and ordinary
  login, character creation, StartGame, logout and native resume/rotation.
  Both modes deliver the same 482 catalog envelopes: 563,202 legacy bytes versus
  71,376 compressed bytes, approximately 87.3% smaller. This is a local transport
  result, not a WAN capacity result or fresh saved-state comparison.
- Gateway native-resume adjacent checks: 22/22.
- Gateway purchase/receipt/persistence adjacent checks: 7/7.
- Independent static review finds no remaining Rust production blocker.
- Windows native transport checks: 5/5. The first run contained one malformed
  test-only legacy UserLocation fixture; correcting its flat x/y layout produced
  a clean second pass without production changes. Real native WebSocket resume:
  1/1, including binary catalog receipt and pre-resume quarantine.
- Clean paired packages, isolated deployment and ordinary public mixed-load
  revalidation remain pending.

The test character exited normally through the native Yes dialog after these
checks; its client log records successful application exit and the monitor
observes zero WebSockets, active sessions and reconnect leases. Windows routing
to the test host currently selects the `Meta` virtual interface. This is a
network-path factor requiring separation, not proof that a particular proxy
caused the losses; no OS/VPN setting was changed.

## Ordinary mixed-workload chasing

The native-cadence test driver now prefers an adjacent monster visible to both
participants and retains its selected reachable target instead of changing to
the nearest monster each step. A pursuit without an attack can temporarily
defer that target for thirty seconds only after at least two successful,
nonzero authoritative movements and either four non-improving movements or
six seconds without improved distance. Plans, corrections and elapsed time
alone cannot trigger this decision. At most sixteen such deferrals are retained.

This strategy never reduces monster HP, supplies medicine, teleports, changes
attack pacing, or discounts failed actions. The separate ambiguous-attack
permanent quarantine, sixty-second activity denominators and positive-hit
requirements remain unchanged. Resume rechecks the target and rebases distance
and receipt sequence so disconnected time cannot count as chasing evidence.

Two tests reproduce the previous target-selection defects before the change;
twenty-one focused checks pass after it, including real socket combat,
late-hit quarantine and sparse authoritative attack-speed cases. The complete
final suite passes 115/115; an independent strategy review finds no remaining
blocker or relaxation of the original gates. This remains an unproven live hunting
strategy, not evidence of a sustainable hour of five-fighter gameplay.

Evidence files remain outside the repository under
`C:/mir2-ui-repair-20260921/playtest-load-20260929` and
`C:/mir2-build/playtest-chat-20260929`. No account passwords, tickets or raw
private snapshots are included in this document.
