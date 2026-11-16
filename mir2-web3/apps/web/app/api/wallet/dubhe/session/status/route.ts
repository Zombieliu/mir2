import { readDubheConnectLogin } from "../../../../../../lib/server/dubhe-connect-auth";

export const runtime = "nodejs";

export function POST(request: Request) {
  return readDubheConnectLogin(request);
}
