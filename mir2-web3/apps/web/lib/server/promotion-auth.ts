import "server-only";

import { createCipheriv, createDecipheriv, createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { createDubheConnectLogin, readDubheConnectLogin } from "./dubhe-connect-auth";
import { promotionDedicatedConfigurationRequired, promotionWalletContext } from "./promotion-wallet-context";

const SESSION_COOKIE = "__Host-mir2-promotion-session";
const CHALLENGE_COOKIE = "__Host-mir2-promotion-challenge";
const CLICK_COOKIE = "__Host-mir2-promotion-click";
const MAX_BODY = 16_384;
const SESSION_MAX_MS = 12 * 60 * 60 * 1000;
const CHALLENGE_MAX_MS = 120_000;
const NO_STORE = { "Cache-Control": "no-store, private", Pragma: "no-cache", "X-Content-Type-Options": "nosniff" };

export class PromotionError extends Error {
  constructor(readonly status: number, readonly code: string) { super(code); }
}
export function promotionFail(status: number, code: string): never { throw new PromotionError(status, code); }
export function promotionJson(value: unknown, status = 200): Response {
  return Response.json(value, { status, headers: NO_STORE });
}
export function promotionError(error: unknown): Response {
  return promotionJson({ error: error instanceof PromotionError ? error.code : "promotion_service_unavailable" },
    error instanceof PromotionError ? error.status : 503);
}
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) promotionFail(502, "invalid_service_response");
  return value as Record<string, unknown>;
}
function origin(request: Request): string {
  // A reverse proxy / Next dev server may expose its internal localhost URL.
  // Pin the public origin in server configuration, never forwarded headers.
  const configured = process.env.MIR2_PROMOTION_PUBLIC_ORIGIN?.trim();
  if (promotionDedicatedConfigurationRequired() && !configured) promotionFail(503, "promotion_public_origin_required");
  const url = new URL(configured || request.url);
  if (url.username || url.password || (configured && configured !== url.origin)) promotionFail(503, "invalid_public_origin");
  if (url.protocol !== "https:" && !(url.protocol === "http:" && !promotionDedicatedConfigurationRequired() && process.env.NODE_ENV !== "production" &&
    ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname))) promotionFail(503, "https_required");
  return url.origin;
}
export function promotionBrowserRequest(request: Request, write = false) {
  const expected = origin(request);
  // Identity is always obtained from our cookie. Browser bearer/account selection is forbidden.
  if (request.headers.has("authorization")) promotionFail(400, "browser_bearer_not_allowed");
  if (write && request.headers.get("origin") !== expected) promotionFail(403, "cross_origin_request");
  const site = request.headers.get("sec-fetch-site");
  if (site === "cross-site") promotionFail(403, "cross_origin_request");
  return expected;
}
function key(): Buffer {
  const secret = process.env.MIR2_PROMOTION_SESSION_SECRET;
  if (!secret || secret.length < 32 || secret === process.env.MIR2_PASSKEY_AUTH_SECRET ||
      secret === process.env.MIR2_PROMOTION_WALLET_AUTH_SECRET) {
    promotionFail(503, "promotion_session_secret_required");
  }
  return createHash("sha256").update("mir2-promotion-cookie-v1\0").update(secret).digest();
}
export function promotionCookieReady() { key(); }
function seal(value: unknown, purpose: string, request: Request): string {
  const iv = randomBytes(12);
  const cipher = createCipheriv("aes-256-gcm", key(), iv);
  cipher.setAAD(Buffer.from(`mir2-promotion-v1\0${purpose}\0${origin(request)}`));
  const ciphertext = Buffer.concat([cipher.update(JSON.stringify(value), "utf8"), cipher.final()]);
  const token = Buffer.concat([iv, cipher.getAuthTag(), ciphertext]).toString("base64url");
  if (token.length > 3800) promotionFail(503, "promotion_cookie_too_large");
  return token;
}
function open(request: Request, name: string, purpose: string): Record<string, unknown> | null {
  key(); // Misconfiguration must never look like an anonymous authenticated fallback.
  const values = (request.headers.get("cookie") ?? "").split(";").map((v) => v.trim())
    .filter((v) => v.startsWith(`${name}=`)).map((v) => v.slice(name.length + 1));
  if (!values.length) return null;
  if (values.length !== 1 || values[0].length > 3800 || !/^[A-Za-z0-9_-]+$/.test(values[0])) {
    promotionFail(401, "invalid_promotion_cookie");
  }
  try {
    const bytes = Buffer.from(values[0], "base64url");
    if (bytes.length < 29) promotionFail(401, "invalid_promotion_cookie");
    const decipher = createDecipheriv("aes-256-gcm", key(), bytes.subarray(0, 12));
    decipher.setAAD(Buffer.from(`mir2-promotion-v1\0${purpose}\0${origin(request)}`));
    decipher.setAuthTag(bytes.subarray(12, 28));
    return record(JSON.parse(Buffer.concat([decipher.update(bytes.subarray(28)), decipher.final()]).toString("utf8")));
  } catch { promotionFail(401, "invalid_promotion_cookie"); }
}
function cookie(response: Response, name: string, token: string, maxAge: number) {
  response.headers.append("set-cookie", `${name}=${token}; Path=/; Max-Age=${maxAge}; HttpOnly; Secure; SameSite=Strict`);
}
function clear(response: Response, name: string) { cookie(response, name, "", 0); }
export async function promotionReadBody(request: Request, limit = MAX_BODY): Promise<Record<string, unknown>> {
  if (request.headers.get("content-type")?.split(";")[0].trim() !== "application/json" && request.body) {
    // Bodyless POSTs also work with NextRequest, which sometimes exposes an empty stream.
    const raw = await readBytes(new Response(request.body), limit);
    if (raw.trim()) promotionFail(415, "json_required");
    return {};
  }
  const raw = request.body ? await readBytes(new Response(request.body), limit) : "";
  try { return raw.trim() ? record(JSON.parse(raw)) : {}; }
  catch { promotionFail(400, "invalid_json_body"); }
}
async function emptyBody(request: Request) {
  if (Object.keys(await promotionReadBody(request, 1024)).length) promotionFail(400, "unexpected_auth_parameters");
}
async function readBytes(response: Response, limit: number): Promise<string> {
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) { await reader.cancel(); promotionFail(413, "response_or_request_too_large"); }
      chunks.push(value);
    }
  } finally { reader.releaseLock(); }
  return Buffer.concat(chunks).toString("utf8");
}
function gateway(): string {
  const value = process.env.MIR2_PROMOTION_GATEWAY_HTTP_URL?.trim() ||
    (promotionDedicatedConfigurationRequired() ? undefined : process.env.MIR2_GATEWAY_HTTP_URL?.trim());
  if (!value) promotionFail(503, "promotion_gateway_not_configured");
  let url: URL;
  try { url = new URL(value); } catch { promotionFail(503, "invalid_promotion_gateway"); }
  // This server configuration is the only destination. No browser URL, redirect or forwarded host is used.
  if (url.username || url.password || url.search || url.hash ||
    (url.protocol !== "https:" && !(url.protocol === "http:" && ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname))) ||
    !/^\/(?:playtest\/?)?$/.test(url.pathname)) promotionFail(503, "invalid_promotion_gateway");
  const allowed = (process.env.MIR2_PROMOTION_GATEWAY_ALLOWED_ORIGINS ?? url.origin).split(",").map((v) => v.trim());
  if (allowed.some(value => {
    try { const target = new URL(value); return value !== target.origin || !!target.username || !!target.password; }
    catch { return true; }
  })) promotionFail(503, "promotion_gateway_not_allowed");
  if (!allowed.includes(url.origin)) promotionFail(503, "promotion_gateway_not_allowed");
  return value.replace(/\/+$/, "");
}
const GATEWAY_PATHS = new Set([
  "/v1/promotions/auth/exchange", "/v1/promotions/auth/password", "/v1/identity/me", "/v1/identity/sessions/revoke",
  ...["public", "invite", "click", "me", "apply", "candidate", "confirm", "members", "settlements", "export", "payout-address", "appeals", "admin"]
    .map((path) => `/v1/promotions/${path}`),
  "/v1/billing/status", "/v1/billing/refresh", "/v1/billing/sui-usdc/checkout",
  "/v1/billing/monthly-card/buy", "/v1/billing/monthly-card/activate",
  "/v1/promotions/game-session",
]);
export async function promotionGateway(path: string, options: { method?: "GET" | "POST"; token?: string; body?: unknown; idempotencyKey?: string; query?: string } = {}) {
  if (!GATEWAY_PATHS.has(path)) promotionFail(404, "promotion_endpoint_not_found");
  const headers = new Headers({ accept: "application/json" });
  if (options.token) headers.set("authorization", `Bearer ${options.token}`);
  if (options.body !== undefined) headers.set("content-type", "application/json");
  if (options.idempotencyKey) headers.set("idempotency-key", options.idempotencyKey);
  // This is the fixed server-to-server checkout origin, never browser input.
  if (path.startsWith("/v1/billing/")) headers.set("origin", new URL(gateway()).origin);
  let response: Response;
  try {
    response = await fetch(`${gateway()}${path}${options.query ?? ""}`, {
      method: options.method ?? "GET", headers,
      ...(options.body === undefined ? {} : { body: JSON.stringify(options.body) }),
      redirect: "error", cache: "no-store", signal: AbortSignal.timeout(8000),
    });
  } catch (error) {
    if (error instanceof PromotionError) throw error;
    promotionFail(502, "promotion_gateway_unavailable");
  }
  if (!response.headers.get("content-type")?.toLowerCase().startsWith("application/json")) promotionFail(502, "invalid_service_response");
  let payload: Record<string, unknown>;
  try { payload = record(JSON.parse(await readBytes(response, 1_048_576))); }
  catch { promotionFail(502, "invalid_service_response"); }
  return { status: response.status, ok: response.ok, payload };
}
type PromotionSession = {
  token: string; accountId: string; sessionId: string; expMs: number; csrfToken: string;
  authMethod?: string; issuedAtMs?: number; walletBound?: boolean;
  walletCredentialId?: string;
};
export async function promotionSession(request: Request, required = true): Promise<PromotionSession | null> {
  const value = open(request, SESSION_COOKIE, "session");
  if (!value) { if (required) promotionFail(401, "promotion_login_required"); return null; }
  const { token, accountId, sessionId, expMs, csrfToken } = value;
  if (typeof token !== "string" || !token || token.length > 4096 || typeof accountId !== "string" || !accountId ||
    typeof sessionId !== "string" || !sessionId || typeof expMs !== "number" || !Number.isSafeInteger(expMs) || expMs <= Date.now() ||
    expMs > Date.now() + SESSION_MAX_MS || typeof csrfToken !== "string" || !/^[A-Za-z0-9_-]{43}$/.test(csrfToken)) {
    promotionFail(401, "promotion_session_expired");
  }
  const verified = await promotionGateway("/v1/identity/me", { token });
  if (!verified.ok || verified.payload.accountId !== accountId || verified.payload.currentSessionId !== sessionId) {
    promotionFail(verified.status >= 500 ? 503 : 401, verified.status >= 500 ? "identity_service_unavailable" : "promotion_session_revoked");
  }
  const credentials = verified.payload.credentials;
  const sessions = verified.payload.sessions;
  const currentSession = Array.isArray(sessions) ? sessions.find((item: unknown) =>
    !!item && typeof item === "object" && (item as Record<string, unknown>).sessionId === sessionId) as Record<string, unknown> | undefined : undefined;
  const walletCredential = Array.isArray(credentials) ? credentials.find((item: unknown) => {
    if (!item || typeof item !== "object") return false;
    const entry = item as Record<string, unknown>;
    return currentSession?.authMethod === "sui_wallet" && currentSession.credentialId === entry.credentialId &&
      entry.credentialKind === "sui_wallet" && entry.revokedAtMs == null && typeof entry.credentialId === "string";
  }) as Record<string, unknown> | undefined : undefined;
  return { token, accountId, sessionId, expMs, csrfToken, walletBound: !!walletCredential,
    walletCredentialId: walletCredential?.credentialId as string | undefined,
    authMethod: typeof value.authMethod === "string" ? value.authMethod : undefined,
    issuedAtMs: typeof value.issuedAtMs === "number" ? value.issuedAtMs : undefined };
}
export function promotionCsrf(request: Request, session: PromotionSession) {
  promotionBrowserRequest(request, true);
  const actual = Buffer.from(request.headers.get("x-promotion-csrf") ?? "");
  const expected = Buffer.from(session.csrfToken);
  if (actual.length !== expected.length || !timingSafeEqual(actual, expected)) promotionFail(403, "promotion_csrf_failed");
}

