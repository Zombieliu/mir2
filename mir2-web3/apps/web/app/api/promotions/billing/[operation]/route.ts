import { withPromotionBackend } from "../../../../../lib/server/promotion-backend";
import { promotionBilling } from "../../../../../lib/server/promotion-billing";
export const runtime = "nodejs";
export const dynamic = "force-dynamic";
type Context = { params: Promise<{ operation: string }> };
export async function GET(request: Request, context: Context) { const params = await context.params; return withPromotionBackend(request, () => promotionBilling(request, params.operation)); }
export async function POST(request: Request, context: Context) { const params = await context.params; return withPromotionBackend(request, () => promotionBilling(request, params.operation)); }
