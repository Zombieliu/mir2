import type { NextConfig } from "next";

const isDevelopment = process.env.NODE_ENV === "development";
const immutableGameAssetCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=31536000, immutable";
const shortRuntimeCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=0, must-revalidate";
const clearAltSvcHeader = { key: "Alt-Svc", value: "clear" };

// Heavy static game media (PNG/WAV/WASM/cursor) is served by Vercel static
// output and the Cloudflare Worker + R2 origin; these server functions only read
// small JSON/meta at runtime, never the media itself. Excluding the media from
// output file tracing keeps each serverless function under Vercel's size limit
// (the /api/scene/crystal function was reported ~819MB before this). The earlier
// excludes missed public/bevy-runtime/**/*.wasm (~106MB) and other public media.
//
// Patterns are matched relative to Next's outputFileTracingRoot. Locally that is
// apps/web, so "./public/**" matches; but Vercel's monorepo build sets the root
// higher (this app imports ../game-client), tracing files as
// mir2-web3/apps/web/public/..., which "./public/**" does NOT match — that is why
// the function stayed huge on Vercel while local `next build` traced ~11.5MB. The
// "**/public/**" patterns match regardless of the traced prefix so the excludes
// apply in both environments. JSON/meta under public/ is intentionally NOT
// excluded because /api/original-ui-meta reads it at runtime.
const heavyPublicMediaTracingExcludes = [
  "./public/**/*.png",
  "./public/**/*.wav",
  "./public/**/*.wasm",
  "./public/**/*.CUR",
  "**/public/**/*.png",
  "**/public/**/*.wav",
  "**/public/**/*.wasm",
  "**/public/**/*.CUR",
];

const nextConfig: NextConfig = {
  reactStrictMode: true,
  devIndicators: false,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  outputFileTracingExcludes: {
    "/api/asset-manifest": heavyPublicMediaTracingExcludes,
    "/api/original-ui-meta": heavyPublicMediaTracingExcludes,
    "/api/scene/crystal": heavyPublicMediaTracingExcludes,
    "/api/qa/map-monster-scenes": heavyPublicMediaTracingExcludes,
    "/qa/map-monsters": heavyPublicMediaTracingExcludes,
  },
  outputFileTracingIncludes: {
    "/api/scene/crystal": [
      "./lib/generated/crystal_respawn_manifest.json",
      "./lib/generated/crystal_starter_map_collision.json",
      "./lib/generated/crystal_starter_map_region.json",
      "./lib/generated/crystal-map-library-meta/**/*.json.gz",
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
        source: "/original-ui/:path*",
        headers: [
          { key: "Cache-Control", value: immutableGameAssetCache },
          { key: "X-Mir2-Asset-Cache", value: "original-ui" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/original-map/:path*",
        headers: [
          { key: "Cache-Control", value: immutableGameAssetCache },
          { key: "X-Mir2-Asset-Cache", value: "original-map" },
          clearAltSvcHeader,
        ],
      },
      {
        source: "/generated/original-map-blend/:path*",
        headers: [
          { key: "Cache-Control", value: immutableGameAssetCache },
          { key: "X-Mir2-Asset-Cache", value: "original-map-blend" },
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
  // LOCAL-DEV CONVENIENCE: same-origin R2 proxy. Set `MIR2_R2_PROXY_BASE` (e.g.
  // https://mir2.obelisk.build) and game assets the local checkout doesn't have
  // (the ~156k uncovered map tiles, R2-only sprites/sounds) are proxied through
  // THIS origin — so a localhost build is same-origin with its assets and the
  // browser never hits a cross-origin CORS wall (R2 sends no Access-Control-
  // Allow-Origin). `fallback` runs only after the filesystem/public + pages
  // miss, so committed/downloaded assets are still served locally. Gated on the
  // env var, so production (assets served same-origin already) is untouched.
  async rewrites() {
    const proxyBase = process.env.MIR2_R2_PROXY_BASE?.replace(/\/+$/, "");
    if (!proxyBase) {
      return [];
    }
    const assetPrefixes = ["original-map", "original-ui", "generated", "bevy-entity-atlases", "Sound"];
    return {
      fallback: assetPrefixes.map((prefix) => ({
        source: `/${prefix}/:path*`,
        destination: `${proxyBase}/${prefix}/:path*`,
      })),
    };
  },
};

export default nextConfig;
