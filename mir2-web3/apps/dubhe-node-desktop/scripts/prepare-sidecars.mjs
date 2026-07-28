import { chmod, copyFile, mkdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const appDirectory = path.resolve(scriptDirectory, "..");
const repositoryRoot = path.resolve(appDirectory, "../..");
const rustTarget =
  process.env.DUBHE_NODE_TARGET?.trim() || detectRustHost();
const executableSuffix = rustTarget.includes("windows") ? ".exe" : "";
const binaries = [
  "home_agent",
  "home_agent_launcher",
  "home_agent_supervisor",
  "zone_host",
];

run("cargo", [
  "+1.89.0",
  "build",
  "--locked",
  "--release",
  "--target",
  rustTarget,
  "-p",
  "mir2-gateway",
  ...binaries.flatMap((binary) => ["--bin", binary]),
]);

const outputDirectory = path.join(appDirectory, "src-tauri", "binaries");
await mkdir(outputDirectory, { recursive: true });
for (const binary of binaries) {
  const source = path.join(
    repositoryRoot,
    "target",
    rustTarget,
    "release",
    `${binary}${executableSuffix}`,
  );
  const destination = path.join(
    outputDirectory,
    `${binary}-${rustTarget}${executableSuffix}`,
  );
  await copyFile(source, destination);
  if (!executableSuffix) await chmod(destination, 0o755);
}

console.log(`Dubhe Node sidecars prepared for ${rustTarget}`);

function detectRustHost() {
  const result = spawnSync("rustc", ["+1.89.0", "-vV"], {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(result.stderr || "failed to resolve Rust host target");
  }
  const host = result.stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("host: "))
    ?.slice("host: ".length)
    .trim();
  if (!host) throw new Error("rustc did not report a host target");
  return host;
}

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw new Error(`${command} failed with exit code ${result.status ?? "unknown"}`);
  }
}
