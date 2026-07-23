import { NextResponse } from "next/server";
import { readDubheNodeConsole } from "../../../lib/dubhe-node";

export const dynamic = "force-dynamic";

export async function GET() {
  const snapshot = await readDubheNodeConsole();
  return NextResponse.json(snapshot, {
    headers: {
      "cache-control": "no-store, max-age=0"
    }
  });
}
