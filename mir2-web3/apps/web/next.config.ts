import path from "node:path";
import { fileURLToPath } from "node:url";

import type { NextConfig } from "next";

const configDir = path.dirname(fileURLToPath(import.meta.url));
const outputFileTracingRoot = path.resolve(configDir, "../../..");

const nextConfig: NextConfig = {
  reactStrictMode: true,
  devIndicators: false,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  outputFileTracingRoot,
  outputFileTracingExcludes: {
    "/api/original-ui-meta": [
      "public/bevy-runtime/**",
      "public/debug/**",
      "public/generated/**",
      "public/original-map/**",
      "public/original-ui/**/*.png",
      "public/original-ui/**/*.wav",
      "public/original-ui/**/*.CUR",
      "public/original-ui/**/*.cur",
      "../admin-api/**",
      "../admin-web/**",
      "../game-client/**",
      "../gateway/**",
      "../simulation/**",
      "../../docs/**",
      "../../packages/game-data/**",
      "../../packages/protocol/**",
    ],
    "/api/scene/crystal": [
      "public/bevy-runtime/**",
      "public/debug/**",
      "public/generated/**",
      "public/original-map/**",
      "public/original-ui/**/*.png",
      "public/original-ui/**/*.wav",
      "public/original-ui/**/*.CUR",
      "public/original-ui/**/*.cur",
      "../admin-api/**",
      "../admin-web/**",
      "../game-client/**",
      "../gateway/**",
      "../simulation/**",
      "../../docs/**",
      "../../packages/game-data/**",
      "../../packages/protocol/**",
    ],
  },
  outputFileTracingIncludes: {
    "/api/original-ui-meta": [
      "public/original-ui/source-libraries.generated.json",
    ],
    "/api/scene/crystal": [
      "../../packages/game-data/data/generated/crystal_respawn_manifest.json",
      "../../packages/game-data/data/generated/crystal_starter_map_collision.json",
      "../../packages/game-data/data/generated/crystal_starter_map_region.json",
    ],
  },
};

export default nextConfig;
