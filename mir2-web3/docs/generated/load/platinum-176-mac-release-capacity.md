# Platinum 1.76 macOS release-load baseline

Date: 2026-07-29
Host class: local Apple Silicon development Mac
Gateway: `target/release/mir2-gateway`, `platinum_176`, file and PostgreSQL account stores
Primary reports:

- `platinum-176-mac-release-capacity-12-at-6-after-guild-lock.json`
- `platinum-176-mac-release-12-active-after-guild-lock.json`
- `platinum-176-mac-release-50-concurrent-after-guild-lock.json`
- `platinum-176-mac-release-50-concurrent-after-startgame-lock.json`
- `platinum-176-mac-release-50-concurrent-after-tick-filter.json`
- `platinum-176-mac-release-64-concurrent-after-tick-filter.json`
- `platinum-176-mac-postgres-100-concurrent.json`
- `platinum-176-mac-postgres-100-concurrent-after-login-tick-guard.json`
- `platinum-176-mac-postgres-redis-100-reuse.json`
- `platinum-176-soak-runner-smoke-10p-30s.json`

## Result

This is a **passed 100-player Mac/PostgreSQL development baseline**, not a
production-capacity or long-duration certificate.

The controlled admission test opened 12 clients, admitted six, and explicitly
capacity-rejected six. Unexpected errors were zero, KeepAlive was 30/30 with
P95 789 ms, and the post-fix Tick probe averaged 21 ms with a 173 ms maximum.

The 12-active-client test then admitted 12/12 with zero errors. KeepAlive was
60/60 with P95 870 ms. Tick averaged 27 ms with a 176 ms maximum, below the
300 ms runtime interval. Peak RSS was 491,028,480 bytes.

The first Ready-barrier 50-client diagnostic held all admitted connections
open until the whole batch reached a terminal state. It established 50
concurrent WebSockets and 50 active-session permits, but only 47 clients
completed StartGame inside 240 seconds. At that load:

- Tick averaged 318 ms and reached 4,942 ms maximum;
- peak RSS was 2,154,823,680 bytes;
- P95 CPU was 501.4%;
- the 50-player gate failed.

The raw historical 50-player report lists 50 errors because the first barrier
implementation made the 47 successful clients wait for the three failed
clients until the barrier itself timed out. The harness has since been fixed
to release the barrier as soon as every client is either ready, rejected, or
failed. The authoritative failure is three StartGame timeouts plus the missed
50-ready concurrency target.

After moving full StartGame world construction outside the guild transaction
lock, the same 50 accounts reached 50/50 Ready with zero StartGame errors, but
the run still failed real-time assertions: KeepAlive was 93/100 with P95
4,645ms, and Tick averaged 1,592ms. The phase probe attributed 1,469ms average
to pending cross-session work.

The final fix prevents each session's private autonomous monster Tick from
updating and broadcasting the shared entity layer. Only delayed player-owned
combat packets may cross that boundary; the Zone Runtime remains authoritative
for monsters. The identical 50-player run then passed every assertion:

- 50/50 concurrent Ready, zero errors;
- KeepAlive 100/100, P95 2,544ms;
- Tick average 1ms, maximum 228ms;
- pending phase average 0ms, maximum 3ms;
- peak RSS 1,471,021,056 bytes.

The next 64-player run also passed every assertion:

- 64/64 concurrent Ready, zero errors;
- KeepAlive 128/128, P95 1,957ms;
- cumulative-process Tick average 1ms, maximum 228ms.

The 64-player RSS peak of 2,256,502,784 bytes is from the same Gateway process
after the 50-player run, so it includes allocator-retained memory and is not an
isolated 64-player footprint measurement.

The first PostgreSQL 100-client fresh-account run failed honestly at 48/100
Ready. All 100 logins succeeded, but login/select-screen sockets were already
ticking complete compatibility worlds before `StartGame`, flooding the clients
with 135,749 messages while they waited for character creation. The 48 admitted
clients still returned 96/96 KeepAlive acknowledgements with P95 1,266ms.