export function promotionSaveClick(request: Request, response: Response, campaignId: string, code: string, payload: Record<string, unknown>) {
  if (typeof payload.clickId !== "string" || !/^[A-Za-z0-9_-]{1,128}$/.test(payload.clickId) ||
    typeof payload.expiresAtMs !== "number" || !Number.isSafeInteger(payload.expiresAtMs) || payload.expiresAtMs <= Date.now() ||
    payload.expiresAtMs > Date.now() + 7 * 24 * 60 * 60 * 1000 + 30_000) promotionFail(502, "invalid_invitation_click");
  cookie(response, CLICK_COOKIE, seal({ clickId: payload.clickId, campaignId, expMs: payload.expiresAtMs,
    codeHash: createHash("sha256").update(code).digest("hex") }, "click", request), Math.ceil((payload.expiresAtMs - Date.now()) / 1000));
}
export function promotionReadClick(request: Request, campaignId: string, code: unknown): string {
  const value = open(request, CLICK_COOKIE, "click");
  if (!value || typeof value.clickId !== "string" || value.campaignId !== campaignId ||
    typeof value.expMs !== "number" || value.expMs <= Date.now() || typeof code !== "string" ||
    value.codeHash !== createHash("sha256").update(code).digest("hex")) promotionFail(409, "promotion_click_required");
  return value.clickId;
}
export function promotionClearClick(response: Response) { clear(response, CLICK_COOKIE); }

