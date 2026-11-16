import { promotionSetting, promotionBackendId } from "./promotion-backend";
import "server-only";
import { promotionWalletNetwork } from "./promotion-wallet-context";

import {
  promotionBrowserRequest, promotionCsrf, promotionError, promotionFail, promotionGateway,
  promotionJson, promotionReadBody, promotionSession, promotionReadClick, promotionSaveClick, promotionClearClick, promotionCookieReady,
} from "./promotion-auth";

const ROUTES: Record<string, readonly string[]> = {
  public: ["GET"], invite: ["GET"], click: ["POST"], me: ["GET"], apply: ["POST"], candidate: ["POST"], confirm: ["POST"],
  members: ["GET"], settlements: ["GET"], export: ["GET"], "payout-address": ["GET", "POST"], appeals: ["POST"], admin: ["GET", "POST"],
};
const PRIVATE_FIELDS = new Set(["token", "credential", "authorization", "sessionToken", "identityToken", "privateKey", "recoveryCodes", "clickId"]);
function safePayload(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(safePayload);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).filter(([key]) => !PRIVATE_FIELDS.has(key)).map(([key, child]) => [key, safePayload(child)]));
}
function bodyWithoutIdentity(value: unknown, path: string[] = [], allowExcludedTarget = false) {
  if (Array.isArray(value)) { for (const child of value) bodyWithoutIdentity(child, [...path, "[]"], allowExcludedTarget); return; }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    const normalized = key.toLowerCase().replace(/[_-]/g, "");
    const explicitTarget = allowExcludedTarget && path.length === 1 && path[0] === "command" && key === "accountId";
    if (!explicitTarget && ["accountid", "playerid", "sessiontoken", "identitytoken", "authorization", "token", "credential", "actor", "actorid", "roles", "role"].includes(normalized)) {
      promotionFail(400, "client_identity_not_allowed");
    }
    bodyWithoutIdentity(child, [...path, key], allowExcludedTarget);
  }
}
function campaignId(value: unknown = promotionSetting("MIR2_PROMOTION_CAMPAIGN_ID")?.trim()): string | null {
  if (value === undefined || value === null || value === "") return null;
  if (typeof value !== "string" || !/^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/.test(value)) promotionFail(400, "invalid_promotion_campaign");
  return value;
}
function query(request: Request, route: string) {
  const params = new URL(request.url).searchParams;
  const allowed = new Set([...(route === "payout-address" ? [] : ["campaignId"]), ...(route === "invite" ? ["code"] : []),
    ...(route === "members" ? ["offset", "limit"] : []), ...(route === "export" ? ["settlementId"] : [])]);
  if (params.size > 4) promotionFail(400, "invalid_promotion_query");
  for (const [key, value] of params) {
    if (!allowed.has(key) || value.length > 256 || params.getAll(key).length !== 1 || /[\r\n\u0000]/.test(value)) {
      promotionFail(400, "invalid_promotion_query");
    }
  }
  if (params.has("limit") && !/^(?:[1-9]|[1-9][0-9]|100)$/.test(params.get("limit")!)) promotionFail(400, "invalid_promotion_query");
  if (params.has("offset") && !/^(?:0|[1-9][0-9]{0,4}|100000)$/.test(params.get("offset")!)) promotionFail(400, "invalid_promotion_query");
  if (route !== "payout-address") {
    const campaign = campaignId(params.get("campaignId") ?? undefined);
    if (campaign) params.set("campaignId", campaign);
    else promotionFail(503, "promotion_campaign_not_configured");
  }
  if (route === "invite" && !params.get("code")) promotionFail(400, "invitation_code_required");
  if (route === "export" && !params.get("settlementId")) promotionFail(400, "settlement_id_required");
  const encoded = params.toString();
  return encoded ? `?${encoded}` : "";
}
function binding(session: Awaited<ReturnType<typeof promotionSession>>) {
  if (!session) return { status: "pending", reason: "wallet_login_required" };
  return session.walletBound ? { status: "linked", reason: "verified_wallet_account_mapping", credentialId: session.walletCredentialId,
    network: promotionWalletNetwork() } :
    { status: "pending", reason: "wallet_control_proof_required" };
}
function permissions(value: unknown): string[] {
  if (Array.isArray(value)) return value.filter((item): item is string => typeof item === "string");
  if (value && typeof value === "object") return Object.entries(value).filter(([, allowed]) => allowed === true).map(([name]) => name);
  return [];
}

