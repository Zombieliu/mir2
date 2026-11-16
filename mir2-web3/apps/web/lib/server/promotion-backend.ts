import "server-only";
import { AsyncLocalStorage } from "node:async_hooks";

type Backend = "legacy" | "windows";
const scope = new AsyncLocalStorage<Backend>();
const CAMPAIGN_HEADER = "x-mir2-promotion-season";
const CAMPAIGN_ID = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/;

/** Profiles are server configuration. A browser can select a season, never a URL or identity. */
export function promotionBackendId(): Backend { return scope.getStore() ?? "legacy"; }

export function promotionSetting(name: `MIR2_PROMOTION_${string}`): string | undefined {
  return process.env[promotionBackendId() === "windows" ? name.replace("MIR2_PROMOTION_", "MIR2_PROMOTION_WINDOWS_") : name];
}

export function promotionCookieName(base: string): string {
  return promotionBackendId() === "windows" ? base.replace("-promotion-", "-promotion-windows-") : base;
}

export function promotionCookieScope(): string {
  return promotionBackendId() === "windows" ? "\0windows-v1" : "";
}

export function promotionCheckoutOrigin(): string {
  const configured = promotionSetting("MIR2_PROMOTION_GATEWAY_HTTP_URL");
  const explicit = promotionSetting("MIR2_PROMOTION_CHECKOUT_ORIGIN");
  if (!explicit && !configured) throw new Error("promotion_checkout_origin_required");
  const value = explicit ?? new URL(configured!).origin;
  const url = new URL(value);
  if (value !== url.origin || url.username || url.password || url.protocol !== "https:") {
    throw new Error("invalid_promotion_checkout_origin");
  }
  return value;
}

function choose(request: Request): Backend {
  const values = new URL(request.url).searchParams.getAll("campaignId");
  if (values.length > 1) throw new Error("invalid_promotion_campaign");
  const header = request.headers.get(CAMPAIGN_HEADER);
  if (header && values.length && values[0] !== header) throw new Error("invalid_promotion_campaign");
  const selected = header ?? values[0] ?? null;
  if (selected !== null && !CAMPAIGN_ID.test(selected)) throw new Error("invalid_promotion_campaign");
  const windowsCampaign = process.env.MIR2_PROMOTION_WINDOWS_CAMPAIGN_ID?.trim();
  if (!windowsCampaign) return "legacy";
  if (!CAMPAIGN_ID.test(windowsCampaign)) throw new Error("promotion_backend_not_configured");
  const legacyCampaigns = (process.env.MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS ?? "").split(",").map(value => value.trim()).filter(Boolean);
  if (!legacyCampaigns.length || legacyCampaigns.some(value => !CAMPAIGN_ID.test(value) || value === windowsCampaign)) {
    throw new Error("promotion_backend_not_configured");
  }
  if (selected && legacyCampaigns.includes(selected)) return "legacy";
  // Unknown IDs cannot silently select a different backend or create a season.
  if (selected && selected !== windowsCampaign) throw new Error("promotion_campaign_not_configured");
  const session = process.env.MIR2_PROMOTION_WINDOWS_SESSION_SECRET;
  const wallet = process.env.MIR2_PROMOTION_WINDOWS_WALLET_AUTH_SECRET;
  if (!session || session.length < 32 || session === process.env.MIR2_PROMOTION_SESSION_SECRET ||
      session === wallet || !wallet || wallet.length < 32 || wallet === process.env.MIR2_PROMOTION_WALLET_AUTH_SECRET) {
    throw new Error("promotion_backend_not_configured");
  }
  return "windows";
}

/** AsyncLocalStorage isolates concurrent tabs/requests without changing process.env. */
export async function withPromotionBackend(request: Request, run: () => Promise<Response>): Promise<Response> {
  let backend: Backend;
  try { backend = choose(request); }
  catch (cause) {
    const code = cause instanceof Error ? cause.message : "promotion_backend_not_configured";
    return Response.json({ error: code }, { status: code === "invalid_promotion_campaign" ? 400 : 503,
      headers: { "Cache-Control": "no-store, private", "X-Content-Type-Options": "nosniff" } });
  }
  return scope.run(backend, run);
}
