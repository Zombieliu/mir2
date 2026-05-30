import type { NextConfig } from "next";

const isDevelopment = process.env.NODE_ENV === "development";
const immutableGameAssetCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=31536000, immutable";
const shortRuntimeCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=0, must-revalidate";
const clearAltSvcHeader = { key: "Alt-Svc", value: "clear" };

const nextConfig: NextConfig = {
  reactStrictMode: true,
  devIndicators: false,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  outputFileTracingExcludes: {
    "/api/asset-manifest": [
      "./public/generated/original-map-blend/**/*.png",
      "./public/original-map/**/*.png",
      "./public/original-ui/**/*.png",
      "./public/original-ui/**/*.wav",
    ],
    "/api/original-ui-meta": [
      "./public/original-ui/**/*.png",
      "./public/original-ui/**/*.wav",
    ],
    "/api/scene/crystal": [
      "./public/original-map/**/*.png",
      "./public/original-ui/**/*.png",
      "./public/original-ui/**/*.wav",
    ],
    "/qa/map-monsters": [
      "./public/original-map/**/*.png",
      "./public/original-ui/**/*.png",
      "./public/original-ui/**/*.wav",
    ],
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
};

export default nextConfig;
