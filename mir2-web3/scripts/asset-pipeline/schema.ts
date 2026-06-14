/**
 * mir2 offline asset pipeline — manifest schema (TypeScript types).
 *
 * The canonical output of the `pack` CLI is a JSON file that matches
 * `AtlasManifest`.  Every field that the runtime already reads from
 * `/bevy-entity-atlases/manifest.json` is preserved verbatim so existing
 * consumers need no changes.  New fields are additive and optional so that
 * a manifest produced by the old `build-bevy-entity-atlas-pack.mjs` script
 * also satisfies the shape (backward-compatible superset).
 *
 * Crystal frame provenance
 * ─────────────────────────
 * MImage (Crystal/Client/MirGraphics/MLibrary.cs:862) carries:
 *   short Width, Height   — pixel dimensions of the frame
 *   short X, Y            — draw offset: where the top-left of the bitmap sits
 *                           relative to the logical anchor point (usually the
 *                           entity's feet in the isometric grid)
 *   short ShadowX, ShadowY — shadow sprite draw offset
 *   byte  Shadow          — shadow flags (bit7 = hasMask)
 *
 * The per-library meta.json exported by export-crystal-ui.mjs stores every
 * field as camelCase (x, y, shadowX, shadowY, hasMask).  This pipeline
 * carries those offsets into the atlas manifest so the runtime can reconstruct
 * the correct draw position without re-reading individual frame meta files.
 */

// ─── per-frame ────────────────────────────────────────────────────────────────

/** Pixel rectangle inside an atlas page (top-left origin). */
export type AtlasRect = {
  /**
   * Stable lookup key: the public-path URL of the source frame plus its
   * pixel dimensions, joined with "|".
   * Example: "/original-ui/Monster/000/0.png|64x76"
   *
   * Matches the `key` field written by `build-bevy-entity-atlas-pack.mjs` so
   * the runtime's `bevyEntityAtlasRectMap()` lookup works unchanged.
   */
  key: string;

  /** Left edge of the frame inside the atlas page (pixels). */
  x: number;
  /** Top edge of the frame inside the atlas page (pixels). */
  y: number;
  /** Frame width in pixels. */
  width: number;
  /** Frame height in pixels. */
  height: number;

  // ── Crystal offset / anchor (additive — absent when unknown) ──────────────

  /**
   * Horizontal draw offset from the logical anchor point (Crystal MImage.X).
   * Positive = shift the bitmap to the right relative to the entity position.
   * Absent when the source frame lacked metadata (e.g. pre-baked atlas inputs).
   */
  offsetX?: number;
  /**
   * Vertical draw offset from the logical anchor point (Crystal MImage.Y).
   * Absent when the source frame lacked metadata.
   */
  offsetY?: number;
  /**
   * Shadow sprite horizontal offset (Crystal MImage.ShadowX).
   * Absent when the source frame lacked metadata or shadow is not applicable.
   */
  shadowX?: number;
  /**
   * Shadow sprite vertical offset (Crystal MImage.ShadowY).
   * Absent when the source frame lacked metadata.
   */
  shadowY?: number;

  /**
   * Source frame index within its library (0-based).
   * Absent when the source frame was not linked to a Crystal library.
   */
  frameIndex?: number;
};

// ─── per-page ─────────────────────────────────────────────────────────────────

/**
 * A single atlas texture page.  The pipeline currently emits one page per
 * atlas entry; multi-page support is a follow-up.
 */
export type AtlasPage = {
  /** Filename of the atlas image, relative to the manifest file. */
  imageFile: string;
  /**
   * Public URL path for the atlas image (absolute, relative to the web root).
   * Matches the `imageUrl` field written by `build-bevy-entity-atlas-pack.mjs`
   * so the runtime's `loadBevyEntityAtlasImage()` call still works.
   */
  imageUrl: string;
  /** Page width in pixels (always a power of two). */
  width: number;
  /** Page height in pixels (always a power of two). */
  height: number;
  /**
   * SHA-256 hex digest of the PNG file bytes.
   * Used by the runtime and service worker to detect stale cached pages.
   */
  sha256: string;
  /** Compressed file size in bytes. */
  imageBytes: number;
  /** Uncompressed RGBA buffer size (width × height × 4). */
  rgbaBytes: number;
};

// ─── per-atlas ────────────────────────────────────────────────────────────────

