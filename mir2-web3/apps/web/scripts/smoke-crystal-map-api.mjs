import { performance } from "node:perf_hooks";
import fs from "node:fs/promises";
import path from "node:path";

const BASE_URL = process.env.MIR2_WEB_BASE_URL ?? process.argv[2] ?? "http://127.0.0.1:3002";
const OUTPUT_PATH = path.resolve(
  process.cwd(),
  "..",
  "..",
  process.env.MIR2_MAP_API_SMOKE_OUT ?? "docs/generated/map/latest-crystal-map-api.json",
);
const REQUEST_TIMEOUT_MS = Number.parseInt(process.env.MIR2_MAP_API_TIMEOUT_MS ?? "60000", 10);
const FIRST_PASS_LIMIT_MS = Number.parseInt(process.env.MIR2_MAP_API_FIRST_PASS_LIMIT_MS ?? "60000", 10);
const WARM_PASS_LIMIT_MS = Number.parseInt(process.env.MIR2_MAP_API_WARM_LIMIT_MS ?? "2000", 10);

const MAPS = [
  { map: "0", x: 200, y: 200 },
  { map: "1", x: 200, y: 200 },
  { map: "2", x: 200, y: 200 },
  { map: "n0", x: 200, y: 200 },
  { map: "HF1", x: 200, y: 200 },
  { map: "HF2", x: 200, y: 200 },
  { map: "HF3", x: 200, y: 200 },
  { map: "D1801", x: 200, y: 200 },
  { map: "HKR", x: 200, y: 200 },
];

const CRITICAL_SCENE = { map: "0", x: 307, y: 232, width: 56, height: 68 };

const results = [];
const failures = [];

for (const pass of ["first", "warm"]) {
  const limitMs = pass === "warm" ? WARM_PASS_LIMIT_MS : FIRST_PASS_LIMIT_MS;

  for (const target of MAPS) {
    try {
      const result = await requestMap(target, pass);
      results.push(result);

      if (result.ms > limitMs) {
        failures.push(`${pass} ${target.map} took ${result.ms} ms; limit is ${limitMs} ms`);
      }
      if (result.status !== 200) {
        failures.push(`${pass} ${target.map} returned HTTP ${result.status}`);
      }
      if (result.cells <= 0) {
        failures.push(`${pass} ${target.map} returned no cells`);
      }
      if (result.sprites <= 0) {
        failures.push(`${pass} ${target.map} returned no sprites`);
      }
    } catch (error) {
      failures.push(`${pass} ${target.map} failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
}

await runCriticalCrystalSceneRegression();

console.table(results);

const summary = {
  baseUrl: BASE_URL,
  generatedAt: new Date().toISOString(),
  requestTimeoutMs: REQUEST_TIMEOUT_MS,
  firstPassLimitMs: FIRST_PASS_LIMIT_MS,
  warmPassLimitMs: WARM_PASS_LIMIT_MS,
  targetCount: MAPS.length,
  requestCount: results.length,
  failures,
  results,
};
await fs.mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
await fs.writeFile(OUTPUT_PATH, `${JSON.stringify(summary, null, 2)}\n`);
console.log(`Wrote ${OUTPUT_PATH}`);

if (failures.length > 0) {
  console.error("Crystal map API smoke failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exitCode = 1;
}

async function requestMap(target, pass) {
  const url = new URL("/api/scene/crystal", BASE_URL);
  url.searchParams.set("map", target.map);
  url.searchParams.set("x", String(target.x));
  url.searchParams.set("y", String(target.y));
  url.searchParams.set("width", String(target.width ?? 24));
  url.searchParams.set("height", String(target.height ?? 18));

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  const started = performance.now();

  try {
    const response = await fetch(url, { signal: controller.signal });
    const body = await response.json();
    const elapsed = Math.round(performance.now() - started);
    const region = body?.originalMapRegion ?? null;
    return {
      pass,
      map: target.map,
      status: response.status,
      error: body?.error ?? null,
      body,
      ms: elapsed,
      title: body?.mapTitle ?? "",
      mini: body?.miniMapIndex ?? "",
      cells: Array.isArray(region?.cells) ? region.cells.length : 0,
      sprites: region?.sprites && typeof region.sprites === "object" ? Object.keys(region.sprites).length : 0,
      width: region?.mapWidth ?? "",
      height: region?.mapHeight ?? "",
    };
  } finally {
    clearTimeout(timeout);
  }
}

async function runCriticalCrystalSceneRegression() {
  const result = await requestMap(CRITICAL_SCENE, "critical");
  if (result.status !== 200) {
    failures.push(`critical scene (${CRITICAL_SCENE.map} at ${CRITICAL_SCENE.x},${CRITICAL_SCENE.y} ${CRITICAL_SCENE.width}x${CRITICAL_SCENE.height}) returned HTTP ${result.status}`);
    if (result.error === "resource_missing") {
      failures.push(`critical scene missing resources: ${JSON.stringify({
        publicPath: result.body?.publicPath,
        libraryKey: result.body?.libraryKey,
        frameIndex: result.body?.frameIndex,
        manifestAssetHash: result.body?.manifestAssetHash,
        assetVersion: result.body?.assetVersion,
      })}`);
      if (!result.body?.publicPath) {
        failures.push("critical scene 424 missing publicPath");
      }
      if (result.body?.libraryKey === undefined || result.body?.libraryKey === null) {
        failures.push("critical scene 424 missing libraryKey");
      }
      if (result.body?.frameIndex === undefined || result.body?.frameIndex === null) {
        failures.push("critical scene 424 missing frameIndex");
      }
      if (!result.body?.manifestAssetHash) {
        failures.push("critical scene 424 missing manifestAssetHash");
      }
      if (!result.body?.assetVersion) {
        failures.push("critical scene 424 missing assetVersion");
      }
    }
    return;
  }
  if (result.sprites <= 0 || result.cells <= 0) {
    failures.push("critical scene (0,307,232 56x68) returned no cells or sprites");
    return;
  }
  if (!result.title) {
    failures.push("critical scene regression returned empty map title");
  }
}
