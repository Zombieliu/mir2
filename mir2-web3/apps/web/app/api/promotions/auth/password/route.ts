import { withPromotionBackend } from "../../../../../lib/server/promotion-backend";
import { promotionPassword } from "../../../../../lib/server/promotion-auth";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export function POST(request: Request) { return withPromotionBackend(request, () => promotionPassword(request)); }
