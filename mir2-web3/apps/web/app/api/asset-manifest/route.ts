import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";

import { NextResponse } from "next/server";

import { ASSET_CACHE_PACKS } from "../../../lib/asset-cache-packs";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const CONTENT_HASH_LIMIT_BYTES = 64 * 1024 * 1024;
const isDevelopment = process.env.NODE_ENV === "development";

type ManifestInput = {
  name: string;
  absolutePath: string;
  digestMode?: "originalAssetHash" | "stableJson" | "raw";
  required?: boolean;
};

type ManifestInputState = {
  name: string;
  exists: boolean;
  size: number | null;
  assetHash?: string;
  contentSha256?: string;
  digestSource?: "assetHash" | "stableJson" | "raw";
  mtimeMs?: number | null;
};

const webRoot = /* turbopackIgnore: true */ process.cwd();
const projectRoot = path.resolve(/* turbopackIgnore: true */ webRoot, "../..");

const manifestInputs: ManifestInput[] = [
  {
    name: "original-asset-manifest",
    absolutePath: path.join(webRoot, "public/original-asset-manifest.generated.json"),
    digestMode: "originalAssetHash",
    required: !isDevelopment,
  },
  {
    name: "asset-service-worker",
    absolutePath: path.join(webRoot, "public/mir2-asset-worker.js"),
    digestMode: "raw",
    required: !isDevelopment,
  },
  {
    name: "original-ui-manifest",
    absolutePath: path.join(webRoot, "public/original-ui/manifest.generated.json"),
    digestMode: "stableJson",
    required: true,
  },
  {
    name: "source-libraries",
    absolutePath: path.join(webRoot, "public/original-ui/source-libraries.generated.json"),
    digestMode: "stableJson",
    required: true,
  },
  {
    name: "sound-index",
    absolutePath: path.join(webRoot, "public/original-ui/sound-index.generated.json"),
    digestMode: "stableJson",
  },
  {
    // Bumps the asset version when the set of committed sound bytes changes, so clients pick up
    // newly-added sounds.
    name: "present-sounds",
    absolutePath: path.join(webRoot, "lib/generated/crystal-present-sounds.generated.json"),
    digestMode: "stableJson",
  },
  {
    // Bumps the asset version when the packed entity atlas changes, busting CDN/SW caches.
    name: "bevy-entity-atlas-manifest",
    absolutePath: path.join(webRoot, "public/bevy-entity-atlases/manifest.json"),
    digestMode: "stableJson",
  },
  {
    name: "full-crystal-client-index",
    absolutePath: path.join(projectRoot, "docs/generated/assets/full-crystal-client-index.json"),
    digestMode: "stableJson",
  },
  {
    name: "crystal-map-coverage",
    absolutePath: path.join(projectRoot, "docs/generated/map/latest-crystal-map-coverage.json"),
    digestMode: "stableJson",
  },
  {
    name: "crystal-map-api",
    absolutePath: path.join(projectRoot, "docs/generated/map/latest-crystal-map-api.json"),
    digestMode: "stableJson",
  },
  {
    name: "crystal-map-gameplay",
    absolutePath: path.join(projectRoot, "docs/generated/map/latest-crystal-map-gameplay.json"),
    digestMode: "stableJson",
  },
];

