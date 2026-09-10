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

No production data was reconciled or unfrozen. An operator reconciliation tool
is still required before a frozen store may be deliberately resumed. These
tests establish process-restart behavior and injected persistence outcomes,
not a guarantee for every storage device's power-loss behavior. The separate
PostgreSQL prepared-kill adapter remains an independent integration task.
