# mir2 offline asset pipeline — Phase 0.3

This directory is the **offline asset pipeline** for the mir2 web client.  It
is the "airlock" between raw Crystal-exported PNG frames and the conditioned
atlas bundles that the runtime loads.

Every file here is additive — no existing app files or generator scripts were
modified.

---

## Files

| File | Purpose |
|---|---|
| `pack.mjs` | CLI atlas packer (the main entrypoint) |
| `schema.ts` | TypeScript type definitions for the manifest schema |
| `README.md` | This file |

---

## Conceptual pipeline

```
Crystal .Lib files
      │
      ▼  (export-crystal-ui.mjs — existing script, unchanged)
apps/web/public/original-ui/**/*.png  +  per-library meta.json
      │
      ▼  (pack.mjs — this pipeline)
apps/web/public/bevy-entity-atlases/
      ├── manifest.json        ← AtlasManifest (schemaVersion=2)
      └── starter-bichon-base.png  ← packed atlas texture
      │
      ▼  (R2 upload — existing upload-r2-assets.mjs, unchanged)
CDN at assets.mir2.obelisk.build/mir2/v/<version>/bevy-entity-atlases/
      │
      ▼  runtime loadBevyEntityAtlasManifest()  (original-client-shell.tsx)
```

---

## CLI reference

```
node scripts/asset-pipeline/pack.mjs [options]
```

All options are optional — the defaults replicate the existing
`build-bevy-entity-atlas-pack.mjs` output exactly.

| Option | Default | Description |
|---|---|---|
| `--category` | `entities` | `entities`, `map`, or `ui` |
| `--roots` | entity starter set | Comma-separated dirs under `public/original-ui/` |
| `--atlasKey` | `starter-bichon-base` | Logical name for this atlas bundle |
| `--outDir` | `public/bevy-entity-atlases` | Directory to write outputs |
| `--publicBase` | `/bevy-entity-atlases` | URL prefix for `imageUrl` fields |
| `--padding` | `1` | Padding in pixels added around each frame |
| `--maxSize` | `4096` | Maximum atlas dimension in pixels |
| `--dry-run` | `false` | Print plan, skip writing files |

### Quick start (from `apps/web/`)

```bash
# Dry-run: see what would be packed without writing anything
node ../../scripts/asset-pipeline/pack.mjs --dry-run

# Full pack (requires public/original-ui/ to be populated)
node ../../scripts/asset-pipeline/pack.mjs

# Custom roots / key
node ../../scripts/asset-pipeline/pack.mjs \
  --category entities \
  --roots "Monster/000,Monster/010,NPC" \
  --atlasKey monster-pack-0 \
  --outDir /tmp/mir2-atlas-out

# Wire into the package.json scripts block as:
#   "assets:pack": "node ../../scripts/asset-pipeline/pack.mjs"
```

---

## Manifest schema (schemaVersion=2)

The manifest is a backward-compatible superset of the legacy
`build-bevy-entity-atlas-pack.mjs` output (schemaVersion=1).  Runtime code
that reads the old shape continues to work unchanged.

New additive fields (absent in legacy manifests):

| Location | Field | Type | Notes |
|---|---|---|---|
| `atlases[].rects[].offsetX` | `number?` | Crystal `MImage.X` draw offset |
| `atlases[].rects[].offsetY` | `number?` | Crystal `MImage.Y` draw offset |
| `atlases[].rects[].shadowX` | `number?` | Crystal `MImage.ShadowX` |
| `atlases[].rects[].shadowY` | `number?` | Crystal `MImage.ShadowY` |
| `atlases[].rects[].frameIndex` | `number?` | 0-based index within source library |
| `atlases[].pages[]` | `AtlasPage[]?` | Per-page detail (multi-page ready) |
| `atlases[].contentHash` | `string?` | SHA-256 over atlas key + rects + image |
| `atlases[].category` | `string?` | `entities` / `map` / `ui` |
| `atlases[].padding` | `number?` | Padding used during packing |
| `stats.pageCount` | `number?` | Number of pages emitted |
| `stats.durationMs` | `number?` | Wall-clock time for the pack run |
| `pipeline` | `object?` | CLI provenance block |

Full type definitions are in `schema.ts`.

### Crystal offset semantics

`offsetX` / `offsetY` come from `MImage.X` / `MImage.Y` in
`Crystal/Client/MirGraphics/MLibrary.cs:862`.  They describe **where the
top-left of the bitmap sits relative to the logical entity anchor** (usually
the character's feet).  A renderer uses them as:

```
draw_x = entity_screen_x + offsetX
draw_y = entity_screen_y + offsetY
```

These are absent for frames whose source library didn't export a `meta.json`
(e.g. if the library was not passed through `export-crystal-ui.mjs`).

---

## KTX2 encode (follow-up)

PNG pages are emitted today.  To add GPU-compressed output:

1. Install `toktx` (from KTX-Software) or `basisu` in the build environment.
2. After `sharp` writes the PNG, shell out:
   ```
   toktx --t2 --bcmp <out>.ktx2 <out>.png
   ```
3. Add a `ktx2Url` field to `AtlasPage` and emit it alongside `imageUrl`.
4. Update `loadBevyEntityAtlasManifest()` to prefer `ktx2Url` when the
   browser exposes `EXT_texture_compression_bptc` or
   `WEBGL_compressed_texture_etc`.

This is a pure follow-up — no runtime changes are needed today because the
runtime already falls back to PNG.

---

## Relationship to existing scripts

| Existing script | Relationship |
|---|---|
| `apps/web/scripts/build-bevy-entity-atlas-pack.mjs` | The pipeline **wraps** its algorithm — identical shelf packer, same output shape.  The legacy script is NOT modified.  Run either one; they produce compatible manifests. |
| `apps/web/scripts/export-crystal-ui.mjs` | Upstream — run it first to populate `public/original-ui/`.  The pipeline reads its `meta.json` outputs to attach Crystal offsets. |
| `apps/web/scripts/upload-r2-assets.mjs` | Downstream — run after packing to push the atlas to R2. |
| `apps/web/scripts/generate-original-asset-manifest.mjs` | Downstream — re-run to bump the `bevy-entity-atlas-manifest` version input so the asset manifest version changes. |

---

## Adding this to `package.json` scripts

In `apps/web/package.json`, optionally add:

```json
"assets:bevy-entity-atlas:build:pipeline": "node ../../scripts/asset-pipeline/pack.mjs",
"assets:bevy-entity-atlas:dry-run":        "node ../../scripts/asset-pipeline/pack.mjs --dry-run"
```

The pipeline is intentionally separate from the existing
`assets:bevy-entity-atlas:build` entry so both scripts can coexist during the
transition.
