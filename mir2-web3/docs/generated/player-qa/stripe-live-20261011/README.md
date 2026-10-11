# Stripe live activation readiness — 2026-10-11

Actual callback route application and Windows-to-public HTTPS probes are
qualified. Live payment activation is blocked by a test-mode credential in
the newly filled private live file. Provider read checks are not claimed;
the verifier stops before making any request with a test key.

The running Gateway is independently observed at4006d988, not the older
01a8 release assumed by the previous handoff. Five billing code files are
byte identical between those revisions. Client updater feed19 is preserved.
This change reloads only Caddy; game processes, saves, installed clients and
all account secrets are untouched. The exact route body and applied receipt
are included. Private credential files, server secrets and database backups
are excluded. Original preparatory mistakes are retained and excluded in INDEX.

See [activation record](../../../STRIPE-LIVE-ACTIVATION-20261011.md).
