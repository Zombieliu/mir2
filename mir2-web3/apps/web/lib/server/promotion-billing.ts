import "server-only";
import { promotionWalletNetwork } from "./promotion-wallet-context";
import { promotionBrowserRequest, promotionCsrf, promotionError, promotionFail, promotionGateway,
  promotionJson, promotionReadBody, promotionSession } from "./promotion-auth";

const ROUTES = {
  status: "/v1/billing/status", refresh: "/v1/billing/refresh", checkout: "/v1/billing/sui-usdc/checkout",
  buy: "/v1/billing/monthly-card/buy", activate: "/v1/billing/monthly-card/activate",
} as const;
const PRIVATE_FIELDS = new Set(["token", "credential", "authorization", "sessionToken", "identityToken", "privateKey", "recoveryCodes"]);
function safe(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(safe);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).filter(([key]) => !PRIVATE_FIELDS.has(key)).map(([key, child]) => [key, safe(child)]));
}

export async function promotionBilling(request: Request, operation: string): Promise<Response> {
  try {
    if (!Object.hasOwn(ROUTES, operation)) promotionFail(404, "promotion_endpoint_not_found");
    const read = operation === "status";
    if (request.method !== (read ? "GET" : "POST")) promotionFail(405, "promotion_method_not_allowed");
    promotionBrowserRequest(request, !read);
    if (promotionWalletNetwork() !== "testnet") promotionFail(403, "testnet_billing_required");
    const session = await promotionSession(request);
    if (!session?.walletBound) promotionFail(403, "promotion_wallet_required");
    let body: Record<string, unknown>;
    let idempotencyKey: string | undefined;
    const query = new URL(request.url).searchParams;
    if (read) {
      if ([...query.keys()].some(key => !["characterIndex", "requestId"].includes(key)) ||
        query.getAll("characterIndex").length !== 1 || query.getAll("requestId").length !== 1 ||
        !/^(0|[1-9][0-9]{0,9})$/.test(query.get("characterIndex")!) || !/^[1-9][0-9]{0,15}$/.test(query.get("requestId")!)) {
        promotionFail(400, "invalid_billing_parameters");
      }
      body = { characterIndex: Number(query.get("characterIndex")), requestId: Number(query.get("requestId")) };
    } else {
      if (query.size) promotionFail(400, "unexpected_write_query");
      promotionCsrf(request, session);
      idempotencyKey = request.headers.get("idempotency-key") ?? "";
      if (!/^[A-Za-z0-9_-]{16,128}$/.test(idempotencyKey)) promotionFail(400, "idempotency_key_required");
      body = await promotionReadBody(request, 4096);
      const fields = ["characterIndex", "requestId", ...(operation === "checkout" ? ["offerId", "orderRequestId"] :
        operation === "buy" ? ["orderRequestId"] : operation === "activate" ? ["itemUniqueId"] : [])];
      if (Object.keys(body).sort().join(",") !== fields.sort().join(",")) promotionFail(400, "invalid_billing_parameters");
      if (["checkout", "buy"].includes(operation) && body.orderRequestId !== idempotencyKey) promotionFail(400, "billing_intent_mismatch");
      if (operation === "checkout" && (typeof body.offerId !== "string" || !/^[A-Za-z0-9_-]{1,128}$/.test(body.offerId))) promotionFail(400, "invalid_billing_parameters");
      if (operation === "activate" && (typeof body.itemUniqueId !== "string" || !/^[1-9][0-9]{0,19}$/.test(body.itemUniqueId) ||
        BigInt(body.itemUniqueId) > 18_446_744_073_709_551_615n)) promotionFail(400, "invalid_billing_parameters");
    }
    if (!Number.isSafeInteger(body.characterIndex) || Number(body.characterIndex) < 0 || Number(body.characterIndex) > 2_147_483_647 ||
      !Number.isSafeInteger(body.requestId) || Number(body.requestId) <= 0) promotionFail(400, "invalid_billing_parameters");
    const result = await promotionGateway(ROUTES[operation as keyof typeof ROUTES], {
      method: read ? "GET" : "POST", token: session.token, ...(read ? { query: `?characterIndex=${body.characterIndex}&requestId=${body.requestId}` } : { body, idempotencyKey }),
    });
    if (result.ok && operation === "checkout") {
      let checkout: URL;
      try { checkout = new URL(String(result.payload.checkoutUrl)); } catch { promotionFail(502, "invalid_checkout_response"); }
      const configured = new URL(process.env.MIR2_PROMOTION_GATEWAY_HTTP_URL!);
      if (checkout.origin !== configured.origin || checkout.pathname !== "/v1/billing/sui-usdc/pay" || checkout.username || checkout.password || checkout.search ||
        !/^#usdc_[a-f0-9]{64}\.[A-Za-z0-9_-]{43}$/.test(checkout.hash)) promotionFail(502, "invalid_checkout_response");
    }
    return promotionJson(result.status >= 500 ? { error: "promotion_billing_unavailable" } : safe(result.payload), result.status);
  } catch (error) { return promotionError(error); }
}
