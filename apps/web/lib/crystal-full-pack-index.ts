export const CRYSTAL_FULL_PACK_SCHEMA_VERSION = 1 as const;
export const CRYSTAL_FULL_PACK_INDEX_KIND = "mir2-crystal-full-pack-index" as const;
export const CRYSTAL_LIBRARY_PACK_KIND = "mir2-crystal-library-pack" as const;
export const CRYSTAL_FULL_PACK_INDEX_URL = "/generated/crystal-packs/full/index.json";

export type CrystalFullPackCountSummary = {
  frameSlotCount: number;
  drawableFrameCount: number;
  noDrawFrameCount: number;
  maskFrameCount: number;
  pageCount: number;
  rectCount: number;
};

export type CrystalFullPackLibraryRecord = CrystalFullPackCountSummary & {
  libraryKey: string;
  sourceSha256: string;
  shardUrl: string;
};

export type CrystalFullPackIndexDocument = {
  schemaVersion: typeof CRYSTAL_FULL_PACK_SCHEMA_VERSION;
  kind: typeof CRYSTAL_FULL_PACK_INDEX_KIND;
  sourceContentHash: string;
  libraries: CrystalFullPackLibraryRecord[];
  summary: CrystalFullPackCountSummary & {
    libraryCount: number;
  };
};

export type CrystalLibraryPackRect = {
  key: string;
  x: number;
  y: number;
  width: number;
  height: number;
  sourceKind: "image" | "mask";
};

export type CrystalLibraryPackPage = {
  key: string;
  sha256?: string;
  width: number;
  height: number;
  imageUrl: string;
  rects: CrystalLibraryPackRect[];
};

type CrystalLibraryPackFrameGeometry = {
  width?: number;
  height?: number;
  x?: number;
  y?: number;
  shadowX?: number;
  shadowY?: number;
  shadow?: number;
  maskWidth?: number | null;
  maskHeight?: number | null;
  maskX?: number | null;
  maskY?: number | null;
};

export type CrystalLibraryPackNoDrawFrame = CrystalLibraryPackFrameGeometry & {
  index: number;
  noDraw: true;
  pageKey?: null;
  rectKey?: null;
  imageUrl?: null;
  maskPageKey?: null;
  maskRectKey?: null;
  maskImageUrl?: null;
};

export type CrystalLibraryPackDrawableFrame = CrystalLibraryPackFrameGeometry & {
  index: number;
  noDraw: false;
  pageKey: string;
  rectKey: string;
  imageUrl?: string;
  maskPageKey?: string | null;
  maskRectKey?: string | null;
  maskImageUrl?: string | null;
};

export type CrystalLibraryPackFrame = CrystalLibraryPackNoDrawFrame | CrystalLibraryPackDrawableFrame;

export type CrystalLibraryPackDocument = {
  schemaVersion: typeof CRYSTAL_FULL_PACK_SCHEMA_VERSION;
  kind: typeof CRYSTAL_LIBRARY_PACK_KIND;
  sourceContentHash: string;
  libraryKey: string;
  sourceSha256: string;
  shardUrl: string;
  frameSlotCount: number;
  pages: CrystalLibraryPackPage[];
  frames: CrystalLibraryPackFrame[];
  summary: CrystalFullPackCountSummary;
};

export type CrystalResolvedPackRect = {
  page: CrystalLibraryPackPage;
  rect: CrystalLibraryPackRect;
  imageUrl: string;
};

export type CrystalResolvedFullPackFrame = {
  libraryKey: string;
  frameIndex: number;
  noDraw: boolean;
  frame: CrystalLibraryPackFrame;
  image: CrystalResolvedPackRect | null;
  mask: CrystalResolvedPackRect | null;
};

export type CrystalFullPackJsonResponse = {
  ok: boolean;
  status?: number;
  json: () => Promise<unknown>;
};

export type CrystalFullPackFetcher = (
  url: string,
  init?: { cache?: "force-cache" | "no-cache" },
) => Promise<CrystalFullPackJsonResponse>;

export type CrystalFullPackLoadOptions = {
  indexUrl?: string;
  fetcher?: CrystalFullPackFetcher;
};

export type CrystalLibraryPackValidationContext = {
  root?: CrystalFullPackIndexDocument;
  record?: CrystalFullPackLibraryRecord;
  indexUrl?: string;
  fetchedUrl?: string;
};

export class CrystalFullPackValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CrystalFullPackValidationError";
  }
}

