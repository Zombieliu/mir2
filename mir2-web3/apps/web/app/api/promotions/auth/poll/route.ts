import { withPromotionBackend } from "../../../../../lib/server/promotion-backend";
import { promotionPoll } from "../../../../../lib/server/promotion-auth";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export function POST(request: Request) { return withPromotionBackend(request, () => promotionPoll(request)); }
