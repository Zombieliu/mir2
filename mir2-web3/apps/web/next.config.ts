import type { NextConfig } from "next";
import { lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { rewriteStandaloneFullPackTrace } from "./lib/standalone-full-pack-tracing";
import { loadProductionAssetConfig, productionAssetConfigName } from "./scripts/lib/production-asset-config.mjs";

const isDevelopment = process.env.NODE_ENV === "development";
const isStandaloneProduction = process.env.MIR2_NEXT_STANDALONE === "1" && !isDevelopment;
const isVercelProduction =
  process.env.VERCEL === "1" && process.env.VERCEL_ENV === "production";
const configDir = path.dirname(fileURLToPath(import.meta.url));
// Next may update the selected tsconfig with fresh distDir type globs.
// An explicit build copy must stay in this Web root; normal builds keep Next defaults.
const buildTsconfigPath = process.env.MIR2_NEXT_TSCONFIG_PATH;
if (buildTsconfigPath !== undefined) {
  const parts = buildTsconfigPath.split("/");
  const invalidPath = () => new Error(
    "MIR2_NEXT_TSCONFIG_PATH must name an existing regular .json file within the Web root using a relative path without links or parent traversal.",
  );
  if (
    !buildTsconfigPath.endsWith(".json") ||
    parts.some((part) => !/^[A-Za-z0-9_.-]+$/.test(part) || part === "." || part === "..")
  ) {
    throw invalidPath();
  }
  let entryPath = configDir;
  for (const [index, part] of parts.entries()) {
    entryPath = path.join(entryPath, part);
    const entry = lstatSync(entryPath);
    const isFile = index === parts.length - 1;
    if (entry.isSymbolicLink() || (isFile ? !entry.isFile() || entry.nlink !== 1 : !entry.isDirectory())) {
      throw invalidPath();
    }
  }
}
const monorepoRoot = path.resolve(configDir, "../../..");
// Reject arbitrary path input even outside hosted production; only the two
// reviewed public profiles can be selected by the build environment.
productionAssetConfigName();
const compiledRuntimeManifest = readFileSync(
  path.resolve(configDir, "lib/generated/bevy_runtime_version.json"), "utf8",
);
// Only the public Core release identity is captured; this is independent of Bevy.
const compiledCoreManifest = readFileSync(
  path.resolve(configDir, "lib/generated/client_core_runtime.json"), "utf8",
);
const productionAssetRelease = isVercelProduction
  ? loadProductionAssetConfig(path.resolve(configDir, "../.."))
  : null;
const bootstrapAssetCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=86400, stale-while-revalidate=604800";
const shortRuntimeCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=0, must-revalidate";
const immutableVersionedRuntimeCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=31536000, immutable";
const clearAltSvcHeader = { key: "Alt-Svc", value: "clear" };
const buildRevision =
  process.env.MIR2_DEPLOY_REVISION?.trim() ||
  process.env.VERCEL_GIT_COMMIT_SHA?.trim() ||
  "";

// The public site shares this origin with the game, while keeping its own build
// and static-chunk namespace. This is server configuration, never request input.
// Downloadable standalone clients retain their local root game entry.
const marketingProxyEnabled = process.env.MIR2_NEXT_STANDALONE !== "1";
const marketingOriginUrl = new URL(
  (marketingProxyEnabled ? process.env.MIR2_MARKETING_ORIGIN?.trim() : undefined) ||
    "https://numeron-legend-of-rebirth.vercel.app",
);
const localMarketingOrigin =
  process.env.VERCEL !== "1" &&
  marketingOriginUrl.protocol === "http:" &&
  ["127.0.0.1", "localhost"].includes(marketingOriginUrl.hostname);
// Next's destination compiler treats IPv6 literal colons as route parameters.
if (
  marketingOriginUrl.username ||
  marketingOriginUrl.password ||
  marketingOriginUrl.pathname !== "/" ||
  marketingOriginUrl.search ||
  marketingOriginUrl.hash ||
  marketingOriginUrl.hostname.includes(":") ||
  (marketingOriginUrl.protocol !== "https:" && !localMarketingOrigin) ||
  marketingOriginUrl.hostname === "mir2.obelisk.build"
) {
  throw new Error("MIR2_MARKETING_ORIGIN must be a separate HTTPS origin (loopback HTTP is allowed locally)");
}
const marketingOrigin = marketingOriginUrl.origin;
const marketingLocales = "zh-CN|en|ja|ko|zh-TW|es|pt-BR|de|fr|ru|pl|tr|th|vi|id|ar";

// Heavy static game media (PNG/WAV/WASM/cursor) is served by Vercel static
// output and the Cloudflare Worker + R2 origin; these server functions only read
// small JSON/meta at runtime, never the media itself. Excluding the media from
// output file tracing keeps each serverless function under Vercel's size limit
// (the /api/scene/crystal function was reported ~819MB before this). The earlier
// excludes missed public/bevy-runtime/**/*.wasm (~106MB) and other public media.
//
// Next joins route excludes against this Web project's directory. Its separate
// shared next-server trace uses outputFileTracingRoot. Keep the established
// route patterns for existing media rules; the standalone full-pack rule below
// applies only to the shared trace. JSON/meta under public/ is intentionally
// not generally excluded because /api/original-ui-meta reads it at runtime.
const heavyPublicMediaTracingExcludes = [
  "./public/**/*.png",
  "./public/**/*.wav",
  "./public/**/*.wasm",
  "./public/**/*.CUR",
  "**/public/**/*.png",
  "**/public/**/*.wav",
  "**/public/**/*.wasm",
  "**/public/**/*.CUR",
  "./public/generated/crystal-packs/full/**",
  "**/public/generated/crystal-packs/full/**",
];

// These source/reference trees are useful to local exporters but are not hosted
// runtime inputs. Excluding them prevents multi-gigabyte local evidence from
// being traced into serverless functions during a prebuilt monorepo deploy.
const localCrystalSourceTracingExcludes = ["../../../downloads/**/*"];
const localDiagnosticsTracingExcludes = ["../../docs/generated/**/*"];

const nextConfig: NextConfig = {
  distDir: process.env.MIR2_NEXT_DIST_DIR || ".next",
  ...(buildTsconfigPath === undefined ? {} : { typescript: { tsconfigPath: buildTsconfigPath } }),
  reactStrictMode: true,
  devIndicators: false,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  // `env` is intentionally build-time and public. A Git revision is not a
  // secret, and capturing it here keeps /version trustworthy even when a host
  // does not expose its system variables to server functions at runtime.
  env: {
    // Standalone delivery verifies the endpoint actually compiled into the UI;
    // setting a public variable only when starting Node cannot change a bundle.
    NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: process.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL?.trim() ?? "",
    MIR2_BEVY_RUNTIME_BUILD_MANIFEST: compiledRuntimeManifest,
    MIR2_CLIENT_CORE_BUILD_MANIFEST: compiledCoreManifest,
    MIR2_BUILD_REVISION: buildRevision,
    MIR2_PINNED_ASSET_VERSION: productionAssetRelease?.version ?? "",
    MIR2_PINNED_ASSET_OBJECT_PREFIX: productionAssetRelease?.objectPrefix ?? "",
    MIR2_PINNED_ASSET_BASE_URL: productionAssetRelease?.assetBaseUrl ?? "",
    MIR2_PINNED_ASSET_BROWSER_FALLBACK_BASE_URLS: JSON.stringify(
      productionAssetRelease?.browserFallbackBaseUrls ?? [],
    ),
    MIR2_PINNED_MAP_ATLAS_ENABLED:
      productionAssetRelease?.mapAtlas?.enabled === true ? "1" : "",
    MIR2_PINNED_MAP_ATLAS_VERIFIED:
      productionAssetRelease?.mapAtlas?.verified === true ? "1" : "",
    MIR2_PINNED_MAP_ATLAS_MANIFEST_PATH:
      productionAssetRelease?.mapAtlas?.manifestPath ?? "",
    MIR2_PINNED_MAP_ATLAS_CONTENT_HASH:
      productionAssetRelease?.mapAtlas?.contentHash ?? "",
    MIR2_PINNED_MAP_ATLAS_PAGE_COUNT:
      String(productionAssetRelease?.mapAtlas?.pageCount ?? ""),
    MIR2_PINNED_MAP_ATLAS_MAX_PAGE_BYTES:
      String(productionAssetRelease?.mapAtlas?.maxPageBytes ?? ""),
    MIR2_PINNED_CRYSTAL_FULL_PACK_ENABLED:
      productionAssetRelease?.fullCrystalPack?.enabled === true ? "1" : "",
    MIR2_PINNED_CRYSTAL_FULL_PACK_VERIFIED:
      productionAssetRelease?.fullCrystalPack?.verified === true ? "1" : "",
    MIR2_PINNED_CRYSTAL_FULL_PACK_PATH:
      productionAssetRelease?.fullCrystalPack?.path ?? "",
    MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH:
      productionAssetRelease?.fullCrystalPack?.contentHash ?? "",
    MIR2_PINNED_CRYSTAL_FULL_PACK_LIBRARY_COUNT:
      String(productionAssetRelease?.fullCrystalPack?.libraryCount ?? ""),
    MIR2_PINNED_CRYSTAL_FULL_PACK_PAGE_COUNT:
      String(productionAssetRelease?.fullCrystalPack?.pageCount ?? ""),
  },
  // Normal local/Vercel builds keep incremental caches. Downloadable bundles
  // opt into standalone output so only traced runtime dependencies ship.
  output: process.env.MIR2_NEXT_STANDALONE === "1" ? "standalone" : undefined,
  ...(isStandaloneProduction ? {
    compiler: {
      async runAfterProductionCompile(metadata: { projectDir: string; distDir: string }) {
        const audit = await rewriteStandaloneFullPackTrace(metadata, configDir);
        console.log(`[mir2-full-pack-trace] ${JSON.stringify(audit)}`);
      },
    },
    experimental: { parallelServerBuildTraces: false },
  } : {}),
  outputFileTracingRoot: monorepoRoot,
  outputFileTracingExcludes: {
    ...(isStandaloneProduction ? { "next-server": ["**/generated/crystal-packs/full/**"] } : {}),
    "/api/asset-manifest": heavyPublicMediaTracingExcludes,
    "/api/original-ui-meta": heavyPublicMediaTracingExcludes,
    "/api/scene/crystal": [
      ...heavyPublicMediaTracingExcludes,
      ...localCrystalSourceTracingExcludes,
    ],
    "/api/scene/collision": [
      ...heavyPublicMediaTracingExcludes,
      ...localCrystalSourceTracingExcludes,
    ],
    "/api/qa/map-monster-scenes": heavyPublicMediaTracingExcludes,
    "/qa/map-monsters": [
      ...heavyPublicMediaTracingExcludes,
      ...localCrystalSourceTracingExcludes,
    ],
    "/api/movement-diagnostics": localDiagnosticsTracingExcludes,
  },
  outputFileTracingIncludes: {
    // NFT can omit native Sharp license/version metadata. Keep each installed
    // binary package and libvips companion complete in downloadable builds.
    ...(isStandaloneProduction ? {
      "/*": ["./node_modules/@img/sharp-*/**/*"],
    } : {}),
    "/api/scene/crystal": [
      "./lib/generated/crystal_respawn_manifest.json",
      "./lib/generated/crystal_starter_map_collision.json",
      "./lib/generated/crystal_starter_map_region.json",
      "./lib/generated/crystal-map-library-meta/**/*.json.gz",
      "./lib/generated/crystal-map-pack/**/*.map.gz",
    ],
    "/api/scene/collision": [
      "./lib/generated/crystal_starter_map_collision.json",
      "./lib/generated/crystal_starter_map_region.json",
      "./lib/generated/crystal-map-pack/**/*.map.gz",
    ],
    "/api/qa/map-monster-scenes": [
      "./lib/generated/crystal_respawn_manifest.json",
    ],
    "/qa/map-monsters": [
      "./lib/generated/crystal_respawn_manifest.json",
      "./lib/generated/crystal_starter_map_collision.json",
      "./lib/generated/crystal_starter_map_region.json",
      "./lib/generated/crystal-map-library-meta/**/*.json.gz",
      "./lib/generated/crystal-map-pack/**/*.map.gz",
    ],
  },
  async headers() {
    return [
      {
        source: "/client-core/:version/:path*",
        headers: [
          { key: "Cache-Control", value: immutableVersionedRuntimeCache },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/bootstrap/:path*",
        headers: [
          { key: "Cache-Control", value: bootstrapAssetCache },
          { key: "X-Mir2-Asset-Cache", value: "bootstrap" },
          clearAltSvcHeader,
        ],
      },
      // Missing game assets may fall back to /api/r2-proxy. Leave Cache-Control
      // to the response so proxy failures keep no-store and successes immutable;
      // a path-wide header here overrides even a rewritten handler's error policy.
      {
        source: "/original-ui/:path*",
        headers: [
          { key: "X-Mir2-Asset-Cache", value: "original-ui" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/original-map/:path*",
        headers: [
          { key: "X-Mir2-Asset-Cache", value: "original-map" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/generated/original-map-blend/:path*",
        headers: [
          { key: "X-Mir2-Asset-Cache", value: "original-map-blend" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/generated/crystal-packs/full/:path*",
        headers: [
          { key: "X-Mir2-Asset-Cache", value: "crystal-full-pack" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/bevy-runtime/:path*",
        headers: [
          { key: "Cache-Control", value: shortRuntimeCache },
          { key: "X-Mir2-Asset-Cache", value: "bevy-runtime" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/bevy-runtime/v/:version/:path*",
        headers: [
          { key: "Cache-Control", value: immutableVersionedRuntimeCache },
          { key: "X-Mir2-Asset-Cache", value: "bevy-runtime-versioned" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/api/asset-manifest",
        headers: [
          { key: "Cache-Control", value: shortRuntimeCache },
          { key: "X-Mir2-Asset-Cache", value: "asset-manifest" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/mir2-asset-worker.js",
        headers: [
          { key: "Cache-Control", value: "public, max-age=0, must-revalidate" },
          { key: "Service-Worker-Allowed", value: "/" },
          clearAltSvcHeader,
        ],
      },
    ];
  },
  // Same-origin R2 fallback for local development AND standalone thin clients.
  // The fallback routes are always compiled because a standalone build receives
  // MIR2_R2_PROXY_BASE at runtime, after next.config.ts has already run.
  // Existing filesystem/public assets still win; only misses reach the handler.
  // When MIR2_R2_PROXY_BASE is unset the handler returns an ordinary 404.
  async rewrites() {
    const assetPrefixes = ["original-map", "original-ui", "generated", "bevy-entity-atlases", "Sound"];
    // Route THROUGH /api/r2-proxy (a Route Handler), NOT straight to R2: a direct
    // rewrite forwards the browser's `Referer: http://localhost:...` to R2, whose
    // hotlink protection then 403s it. The handler does its own server-side fetch
    // (Node fetch sends no Referer) so R2 returns 200.
    return {
      // proxy.ts validates immutable runtime requests before any local rewrite.
      beforeFiles: [
        ...(marketingProxyEnabled ? [
          { source: "/", destination: `${marketingOrigin}/zh-CN` },
          {
            source: `/:locale(${marketingLocales})`,
            destination: `${marketingOrigin}/:locale`,
          },
          {
            source: `/:locale(${marketingLocales})/:path+`,
            destination: `${marketingOrigin}/:locale/:path+`,
          },
          {
            source: "/marketing-assets/_next/static/:path*",
            destination: `${marketingOrigin}/marketing-assets/_next/static/:path*`,
          },
          { source: "/gameplay/:path*", destination: `${marketingOrigin}/gameplay/:path*` },
          { source: "/og.png", destination: `${marketingOrigin}/og.png` },
          { source: "/explore/:path*", destination: `${marketingOrigin}/explore/:path*` },
          ...["/api/attribution", "/api/founder-interest", "/api/game-status", "/robots.txt", "/sitemap.xml", "/icon"].map((source) => ({
            source,
            destination: `${marketingOrigin}${source}`,
          })),
        ] : []),
      ],
      fallback: assetPrefixes.map((prefix) => ({
        source: `/${prefix}/:path*`,
        destination: `/api/r2-proxy/${prefix}/:path*`,
      })),
    };
  },
};

export default nextConfig;