type RectLocation = {
  page: CrystalLibraryPackPage;
  rect: CrystalLibraryPackRect;
};

const HASH_PATTERN = /^[a-f0-9]{64}$/;
const URL_VALIDATION_ORIGIN = "https://mir2.invalid/";

let rootIndexPromise: Promise<CrystalFullPackRuntimeIndex> | null = null;
let resolvedRootIndex: CrystalFullPackRuntimeIndex | null = null;
let cacheGeneration = 0;

/**
 * Crystal library keys are slash-normalized, have an optional .Lib suffix removed,
 * and retain source casing. Runtime lookup is case-insensitive because the source
 * client resolves these paths on Windows.
 */
export function normalizeCrystalFullPackLibraryKey(libraryKey: string): string {
  if (typeof libraryKey !== "string") {
    throw new CrystalFullPackValidationError("libraryKey must be a string");
  }

  const normalized = libraryKey
    .trim()
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/")
    .replace(/\.lib$/i, "");

  if (
    !normalized ||
    normalized.includes("\0") ||
    normalized.includes("?") ||
    normalized.includes("#") ||
    normalized.split("/").some((segment) => segment === "." || segment === "..")
  ) {
    throw new CrystalFullPackValidationError(`Invalid Crystal library key: ${libraryKey}`);
  }
  return normalized;
}

export function validateCrystalFullPackIndex(value: unknown): value is CrystalFullPackIndexDocument {
  assertRecord(value, "root index");
  assertEqual(value.kind, CRYSTAL_FULL_PACK_INDEX_KIND, "root index.kind");
  assertEqual(value.schemaVersion, CRYSTAL_FULL_PACK_SCHEMA_VERSION, "root index.schemaVersion");
  assertSha256(value.sourceContentHash, "root index.sourceContentHash");
  assertRecord(value.summary, "root index.summary");
  assertCountSummary(value.summary, "root index.summary");
  assertInteger(value.summary.libraryCount, "root index.summary.libraryCount", 0);

  if (!Array.isArray(value.libraries) || value.libraries.length === 0) {
    fail("root index.libraries must be a non-empty array");
  }

  const normalizedKeys = new Set<string>();
  const shardUrls = new Set<string>();
  const aggregate = emptyCountSummary();

  for (const [index, rawRecord] of value.libraries.entries()) {
    const label = `root index.libraries[${index}]`;
    assertRecord(rawRecord, label);
    assertNonEmptyString(rawRecord.libraryKey, `${label}.libraryKey`);
    const normalizedKey = normalizeCrystalFullPackLibraryKey(rawRecord.libraryKey);
    if (normalizedKey !== rawRecord.libraryKey) {
      fail(`${label}.libraryKey must be normalized as ${normalizedKey}`);
    }
    const lookupKey = foldedLibraryKey(normalizedKey);
    if (normalizedKeys.has(lookupKey)) {
      fail(`Duplicate normalized Crystal library key: ${normalizedKey}`);
    }
    normalizedKeys.add(lookupKey);

    assertSha256(rawRecord.sourceSha256, `${label}.sourceSha256`);
    assertAssetUrl(rawRecord.shardUrl, `${label}.shardUrl`);
    const normalizedUrl = canonicalUrl(rawRecord.shardUrl, CRYSTAL_FULL_PACK_INDEX_URL);
    if (shardUrls.has(normalizedUrl)) {
      fail(`Duplicate Crystal library shard URL: ${rawRecord.shardUrl}`);
    }
    shardUrls.add(normalizedUrl);

    assertCountSummary(rawRecord, label);
    addCounts(aggregate, rawRecord as unknown as CrystalFullPackCountSummary);
  }

  assertEqual(value.summary.libraryCount, value.libraries.length, "root index.summary.libraryCount");
  assertCountsEqual(value.summary, aggregate, "root index.summary");
  return true;
}

