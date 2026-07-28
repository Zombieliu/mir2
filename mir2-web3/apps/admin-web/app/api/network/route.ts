import { NextResponse } from "next/server";
import { readDubheNetwork } from "../../../lib/dubhe-network";

export const dynamic = "force-dynamic";

export async function GET() {
  return NextResponse.json(await readDubheNetwork(), {
    headers: {
      "cache-control": "no-store, max-age=0"
    }
  });
}
