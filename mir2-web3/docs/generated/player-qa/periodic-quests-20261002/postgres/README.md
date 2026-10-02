# Live PostgreSQL periodic reward proof

The exact opt-in integration test passed once (112.30 seconds, 18 other tests
filtered). Its compiled Windows caller connected over a pinned encrypted SSH
loopback to the existing server PostgreSQL 16.13. It used only the reviewed
limited role/database `mir2_periodic_qa_20261002_claims` and a fresh retained
schema. This did not connect to a human account database or the game realm.

The daily and weekly cases accepted through normal packet paths, persisted
the acceptance reward, and used explicitly declared Ready-count fixtures.
A separate connection held the account row lock; no Finish result/ACK became
available before COMMIT. After release, independent account JSON, full save
and normalized EXP/gold/version projections agreed. An independently loaded
stale configuration could not claim again; retry and fresh login did not award
a second reward. These fixture counts do not prove naturally completed hunts
or 30–60-minute pacing.

The first remote cold compilation was interrupted for real host memory
pressure before any test assertion ran. Its worker observation and root stop
receipt retain their distinct timestamps and observed memory. It is neither
a passing test nor a failed assertion. Root switched to the already compiled
Windows binary to avoid remote compiler load while ordinary gameplay ran.

The [live result](periodic-postgres-r2-windows-result.json),
[original log](periodic-postgres-r2-windows.log),
[root review](root-review.json) and [inventory](inventory.json) bind the exact
source/binary and evidence bytes. Private passwords, database URLs, account
blobs, SSH private key and private scenario inputs are excluded.