export function validateCrystalLibraryPack(
  value: unknown,
  context: CrystalLibraryPackValidationContext = {},
): value is CrystalLibraryPackDocument {
  assertRecord(value, "library shard");
  assertEqual(value.kind, CRYSTAL_LIBRARY_PACK_KIND, "library shard.kind");
  assertEqual(value.schemaVersion, CRYSTAL_FULL_PACK_SCHEMA_VERSION, "library shard.schemaVersion");
  assertSha256(value.sourceContentHash, "library shard.sourceContentHash");
  assertNonEmptyString(value.libraryKey, "library shard.libraryKey");
  const normalizedLibraryKey = normalizeCrystalFullPackLibraryKey(value.libraryKey);
  if (normalizedLibraryKey !== value.libraryKey) {
    fail(`library shard.libraryKey must be normalized as ${normalizedLibraryKey}`);
  }
  assertSha256(value.sourceSha256, "library shard.sourceSha256");
  assertAssetUrl(value.shardUrl, "library shard.shardUrl");
  assertInteger(value.frameSlotCount, "library shard.frameSlotCount", 0);
  assertRecord(value.summary, "library shard.summary");
  assertCountSummary(value.summary, "library shard.summary");
  assertEqual(value.frameSlotCount, value.summary.frameSlotCount, "library shard.frameSlotCount");

  if (!Array.isArray(value.pages)) fail("library shard.pages must be an array");
  if (!Array.isArray(value.frames)) fail("library shard.frames must be an array");

  const pagesByKey = new Map<string, CrystalLibraryPackPage>();
  const pageUrls = new Set<string>();
  const rectsByKey = new Map<string, RectLocation>();
  let rectCount = 0;

  for (const [pageIndex, rawPage] of value.pages.entries()) {
    const pageLabel = `library shard.pages[${pageIndex}]`;
    assertRecord(rawPage, pageLabel);
    assertNonEmptyString(rawPage.key, `${pageLabel}.key`);
    if (pagesByKey.has(rawPage.key)) fail(`Duplicate library page key: ${rawPage.key}`);
    assertInteger(rawPage.width, `${pageLabel}.width`, 1);
    assertInteger(rawPage.height, `${pageLabel}.height`, 1);
    assertAssetUrl(rawPage.imageUrl, `${pageLabel}.imageUrl`);
    const normalizedPageUrl = canonicalUrl(rawPage.imageUrl, value.shardUrl);
    if (pageUrls.has(normalizedPageUrl)) fail(`Duplicate library page URL: ${rawPage.imageUrl}`);
    pageUrls.add(normalizedPageUrl);
    validateOptionalPageHash(rawPage, pageLabel);
    if (!Array.isArray(rawPage.rects) || rawPage.rects.length === 0) {
      fail(`${pageLabel}.rects must be a non-empty array`);
    }

    const page = rawPage as unknown as CrystalLibraryPackPage;
    pagesByKey.set(page.key, page);
    for (const [rectIndex, rawRect] of rawPage.rects.entries()) {
      const rectLabel = `${pageLabel}.rects[${rectIndex}]`;
      assertRecord(rawRect, rectLabel);
      assertNonEmptyString(rawRect.key, `${rectLabel}.key`);
      if (rectsByKey.has(rawRect.key)) fail(`Duplicate library rect: ${rawRect.key}`);
      assertInteger(rawRect.x, `${rectLabel}.x`, 0);
      assertInteger(rawRect.y, `${rectLabel}.y`, 0);
      assertInteger(rawRect.width, `${rectLabel}.width`, 1);
      assertInteger(rawRect.height, `${rectLabel}.height`, 1);
      if (rawRect.sourceKind !== "image" && rawRect.sourceKind !== "mask") {
        fail(`${rectLabel}.sourceKind must be image or mask`);
      }
      if (rawRect.x + rawRect.width > rawPage.width || rawRect.y + rawRect.height > rawPage.height) {
        fail(`Library rect ${rawRect.key} exceeds page ${rawPage.key}`);
      }
      rectsByKey.set(rawRect.key, { page, rect: rawRect as unknown as CrystalLibraryPackRect });
      rectCount += 1;
    }
  }

  const referencedRects = new Set<string>();
  let drawableFrameCount = 0;
  let noDrawFrameCount = 0;
  let maskFrameCount = 0;

  if (value.frames.length !== value.frameSlotCount) {
    fail(`library shard.frames length ${value.frames.length} does not match frameSlotCount ${value.frameSlotCount}`);
  }

  for (const [position, rawFrame] of value.frames.entries()) {
    const frameLabel = `library shard.frames[${position}]`;
    assertRecord(rawFrame, frameLabel);
    assertInteger(rawFrame.index, `${frameLabel}.index`, 0);
    if (rawFrame.index !== position) {
      fail(`${frameLabel}.index must equal its frame slot ${position}`);
    }
    if (typeof rawFrame.noDraw !== "boolean") fail(`${frameLabel}.noDraw must be a boolean`);
    validateOptionalFrameGeometry(rawFrame, frameLabel);

    if (rawFrame.noDraw) {
      assertNoReference(rawFrame.pageKey, `${frameLabel}.pageKey`);
      assertNoReference(rawFrame.rectKey, `${frameLabel}.rectKey`);
      assertNoReference(rawFrame.imageUrl, `${frameLabel}.imageUrl`);
      assertNoReference(rawFrame.maskPageKey, `${frameLabel}.maskPageKey`);
      assertNoReference(rawFrame.maskRectKey, `${frameLabel}.maskRectKey`);
      assertNoReference(rawFrame.maskImageUrl, `${frameLabel}.maskImageUrl`);
      noDrawFrameCount += 1;
      continue;
    }

    assertNonEmptyString(rawFrame.pageKey, `${frameLabel}.pageKey`);
    assertNonEmptyString(rawFrame.rectKey, `${frameLabel}.rectKey`);
    const imageLocation = validateRectReference(
      rawFrame.pageKey,
      rawFrame.rectKey,
      "image",
      pagesByKey,
      rectsByKey,
      referencedRects,
      frameLabel,
    );
    validateOptionalResolvedUrl(rawFrame.imageUrl, imageLocation.page.imageUrl, value.shardUrl, `${frameLabel}.imageUrl`);
    validateOptionalFrameDimensions(rawFrame, imageLocation.rect, frameLabel);
    drawableFrameCount += 1;

    const hasMaskPage = rawFrame.maskPageKey !== null && rawFrame.maskPageKey !== undefined;
    const hasMaskRect = rawFrame.maskRectKey !== null && rawFrame.maskRectKey !== undefined;
    if (hasMaskPage !== hasMaskRect) {
      fail(`${frameLabel} must provide both maskPageKey and maskRectKey`);
    }
    if (hasMaskPage) {
      assertNonEmptyString(rawFrame.maskPageKey, `${frameLabel}.maskPageKey`);
      assertNonEmptyString(rawFrame.maskRectKey, `${frameLabel}.maskRectKey`);
      const maskLocation = validateRectReference(
        rawFrame.maskPageKey,
        rawFrame.maskRectKey,
        "mask",
        pagesByKey,
        rectsByKey,
        referencedRects,
        frameLabel,
      );
      validateOptionalResolvedUrl(
        rawFrame.maskImageUrl,
        maskLocation.page.imageUrl,
        value.shardUrl,
        `${frameLabel}.maskImageUrl`,
      );
      validateOptionalMaskDimensions(rawFrame, maskLocation.rect, frameLabel);
      maskFrameCount += 1;
    } else {
      assertNoReference(rawFrame.maskImageUrl, `${frameLabel}.maskImageUrl`);
    }
  }

  if (referencedRects.size !== rectsByKey.size) {
    const orphan = [...rectsByKey.keys()].find((key) => !referencedRects.has(key));
    fail(`Unreferenced library rect: ${orphan ?? "unknown"}`);
  }

  const actualCounts: CrystalFullPackCountSummary = {
    frameSlotCount: value.frames.length,
    drawableFrameCount,
    noDrawFrameCount,
    maskFrameCount,
    pageCount: value.pages.length,
    rectCount,
  };
  assertCountsEqual(value.summary, actualCounts, "library shard.summary");
  validateShardAgainstContext(value as unknown as CrystalLibraryPackDocument, context);
  return true;
}

