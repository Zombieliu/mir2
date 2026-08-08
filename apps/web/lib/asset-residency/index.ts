/**
 * Public API barrel for the asset-residency module.
 *
 * Import from this file in consuming code:
 *   import { createAssetResidency } from "../lib/asset-residency";
 *
 * Browser IO adapters are in a separate entry point so server-side / test
 * code never pulls in IndexedDB references:
 *   import { createBrowserIdbStore, createBrowserAtlasFetcher } from
 *     "../lib/asset-residency/browser-adapters";
 */

export type {
  AtlasPagePayload,
  AtlasRect,
  PersistentStore,
  AtlasFetcher,
  AssetResidencyConfig,
  AssetResidencyStats,
  AssetResidencyManager,
} from "./types";

export { createAssetResidency } from "./manager";
export { estimateAtlasPagePayloadBytes } from "./types";
