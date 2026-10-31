import { readDubheConnectLogin } from "../../../../../../../web/lib/server/dubhe-connect-auth";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export function POST(request: Request) {
  return readDubheConnectLogin(request);
}
