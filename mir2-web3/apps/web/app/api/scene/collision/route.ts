import { NextResponse } from "next/server";

import { loadCrystalCollisionRegion, loadCrystalQuestCollisionRegion, QuestCollisionError } from "../../../../lib/crystal-map-loader";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const MAX_COLLISION_SPAN = 256;

export async function GET(request: Request) {
  const url = new URL(request.url);
  if (url.searchParams.get("quest") === "1") {
    try {
      const mapFileName = url.searchParams.get("map");
      const minX = strictNumberParam(url.searchParams.get("minX"));
      const minY = strictNumberParam(url.searchParams.get("minY"));
      const maxX = strictNumberParam(url.searchParams.get("maxX"));
      const maxY = strictNumberParam(url.searchParams.get("maxY"));
      if (mapFileName === null || [minX, minY, maxX, maxY].some(value => value === null))
        throw new QuestCollisionError("invalidInput");
      const collision = loadCrystalQuestCollisionRegion({
        mapFileName, minX: minX!, maxX: maxX!, minY: minY!, maxY: maxY!,
      });
      return NextResponse.json(collision, { headers: {
        "Cache-Control": "public, max-age=300, s-maxage=86400, stale-while-revalidate=604800",
        "X-Mir2-Collision-Cell-Count": String(collision.blockedCells.length),
      } });
    } catch (error) {
      const code = error instanceof QuestCollisionError ? error.code : "collisionUnavailable";
      return NextResponse.json({ schemaVersion: 1, error: code }, {
        status: code === "invalidInput" ? 400 : 424, headers: { "Cache-Control": "no-store" },
      });
    }
  }
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

function strictNumberParam(value: string | null) {
  if (value === null) return null;
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new QuestCollisionError("invalidInput");
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) throw new QuestCollisionError("invalidInput");
  return parsed;
}
