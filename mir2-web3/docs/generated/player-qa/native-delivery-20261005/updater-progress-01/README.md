# Native updater local progress — source Candidate

Root integrated the issued `4c60c323aed11829abae6ee5ce6e13c675a3c1c3`
runtime baseline into the downloader branch, then the four-file progress patch
and one adjacent test. The six baseline files plus overlay produce nine unique
changed files. Root's updater had no tracked changes since the common ancestor;
untracked paths were checked before writing. Only five selected Rust files were
formatted. The older publication/build scripts were not backfilled in this slice.

Unpacking, installed-file checking, cache staging, preparation, installation and
recovery now expose actual completed/total counts. Display snapshots are throttled
to 200 ms; boundaries and final counts are forced. Phase-end `Instant` traces
separate payload resolution and local copy/flush time. No per-file disk trace is
added. Nine locales distinguish these phases from downloading. The infallible
observer cannot cancel an applying transaction; the original SHA/fsync, schema 1,
four fresh exact-game process guards and filesystem routines are preserved.

Root independently passes 10 new progress tests, four transaction tests, two bad
bundle tests, bundle fallback and delta fallback: 18 distinct selected tests,
overlapping the worker's 18. Windows MSVC Rust 1.95, offline/locked/jobs2 checks
both entrypoints without warnings. Raw commands, logs, minimum disk-space checks,
actual source hashes and integration receipts are under `root/`.

`BYTE-EXACT-PROOF.tar.gz` retains all 98 original worker files, its original freeze
and the supplemental tracked-count correction. It contains inert ignored Python
bytecode as evidence; these are not build inputs. The original review incorrectly
called 41 files tracked: the correction identifies **39 tracked files plus two
ignored .pyc files**. Three root mechanical refusals before repository writes are
also retained; they concern CRLF/blob comparison and that count, not product REDs.
The original three genuine assertion REDs remain in the archive.

The public engine and installer were not replaced. This is source and library
verification, not a new release executable, native window, 125,965-file install,
affected laptop or speed improvement. The old 41.16-minute library install is not
relabelled or extrapolated into a new installation time. Real release/HTTPS/CDN
and human gates remain open; game, Gateway and player saves are unchanged.