export async function promotionBootstrap(request: Request): Promise<Response> {
  try {
    promotionBrowserRequest(request);
    const session = await promotionSession(request, false);
    const gameClient = promotionBackendId() === "windows" ? "windows" : "web";
    const selection = new URL(request.url).searchParams;
    if ([...selection.keys()].some(key => key !== "campaignId") || selection.getAll("campaignId").length > 1) {
      promotionFail(400, "invalid_promotion_query");
    }
    const campaignSelection = campaignId(selection.get("campaignId") ?? undefined);
    if (!campaignSelection) return promotionJson({
      authenticated: !!session, csrfToken: session?.csrfToken ?? null, gameClient, campaign: null, campaignStatus: "unconfigured",
      characters: [], leader: null, dashboard: null, sourceGaps: ["promotion_campaign_not_configured"], permissions: [], binding: binding(session),
      public: { status: "unconfigured", dataMode: "no_campaign", recruitmentEnabled: false, commissionEnabled: false, payoutExecutionEnabled: false },
    });
    const campaignQuery = `?campaignId=${encodeURIComponent(campaignSelection)}`;
    const result = await promotionGateway("/v1/promotions/public", { query: campaignQuery });
    if (!result.ok) promotionFail(result.status, "promotion_campaign_unavailable");
    const publicData = result.payload;
    const campaign = publicData.campaign ?? (publicData.campaignId && publicData.status !== "unconfigured" ? safePayload(publicData) : null);
    if (!session) return promotionJson({
      authenticated: false, csrfToken: null, gameClient, campaign, campaignStatus: publicData.status ?? null,
      characters: [], leader: null, dashboard: null, sourceGaps: publicData.sourceGaps ?? [], permissions: [],
      binding: binding(session), public: safePayload(publicData),
    });
    const mine = await promotionGateway("/v1/promotions/me", { token: session.token, query: campaignQuery });
    if (!mine.ok) promotionFail(mine.status, "promotion_account_unavailable");
    const me = mine.payload;
    return promotionJson({
      authenticated: true, csrfToken: session.csrfToken, gameClient, campaign, campaignStatus: publicData.status ?? null,
      characters: Array.isArray(me.characters) ? safePayload(me.characters) : [],
      leader: safePayload(me.leader ?? null), dashboard: safePayload(me.dashboard ?? null),
      sourceGaps: me.sourceGaps ?? publicData.sourceGaps ?? [], permissions: permissions(me.permissions),
      binding: binding(session),
      me: safePayload(me), public: safePayload(publicData),
    });
  } catch (error) { return promotionError(error); }
}

export async function promotionProxy(request: Request, path: string[]): Promise<Response> {
  try {
    if (path.length !== 1 || !Object.hasOwn(ROUTES, path[0])) promotionFail(404, "promotion_endpoint_not_found");
    // Accept the earlier frontend public?code shape without forwarding unknown fields
    // to the gateway's strict public query parser.
    const route = path[0] === "public" && new URL(request.url).searchParams.has("code") ? "invite" : path[0];
    if (!ROUTES[route].includes(request.method)) promotionFail(405, "promotion_method_not_allowed");
    const write = request.method === "POST";
    promotionBrowserRequest(request, write);
    if (route === "click") promotionCookieReady();
    const search = write ? "" : query(request, route);
    if (write && new URL(request.url).search) promotionFail(400, "unexpected_write_query");
    const session = ["public", "invite", "click"].includes(route) ? null : await promotionSession(request);
    let body: Record<string, unknown> | undefined;
    let idempotencyKey: string | undefined;
    if (write) {
      if (route !== "click") {
        promotionCsrf(request, session!);
        idempotencyKey = request.headers.get("idempotency-key") ?? "";
        if (!/^[A-Za-z0-9_-]{16,128}$/.test(idempotencyKey)) promotionFail(400, "idempotency_key_required");
      }
      body = await promotionReadBody(request);
      const command = body.command && typeof body.command === "object" && !Array.isArray(body.command) ? body.command as Record<string, unknown> : null;
      bodyWithoutIdentity(body, [], route === "admin" && command?.action === "excludeMember");
      if (route !== "payout-address") {
        const campaign = campaignId(body.campaignId);
        if (!campaign) promotionFail(503, "promotion_campaign_not_configured");
        body.campaignId = campaign;
      }
      if (route === "click" && (Object.keys(body).sort().join(",") !== "campaignId,code" ||
        typeof body.code !== "string" || !body.code || body.code.length > 128 || body.code !== body.code.trim())) promotionFail(400, "invalid_invitation_code");
      if (route === "candidate") {
        if (Object.hasOwn(body, "clickId")) promotionFail(400, "browser_click_id_not_allowed");
        if (body.source === "invite_link") body.clickId = promotionReadClick(request, body.campaignId as string, body.code);
      }
    }
    const result = await promotionGateway(`/v1/promotions/${route}`, {
      method: write ? "POST" : "GET", token: session?.token, body, idempotencyKey, query: search,
    });
    // SQL, infrastructure and other server diagnostics stay on the trusted side.
    const response = promotionJson(result.status >= 500 ? { error: "promotion_gateway_unavailable" } : safePayload(result.payload), result.status);
    if (result.ok && route === "click") promotionSaveClick(request, response, body!.campaignId as string, body!.code as string, result.payload);
    if (result.ok && route === "confirm") promotionClearClick(response);
    return response;
  } catch (error) { return promotionError(error); }
}
