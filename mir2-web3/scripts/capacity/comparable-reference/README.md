# Crystal / modern common workload

This harness keeps local game-core comparison separate from a production CCU
claim. A planned stage always has `capacityClaimAllowed: false`; measurements,
ordinary lifecycle checks and an independently verified resource boundary are
required before making even a bounded conclusion.

`workload.v2.json` defines the current common actions, thresholds, immutable deadlines,
and aggregate 4 logical CPU / 8 GiB committed-memory server budget. `workload.py`
materializes identical tickets from navigation exported using the original
Crystal library. Both the raw JSON file hashes and canonical hashes are pinned.
The active original DogYoHyun map differs from the same-named historical packed
map; use the active map SHA in the workload definition.
`workload.v1.json` is retained as historical input. Current dense runs use the
corrected `plan-*.v3.json` schedules; explicit melee runs use `combat_workload.py`,
the external `combat-contract.v1.json`, and the corrected melee-v4 schedule.
The topic evidence record in `docs/evidence/comparable-capacity-20261113/`
distinguishes failed historical runs from fresh results.

Run a 2-player functional calibration before the 25, 50 and 100-player 300-second
stages. Score every actor and complete minute separately. Timeouts remain failed
after late packets; unsent offers remain in the completion denominator. Each
action uses the one absolute deadline derived from its planned time. Scheduling
P95 uses actual send time minus planned time for sent actions. Never extend a
deadline, erase an earlier failed window or infer completion from unpaired chat.

Equipment checks use ordinary Remove/Equip ACKs, followed by normal logout,
transport close and a new authenticated connection that reads full character
state within the common post-ACK 15-second bound. These lifecycle checks happen
outside steady traffic. All players log out normally; representative fresh
logins and offline readback of every saved actor verify persistence.

`run_stage.py` starts only new owned processes. `service_host.py` wraps either
server with normal shutdown control. The wrapper and every game child share one
`windows_service_job.py` Job; the injector uses a separate mask and budget. The
controller records effective limits, all member PIDs, receive-independent
resource timestamps, real terminal exit codes, and hashes of this run's driver
evidence. Resource sampling is fixed at 1000 ms nominal, 2500 ms maximum gap.
No existing PID is attached or stopped. All generated artifacts live on C.

The first layer uses original file persistence versus modern File authority and
in-process session cache. This does not qualify PostgreSQL/Redis or a cloud
machine, and excludes OS and injector memory explicitly. Preserve normal login
rate limits and the local authentication attempt history across stages. Keep
the existing public test cohort and its ledger unchanged. Never change VPN,
routes, firewall, power policy or Windows Update for this comparison.

Adapter implementations and their ordinary wire contracts are in `crystal/`
and `modern/`; independent offline checks are in `verification/`. Preserve
failed runs, frozen original game libraries and historical build snapshots.
