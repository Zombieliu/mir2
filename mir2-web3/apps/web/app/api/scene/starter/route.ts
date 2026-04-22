import { NextResponse } from "next/server";

import { loadStarterSceneBlueprint } from "../../../../lib/load-starter-scene";

export async function GET() {
  const blueprint = await loadStarterSceneBlueprint();
  return NextResponse.json(blueprint);
}
