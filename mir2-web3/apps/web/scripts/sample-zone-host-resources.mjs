import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const sshHost = requiredEnv("MIR2_RESOURCE_SAMPLE_SSH_HOST");
const sshKey = requiredEnv("MIR2_RESOURCE_SAMPLE_SSH_KEY");
const outputPath = path.resolve(requiredEnv("MIR2_RESOURCE_SAMPLE_OUT"));
const intervalMs = numberFromEnv("MIR2_RESOURCE_SAMPLE_INTERVAL_MS", 2_000);
const durationMs = numberFromEnv("MIR2_RESOURCE_SAMPLE_DURATION_MS", 0);

let stopping = false;
process.on("SIGINT", () => {
  stopping = true;
});
process.on("SIGTERM", () => {
  stopping = true;
});

await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.writeFile(outputPath, "");

const startedAt = Date.now();
let samples = 0;
while (!stopping && (durationMs === 0 || Date.now() - startedAt < durationMs)) {
  const sampleStartedAt = Date.now();
  const sample = await collectSample();
  await fs.appendFile(outputPath, `${JSON.stringify(sample)}\n`);
  samples += 1;
  const remainingMs = intervalMs - (Date.now() - sampleStartedAt);
  if (remainingMs > 0 && !stopping) {
    await delay(remainingMs);
  }
}

console.log(`Resource sampling completed: samples=${samples}, out=${outputPath}`);

async function collectSample() {
  const sampledAtMs = Date.now();
  const errors = {};
  const [gateway, zone, gatewayHealth, zoneHealth] = await Promise.all([
    remoteSystemdProperties("mir2-gateway.service").catch((error) => {
      errors.gateway = String(error?.message ?? error);
      return null;
    }),
    remoteSystemdProperties("mir2-zone-ucloud.service").catch((error) => {
      errors.zone = String(error?.message ?? error);
      return null;
    }),
    remoteJson("curl -fsS http://127.0.0.1:7110/health").catch((error) => {
      errors.gatewayHealth = String(error?.message ?? error);
      return null;
    }),
    remoteJson("curl -fsS http://127.0.0.1:17020/healthz").catch((error) => {
      errors.zoneHealth = String(error?.message ?? error);
      return null;
    }),
  ]);
  return {
    sampledAtMs,
    gateway,
    zone,
    gatewayCapacity: gatewayHealth?.capacity ?? null,
    gatewayCache: gatewayHealth?.session_cache ?? null,
    zoneHealth: zoneHealth?.health ?? null,
    zoneCheckpoint: zoneHealth?.checkpoint ?? null,
    errors,
  };
}

async function remoteSystemdProperties(service) {
  const stdout = await remoteText(
    `systemctl show ${service} -p NRestarts -p MemoryCurrent -p MemoryPeak -p CPUUsageNSec`,
  );
  return Object.fromEntries(
    stdout
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const separator = line.indexOf("=");
        const key = line.slice(0, separator);
        const raw = line.slice(separator + 1);
        const value = Number(raw);
        return [key, Number.isFinite(value) ? value : raw];
      }),
  );
}

async function remoteJson(command) {
  return JSON.parse(await remoteText(command));
}

async function remoteText(command) {
  const { stdout } = await execFileAsync(
    "ssh",
    ["-i", sshKey, sshHost, command],
    { maxBuffer: 4 * 1024 * 1024, timeout: 15_000 },
  );
  return stdout;
}

function requiredEnv(name) {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function numberFromEnv(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative number`);
  }
  return value;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
