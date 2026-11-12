import { createGatewayCatalogDecoder, type GatewayCatalogDecoder, type GatewayCatalogWasmModule } from "./gateway-catalog-transport";

type Descriptor = { abiVersion: 1; version: string; sourceSha256: string; files: Record<string, { bytes: number; sha256: string }> };
const hex = /^[a-f0-9]{64}$/;
function fields(value: unknown, names: readonly string[]): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).sort().join("|") === [...names].sort().join("|");
}

export function gatewayCatalogDescriptor(value: unknown, sourceSha256: string, siblings: readonly string[]): Descriptor | null {
  // Existing immutable three-policy releases remain legacy Text clients.
  if (value === undefined) return null;
  const bad = () => new Error("Invalid shared gateway catalog manifest");
  if (!fields(value, ["abiVersion", "version", "sourceSha256", "files"]) || value.abiVersion !== 1
    || typeof value.version !== "string" || !hex.test(value.version) || siblings.includes(value.version)
    || value.sourceSha256 !== sourceSha256 || !hex.test(sourceSha256)
    || !fields(value.files, ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"])) throw bad();
  for (const name of ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"]) {
    const file = value.files[name];
    if (!fields(file, ["bytes", "sha256"]) || typeof file.bytes !== "number" || !Number.isSafeInteger(file.bytes) || file.bytes <= 0
      || (name.endsWith(".wasm") ? file.bytes >= 262144 : file.bytes > 204800)
      || typeof file.sha256 !== "string" || !hex.test(file.sha256)) throw bad();
  }
  return value as Descriptor;
}

export async function loadGatewayCatalogRuntime(value: unknown, sourceSha256: string, siblings: readonly string[], origin: string, attempt: number): Promise<GatewayCatalogDecoder | null> {
  const descriptor = gatewayCatalogDescriptor(value, sourceSha256, siblings);
  if (!descriptor) return null;
  const base = new URL(`/client-core/${descriptor.version}/`, origin), url = new URL("mir2_platform_web.js", base);
  if (attempt > 0) url.searchParams.set("retry", String(attempt));
  const module = await import(/* webpackIgnore: true */ url.href) as GatewayCatalogWasmModule & {
    default(options: { module_or_path: URL }): Promise<unknown>;
  };
  await module.default({ module_or_path: new URL("mir2_platform_web_bg.wasm", base) });
  return createGatewayCatalogDecoder(module);
}
