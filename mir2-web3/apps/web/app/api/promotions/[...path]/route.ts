import { withPromotionBackend } from "../../../../lib/server/promotion-backend";
import { promotionProxy } from "../../../../lib/server/promotion-api";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
type Context = { params: Promise<{ path: string[] }> };
export async function GET(request: Request, context: Context) { const params = await context.params; return withPromotionBackend(request, () => promotionProxy(request, params.path)); }
export async function POST(request: Request, context: Context) { const params = await context.params; return withPromotionBackend(request, () => promotionProxy(request, params.path)); }
