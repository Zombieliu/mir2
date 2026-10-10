# Source terrain and ordinary collision Candidate — 2026-10-10

Status: implemented in the normal collision entry, locally verified as stated
below; new commit/push, exact new Linux package, deployment and human acceptance
are separate gates. This is not complete Source `Map.Load`, CHECKHUM, P7 or
100% Classic/Mentor. Base is actually pushed01e3d08791c2639b3375c39d46fbf4f00ca53f75.

## Result and original behavior

`runtime/source_map_loading.rs` directly decodes Source formats0–7/100. The
normal `map.rs::parse_runtime_map_collision` uses that decoder, replacing515
lines of duplicated format decoding while preserving the existing canonical
`<normalized>.map` output and cache/dynamic-door projection. Three verified
Source discrepancies are corrected: FindType's0F branch does not require CRLF;
v4 advances12 bytes per cell; v0/v2/v3 assign no-floor HighWall last, overriding
earlier LowWall. Original C# anchors are `Server/MirEnvir/Map.cs:73–429` and
`GetWalkableCells:516–525`; its actual SHA256 is
`4df569012db86428b82b3c4994a1b0e982b1da7b5ed0b58dcf1930020ed4d5d1`.

Two private decode domains remain explicit. Raw preparation requires complete
declared records, retaining original ordinal/index/literal FileName, raw bytes
SHA256, every cell and x-major base Walk points. Closed doors and arrival
protection do not remove Source base Walk cells. Its private fields have no
serde, filesystem access, configured fallback, loaded receipt or population
authority. Ordinary collision follows Source's **actual reads**: ignored
trailing bytes and unread last-cell skip bytes are valid; every actual required
field must still exist. This wider domain cannot construct Prepared identity.

Both domains require positive bounded dimensions and checked arithmetic;
limits are4096 per side,4,194,304 cells and128MiB supplied raw bytes. These are
preparation/decoder limits, not assertions that Source rejects every larger map.
Unavailable preparation is not Source load failure or a known-empty world.
Existing fallback code is retained; the accepted parser domain changes, so
identical fallback behavior for every possible input is **not** claimed.
File reads and gzip decompression are not made bounded by an after-read decoder
limit. Normal decoding adds a temporary cell vector/second traversal; its path
does not allocate the unused base-Walk list. No cold-load/RSS improvement is
claimed from the retained clone benchmark.

## Actual local verification

Final library run passes32 distinct tests,0 failures; the original explicit
clone benchmark remains1 ignored and is excluded. It covers18 raw/normal-domain
checks and14 unchanged actual collision/alias/cache/door/shop-exit/mine/fishing
checks. The final optimized Gateway rerun on the corrected source passes all
4 original tests: 36 distinct final checks pass, with the one original benchmark
excluded. Original 500ms TownTeleport and authentication/skill assertions are
unchanged. Earlier repeated runs are retained and not added to the distinct count.

An isolated .NET9 harness executes the **unmodified** original C# FindType,
nine LoadMapCells methods, AddDoor and GetWalkableCells. Minimal Cell/Door
fields preserve the exact used attribute/default/door-sharing behavior; this
does not run Map.Load, NPC scripts, Source server boot or any real account save.
A separate Rust binary includes the actual production module by path and uses
the real game-data crate. It compares raw SHA, dimensions and x-major hashes of
every cell attribute/door presence+index/fishing attribute and every base Walk
coordinate, plus actual ordinary collision versus prepared terrain when the
latter is available.

The imported464 MapInfo records supply463 nonempty Server Maps inputs plus one
literal empty placeholder, which is not decoded. **All463** strict preparations
and ordinary collisions match the original reader,0 mismatch/0 unavailable.
Actual formats are1×456,5×5,100×2. The source template DB's fresh hash is
`829121f4d762ea4427325a767e326a9c04794eb51450ada8a72a0b857705f075`;
this does not prove that a new original DB boot/load succeeded.

The **entire actual bundled gzip pack has1,620 files**,872,410,502 decompressed
bytes; this is distinct from the208 profile/209 native-resource counts and is
not a successful-world-map count. The first widened probe exposed19 strict
trailing-byte preparation gaps and failed; that original failure is retained.
After the normal Source-read correction, **all1,620 ordinary collisions** match
C# with0 mismatch. Strict preparation still explicitly remains unavailable for
those19, rather than inventing a loaded receipt. Pack formats are0×237,
1×1370,3×1,5×6,100×6; the other four formats have synthetic tests and Source
inspection only. These overlapping corpora are not2,083 distinct maps, and
neither proves client rendering or every preferred raw-client directory.

The temporary probe's initial sha2 hex-format compile failures and rejected
PowerShell repair are retained; only the probe serializer was repaired. Original
C# methods and original product assertions were not altered to make them pass.
Input/output hashes, exact runners, final read-only review and every retained
failure are in the [original archive](generated/player-qa/source-map-terrain-20261010/candidate-01/original-evidence.zip)
and [byte-verified receipt](generated/player-qa/source-map-terrain-20261010/candidate-01/EVIDENCE.json).
Selected local hashes do not claim whole Cargo/toolchain/workspace input closure.
Earlier integration metadata is preserved; FINAL-PROVENANCE.json supplies the
actual final hashes. The evidence flags are the pre-commit checkpoint.

## Release and next boundaries

Predecessor 01e3 is actually pushed with guarded GitHub readbacks. Its Linux
run 37993166469 completed the main build and genuine package upload successfully;
the first separate siege job failed before tests pulling postgres:16 (Docker Hub
timeout/rate limit), so attempt 1 failed overall. One actual failed-job-only
rerun was accepted. Snapshot 13 at 2026-10-09 22:34:16 UTC shows the main build
success retained and the siege job running its live protocol checks. No pipeline
or artifact qualification is inferred while that job is incomplete.
New-source Linux/package/published checks remain separate. A fresh public read
still shows Gatewayc704/R23/feed18,1 WS connection, other capacity counters0,
TCP7200=0 and spectator viewers0; qualified earlierecd81 rollout stays held
before staging. Original realm/config/saves and other games are preserved.

Next P7 work must initialize real respawns/routes, actual NPC/script objects,
safe-zone objects and mining in Source order before issuing a boot/input-bound
opaque completed-load receipt. Source failure after allocations must retain
consumed object-ID progression. A global directory must preserve all464 original
ordinals and determinate outcomes, select successful duplicates in original
order before topology/instance ownership and never turn Unavailable into zero.
Then wire ordinary admitted NPC/page/body/default-FIFO identity, deterministic
read transcripts and same-Source-CAS reservations/ordered world outbox.
Cold snapshots cannot mint live receipts. These gates, natural/native/human
P1–P7/Mentor acceptance and complete original gameplay remain open.
The expired heartbeat remains paused; no old blocked Goal is marked complete.
