# Native immutable payload resume — source Candidate

The four production files are `update.rs`, new `download.rs`, `fs_safe.rs` and
the narrow `delivery.rs` fallback change; `resume_tests.rs` is the new test file.
Parent commit is `2afdac8ca4d06e30d9fe4d40c283d301b884ace3`. No dependencies,
compiled origins, discovery/CMS rules, transaction schema 1 or existing fresh
running-game guards changed. The public engine remains unchanged.

Only the pinned HTTPS payload adapter retains interrupted scratch. The old
`Source::download` compatibility API remains; legacy adapters still restart.
Partial bytes live under `.update/downloads/<signed SHA>.part`, remain untrusted,
and are never extracted or installed before the full signed size/SHA passes.
The writable handle supplies the actual resume offset, rejects reparse/hardlink
targets, checks its resolved local path, and denies concurrent write/delete on
Windows. UTF-16 path comparison avoids lossy conversion and supports Unicode
and long local paths.

A strict single-range 206 must match the retained offset, signed total and final
byte; interpreted header cardinality, identity encoding, body size and SHA are
checked. A 200 that ignores Range resets rather than appends. An exact 416 total
permits one same-origin ordinary GET; it never proves completion. Redirects and
malformed protocol fail closed. Connection/timeout/EOF, user cancellation and
explicit temporary statuses preserve only synchronized, regular bounded bytes.
Integrity failures discard scratch. Wire counters exclude disk-reused bytes.

An interrupted bundle/delta now propagates out before raw-file staging. It no
longer converts one lost connection into thousands of asset requests. Genuine
corrupt accelerator fallback is preserved and passes the adjacent checks.

Two actual loopback regressions were recorded before their fixes: original
interruption deleted the prefix (0/1), then the accelerator swallowed interruption
and returned raw fallback (0/1). Both original sources and raw commands are in
the archive. Two later grouped-fixture failures are also retained: pinned ureq
tolerates an unknown colonless header and discards an invalid unknown header
name. Those unsupported generic grammar assertions were replaced by 101 valid
headers to exercise the dependency's actual `BadHeader` limit. Production checks
were not weakened; this slice does not claim a replacement HTTP grammar parser.
A proof-helper parse refusal before any writes is retained separately.

Final actual Windows MSVC/Rust 1.95 library results are **65 passed, 0 failed,
4 existing signed-integration cases ignored**. The 14 new checks are included
in 65 and overlap the earlier focused runs. They exercise real loopback HTTP
and actual Windows file handles with synthetic immutable FileEntry fixtures;
they do not verify real public HTTPS, CMS or the CDN. Both Windows entrypoints
check offline/locked/jobs2 without warnings. Source hashes match both final
command receipts and the byte-exact snapshots.

`BYTE-EXACT-PROOF.tar.gz` contains 86 inert regular members, 143,212 bytes,
SHA256 `29d52cd690a5bbf7950592a56fd78d966e89489fd99e1d9e7de51733397a53ac`.
`PROOF-INVENTORY.json` hashes every member. Owned temporary trees, compiler
artifacts and user data are excluded. No live Gateway/Caddy/feed, F installation
or player saves changed.

The new engine is not built or published. Actual HTTPS interruption/resume,
CMS/release-engine installation, affected laptop throughput and R2 stage/promote
remain open. Existing two Cloudflare 401 results still require effective
authorization. Resume reduces re-downloads; it does not establish a faster link
or reclassify the previous 41.16-minute local installation measurement.
