import { NextResponse } from "next/server";

import {
  getCrystalOriginalAssetManifestMetadata,
  getManifestVersionForErrors,
  isCrystalResourceMissingError,
} from "../../../../lib/crystal-map-loader";
import { loadCachedCrystalSceneBlueprint } from "../../../../lib/scene-blueprint-cache";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const isDevelopment = process.env.NODE_ENV === "development";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const manifestInfo = getCrystalOriginalAssetManifestMetadata();
  const manifestAssetHash = manifestInfo?.assetHash ?? null;
  const assetVersion = getManifestVersionForErrors() || null;

  let result: Awaited<ReturnType<typeof loadCachedCrystalSceneBlueprint>>;
  try {
    result = await loadCachedCrystalSceneBlueprint({
      mapFileName: url.searchParams.get("map") ?? "0",
      centerX: numberParam(url.searchParams.get("x")),
      centerY: numberParam(url.searchParams.get("y")),
      width: numberParam(url.searchParams.get("width")),
      height: numberParam(url.searchParams.get("height")),
    });
  } catch (error) {
    if (isCrystalResourceMissingError(error)) {
      const publicPathCandidate = error.resourcePath;
      const missingPublicPath =
        typeof publicPathCandidate === "string" &&
        (publicPathCandidate.startsWith("/original-map/") || publicPathCandidate.startsWith("/original-ui/"))
          ? publicPathCandidate
          : null;
      return NextResponse.json(
        {
          error: "resource_missing",
          message: error.message,
          publicPath: missingPublicPath,
          libraryKey: error.libraryKey ?? null,
          frameIndex: error.frameIndex ?? null,
          manifestAssetHash: error.manifestAssetHash ?? manifestAssetHash,
          assetVersion: error.manifestAssetVersion ?? assetVersion,
          resource: {
            type: error.resourceType,
            path: error.resourcePath,
            mapFileName: error.mapFileName ?? null,
            libraryKey: error.libraryKey ?? null,
            frameIndex: error.frameIndex ?? null,
          },
        },
        { status: 424 },
      );
    }
    throw error;
  }

  const missingAssetCount = result.blueprint.originalMapRegion?.missingAssets?.length ?? 0;

  return NextResponse.json(result.blueprint, {
    headers: {
      "Cache-Control": isDevelopment
        ? "public, max-age=0, must-revalidate"
        : "public, max-age=300, stale-while-revalidate=3600",
      "X-Mir2-Scene-Cache": result.cacheStatus,
      "X-Mir2-Scene-Cache-Key": result.cacheKey,
      "X-Mir2-Original-Map-Sprite-Count": String(
        Object.keys(result.blueprint.originalMapRegion?.sprites ?? {}).length,
      ),
      "X-Mir2-Original-Map-Cell-Count": String(result.blueprint.originalMapRegion?.cells.length ?? 0),
      // 0 when every referenced asset resolved. Non-zero means the scene rendered with some
      // sprites skipped (graceful degradation); the missing paths are listed in the blueprint
      // under originalMapRegion.missingAssets. Release gates run with MIR2_STRICT_ASSET_RESOLUTION=1
      // so any miss instead surfaces as HTTP 424 above.
      "X-Mir2-Missing-Asset-Count": String(missingAssetCount),
    },
  });
}

function numberParam(value: string | null) {
  if (value === null) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}
