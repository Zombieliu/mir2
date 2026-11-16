import { withPromotionBackend } from "../../../../lib/server/promotion-backend";
import { promotionGameSession } from "../../../../lib/server/promotion-game";
export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export function POST(request: Request) { return withPromotionBackend(request, () => promotionGameSession(request)); }