export class CrystalLibraryPackRuntimeIndex {
  readonly document: CrystalLibraryPackDocument;

  private readonly pagesByKey = new Map<string, CrystalLibraryPackPage>();
  private readonly rectsByKey = new Map<string, CrystalLibraryPackRect>();

  constructor(document: CrystalLibraryPackDocument) {
    this.document = document;
    for (const page of document.pages) {
      this.pagesByKey.set(page.key, page);
      for (const rect of page.rects) this.rectsByKey.set(rect.key, rect);
    }
  }

  getFrameRecord(frameIndex: number): CrystalLibraryPackFrame | null {
    if (!Number.isSafeInteger(frameIndex) || frameIndex < 0 || frameIndex >= this.document.frames.length) {
      return null;
    }
    return this.document.frames[frameIndex] ?? null;
  }

  resolveFrame(frameIndex: number): CrystalResolvedFullPackFrame | null {
    const frame = this.getFrameRecord(frameIndex);
    if (!frame) return null;
    if (frame.noDraw) {
      return {
        libraryKey: this.document.libraryKey,
        frameIndex,
        noDraw: true,
        frame,
        image: null,
        mask: null,
      };
    }

    const imagePage = this.pagesByKey.get(frame.pageKey);
    const imageRect = this.rectsByKey.get(frame.rectKey);
    if (!imagePage || !imageRect) {
      throw new CrystalFullPackValidationError(`Validated image reference disappeared for ${this.document.libraryKey}#${frameIndex}`);
    }

    let mask: CrystalResolvedPackRect | null = null;
    if (frame.maskPageKey && frame.maskRectKey) {
      const maskPage = this.pagesByKey.get(frame.maskPageKey);
      const maskRect = this.rectsByKey.get(frame.maskRectKey);
      if (!maskPage || !maskRect) {
        throw new CrystalFullPackValidationError(`Validated mask reference disappeared for ${this.document.libraryKey}#${frameIndex}`);
      }
      mask = { page: maskPage, rect: maskRect, imageUrl: maskPage.imageUrl };
    }

    return {
      libraryKey: this.document.libraryKey,
      frameIndex,
      noDraw: false,
      frame,
      image: { page: imagePage, rect: imageRect, imageUrl: imagePage.imageUrl },
      mask,
    };
  }
}

