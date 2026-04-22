import { NextResponse } from "next/server";

import { loadCrystalSceneBlueprint } from "../../../../lib/crystal-map-loader";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const blueprint = await loadCrystalSceneBlueprint({
    mapFileName: url.searchParams.get("map") ?? "0",
    centerX: numberParam(url.searchParams.get("x")),
    centerY: numberParam(url.searchParams.get("y")),
    width: numberParam(url.searchParams.get("width")),
    height: numberParam(url.searchParams.get("height")),
  });

  return NextResponse.json(blueprint);
}

function numberParam(value: string | null) {
  if (value === null) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}
