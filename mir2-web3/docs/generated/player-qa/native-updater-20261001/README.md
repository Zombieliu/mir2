# Native updater candidate — 2026-10-01

User requested automatic updating so players do not reinstall every hotfix.
The separate native launcher/scratch engine and transactional per-file updater
are implemented. The compiled r7 installer combines new display Candidate07
with an independently attested updater; immutable earlier releases are preserved.

Verified: Rust updater24/24 including actual Windows CMS crypto fixtures,
native nine-language status window at100/150/200% DPI, install mutex/file lock,
process identity, path/manifest/freshness/downgrade and interrupted activation.
Five receipt regressions cover newer installer seeds, retained older history,
same-sequence conflicts, seed-only binding and accepted-release precedence.
The locale regression checks the game's roaming preference before legacy local
settings. Archive helper28passed/3 Linux-only skips; actual Windows junction,
symlink, hardlink and ADS rejection pass. Deployment-plan/publisher checks16/16;
PowerShell security gates28/28. Nine-language installer closure and syntax-only
Inno compile pass, including exclusive install locking and old journal retirement.

The original r5→r6 package test rejected r5's legacy SHA1 CMS, as required by the
SHA256-only updater. That failed fixture is retained, not re-signed/relabelled.
The passed final integration gate uses unchanged SHA256-signed r6 as its previous
release and new Candidate07, including a different authenticated engine, delta
payload checks, personal-file preservation and first-launch rollback.

Final clean source/build is `fefd18370a947848d4f2f86ee3468ea4251fe011`.
The strict123035-file game Candidate and nine-file supplemental updater bundle
pass CMS/key-pin/hash/source binding; independent installer inputs pass and
actual Inno compilation exits0. Setup is603522410bytes, SHA256
`A34C2A300585ED66877BA238D84A5C082D85C7C86AF24F85E8D47F9794B30D97`.
[Installer handoff](installer-handoff.json) records compiler/runtime identities.

[Actual signed-package integration](real-package-delta.json) passes both tests:
all new payload hashes,123028 unchanged files, preserved personal data, pending
first-launch obligation, engine activation/rollback and failed-feed quarantine.
Only three game files plus the engine are downloaded:112319925bytes (~107MiB).

[Static HTTPS publication](deployment.json) promotes immutable game/engine paths
and publishes signed sequence3 at
`https://165.154.65.136.sslip.io/client-updates/latest.json`.
Live TLS GET/HEAD, discovery no-store, immutable cache,405 POST and404 directory
checks pass. Both gateway revisions/health and zero active-session counts are
unchanged before/after; no game-server binaries, accounts or saves are changed.

[Real native launcher HTTPS test](live-https.json) upgrades an isolated r6 copy
through the public endpoint, downloads exactly the same four files, authenticates
the new engine and preserves the personal diagnostic. The second launch/check
downloads0bytes. No player gameplay or actual installed copy is started/edited.
Check-only deliberately leaves first-launch health acceptance pending.
The affected laptop and real full-installer human acceptance remain open.
Setup/game/launcher retain `NotSigned` public Authenticode status; internal CMS
does not solve the previously reported application-control4551.
Capacity stays paused; no full-game or50–100-player acceptance is claimed.

[Display evidence](../native-display-20261001/README.md).
[Implementation and operational limits](../../../../apps/game-client/windows-updater/README.md).
