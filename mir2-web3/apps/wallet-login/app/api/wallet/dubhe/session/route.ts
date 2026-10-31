import { createDubheConnectLogin } from "../../../../../../web/lib/server/dubhe-connect-auth";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export function POST(request: Request) {
  // Framework route context is not a server-created signing configuration.
  return createDubheConnectLogin(request);
}
