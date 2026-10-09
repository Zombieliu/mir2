# Bounded Source map I/O Candidate — 2026-10-10

Raw and packaged map reads now have a real input/output limit rather than
reading an arbitrary file or decompression stream into memory before rejecting
it. The shared terrain domain is128 MiB for raw/decoded bytes. Actual compressed
reads and prefetch stop at129 MiB, with at most one rejection probe byte. These
are preparation resource limits, not original Crystal `Map.Load` rules.

`source_map_io.rs` grows the output Vec through fallible, geometric reservations.
Its successful returned length and reported capacity remain within the limit.
Fixed8 KiB stack buffers bound prefetch/output staging. At exactly the decoded
limit it performs a nonempty read through the gzip decoder: actual EOF plus a
valid CRC/ISIZE footer are required. A truncated or invalid first member cannot
publish partial bytes. Original single-member semantics remain: later members
and ignored trailing data are not scanned or rejected by whole-file length.

Ordinary collision retains the existing raw-file precedence. Actual I/O or
output-allocation failure may try the gzip pack; a raw file exceeding the
existing decoder domain cannot add a new pack fallback. Successful raw reading
followed by parsing failure still suppresses pack fallback on that path. The
world path separately retains its original raw read/parse-failure fallback.
Typed errors keep a real `InvalidData` reader failure separate from a size limit.

This is not a guarantee for process RSS, allocator footprint, concurrent caches
or every OOM. Pinned flate2 still makes finite non-fallible header/deflate
workspace allocations internally; actual allocation exhaustion was not fault
injected. No map identity, `Map.Load` success, loaded directory, empty-map
authority, world initialization or durable population receipt is produced.

## Actual final checks

Root ran the final library selectors:59 passed,0 failed,1 original benchmark
ignored. This includes27 bounded-I/O cases,18 Source terrain cases and14
ordinary collision checks. All4 original optimized Gateway map/poison/Purification
stories pass, retaining the real500ms scroll and auth/permission assertions.
These are63 distinct final tests; earlier repeated runs are not added.

The actual production raw/gzip entry points were compared to retained oracle
byte hashes:463 nonempty Source Maps files and1,620 actual gzip pack records all
match, with0 mismatch. The empty Source file-name record is not decoded. The
corpora overlap and do not mean2,083 distinct maps or complete client rendering.
The before/after19 selected production/probe/oracle inputs are stable; this is
not complete Cargo/toolchain/workspace input closure. Original collision layout
and strict preparation limitations remain documented in the
[terrain Candidate](CLASSIC-SOURCE-MAP-TERRAIN-20261010.md).

Read-only review found no blocking issue on the final five integration files;
the worker edited only its assigned leaf, and Root alone integrated/tests/Git.
The [checkpoint receipt](generated/player-qa/source-map-io-20261010/candidate-01/EVIDENCE.json)
and [original byte-verified archive](generated/player-qa/source-map-io-20261010/candidate-01/original-evidence.zip)
contain commands, selected hashes, final logs, byte comparisons, unchanged
oracle inputs, actual source and review scope. Their flags are a pre-commit
checkpoint and cannot be upgraded without new actual follow-up records.

## Release and remaining work

Parent64b6 is actually pushed and frozen in t64. Its Linux run
[38001876065](https://github.com/Zombieliu/mir2/actions/runs/38001876065) was still
in progress at the last archived readback; a failed API EOF query is retained.
Predecessor01e3 Linux/genuine package qualification is separate and cannot
qualify this I/O source. This new Candidate requires its own exact Git readback,
Linux gates/package verification and drained production preflight.

Last observed public Gatewayc704/R23/sourcefaef/signedfeed18 had one undrained
WS connection; no staging/switch, forced kick, real save or other-game change
occurred. Ordered actual Source map respawns/routes/NPC scripts/safe zones/mines,
successful-load receipts/global directory, ordinary admitted NPC page/body/FIFO,
Source-CAS world outbox and fullP1–P7/Mentor/native/human remain unfinished. The
expired heartbeat stays paused and no blocked Goal is marked complete.