function establishSession(grant: Record<string, unknown>, request: Request): Response {
  const identity = record(grant.session);
  if (typeof grant.token !== "string" || !grant.token || grant.token.length > 4096 || typeof identity.accountId !== "string" ||
    !identity.accountId || typeof identity.sessionId !== "string" || !identity.sessionId ||
    typeof identity.expiresAtMs !== "number" || !Number.isSafeInteger(identity.expiresAtMs) || identity.expiresAtMs <= Date.now()) {
    promotionFail(502, "invalid_identity_session");
  }
  const session: PromotionSession = { token: grant.token, accountId: identity.accountId, sessionId: identity.sessionId,
    expMs: Math.min(identity.expiresAtMs, Date.now() + SESSION_MAX_MS), csrfToken: randomBytes(32).toString("base64url"),
    authMethod: typeof identity.authMethod === "string" ? identity.authMethod : undefined,
    issuedAtMs: typeof identity.issuedAtMs === "number" ? identity.issuedAtMs : undefined };
  const response = promotionJson({ status: "approved" });
  cookie(response, SESSION_COOKIE, seal(session, "session", request), Math.floor((session.expMs - Date.now()) / 1000));
  clear(response, CHALLENGE_COOKIE);
  return response;
}

export async function promotionPassword(request: Request): Promise<Response> {
  try {
    promotionBrowserRequest(request, true); key(); gateway();
    const body = await promotionReadBody(request, 2048);
    if (Object.keys(body).sort().join(",") !== "accountId,password" || typeof body.accountId !== "string" ||
      !body.accountId.trim() || body.accountId.length > 128 || /[\r\n\u0000]/.test(body.accountId) ||
      typeof body.password !== "string" || !body.password || body.password.length > 256 || /[\r\n\u0000]/.test(body.password)) {
      promotionFail(400, "invalid_account_credentials");
    }
    // This endpoint proves an existing password account. It cannot register accounts,
    // accept a display name as ownership, or carry a browser-selected role.
    const exchanged = await promotionGateway("/v1/promotions/auth/password", { method: "POST", body });
    if (!exchanged.ok) promotionFail(exchanged.status, "password_verification_failed");
    return establishSession(exchanged.payload, request);
  } catch (error) { return promotionError(error); }
}
export async function promotionChallenge(request: Request): Promise<Response> {
  try {
    promotionBrowserRequest(request, true); key(); gateway(); await emptyBody(request);
    const context = promotionWalletContext();
    const result = await createDubheConnectLogin(new Request(request.url, { method: "POST", headers: { origin: origin(request) } }), context);
    if (!result.ok) return promotionJson({ error: "wallet_challenge_unavailable" }, result.status);
    const payload = record(await result.json());
    const { intentToken, approvalUrl, expiresAt } = payload;
    if (typeof intentToken !== "string" || typeof approvalUrl !== "string" || typeof expiresAt !== "number" ||
      expiresAt <= Date.now() || expiresAt > Date.now() + CHALLENGE_MAX_MS + 30_000) promotionFail(502, "invalid_wallet_challenge");
    const existing = open(request, SESSION_COOKIE, "session");
    const value = { intentToken, intentHash: createHash("sha256").update(intentToken).digest("hex"),
      nonce: randomBytes(32).toString("base64url"), expMs: expiresAt,
      linkSessionId: existing?.authMethod === "password" ? existing.sessionId : undefined };
    const response = promotionJson({ status: "pending", approvalUrl, expiresAt });
    cookie(response, CHALLENGE_COOKIE, seal(value, "challenge", request), Math.ceil((expiresAt - Date.now()) / 1000));
    return response;
  } catch (error) { return promotionError(error); }
}
export async function promotionPoll(request: Request): Promise<Response> {
  let retire = false;
  try {
    promotionBrowserRequest(request, true); await emptyBody(request);
    const context = promotionWalletContext();
    const challenge = open(request, CHALLENGE_COOKIE, "challenge");
    if (!challenge || typeof challenge.intentToken !== "string" || typeof challenge.nonce !== "string" ||
      !/^[A-Za-z0-9_-]{43}$/.test(challenge.nonce) || typeof challenge.expMs !== "number" ||
      challenge.intentHash !== createHash("sha256").update(challenge.intentToken).digest("hex")) promotionFail(401, "wallet_browser_challenge_required");
    if (challenge.expMs <= Date.now()) { const response = promotionJson({ status: "expired" }); clear(response, CHALLENGE_COOKIE); return response; }
    const result = await readDubheConnectLogin(new Request(request.url, {
      method: "POST", headers: { origin: origin(request), authorization: `Bearer ${challenge.intentToken}` },
    }), context);
    if (!result.ok) promotionFail(result.status, "wallet_signature_not_verified");
    const payload = record(await result.json());
    if (payload.status === "pending") return promotionJson({ status: "pending" });
    retire = true;
    if (payload.status === "expired" || payload.status === "rejected") {
      const response = promotionJson({ status: payload.status }); clear(response, CHALLENGE_COOKIE); return response;
    }
    const login = record(payload.login);
    if (payload.status !== "approved" || login.provider !== "suiWallet" || typeof login.token !== "string" ||
      typeof login.expiresAt !== "number" || login.expiresAt <= Date.now() || login.expiresAt > challenge.expMs) promotionFail(401, "invalid_wallet_proof");
    // The gateway consumes the real Dubhe proof jti once, resolves its existing account mapping,
    // and issues its existing persisted, revocable identity session. Never exchange a browser account ID.
    let linkingToken: string | undefined;
    if (challenge.linkSessionId !== undefined) {
      const existing = await promotionSession(request);
      if (existing!.sessionId !== challenge.linkSessionId || existing!.authMethod !== "password" ||
        typeof existing!.issuedAtMs !== "number" || existing!.issuedAtMs <= Date.now() - 10 * 60 * 1000 ||
        existing!.issuedAtMs > Date.now() + 30_000) promotionFail(401, "fresh_password_verification_required");
      linkingToken = existing!.token;
    }
    const exchanged = await promotionGateway("/v1/promotions/auth/exchange", {
      method: "POST", token: linkingToken, body: { provider: "suiWallet", credential: login.token },
    });
    if (!exchanged.ok) promotionFail(exchanged.status, "wallet_session_exchange_failed");
    return establishSession(exchanged.payload, request);
  } catch (error) { const response = promotionError(error); if (retire) clear(response, CHALLENGE_COOKIE); return response; }
}
export async function promotionLogout(request: Request): Promise<Response> {
  try {
    promotionBrowserRequest(request, true); await emptyBody(request);
    const session = await promotionSession(request);
    promotionCsrf(request, session!);
    const upstream = await promotionGateway("/v1/identity/sessions/revoke", {
      method: "POST", token: session!.token, body: { sessionId: session!.sessionId, reason: "promotion_logout" },
    });
    if (!upstream.ok) promotionFail(upstream.status, "promotion_logout_revocation_failed");
    const response = promotionJson({ accepted: true }); clear(response, SESSION_COOKIE); clear(response, CHALLENGE_COOKIE); clear(response, CLICK_COOKIE); return response;
  } catch (error) { return promotionError(error); }
}
