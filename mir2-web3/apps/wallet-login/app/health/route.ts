export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export function GET() {
  return Response.json({
    ready: true,
    revision: process.env.MIR2_WALLET_LOGIN_BUILD_REVISION,
  }, { headers: { "Cache-Control": "no-store" } });
}