export class CrystalFullPackRuntimeIndex {
  readonly document: CrystalFullPackIndexDocument;
  readonly indexUrl: string;

  private readonly fetcher: CrystalFullPackFetcher;
  private readonly recordsByKey = new Map<string, CrystalFullPackLibraryRecord>();
  private readonly libraryPromises = new Map<string, Promise<CrystalLibraryPackRuntimeIndex>>();

  constructor(document: CrystalFullPackIndexDocument, indexUrl: string, fetcher: CrystalFullPackFetcher) {
    this.document = document;
    this.indexUrl = indexUrl;
    this.fetcher = fetcher;
    for (const record of document.libraries) {
      this.recordsByKey.set(foldedLibraryKey(record.libraryKey), record);
    }
  }

  getLibraryRecord(libraryKey: string): CrystalFullPackLibraryRecord | null {
    const normalizedKey = normalizeCrystalFullPackLibraryKey(libraryKey);
    return this.recordsByKey.get(foldedLibraryKey(normalizedKey)) ?? null;
  }

  loadLibrary(libraryKey: string): Promise<CrystalLibraryPackRuntimeIndex | null> {
    const record = this.getLibraryRecord(libraryKey);
    if (!record) return Promise.resolve(null);

    const cacheKey = foldedLibraryKey(record.libraryKey);
    const cached = this.libraryPromises.get(cacheKey);
    if (cached) return cached;

    const fetchUrl = resolveRuntimeUrl(record.shardUrl, this.indexUrl);
    let pending: Promise<CrystalLibraryPackRuntimeIndex>;
    pending = fetchJson(this.fetcher, fetchUrl, `Crystal library shard ${record.libraryKey}`, "force-cache")
      .then((payload) => {
        if (!validateCrystalLibraryPack(payload, {
          root: this.document,
          record,
          indexUrl: this.indexUrl,
          fetchedUrl: fetchUrl,
        })) {
          fail(`Crystal library shard validation failed for ${record.libraryKey}`);
        }
        return new CrystalLibraryPackRuntimeIndex(payload);
      })
      .catch((error) => {
        if (this.libraryPromises.get(cacheKey) === pending) this.libraryPromises.delete(cacheKey);
        throw error;
      });
    this.libraryPromises.set(cacheKey, pending);
    return pending;
  }

  async resolveFrame(libraryKey: string, frameIndex: number): Promise<CrystalResolvedFullPackFrame | null> {
    const library = await this.loadLibrary(libraryKey);
    return library?.resolveFrame(frameIndex) ?? null;
  }

  resetForTests(): void {
    this.libraryPromises.clear();
  }
}

export function loadCrystalFullPackIndex(
  options: CrystalFullPackLoadOptions = {},
): Promise<CrystalFullPackRuntimeIndex> {
  if (rootIndexPromise) return rootIndexPromise;

  const indexUrl = options.indexUrl ?? CRYSTAL_FULL_PACK_INDEX_URL;
  assertAssetUrl(indexUrl, "Crystal full-pack index URL");
  const fetcher = options.fetcher ?? defaultCrystalFullPackFetcher;
  const generation = cacheGeneration;
  let pending: Promise<CrystalFullPackRuntimeIndex>;
  pending = fetchJson(fetcher, indexUrl, "Crystal full-pack root index", "no-cache")
    .then((payload) => {
      if (!validateCrystalFullPackIndex(payload)) fail("Crystal full-pack root index validation failed");
      const runtime = new CrystalFullPackRuntimeIndex(payload, indexUrl, fetcher);
      if (cacheGeneration === generation) resolvedRootIndex = runtime;
      return runtime;
    })
    .catch((error) => {
      if (cacheGeneration === generation && rootIndexPromise === pending) rootIndexPromise = null;
      throw error;
    });
  rootIndexPromise = pending;
  return pending;
}

