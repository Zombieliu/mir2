# Native updater candidate — 2026-10-01

User requested automatic updating so players do not reinstall every hotfix.
The separate native launcher/scratch engine and transactional per-file updater
are implemented. The final r7 installer will combine new display Candidate07
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
The actual final integration gate uses unchanged SHA256-signed r6 as its previous
release and new Candidate07, including a different authenticated engine, delta
payload checks, personal-file preservation and first-launch rollback.

Final clean build, actual r6→r7 integration, full installer and public HTTPS
feed validation are still pending. No installed player game/server/save changed.
Capacity remains paused; no full-game or public Windows publisher acceptance.

[Display evidence](../native-display-20261001/README.md).
[Implementation and operational limits](../../../../apps/game-client/windows-updater/README.md).
