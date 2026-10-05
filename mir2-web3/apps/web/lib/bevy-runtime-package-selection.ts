import {
  validateBevyRuntimeManifest,
  type NormalizedBevyRuntimeManifest,
  type NormalizedBevyRuntimePackage,
  type BevyRuntimeBackend,
} from "./bevy-runtime-manifest.mjs";
import { isWebGl2SharedCanvasPrototype } from "./bevy-shared-canvas-mode";

export type BevyRuntimeStartup = Readonly<{
  runtimeAllowed: boolean;
  reason: "ready" | "invalid-manifest" | "legacy-shared-unknown";
  manifest: NormalizedBevyRuntimeManifest | null;
  requestedSharedCanvas: boolean;
  sharedCanvasPrototype: boolean;
}>;

/** Decide the immutable canvas/package mode before mounting or importing WASM. */
export function selectBevyRuntimeStartup(rawManifest: unknown, search: string): BevyRuntimeStartup {
  const requestedSharedCanvas = typeof search === "string" && isWebGl2SharedCanvasPrototype(search);
  let manifest: NormalizedBevyRuntimeManifest;
  try {
    if (typeof search !== "string") throw new TypeError("invalid search");
    manifest = validateBevyRuntimeManifest(rawManifest, "runtime manifest");
  } catch {
    return Object.freeze({ runtimeAllowed: false, reason: "invalid-manifest", manifest: null,
      requestedSharedCanvas, sharedCanvasPrototype: false });
  }
  if (manifest.schemaVersion === 1 && requestedSharedCanvas) {
    return Object.freeze({ runtimeAllowed: false, reason: "legacy-shared-unknown", manifest,
      requestedSharedCanvas, sharedCanvasPrototype: false });
  }
  const hasSharedPackage = manifest.packages.some((item) => item.id === "webgl2-shared");
  return Object.freeze({ runtimeAllowed: true, reason: "ready", manifest, requestedSharedCanvas,
    sharedCanvasPrototype: requestedSharedCanvas && hasSharedPackage });
}

/** Resolve the actual backend to the selected immutable package. */
export function getBevyRuntimePackageForBackend(
  startup: BevyRuntimeStartup,
  backend: BevyRuntimeBackend,
): NormalizedBevyRuntimePackage | null {
  if (!startup.runtimeAllowed || !startup.manifest) return null;
  if (backend !== "webgpu" && backend !== "webgl2") return null;
  if (startup.requestedSharedCanvas && backend !== "webgl2") return null;
  if (startup.sharedCanvasPrototype && backend !== "webgl2") return null;
  const id = backend === "webgpu" ? "webgpu"
    : startup.sharedCanvasPrototype ? "webgl2-shared" : "webgl2";
  return startup.manifest.packages.find((item) => item.id === id) ?? null;
}

export type BevyRuntimeUiCapabilities = Readonly<{
  schemaVersion: 1;
  backend: BevyRuntimeBackend | "native";
  questUiAbiVersion: 0 | 1;
  bagUiAbiVersion: 0 | 1;
  primarySharedUiCompiled: boolean;
  primarySharedUiStartup: boolean;
}>;

function capabilityObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readCapabilities(getter: () => string): BevyRuntimeUiCapabilities {
  let value: unknown;
  try { value = JSON.parse(getter()); }
  catch { throw new Error("runtime UI capability getter failed"); }
  if (!capabilityObject(value)
      || Object.keys(value).length !== 6
      || !["schemaVersion", "backend", "questUiAbiVersion", "bagUiAbiVersion",
        "primarySharedUiCompiled", "primarySharedUiStartup"].every((key) => Object.hasOwn(value, key))
      || value.schemaVersion !== 1
      || !["webgpu", "webgl2", "native"].includes(value.backend as string)
      || ![0, 1].includes(value.questUiAbiVersion as number)
      || value.bagUiAbiVersion !== value.questUiAbiVersion
      || typeof value.primarySharedUiCompiled !== "boolean"
      || typeof value.primarySharedUiStartup !== "boolean") {
    throw new Error("invalid runtime UI capabilities");
  }
  return Object.freeze({
    schemaVersion: 1 as const,
    backend: value.backend as BevyRuntimeBackend | "native",
    questUiAbiVersion: value.questUiAbiVersion as 0 | 1,
    bagUiAbiVersion: value.bagUiAbiVersion as 0 | 1,
    primarySharedUiCompiled: value.primarySharedUiCompiled as boolean,
    primarySharedUiStartup: value.primarySharedUiStartup as boolean,
  });
}

/** Call synchronously after WASM initialization and immediately before boot. */
export function assertBevyRuntimeStartupAgreement(
  startup: BevyRuntimeStartup,
  backend: BevyRuntimeBackend,
  getter: (() => string) | null | undefined,
  currentSearch: string,
): BevyRuntimeUiCapabilities | null {
  const selected = getBevyRuntimePackageForBackend(startup, backend);
  if (!selected || typeof currentSearch !== "string"
      || isWebGl2SharedCanvasPrototype(currentSearch) !== startup.requestedSharedCanvas) {
    throw new Error("runtime startup selection changed or is unavailable");
  }
  if (typeof getter !== "function") {
    if (startup.manifest?.schemaVersion === 1 && !startup.requestedSharedCanvas
        && !startup.sharedCanvasPrototype && selected.backend === backend) return null;
    throw new Error("runtime UI capability getter missing");
  }
  const actual = readCapabilities(getter);
  if (actual.backend !== backend || actual.primarySharedUiStartup !== startup.sharedCanvasPrototype) {
    throw new Error("runtime backend or canvas mode disagreement");
  }
  if (startup.manifest?.schemaVersion === 2) {
    if (actual.questUiAbiVersion !== selected.questUiAbiVersion
        || actual.bagUiAbiVersion !== selected.bagUiAbiVersion
        || actual.primarySharedUiCompiled !== selected.primarySharedUiCompiled) {
      throw new Error("runtime compiled UI capabilities disagree with manifest");
    }
  } else if (backend === "webgpu" && actual.primarySharedUiCompiled) {
    throw new Error("legacy GPU runtime reports GL2 primary UI capability");
  }
  return actual;
}

export type BevyRuntimeBootGate = Readonly<{ kind: "bevy-runtime-boot-gate" }>;
const documentBootGate: BevyRuntimeBootGate = Object.freeze({ kind: "bevy-runtime-boot-gate" });
let documentBootAttempted = false;

export function createBevyRuntimeBootGate(): BevyRuntimeBootGate {
  return documentBootGate;
}

/** Mark the document's boot attempt before callback, including a throwing callback. */
export function runBevyRuntimeBootOnce<T>(gate: BevyRuntimeBootGate, callback: () => T): T {
  if (gate !== documentBootGate || documentBootAttempted) {
    throw new Error("Bevy runtime boot already attempted or gate invalid");
  }
  documentBootAttempted = true;
  return callback();
}