export function loadCrystalFullPackLibrary(
  libraryKey: string,
  options: CrystalFullPackLoadOptions = {},
): Promise<CrystalLibraryPackRuntimeIndex | null> {
  return loadCrystalFullPackIndex(options).then((index) => index.loadLibrary(libraryKey));
}

export function resolveCrystalFullPackFrame(
  libraryKey: string,
  frameIndex: number,
  options: CrystalFullPackLoadOptions = {},
): Promise<CrystalResolvedFullPackFrame | null> {
  return loadCrystalFullPackIndex(options).then((index) => index.resolveFrame(libraryKey, frameIndex));
}

export function resetCrystalFullPackIndexForTests(): void {
  cacheGeneration += 1;
  resolvedRootIndex?.resetForTests();
  resolvedRootIndex = null;
  rootIndexPromise = null;
}

function validateShardAgainstContext(
  shard: CrystalLibraryPackDocument,
  context: CrystalLibraryPackValidationContext,
): void {
  const { root, record } = context;
  const indexUrl = context.indexUrl ?? CRYSTAL_FULL_PACK_INDEX_URL;
  if (root && shard.sourceContentHash !== root.sourceContentHash) {
    fail(`library shard sourceContentHash mismatch for ${shard.libraryKey}`);
  }
  if (record) {
    if (foldedLibraryKey(shard.libraryKey) !== foldedLibraryKey(record.libraryKey)) {
      fail(`library shard libraryKey mismatch: expected ${record.libraryKey}, received ${shard.libraryKey}`);
    }
    if (shard.sourceSha256 !== record.sourceSha256) {
      fail(`library shard sourceSha256 mismatch for ${record.libraryKey}`);
    }
    if (canonicalUrl(shard.shardUrl, indexUrl) !== canonicalUrl(record.shardUrl, indexUrl)) {
      fail(`library shard shardUrl mismatch for ${record.libraryKey}`);
    }
    assertCountsEqual(shard.summary, record, `library shard/root counts for ${record.libraryKey}`);
  }
  if (context.fetchedUrl && canonicalUrl(shard.shardUrl, indexUrl) !== canonicalUrl(context.fetchedUrl, indexUrl)) {
    fail(`library shard URL does not match fetched URL for ${shard.libraryKey}`);
  }
}

function validateRectReference(
  pageKey: string,
  rectKey: string,
  sourceKind: "image" | "mask",
  pagesByKey: Map<string, CrystalLibraryPackPage>,
  rectsByKey: Map<string, RectLocation>,
  referencedRects: Set<string>,
  label: string,
): RectLocation {
  const page = pagesByKey.get(pageKey);
  if (!page) fail(`${label} references missing page ${pageKey}`);
  const location = rectsByKey.get(rectKey);
  if (!location) fail(`${label} references missing rect ${rectKey}`);
  if (location.page.key !== pageKey) {
    fail(`${label} rect ${rectKey} belongs to page ${location.page.key}, not ${pageKey}`);
  }
  if (location.rect.sourceKind !== sourceKind) {
    fail(`${label} rect ${rectKey} is ${location.rect.sourceKind}, expected ${sourceKind}`);
  }
  if (referencedRects.has(rectKey)) fail(`Library rect ${rectKey} is referenced more than once`);
  referencedRects.add(rectKey);
  return location;
}

function validateOptionalPageHash(page: Record<string, unknown>, label: string): void {
  if (page.sha256 !== undefined) assertSha256(page.sha256, `${label}.sha256`);
  if (typeof page.key === "string" && page.key.startsWith("sha256:")) {
    const keyHash = page.key.slice("sha256:".length);
    assertSha256(keyHash, `${label}.key`);
    if (page.sha256 !== keyHash) fail(`${label}.sha256 does not match its content-addressed page key`);
  }
  if (typeof page.sha256 === "string" && typeof page.imageUrl === "string") {
    const fileName = new URL(page.imageUrl, URL_VALIDATION_ORIGIN).pathname.split("/").pop() ?? "";
    const fileHash = fileName.match(/^([a-f0-9]{64})\.png$/i)?.[1]?.toLowerCase();
    if (fileHash && fileHash !== page.sha256) fail(`${label}.imageUrl hash does not match sha256`);
  }
}

