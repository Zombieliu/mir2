import { NextResponse } from "next/server";

import { loadCrystalCollisionRegion } from "../../../../lib/crystal-map-loader";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const MAX_COLLISION_SPAN = 256;

export async function GET(request: Request) {
  const url = new URL(request.url);
  const minX = numberParam(url.searchParams.get("minX")) ?? 0;
  const minY = numberParam(url.searchParams.get("minY")) ?? 0;
  const maxX = Math.min(
    numberParam(url.searchParams.get("maxX")) ?? minX,
    minX + MAX_COLLISION_SPAN - 1,
  );
  const maxY = Math.min(
    numberParam(url.searchParams.get("maxY")) ?? minY,
    minY + MAX_COLLISION_SPAN - 1,
  );
  const collision = loadCrystalCollisionRegion({
    mapFileName: url.searchParams.get("map") ?? "0",
    minX,
    maxX,
    minY,
    maxY,
  });
  return NextResponse.json(collision, {
    headers: {
      "Cache-Control": "public, max-age=300, s-maxage=86400, stale-while-revalidate=604800",
      "X-Mir2-Collision-Cell-Count": String(collision.blockedCells.length),
    },
  });
}

function numberParam(value: string | null) {
  if (value === null) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}
