import { withPromotionBackend } from "../../../../lib/server/promotion-backend";
import { promotionBootstrap } from "../../../../lib/server/promotion-api";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
export function GET(request: Request) { return withPromotionBackend(request, () => promotionBootstrap(request)); }