function validateOptionalFrameGeometry(frame: Record<string, unknown>, label: string): void {
  const minimumDimension = frame.noDraw === true ? -32768 : 1;
  for (const field of ["width", "height"] as const) {
    if (frame[field] !== undefined) assertInteger(frame[field], `${label}.${field}`, minimumDimension);
  }
  for (const field of ["x", "y", "shadowX", "shadowY", "shadow"] as const) {
    if (frame[field] !== undefined) assertInteger(frame[field], `${label}.${field}`, Number.MIN_SAFE_INTEGER);
  }
  for (const field of ["maskWidth", "maskHeight"] as const) {
    if (frame[field] !== undefined && frame[field] !== null) assertInteger(frame[field], `${label}.${field}`, 1);
  }
  for (const field of ["maskX", "maskY"] as const) {
    if (frame[field] !== undefined && frame[field] !== null) {
      assertInteger(frame[field], `${label}.${field}`, Number.MIN_SAFE_INTEGER);
    }
  }
}

function validateOptionalFrameDimensions(
  frame: Record<string, unknown>,
  rect: CrystalLibraryPackRect,
  label: string,
): void {
  if (frame.width !== undefined && frame.width !== rect.width) fail(`${label}.width does not match image rect width`);
  if (frame.height !== undefined && frame.height !== rect.height) fail(`${label}.height does not match image rect height`);
}

function validateOptionalMaskDimensions(
  frame: Record<string, unknown>,
  rect: CrystalLibraryPackRect,
  label: string,
): void {
  if (frame.maskWidth !== undefined && frame.maskWidth !== null && frame.maskWidth !== rect.width) {
    fail(`${label}.maskWidth does not match mask rect width`);
  }
  if (frame.maskHeight !== undefined && frame.maskHeight !== null && frame.maskHeight !== rect.height) {
    fail(`${label}.maskHeight does not match mask rect height`);
  }
}

function validateOptionalResolvedUrl(
  candidate: unknown,
  expected: string,
  baseUrl: string,
  label: string,
): void {
  if (candidate === undefined || candidate === null) return;
  assertAssetUrl(candidate, label);
  if (canonicalUrl(candidate, baseUrl) !== canonicalUrl(expected, baseUrl)) {
    fail(`${label} does not match its referenced page URL`);
  }
}

function assertCountSummary(
  value: Record<string, unknown>,
  label: string,
): asserts value is Record<string, unknown> & CrystalFullPackCountSummary {
  assertInteger(value.frameSlotCount, `${label}.frameSlotCount`, 0);
  assertInteger(value.drawableFrameCount, `${label}.drawableFrameCount`, 0);
  assertInteger(value.noDrawFrameCount, `${label}.noDrawFrameCount`, 0);
  assertInteger(value.maskFrameCount, `${label}.maskFrameCount`, 0);
  assertInteger(value.pageCount, `${label}.pageCount`, 0);
  assertInteger(value.rectCount, `${label}.rectCount`, 0);
  if (value.frameSlotCount !== value.drawableFrameCount + value.noDrawFrameCount) {
    fail(`${label} frame counts do not add up to frameSlotCount`);
  }
  if (value.maskFrameCount > value.drawableFrameCount) {
    fail(`${label}.maskFrameCount cannot exceed drawableFrameCount`);
  }
  if (value.rectCount !== value.drawableFrameCount + value.maskFrameCount) {
    fail(`${label}.rectCount must equal drawableFrameCount + maskFrameCount`);
  }
  if (value.rectCount > 0 && value.pageCount === 0) {
    fail(`${label}.pageCount must be positive when rects are present`);
  }
  if (value.pageCount > value.rectCount) {
    fail(`${label}.pageCount cannot exceed rectCount`);
  }
}

function assertCountsEqual(
  actual: Record<string, unknown>,
  expected: Record<string, unknown>,
  label: string,
): void {
  for (const field of [
    "frameSlotCount",
    "drawableFrameCount",
    "noDrawFrameCount",
    "maskFrameCount",
    "pageCount",
    "rectCount",
  ] as const) {
    if (actual[field] !== expected[field]) {
      fail(`${label}.${field} mismatch: expected ${expected[field]}, received ${actual[field]}`);
    }
  }
}

