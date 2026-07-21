# Crystal Full Asset Pipeline

Status: lossless PNG/CAS baseline implemented, locally accepted on
2026-07-14, and packaged for reproducible developer handoff on 2026-07-22.

## Decision

The production baseline is **complete offline conversion plus lazy, sharded
delivery**. It is not one giant atlas loaded into memory.

- Convert every Crystal `.Lib` ahead of release.
- Publish one small index, one manifest shard per library, and immutable
  content-addressed PNG pages.
- Load only the libraries and pages required by the current scene.
- Bound decoded GPU memory with device-tier LRU caches.
- Keep the legacy per-frame path as a rollback while runtime consumers migrate.

This gives complete source coverage without making a low-end device download or
decode the whole client. A monolithic resident atlas would require about
58.65 GiB of decoded RGBA memory and is therefore not a viable Web architecture.

## Verified Baseline

The committed coverage report is
`docs/generated/assets/crystal-full-pack-coverage.generated.json`.

| Metric | Verified value |
| --- | ---: |
| Source libraries | 1,440 / 1,440 |
| Frame slots classified | 2,143,132 / 2,143,132 |
| Packed image frames | 1,869,869 |
| No-draw frames | 273,263 |
| Packed masks | 120 |
| FrameSet actions | 3,643 |
| Logical page references | 4,459 |
| Unique CAS PNG pages | 4,446 |
| Network PNG bytes | 7,342,139,517 |
| Full decoded RGBA bytes | 62,972,592,128 |
| Content hash | `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e` |

Verification recalculates library manifests, cross-references, page hashes,
frame classifications, placements, and path boundaries. Build output is
resumable and written atomically. Pruning refuses filtered input and only
removes unreferenced files inside the configured output root.

## Output Contract

The local build writes under
`apps/web/public/generated/crystal-packs/full/`:

- `index.json`: compact library catalog and content hash.
- `libraries/<library>.json`: one lazy manifest shard per Crystal library.
- `pages/<sha256>.png`: immutable, deduplicated atlas pages.
- `plan.json`: deterministic sizing and build plan.

The generated payload is intentionally ignored by Git. Production should build
it in an asset job and publish pages before manifests and the mutable release
pointer. JSON should be Brotli-compressed; CAS pages should use immutable cache
headers.

## Developer Distribution And Integrity Gate

The approved developer bundle is pinned by
`config/developer-assets.json` and published as a private GitHub Release. The
current bundle has these immutable identifiers:

- Release tag: `developer-assets-f71b89aa3850`
- Full-pack content hash: `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`
- Deterministic USTAR SHA-256: `d8dd209e47a5f03eb41b1b03758383b102a9a25d11fedc32bb1d71ec700b0fd9`
- Archive bytes: `9,751,758,336`, split into seven verified Release assets

`scripts/package-developer-assets.ps1` accepts only the exact index closure:
one index, 1,440 library shards, and 4,446 unique PNG pages. It verifies every
page hash before writing a deterministic USTAR archive. The installer verifies
every part, the reconstructed archive, safe USTAR entry types and paths, and
the extracted full-pack closure before an atomic directory swap.

The R2 release path is separate from the private developer bundle. A full-pack
release manifest must contain exactly 5,887 full-pack objects with source size
and SHA-256, must use the streaming `r2-s3` uploader, and must publish pages and
shards before the mutable pointer. `release-doctor.mjs` probes all 5,887 remote
objects when `--requireFullCrystalPack true` is enabled. The hosted workflow
intentionally refuses to upload this ignored 10 GB input; publish from an
authorized local machine, then use the workflow's existing-release verification
path. Do not describe R2 as live until that full remote probe passes.

## Runtime Adoption

Entity presentation now resolves the full-pack shard first and falls back to
legacy exported frames when disabled or unavailable. `?crystalFullPack=0`
provides the rollback. Both Bevy atlas construction and the raw WebGL2 entity
layer use bounded residency instead of retaining every decoded page.

This does **not** mean every runtime surface has moved into this generic pack:

- Map rendering keeps its regional scene-atlas and standalone-image paths.
- HUD and login/select art keep their dedicated UI loading path.
- Audio keeps the SoundList/WAV pipeline.
- Some effect-specific paths still use dedicated effect exports.

Those consumers can share the immutable delivery and residency primitives, but
forcing all of them into one atlas would increase churn and memory pressure.

## Device Tiers

| Tier | Bevy entity memory | Bevy entries | WebGL2 memory | WebGL2 entries | Scene prewarm |
| --- | ---: | ---: | ---: | ---: | --- |
| Low, at most 2 GiB | 64 MiB | 8 | 64 MiB | 12 | 192 frames, concurrency 3, background off |
| Low, 4 GiB or coarse pointer | 96 MiB | 8 | 96 MiB | 20 | 192 frames, concurrency 3, background off |
| Medium | 160 MiB | 16 | 160 MiB | 32 | 480 frames, concurrency 5, after playable |
| High | 256 MiB | 24 | 256 MiB | 48 | source limit, concurrency 8, after playable |

`?renderTier=low|medium|high` forces a tier for QA.
`?prewarmBackground=off|immediate|afterPlayable` controls background prewarm.
Automatic low-tier selection uses `navigator.deviceMemory`, coarse-pointer
input, and the maximum supported texture size.

## Low-End Acceptance

Evidence is under
`docs/generated/player-qa/full-asset-pack-low-tier/`.

- Forced low tier, WebGL2, Bichon: 13 resident pages and 1,598 rects used
  58,379,430 decoded bytes, below the 64 MiB target.
- Four movement commands sent and acknowledged; all 28 strict movement/render
  assertions passed with no residual plan, DOM entity fallback, 404, or critical
  console error.
- Low-tier prewarm requested 403 assets and completed 403 with zero failures,
  rather than warming the full source catalog.
- Cold first playable was 5,190.6 ms with 18,993,684 transfer bytes.
- Warm transfer was 600 bytes; CacheStorage used 69,027,432 bytes, below the
  256 MiB QA ceiling.

These are desktop emulation and local-network gates. Brazil release acceptance
still requires physical 2 GiB and 4 GiB Android devices, throttled 4G, repeated
map transitions, memory-pressure/background-resume testing, and CDN telemetry.

## Commands

Run from `apps/web`:

```powershell
npm run generate:crystal-source-snapshot
npm run generate:crystal-pack-catalog
npm run assets:full-pack:plan
npm run assets:full-pack:build
npm run assets:full-pack:verify
npm run assets:full-pack:prune

npm run test:crystal-library
npm run test:full-asset-pack
npm run test:full-pack-index
npm run test:full-pack-bevy
npm run test:render-tier
npm run test:asset-prewarm-policy
npx tsc --noEmit
```

## Compression Boundary

Lossless PNG remains the compatibility baseline. KTX2/UASTC can be added as an
optional page variant only after WebGL2 support, alpha/pixel-diff validation,
device fallback, and real download/memory measurements pass. Do not make ETC1S
or WebGPU-only texture delivery the sole path for Brazil low-end clients.
