import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";

import { NextResponse } from "next/server";

import { ASSET_CACHE_PACKS } from "../../../lib/asset-cache-packs";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const CONTENT_HASH_LIMIT_BYTES = 8 * 1024 * 1024;
const isDevelopment = process.env.NODE_ENV === "development";

type ManifestInput = {
  name: string;
  absolutePath: string;
  required?: boolean;
};

type ManifestInputState = {
  name: string;
  exists: boolean;
  size: number | null;
  mtimeMs: number | null;
  contentSha256?: string;
};

const webRoot = process.cwd();
const projectRoot = path.resolve(webRoot, "../..");

const manifestInputs: ManifestInput[] = [
  {
    name: "original-ui-manifest",
    absolutePath: path.join(webRoot, "public/original-ui/manifest.generated.json"),
    required: true,
  },
  {
    name: "source-libraries",
    absolutePath: path.join(webRoot, "public/original-ui/source-libraries.generated.json"),
    required: true,
  },
  {
    name: "sound-index",
    absolutePath: path.join(webRoot, "public/original-ui/sound-index.generated.json"),
  },
  {
    name: "full-crystal-client-index",
    absolutePath: path.join(projectRoot, "docs/generated/assets/full-crystal-client-index.json"),
  },
  {
    name: "crystal-map-coverage",
    absolutePath: path.join(projectRoot, "docs/generated/map/latest-crystal-map-coverage.json"),
  },
  {
    name: "crystal-map-api",
    absolutePath: path.join(projectRoot, "docs/generated/map/latest-crystal-map-api.json"),
  },
  {
    name: "crystal-map-gameplay",
    absolutePath: path.join(projectRoot, "docs/generated/map/latest-crystal-map-gameplay.json"),
  },
];

export async function GET() {
  const inputs = [
    ...(await Promise.all(manifestInputs.map(readInputState))),
    createResourcePacksInputState(),
  ];
  const version = createAssetVersion(inputs);
  const remoteAssets = createRemoteAssetConfig(version);
  const response = {
    schemaVersion: 1,
    version,
    generatedAt: new Date().toISOString(),
    staticPrefixes: ["/original-ui/", "/original-map/", "/generated/original-map-blend/"],
    apiPrefixes: ["/api/scene/crystal", "/api/original-ui-meta"],
    runtimeCaches: {
      staticAssetMaxEntries: 20000,
      sceneBlueprintMaxEntries: 512,
      apiMetadataMaxEntries: 512,
    },
    remoteAssets,
    resourcePacks: ASSET_CACHE_PACKS,
    inputs,
  };

  return NextResponse.json(response, {
    headers: {
      "Cache-Control": isDevelopment
        ? "public, max-age=0, must-revalidate"
        : "public, max-age=60, stale-while-revalidate=300",
      "X-Mir2-Asset-Manifest-Version": version,
    },
  });
}

async function readInputState(input: ManifestInput): Promise<ManifestInputState> {
  try {
    const stats = await stat(input.absolutePath);
    const state: ManifestInputState = {
      name: input.name,
      exists: true,
      size: stats.size,
      mtimeMs: Math.trunc(stats.mtimeMs),
    };

    if (stats.isFile() && stats.size <= CONTENT_HASH_LIMIT_BYTES) {
      const contents = await readFile(input.absolutePath);
      state.contentSha256 = createHash("sha256").update(contents).digest("hex");
    }

    return state;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT" && !input.required) {
      return {
        name: input.name,
        exists: false,
        size: null,
        mtimeMs: null,
      };
    }
    throw error;
  }
}

function createResourcePacksInputState(): ManifestInputState {
  const contents = JSON.stringify(ASSET_CACHE_PACKS);
  return {
    name: "asset-cache-packs",
    exists: true,
    size: Buffer.byteLength(contents),
    mtimeMs: null,
    contentSha256: createHash("sha256").update(contents).digest("hex"),
  };
}

function createAssetVersion(inputs: ManifestInputState[]) {
  const hash = createHash("sha256");
  hash.update(process.env.MIR2_ASSET_CACHE_BUSTER ?? "");
  for (const input of inputs) {
    hash.update(input.name);
    hash.update(input.exists ? "1" : "0");
    hash.update(String(input.size ?? ""));
    hash.update(String(input.mtimeMs ?? ""));
    hash.update(input.contentSha256 ?? "");
  }
  return hash.digest("hex").slice(0, 16);
}

function createRemoteAssetConfig(version: string) {
  const configuredBaseUrl =
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? process.env.MIR2_ASSET_BASE_URL ?? "";
  const objectPrefixTemplate = process.env.MIR2_ASSET_OBJECT_PREFIX ?? "mir2/v/{version}";
  const objectPrefix = normalizeObjectPrefix(resolveTemplate(objectPrefixTemplate, version));
  const assetBaseUrl = normalizeAssetBaseUrl(resolveTemplate(configuredBaseUrl, version));

  return {
    enabled: Boolean(assetBaseUrl),
    assetBaseUrl: assetBaseUrl || null,
    objectPrefix,
    pathMode: "mirror-local-public-path",
    cacheKeyMode: "same-origin-request",
    corsRequired: true,
  };
}

function resolveTemplate(value: string, version: string) {
  return value.replaceAll("{version}", version);
}

function normalizeAssetBaseUrl(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return "";
  return trimmed.replace(/\/+$/, "");
}

function normalizeObjectPrefix(value: string) {
  return value.trim().replace(/^\/+|\/+$/g, "");
}
