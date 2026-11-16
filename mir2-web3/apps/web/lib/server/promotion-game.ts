import "server-only";
import { promotionWalletNetwork } from "./promotion-wallet-context";
import { promotionBrowserRequest, promotionCsrf, promotionError, promotionFail, promotionGateway,
  promotionJson, promotionReadBody, promotionSession } from "./promotion-auth";

export async function promotionGameSession(request: Request): Promise<Response> {
  try {
    promotionBrowserRequest(request, true);
    if (request.method !== "POST") promotionFail(405, "promotion_method_not_allowed");
    if (new URL(request.url).search || Object.keys(await promotionReadBody(request, 1024)).length) promotionFail(400, "unexpected_game_session_parameters");
    if (promotionWalletNetwork() !== "testnet") promotionFail(403, "testnet_game_required");
    const session = await promotionSession(request);
    if (!session?.walletBound) promotionFail(403, "promotion_wallet_required");
    promotionCsrf(request, session);
    const result = await promotionGateway("/v1/promotions/game-session", { method: "POST", token: session.token, body: {} });
    if (!result.ok) return promotionJson({ error: "promotion_game_session_unavailable" }, result.status);
    const { accountId, token, expiresAt, realmId } = result.payload;
    if (accountId !== session.accountId || typeof token !== "string" || token.length < 32 || token.length > 4096 || token === session.token ||
      typeof expiresAt !== "number" || !Number.isSafeInteger(expiresAt) || expiresAt <= Date.now() || expiresAt > Date.now() + 90_000 ||
      realmId !== "numeron-promotion-testnet") promotionFail(502, "invalid_game_session_response");
    // A short-lived, one-use gameplay grant, never the commercial identity bearer.
    return promotionJson({ accountId, token, expiresAt, realmId, provider: "suiWallet" });
  } catch (error) { return promotionError(error); }
}
