import { createDubheConnectLogin } from "../../../../../lib/server/dubhe-connect-auth";

export const runtime = "nodejs";

export function POST(request: Request) {
  // Next's route context must never become a server-created auth configuration.
  return createDubheConnectLogin(request);
}
