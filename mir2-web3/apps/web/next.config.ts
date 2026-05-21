import type { NextConfig } from "next";

const isDevelopment = process.env.NODE_ENV === "development";
const immutableGameAssetCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=31536000, immutable";
const shortRuntimeCache = isDevelopment
  ? "public, max-age=0, must-revalidate"
  : "public, max-age=0, must-revalidate";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  devIndicators: false,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  outputFileTracingExcludes: {
    "/api/original-ui-meta": [
      "./public/original-ui/**/*.png",
      "./public/original-ui/**/*.wav",
    ],
    "/api/scene/crystal": [
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
    ],
  },
  async headers() {
    return [
      {
        source: "/original-ui/:path*",
        headers: [
          { key: "Cache-Control", value: immutableGameAssetCache },
          { key: "X-Mir2-Asset-Cache", value: "original-ui" },
        ],
      },
      {
        source: "/original-map/:path*",
        headers: [
          { key: "Cache-Control", value: immutableGameAssetCache },
          { key: "X-Mir2-Asset-Cache", value: "original-map" },
        ],
      },
      {
        source: "/bevy-runtime/:path*",
        headers: [
          { key: "Cache-Control", value: shortRuntimeCache },
          { key: "X-Mir2-Asset-Cache", value: "bevy-runtime" },
        ],
      },
      {
        source: "/api/asset-manifest",
        headers: [
          { key: "Cache-Control", value: shortRuntimeCache },
          { key: "X-Mir2-Asset-Cache", value: "asset-manifest" },
        ],
      },
      {
        source: "/mir2-asset-worker.js",
        headers: [
          { key: "Cache-Control", value: "public, max-age=0, must-revalidate" },
          { key: "Service-Worker-Allowed", value: "/" },
        ],
      },
    ];
  },
};

export default nextConfig;
