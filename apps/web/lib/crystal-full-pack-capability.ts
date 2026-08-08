type CrystalFullPackCapabilityOptions = {
  queryValue?: string | null;
  configuredValue?: string | null;
  remoteAssetBaseUrl?: string | null;
  releaseCapability?: boolean | null;
};

const ENABLED_VALUES = new Set(["1", "true", "on"]);
const DISABLED_VALUES = new Set(["0", "false", "off"]);

/**
 * Remote releases do not necessarily contain the optional multi-gigabyte
 * Crystal full pack. Keep it opt-in for CDN-backed builds so a missing pack
 * does not create a known 404 before the live-atlas fallback runs.
 */
export function shouldLoadCrystalFullPack({
  queryValue,
  configuredValue,
  remoteAssetBaseUrl,
  releaseCapability,
}: CrystalFullPackCapabilityOptions): boolean {
  const query = normalizeFlag(queryValue);
  if (ENABLED_VALUES.has(query)) return true;
  if (DISABLED_VALUES.has(query)) return false;

  const configured = normalizeFlag(configuredValue);
  if (ENABLED_VALUES.has(configured)) return true;
  if (DISABLED_VALUES.has(configured)) return false;

  if (typeof releaseCapability === "boolean") return releaseCapability;

  return !remoteAssetBaseUrl?.trim();
}

function normalizeFlag(value: string | null | undefined): string {
  return value?.trim().toLowerCase() ?? "";
}
