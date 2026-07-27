import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const desktopDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(desktopDirectory, "../..");

const checks = [
  ["cargo", ["+1.89.0", "check", "-p", "mir2-gateway", "--bin", "home_local_stack_fixture"]],
  ["cargo", ["+1.89.0", "check", "-p", "mir2-gateway", "--bin", "home_player_probe"]],
  ["cargo", ["+1.89.0", "test", "-p", "mir2-gateway", "--bin", "home_enrollment_service"]],
  ["cargo", ["+1.89.0", "test", "-p", "mir2-gateway", "--bin", "home_telemetry_collector"]],
  ["cargo", ["+1.89.0", "test", "-p", "mir2-gateway", "--bin", "home_agent_supervisor"]],
  ["cargo", ["+1.89.0", "test", "-p", "mir2-gateway", "home_tunnel", "--lib"]],
  ["cargo", ["+1.89.0", "test", "-p", "mir2-gateway", "--test", "home_tunnel"]],
  [
    "cargo",
    [
      "+1.89.0",
      "test",
      "--manifest-path",
      "apps/dubhe-node-desktop/src-tauri/Cargo.toml",
    ],
  ],
  ["npm", ["--prefix", "apps/dubhe-node-desktop", "run", "build"]],
];

for (const [command, args] of checks) {
  console.log(`\n$ ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log("\nDUBHE_NODE_ACCEPTANCE_OK");
