import type { NextConfig } from "next";
import path from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(appRoot, "../../..");
const revision = process.env.MIR2_WALLET_LOGIN_BUILD_REVISION?.trim() ?? "";
if (!/^[a-f0-9]{40}$/.test(revision)) {
  throw new Error("MIR2_WALLET_LOGIN_BUILD_REVISION must be the full source SHA");
}

const config: NextConfig = {
  output: "standalone",
  outputFileTracingRoot: repositoryRoot,
  poweredByHeader: false,
  env: { MIR2_WALLET_LOGIN_BUILD_REVISION: revision },
  webpack(config) {
    // Shared server code lives in apps/web; this service installs its own lock.
    config.resolve.modules = [path.join(appRoot, "node_modules"), ...config.resolve.modules];
    return config;
  },
};

export default config;
