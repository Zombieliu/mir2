import type { SuiLoginToken } from "./passkey-auth";
import { promotionSeasonHeaders } from "./promotion-season-context";

export const PROMOTION_GAME_REALM = "numeron-promotion-testnet";
export const PROMOTION_GAME_WS = "wss://promotion-playtest.165.154.65.136.sslip.io/ws";
export function isPromotionGamePath(pathname: string): boolean { return pathname === "/promotions/play" || pathname === "/promotions/play/"; }
export async function requestPromotionGameLogin(): Promise<SuiLoginToken> {
  const status = await fetch("/api/promotions/bootstrap", { credentials: "same-origin", cache: "no-store", redirect: "error", headers: promotionSeasonHeaders(), signal: AbortSignal.timeout(15_000) });
  const bootstrap = await status.json();
  if (!status.ok || bootstrap.authenticated !== true || bootstrap.binding?.status !== "linked" ||
    typeof bootstrap.csrfToken !== "string") throw new Error("promotion_wallet_login_required");
  const response = await fetch("/api/promotions/game-session", { method: "POST", credentials: "same-origin", cache: "no-store", redirect: "error",
    headers: { ...promotionSeasonHeaders(), "content-type": "application/json", "x-promotion-csrf": bootstrap.csrfToken }, body: "{}", signal: AbortSignal.timeout(15_000) });
  const value = await response.json();
  if (!response.ok || value.realmId !== PROMOTION_GAME_REALM || typeof value.accountId !== "string" || !value.accountId ||
    typeof value.token !== "string" || value.token.length < 32 || typeof value.expiresAt !== "number" ||
    !Number.isSafeInteger(value.expiresAt) || value.expiresAt <= Date.now() || value.expiresAt > Date.now() + 90_000) throw new Error("promotion_game_login_unavailable");
  return { accountId: value.accountId, token: value.token, expiresAt: value.expiresAt, provider: "suiWallet" };
}