export async function GET() {
  const inputs = [
    ...(await Promise.all(manifestInputs.map(readInputState))),
    createResourcePacksInputState(),
  ];
  const version = createAssetVersion(inputs);
  const remoteAssets = createRemoteAssetConfig(version);
  const cacheNamespace = createCacheNamespace(version, inputs, remoteAssets);
  const response = {
    schemaVersion: 1,
    version,
    cacheNamespace,
    versionSource: process.env.MIR2_ASSET_VERSION ? "env:MIR2_ASSET_VERSION" : "stable-input-digest",
    generatedAt: new Date().toISOString(),
    staticPrefixes: [
      "/original-ui/",
      "/original-map/",
      "/generated/original-map-blend/",
      "/bevy-entity-atlases/",
    ],
    apiPrefixes: ["/api/scene/crystal", "/api/original-ui-meta"],
    runtimeCaches: {
      staticAssetMaxEntries: 20000,
      staticCriticalMaxEntries: 3000,
      staticBackgroundMaxEntries: 6000,
      staticRuntimeMaxEntries: 16000,
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
    };

    if (stats.isFile() && stats.size <= CONTENT_HASH_LIMIT_BYTES) {
      const contents = await readFile(input.absolutePath);
      if (input.digestMode === "originalAssetHash") {
        const payload = JSON.parse(contents.toString("utf8")) as { assetHash?: unknown };
        const assetHash = typeof payload.assetHash === "string" ? payload.assetHash : "";
        if (!assetHash) {
          throw new Error(`Original asset manifest is missing assetHash: ${input.absolutePath}`);
        }
        state.assetHash = assetHash;
        state.digestSource = "assetHash";
      } else if (input.digestMode === "stableJson") {
        state.contentSha256 = createHash("sha256")
          .update(stableJsonDigestInput(contents.toString("utf8")))
          .digest("hex");
        state.digestSource = "stableJson";
      } else {
        state.contentSha256 = createHash("sha256").update(contents).digest("hex");
        state.digestSource = "raw";
      }
    }

    return state;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT" && !input.required) {
      return {
        name: input.name,
        exists: false,
        size: null,
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
    contentSha256: createHash("sha256").update(contents).digest("hex"),
    digestSource: "raw",
  };
}

function createAssetVersion(inputs: ManifestInputState[]) {
  const configuredVersion = normalizeAssetVersion(process.env.MIR2_ASSET_VERSION ?? "");
  if (configuredVersion) {
    return configuredVersion;
  }

  const hash = createHash("sha256");
  hash.update(process.env.MIR2_ASSET_CACHE_BUSTER ?? "");
  for (const input of inputs) {
    hash.update(input.name);
    hash.update(input.exists ? "1" : "0");
    if (input.name === "original-asset-manifest") {
      hash.update(input.assetHash ?? "");
    } else {
      hash.update(String(input.size ?? ""));
      hash.update(input.contentSha256 ?? "");
    }
  }
  return hash.digest("hex").slice(0, 16);
}

function createCacheNamespace(
  version: string,
  inputs: ManifestInputState[],
  remoteAssets: ReturnType<typeof createRemoteAssetConfig>,
) {
  const hash = createHash("sha256");
  hash.update("mir2-asset-cache-namespace-v2");
  hash.update(process.env.MIR2_ASSET_CACHE_BUSTER ?? "");
  hash.update(process.env.VERCEL_GIT_COMMIT_SHA ?? "");
  hash.update(version);
  hash.update(remoteAssets.assetBaseUrl ?? "");
  hash.update(remoteAssets.objectPrefix ?? "");
  for (const input of inputs) {
    hash.update(input.name);
    hash.update(input.exists ? "1" : "0");
    hash.update(String(input.size ?? ""));
    hash.update(input.assetHash ?? "");
    hash.update(input.contentSha256 ?? "");
  }
  return normalizeAssetVersion(`${version}-${hash.digest("hex").slice(0, 12)}`);
}

function stableJsonDigestInput(contents: string) {
  try {
    return stableJsonStringify(stripVolatileJsonFields(JSON.parse(contents)));
  } catch {
    return contents;
  }
}

function stripVolatileJsonFields(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(stripVolatileJsonFields);
  }
  if (value && typeof value === "object") {
    const output: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(value)) {
      if (key === "generatedAt" || key === "exportedAt" || key === "mtimeMs") {
        continue;
      }
      output[key] = stripVolatileJsonFields(child);
    }
    return output;
  }
  return value;
}

function stableJsonStringify(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableJsonStringify).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value as Record<string, unknown>)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJsonStringify((value as Record<string, unknown>)[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function normalizeAssetVersion(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return "";
  const normalized = trimmed.replace(/[^a-zA-Z0-9._-]/g, "-").replace(/-+/g, "-").replace(/^-+|-+$/g, "");
  return normalized.slice(0, 80);
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