The Gateway now starts its per-session runtime Tick only after `StartGame`
establishes an active identity. The load harness also reports non-capacity
Gateway errors and recognizes capacity rejection during login and character
creation instead of mislabelling it as a timeout. With all fresh-account
in-flight limits set to the 100-player Gate, the final PostgreSQL run passed:

- 100/100 WebSockets, logins, character creations, StartGame completions and
  simultaneous Ready clients;
- zero capacity rejections, unexpected errors, failed clients or server errors;
- KeepAlive 200/200, P95 2,766ms against the 3,000ms Gate;
- Tick average below 1ms, maximum 108ms; shared command maximum 13ms;
- 100 PostgreSQL saves averaged 46ms and reached 104ms maximum;
- isolated peak RSS 1,380,171,776 bytes; CPU P95 802.5%;
- PostgreSQL independently confirmed 100 persisted accounts and 100 characters.

A second 100-player run reused those PostgreSQL characters while forcing the
Redis session/routing cache. It also passed every assertion:

- 100/100 LoginSuccess, StartGame and simultaneous Ready; zero errors,
  rejections, failed clients or server errors;
- Redis health reported 100 records, 100 route leases, zero stale records and
  `backend=redis`;
- KeepAlive 200/200, P95 2,639ms; Tick average 1ms/maximum 84ms;
- 100 PostgreSQL saves averaged 43ms and reached 128ms maximum;
- isolated peak RSS 1,525,678,080 bytes.

The new duration-driven soak runner was validated with a 10-player,
30-second Postgres+Redis smoke. It ran 15 actions per client, held 10/10
concurrent Ready, returned KeepAlive 150/150, and passed with zero errors.
During the run it wrote a five-second `.partial.json` checkpoint containing
progress, last RSS/CPU sample and bounded error lists; normal completion wrote
the final report and removed the checkpoint. The required two-hour execution
remains an open staging Gate.

## Fixed bottleneck

Every ordinary World Tick was previously classified as a guild transaction,
so all player world ticks held the same guild mutex for their complete
execution—even with no guild and no conquest campaign. The pre-fix six-player
probe averaged 7,668 ms and reached 14,239 ms.

The Gateway now excludes ordinary ticks from that transaction-wide lock.
Only the elected Sabuk scheduler mutation and a real palace-occupancy change
take the guild lock around their shared read/commit. Focused conquest
scheduler and palace-capture tests pass.

At six active clients this reduced average Tick wall time from 7,668 ms to
21 ms (about 365x) and maximum Tick time from 14,239 ms to 173 ms, without
changing the passed capacity/KeepAlive outcome.

StartGame previously held the same lock while rebuilding the player's entire
local world. It now performs that expensive work concurrently and takes the
lock only to reconcile loaded guild/conquest state with the shared copy.

Finally, every personal `SimulationSession` was forwarding its autonomous
monster Tick packets into the shared map and peer queues even though the Zone
Runtime already owns those monsters. At 50 players this produced an O(N²)
packet storm. Filtering the boundary reduced Tick average from 1,592ms to 1ms
and the pending phase from 1,469ms to effectively zero.

## Gate interpretation

What this run proves:

- the release build can authenticate and run 12 active clients;
- the same Mac release build can authenticate and hold 50 and 64 concurrent
  Ready clients inside the current action/KeepAlive SLO;
- the Mac release build can create, persist and run 100 fresh accounts and
  characters concurrently against PostgreSQL inside the same SLO;
- per-account character indices are selected from `LoginSuccess` correctly;
- admission control rejects excess StartGame work predictably;
- admitted clients complete short action loops with zero command errors and
  100% KeepAlive acknowledgement inside the observation window;
- the load report captures Gateway `/metrics` and `/health` before/after
  snapshots and verifies actual concurrent Ready count with a barrier;
- the former global guild-lock serialization and duplicate personal-monster
  broadcast path are removed.

What remains before G20 can pass:

- define and test the final launch target on production-like infrastructure;
- define movement/combat/trade tick and response SLOs;
- rerun 50, 100, and target concurrency on Windows/cloud staging;
- complete the two-hour PostgreSQL + Redis soak on Windows Staging or a cloud
  staging environment.