/**
 * One named atlas bundle — a logical grouping of frames from related source
 * directories, packed onto one or more texture pages.
 *
 * The fields `key`, `label`, `width`, `height`, `sourceCount`, `imageBytes`,
 * `rgbaBytes`, `roots`, `imageUrl`, and `rects` match the shape written by
 * `build-bevy-entity-atlas-pack.mjs` exactly so that existing runtime code
 * that reads `/bevy-entity-atlases/manifest.json` needs no changes.
 */
export type AtlasEntry = {
  /** Stable machine-readable name for this atlas bundle. */
  key: string;
  /** Human-readable description of the atlas contents. */
  label?: string;

  // ── legacy scalar dimensions (single-page compatibility) ─────────────────
  /** Total atlas width (equals the first page's width for single-page atlases). */
  width: number;
  /** Total atlas height (equals the first page's height for single-page atlases). */
  height: number;

  /** Number of source frames packed into this atlas. */
  sourceCount?: number;
  /**
   * Compressed image bytes (first page — present for backward compatibility;
   * inspect `pages[].imageBytes` for multi-page atlases).
   */
  imageBytes?: number;
  /** Uncompressed RGBA bytes for the first page. */
  rgbaBytes?: number;

  /**
   * Source directory roots relative to `public/original-ui/`.
   * Matches the `roots` field written by the legacy packer.
   */
  roots?: string[];

  /**
   * Public URL for the first (or only) atlas page image.
   * Kept at the top level for backward compatibility with the runtime.
   */
  imageUrl?: string;
  /**
   * Public URL for a pre-baked raw-RGBA pixels blob for the first page.
   * Optional — allows the runtime to skip GPU upload when available.
   */
  pixelsUrl?: string;

  // ── flat rect list (backward compat with legacy manifest) ─────────────────
  /**
   * All frame rects packed across all pages, in source-key order.
   * This is the field the runtime currently iterates to build its lookup map.
   */
  rects: AtlasRect[];

  // ── multi-page detail (additive) ──────────────────────────────────────────
  /**
   * Ordered list of texture pages.  For single-page atlases this is a
   * one-element array and duplicates the scalar `width`/`height`/`imageUrl`
   * fields above.  For multi-page atlases each rect carries a `pageIndex`.
   * Absent in legacy manifests (additive field).
   */
  pages?: AtlasPage[];

  /**
   * Content hash over the atlas key + all source frame keys + page SHA-256s.
   * Changes whenever any source frame is added, removed, or modified.
   * Absent in legacy manifests (additive field).
   */
  contentHash?: string;

  // ── provenance ────────────────────────────────────────────────────────────
  /** Category this atlas belongs to: "entities", "map", or "ui". */
  category?: "entities" | "map" | "ui";
  /** Padding in pixels added around each frame during packing. */
  padding?: number;
};

// ─── top-level manifest ───────────────────────────────────────────────────────

/**
 * Top-level shape of the file written to `<outDir>/manifest.json`.
 *
 * Backward-compatible with the manifest produced by
 * `apps/web/scripts/build-bevy-entity-atlas-pack.mjs`:
 *   { schemaVersion, kind, generatedAt, atlases[], stats{} }
 *
 * The `pipeline` block is additive and absent in legacy manifests.
 */
export type AtlasManifest = {
  /**
   * Schema version for the pipeline-produced manifest.
   * Starts at 2 to distinguish from legacy (schemaVersion=1) manifests.
   * Legacy consumers that ignore unknown fields will continue to work.
   */
  schemaVersion: number;
  /** Stable kind marker for the manifest format. */
  kind: "mir2-bevy-entity-atlas-manifest";
  /** ISO-8601 timestamp of when this manifest was generated. */
  generatedAt: string;

  /** All atlases packed in this run. */
  atlases: AtlasEntry[];

  /** Aggregate statistics for the whole pack run. */
  stats: {
    sourceCount: number;
    roots: string[];
    imageBytes: number;
    rgbaBytes: number;
    /** Number of atlas pages emitted (additive — absent in legacy manifests). */
    pageCount?: number;
    /** Wall-clock milliseconds taken for the pack run. */
    durationMs?: number;
  };

  /**
   * Pipeline provenance block.  Absent in legacy manifests (additive).
   */
  pipeline?: {
    /** Pipeline CLI version string. */
    version: string;
    /** Category that was packed ("entities", "map", or "ui"). */
    category: string;
    /** Padding in pixels used during packing. */
    padding: number;
    /** Maximum atlas dimension in pixels. */
    maxSize: number;
    /** Whether this was a dry-run (no files written). */
    dryRun: boolean;
    /** Source frame root directories relative to `public/original-ui/`. */
    roots: string[];
  };
};
