export const LEGACY_PRODUCTION_ASSET_CONFIG: "production-web-assets.json";
export const NUMERON_PRODUCTION_ASSET_CONFIG: "numeron-r16-web-assets.json";
export interface ProductionAssetConfig {
  schemaVersion: number;
  profile?: string;
  version: string;
  objectPrefix: string;
  assetBaseUrl: string;
  remoteReleaseUrl: string;
  remoteReleaseSha256?: string;
  fallbackObjectPrefix?: string;
  browserFallbackBaseUrls?: string[];
  bevyRuntime?: { enabled: boolean; version: string; assetBaseUrl: string };
  mapAtlas?: { enabled?: boolean; verified?: boolean; manifestPath?: string | null; contentHash?: string | null; pageCount?: number; maxPageBytes?: number };
  fullCrystalPack?: { enabled?: boolean; verified?: boolean; path?: string; contentHash?: string | null; libraryCount?: number; pageCount?: number };
}
export function productionAssetConfigName(selection?: string): string;
export function resolveProductionAssetConfigPath(root?: string, selection?: string): string;
export function loadProductionAssetConfig(root?: string, selection?: string): ProductionAssetConfig;
export function assertNumeronFiniteProfile(config: unknown): void;
export function verifyProductionRemoteManifest(config: ProductionAssetConfig, fetchImpl?: typeof fetch): Promise<{ verified: boolean; skipped?: boolean; reason?: string; bytes?: number; sha256?: string }>;
