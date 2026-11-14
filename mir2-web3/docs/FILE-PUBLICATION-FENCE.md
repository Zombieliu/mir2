# File publication fence evidence

2026-09-10. Implemented and regression tested; this is persistence evidence,
not game-menu visual acceptance.

The canonical File authority retains one OS writer lock. The same handle reads
and updates its durable marker: opening a second handle after locking fails on
Windows with ERROR_LOCK_VIOLATION and is deliberately avoided.

Before a File-backed publication invokes an external repository or writes the
File image, it writes and syncs a nonempty pending marker. Complete success or
verified compensation explicitly settles it. Unknown outcomes and unsettled
scope exits retain the marker and freeze the live authority. A new authority
or process treats any nonempty marker, including malformed bytes, as frozen.
Ordinary reload and process restart never clear it.

Covered production paths are snapshot saves, scoped AccountStore transactions,
Mirror-first publication/compensation, and the server-owned Guild clock pump.
The clock checks writability again after acquiring the publication mutex.
Definite File failures settle the marker and permit retry; the generic external
writer returns strings, so unproven external failures conservatively retain the
fence instead of inferring a safe retry from message text.

Evidence on the Windows host:

- Configuration suite: 115 passed, 15 explicitly ignored PostgreSQL tests;
  `C:/mir2-build/root-publication-config-tests.log`.
- Dedicated PostgreSQL clock and XP compensation suite: 9 passed;
  `C:/mir2-build/root-publication-postgres-tests.log`.
- Final File authority and publication-boundary suite: 10 passed, including
  an actual child-process reopen, malformed marker preservation, a callback
  observing the marker before external persistence, and definite-failure retry;
  `C:/mir2-build/root-publication-boundary-tests.log`.

No production data was reconciled or unfrozen. A bounded development File-only
operator tool is now available as `file_store_reconcile`; production/Mirror and
item-identity multi-image reconciliation remain unsupported. These
tests establish process-restart behavior and injected persistence outcomes,
not a guarantee for every storage device's power-loss behavior. The separate
PostgreSQL prepared-kill adapter remains an independent integration task.

## Development File-only recovery

The operator must stop clients/server, establish that the interrupted publisher
was File-only, retain the account image, writer marker, authenticated recovery
directory and private MAC key, and review the exact account SHA-256. The tool
requires explicit development/File environment, rejects the authoritative
PostgreSQL-required conditions and any configured DB URL, and refuses identity
sidecars, non-pending markers, changed hashes, invalid/legacy images and a live
writer lock. A dry run validates without settling. The explicit
`--accept-current-file-image` operation accepts the reviewed complete image;
it does not infer an unknown transaction's outcome or rewrite account bytes.

Raw image/marker backups and the operator decision are synced before settlement.
An independent `.reconciliation.pending` guard is persisted before truncating
the writer marker. FileAuthority treats any such guard as frozen, even if the
writer marker is empty. The guard is removed only after successful marker sync.
Interrupted reconciliation requires separate review. Use a Gateway containing
this guard support (r19 or newer in the local evidence); older binaries must not
be used to resume this workflow. Trusted cooperating local filesystem writers
remain a boundary; this is not a general production recovery utility.

Tests: CLI 4/4 and FileAuthority interrupted-guard 1/1. After settlement, use
the unchanged MAC key and existing authenticated startup journal replay; require
zero quarantine and verify checkpoint restoration before resuming players.
`main.rs` now has an opt-in console control for this local workflow. Only when
`MIR2_GATEWAY_STDIN_SHUTDOWN=1` is set does the console command `shutdown`
request shutdown and join the account-store clock; the default is disabled and
stdin EOF does not stop the Gateway. r20 was built at SHA-256
`021CC291B56A6F3AB541A9F97A68EC489D8EA1E6E45FE0A9FED0CC5838C2797E`.
After players disconnected, the explicit console command logged
`gateway operator shutdown received; settling...` in
`gateway/stdin-shutdown-r20.log`, exited with code 0 and left the publication
marker empty. It restarted at PID 189400 in PTY session 84801. This is strong
evidence for the idle explicit-console path only; full active-session shutdown
draining remains a separate gate.