function emptyCountSummary(): CrystalFullPackCountSummary {
  return {
    frameSlotCount: 0,
    drawableFrameCount: 0,
    noDrawFrameCount: 0,
    maskFrameCount: 0,
    pageCount: 0,
    rectCount: 0,
  };
}

function addCounts(target: CrystalFullPackCountSummary, source: CrystalFullPackCountSummary): void {
  target.frameSlotCount += source.frameSlotCount;
  target.drawableFrameCount += source.drawableFrameCount;
  target.noDrawFrameCount += source.noDrawFrameCount;
  target.maskFrameCount += source.maskFrameCount;
  target.pageCount += source.pageCount;
  target.rectCount += source.rectCount;
}

function foldedLibraryKey(libraryKey: string): string {
  return normalizeCrystalFullPackLibraryKey(libraryKey).toLowerCase();
}

function assertRecord(value: unknown, label: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
}

function assertNonEmptyString(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    fail(`${label} must be a non-empty string`);
  }
}

function assertInteger(value: unknown, label: string, minimum: number): asserts value is number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum) {
    fail(`${label} must be a safe integer >= ${minimum}`);
  }
}

function assertSha256(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !HASH_PATTERN.test(value)) {
    fail(`${label} must be a lowercase SHA-256 hex digest`);
  }
}

function assertEqual(actual: unknown, expected: unknown, label: string): void {
  if (actual !== expected) fail(`${label} must equal ${String(expected)}`);
}

function assertNoReference(value: unknown, label: string): void {
  if (value !== undefined && value !== null) fail(`${label} must be absent for a noDraw frame`);
}

function assertAssetUrl(value: unknown, label: string): asserts value is string {
  assertNonEmptyString(value, label);
  if (value.includes("\\") || /[\u0000-\u001f]/.test(value)) fail(`${label} is not a valid asset URL`);
  let parsed: URL;
  try {
    parsed = new URL(value, URL_VALIDATION_ORIGIN);
  } catch {
    fail(`${label} is not a valid asset URL`);
  }
  if ((parsed.protocol !== "http:" && parsed.protocol !== "https:") || parsed.username || parsed.password || parsed.hash) {
    fail(`${label} must be an HTTP(S) asset URL without credentials or a fragment`);
  }
  for (const rawSegment of parsed.pathname.split("/")) {
    let segment: string;
    try {
      segment = decodeURIComponent(rawSegment);
    } catch {
      fail(`${label} contains invalid URL encoding`);
    }
    if (segment === "." || segment === "..") fail(`${label} must not contain path traversal`);
  }
}

function canonicalUrl(value: string, baseUrl: string): string {
  const base = new URL(baseUrl, URL_VALIDATION_ORIGIN);
  return new URL(value, base).href;
}

function resolveRuntimeUrl(value: string, baseUrl: string): string {
  const resolved = new URL(value, new URL(baseUrl, URL_VALIDATION_ORIGIN));
  const baseIsAbsolute = /^[a-z][a-z0-9+.-]*:/i.test(baseUrl);
  if (baseIsAbsolute || resolved.origin !== new URL(URL_VALIDATION_ORIGIN).origin) return resolved.href;
  return `${resolved.pathname}${resolved.search}`;
}

function fetchJson(
  fetcher: CrystalFullPackFetcher,
  url: string,
  label: string,
  cache: "force-cache" | "no-cache",
): Promise<unknown> {
  return Promise.resolve()
    .then(() => fetcher(url, { cache }))
    .then((response) => {
      if (!response || typeof response.ok !== "boolean" || typeof response.json !== "function") {
        throw new Error(`${label} returned an invalid fetch response`);
      }
      if (!response.ok) {
        const status = typeof response.status === "number" ? ` (${response.status})` : "";
        throw new Error(`${label} request failed${status}: ${url}`);
      }
      return response.json();
    });
}

const defaultCrystalFullPackFetcher: CrystalFullPackFetcher = (url, init) => {
  const fetcher = (globalThis as unknown as { fetch?: CrystalFullPackFetcher }).fetch;
  if (!fetcher) return Promise.reject(new Error("Global fetch is unavailable; provide a CrystalFullPackFetcher"));
  return fetcher(url, init);
};

function fail(message: string): never {
  throw new CrystalFullPackValidationError(message);
}
