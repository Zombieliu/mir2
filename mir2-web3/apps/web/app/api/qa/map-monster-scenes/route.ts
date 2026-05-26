import { NextResponse } from "next/server";

import {
  buildQaMapMonsterScenes,
  getQaMapMonsterScene,
  normalizeQaMapMonsterLimit,
} from "../../../../lib/qa-map-monster-scenes";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const isDevelopment = process.env.NODE_ENV === "development";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const limitPerMap = normalizeQaMapMonsterLimit(url.searchParams.get("limitPerMap"));
  const sceneKey = url.searchParams.get("scene");

  if (sceneKey !== null) {
    const { list, scene } = getQaMapMonsterScene(sceneKey, limitPerMap);
    return NextResponse.json(
      {
        generatedAt: list.generatedAt,
        limitPerMap: list.limitPerMap,
        sourceMapCount: list.sourceMapCount,
        respawnMapCount: list.respawnMapCount,
        positiveRespawnRowCount: list.positiveRespawnRowCount,
        sceneCount: list.sceneCount,
        scene,
      },
      { status: scene ? 200 : 404, headers: responseHeaders() },
    );
  }

  return NextResponse.json(buildQaMapMonsterScenes(limitPerMap), {
    headers: responseHeaders(),
  });
}

function responseHeaders() {
  return {
    "Cache-Control": isDevelopment
      ? "public, max-age=0, must-revalidate"
      : "public, max-age=300, stale-while-revalidate=3600",
    "X-Mir2-QA-Source": "crystal-respawn-manifest",
  };
}
