import "server-only";

import { createHmac, timingSafeEqual } from "node:crypto";
import { isValidSuiAddress, normalizeSuiAddress } from "@mysten/sui/utils";
import { verifyPersonalMessageSignature } from "@mysten/sui/verify";

import {
  issueGatewayPasskeyToken,
  passkeyGatewaySecret,
  passkeyRequestOriginAllowed,
} from "./passkey-auth";

// Dubhe Connect v2 contract, pinned to upstream c318c458490bd0115f4df35827d6c418d7c1227b:
// github.com/0xobelisk/dubhe-wallet/packages/shared/src/{connect,connectVerify}.ts
// The upstream @dubhe-hub/shared package is private and is not published to npm.
const INTENT_AUTH = "mir2-dubhe-connect-intent-v1";
const SESSION_TTL_MS = 120_000;
const CLOCK_SKEW_MS = 30_000;
const MAX_RELAY_BYTES = 32_768;
const MAX_INTENT_LENGTH = 12_000;
const APP_NAME = "Mir2 Web3";
const STATEMENT = "Sign in to Mir2 Web3 with Dubhe Wallet.";
const NO_STORE = { "Cache-Control": "no-store", Pragma: "no-cache" };

type ConnectRequest = {
  appName: string;
  origin: string;
  network: "testnet" | "mainnet";
  walletOrigin: string;
  statement: string;
  nonce: string;
  requestedAt: string;
  expiresAt: string;
};

type ConnectEnvelope = {
  id: string;
  relayOrigin: string;
  approvalUrl: string;
  pollUrl: string;
  request: ConnectRequest;
};

type ConnectSession = ConnectEnvelope & {
  status: "pending" | "approved" | "rejected" | "expired";
  result?: {
    account: { address: string; publicKey: string; chains: string[] };
    message: string;
    bytes: string;
    signature: string;
    approvedAt: string;
  };
};

type ConnectIntent = {
  auth: typeof INTENT_AUTH;
  iatMs: number;
  expMs: number;
  session: ConnectEnvelope;
};

class LoginError extends Error {
  constructor(readonly status: number) {
    super("Dubhe Wallet login could not be verified");
  }
}

function fail(status = 401): never {
  throw new LoginError(status);
}

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail();
  return value as Record<string, unknown>;
}

function text(value: unknown, maxLength: number): string {
  if (typeof value !== "string" || !value.length || value.length > maxLength) fail();
  if (value !== value.trim() || /[\r\n\u0000]/.test(value)) fail();
  return value;
}

function secureOrigin(value: unknown): string {
  const input = text(value, 2_048);
  const url = new URL(input);
  if (
    url.username || url.password ||
    (url.protocol !== "https:" && !(url.protocol === "http:" &&
      ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)))
  ) fail();
  if (input !== url.origin) fail();
  return url.origin;
}

function config() {
  const relayOrigin = secureOrigin(
    process.env.MIR2_DUBHE_CONNECT_RELAY_URL?.trim() ||
      "https://dubhe-connect-relay.zombiesliu.workers.dev",
  );
  const walletOrigin = secureOrigin(
    process.env.MIR2_DUBHE_WALLET_ORIGIN?.trim() || "https://wallet.dubhe.obelisk.build",
  );
  const network = process.env.MIR2_DUBHE_CONNECT_NETWORK?.trim() || "testnet";
  if (network !== "testnet" && network !== "mainnet") fail(503);
  return { relayOrigin, walletOrigin, network };
}

function timestamp(value: unknown): string {
  const input = text(value, 40);
  const parsed = Date.parse(input);
  // The wallet/relay contract uses Date.toISOString(), not numeric epochs.
  if (!Number.isFinite(parsed) || new Date(parsed).toISOString() !== input) fail();
  return input;
}

function envelope(value: unknown): ConnectEnvelope {
  const session = object(value);
  const request = object(session.request);
  const id = text(session.id, 256);
  if (!/^[A-Za-z0-9_-]+$/.test(id)) fail();
  const relayOrigin = secureOrigin(session.relayOrigin);
  const walletOrigin = secureOrigin(request.walletOrigin);
  const origin = secureOrigin(request.origin);
  if (request.callbackUrl !== undefined && request.callbackUrl !== null) fail();
  const network = text(request.network, 40);
  if (network !== "testnet" && network !== "mainnet") fail();
  const requestedAt = timestamp(request.requestedAt);
  const expiresAt = timestamp(request.expiresAt);
  const ttl = Date.parse(expiresAt) - Date.parse(requestedAt);
  if (ttl <= 0 || ttl > SESSION_TTL_MS) fail();
  const approvalUrl = text(session.approvalUrl, 4_096);
  const approval = new URL(approvalUrl);
  if (
    approval.origin !== walletOrigin || approval.pathname !== "/connect/approve" ||
    approval.username || approval.password || approval.hash ||
    approval.searchParams.getAll("session").length !== 1 ||
    approval.searchParams.getAll("relay").length !== 1 ||
    [...approval.searchParams.keys()].some((key) => key !== "session" && key !== "relay") ||
    approval.searchParams.get("session") !== id ||
    approval.searchParams.get("relay") !== relayOrigin
  ) fail();
  const pollUrl = text(session.pollUrl, 4_096);
  if (pollUrl !== `${relayOrigin}/sessions/${id}`) fail();
  return {
    id, relayOrigin, approvalUrl, pollUrl,
    request: {
      appName: text(request.appName, 120), origin, network, walletOrigin,
      statement: text(request.statement, 600), nonce: text(request.nonce, 160),
      requestedAt, expiresAt,
    },
  };
}

function sessionStatus(value: unknown): ConnectSession["status"] {
  if (value !== "pending" && value !== "approved" && value !== "rejected" && value !== "expired") fail();
  return value;
}

function signIntent(intent: ConnectIntent): string {
  const payload = Buffer.from(JSON.stringify(intent)).toString("base64url");
  const mac = createHmac("sha256", passkeyGatewaySecret())
    .update(`${INTENT_AUTH}\0${payload}`).digest("base64url");
  return `${payload}.${mac}`;
}

function verifyIntent(token: string, origin: string): ConnectIntent {
  if (token.length > MAX_INTENT_LENGTH || !/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/.test(token)) fail();
  const [payload, mac] = token.split(".");
  const actual = Buffer.from(mac, "base64url");
  const expected = createHmac("sha256", passkeyGatewaySecret())
    .update(`${INTENT_AUTH}\0${payload}`).digest();
  if (actual.length !== expected.length || !timingSafeEqual(actual, expected)) fail();
  const intent = object(JSON.parse(Buffer.from(payload, "base64url").toString("utf8")));
  const trusted = envelope(intent.session);
  const { relayOrigin, walletOrigin, network } = config();
  const iatMs = intent.iatMs;
  const expMs = intent.expMs;
  if (
    intent.auth !== INTENT_AUTH || typeof iatMs !== "number" || typeof expMs !== "number" ||
    !Number.isSafeInteger(iatMs) || !Number.isSafeInteger(expMs) ||
    iatMs > Date.now() + CLOCK_SKEW_MS || expMs <= iatMs || expMs - iatMs > SESSION_TTL_MS ||
    expMs !== Date.parse(trusted.request.expiresAt) || trusted.request.origin !== origin ||
    trusted.relayOrigin !== relayOrigin || trusted.request.walletOrigin !== walletOrigin ||
    trusted.request.network !== network || trusted.request.appName !== APP_NAME ||
    trusted.request.statement !== STATEMENT
  ) fail();
  return { auth: INTENT_AUTH, iatMs, expMs, session: trusted };
}

async function boundedText(response: Response, maxBytes: number): Promise<string> {
  if (!response.ok || !response.body) fail(502);
  const reader = response.body.getReader();
  const parts: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > maxBytes) {
        await reader.cancel();
        fail(502);
      }
      parts.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  return Buffer.concat(parts).toString("utf8");
}

async function boundedJson(response: Response, maxBytes: number): Promise<unknown> {
  return JSON.parse(await boundedText(response, maxBytes));
}

async function relayJson(url: string, init?: RequestInit) {
  const response = await fetch(url, {
    ...init, cache: "no-store", redirect: "error", signal: AbortSignal.timeout(8_000),
  });
  return boundedJson(response, MAX_RELAY_BYTES);
}

async function allowedOrigin(request: Request): Promise<string> {
  const origin = request.headers.get("origin");
  if (!origin || !passkeyRequestOriginAllowed(origin, request.url)) fail(403);
  secureOrigin(origin);
  // These endpoints accept no client-selected relay, session, network or proof.
  if (request.body) {
    // NextRequest can expose an empty stream for a bodyless POST. Treat that
    // exactly like Request.body === null while still bounding any supplied body.
    const raw = await boundedText(new Response(request.body), 1_024).catch(() => fail(400));
    if (raw.trim()) {
      let body: unknown;
      try { body = JSON.parse(raw); } catch { fail(400); }
      if (Object.keys(object(body)).length) fail(400);
    }
  }
  return origin;
}

function response(data: unknown, status = 200) {
  return Response.json(data, { status, headers: NO_STORE });
}

function errorResponse(error: unknown) {
  return response({ error: "Dubhe Wallet login is unavailable or could not be verified" },
    error instanceof LoginError ? error.status : 503);
}

export async function createDubheConnectLogin(request: Request): Promise<Response> {
  try {
    const origin = await allowedOrigin(request);
    const { relayOrigin, walletOrigin, network } = config();
    // Fail closed before creating a public relay session if login signing is unconfigured.
    passkeyGatewaySecret();
    const iatMs = Date.now();
    const created = object(await relayJson(`${relayOrigin}/sessions`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        appName: APP_NAME, origin, network, walletOrigin, statement: STATEMENT,
        expiresInMs: SESSION_TTL_MS,
      }),
    }));
    const trusted = envelope(created);
    const expiresAt = Date.parse(trusted.request.expiresAt);
    const requestedAt = Date.parse(trusted.request.requestedAt);
    if (
      sessionStatus(created.status) !== "pending" || created.result !== undefined ||
      trusted.relayOrigin !== relayOrigin || trusted.request.walletOrigin !== walletOrigin ||
      trusted.request.network !== network || trusted.request.origin !== origin ||
      trusted.request.appName !== APP_NAME || trusted.request.statement !== STATEMENT ||
      Math.abs(requestedAt - iatMs) > CLOCK_SKEW_MS || expiresAt <= Date.now() ||
      expiresAt > iatMs + SESSION_TTL_MS + CLOCK_SKEW_MS
    ) fail(502);
    // Keep the proof deadline fixed, even across repeated polls or serverless instances.
    const intentIssuedAt = Math.max(iatMs, requestedAt);
    const intentToken = signIntent({
      auth: INTENT_AUTH, iatMs: intentIssuedAt, expMs: expiresAt, session: trusted,
    });
    return response({ sessionId: trusted.id, approvalUrl: trusted.approvalUrl, expiresAt, intentToken });
  } catch (error) {
    return errorResponse(error);
  }
}

export function buildDubheConnectLoginMessage(session: ConnectEnvelope): string {
  const { request } = session;
  return [
    "Dubhe Connect", "Version: 2", "", `Session ID: ${session.id}`,
    `Relay Origin: ${session.relayOrigin}`, `Wallet Origin: ${request.walletOrigin}`,
    `App: ${request.appName}`, `Origin: ${request.origin}`, `Network: ${request.network}`,
    "Callback URL: none", `Nonce: ${request.nonce}`, `Issued At: ${request.requestedAt}`,
    `Expires At: ${request.expiresAt}`, "", request.statement,
  ].join("\n");
}

export async function readDubheConnectLogin(request: Request): Promise<Response> {
  try {
    const origin = await allowedOrigin(request);
    const authorization = request.headers.get("authorization") ?? "";
    if (!authorization.startsWith("Bearer ")) fail();
    const intent = verifyIntent(authorization.slice(7), origin);
    if (intent.expMs <= Date.now()) return response({ status: "expired" });
    // Never fetch a poll URL or relay supplied by a browser or callback query string.
    const value = object(await relayJson(`${intent.session.relayOrigin}/sessions/${intent.session.id}`));
    const trusted = envelope(value);
    if (JSON.stringify(trusted) !== JSON.stringify(intent.session)) fail();
    const status = sessionStatus(value.status);
    if (intent.expMs <= Date.now() || status === "expired") return response({ status: "expired" });
    if (status === "pending" || status === "rejected") return response({ status });

    const result = object(value.result);
    const account = object(result.account);
    const address = text(account.address, 66);
    if (!isValidSuiAddress(address)) fail();
    const message = buildDubheConnectLoginMessage(trusted);
    const messageBytes = new TextEncoder().encode(message);
    const approvedAt = Date.parse(timestamp(result.approvedAt));
    if (
      result.message !== message || result.bytes !== Buffer.from(messageBytes).toString("base64") ||
      approvedAt < Date.parse(trusted.request.requestedAt) || approvedAt > intent.expMs ||
      approvedAt > Date.now() + CLOCK_SKEW_MS || !Array.isArray(account.chains) ||
      !account.chains.includes(`sui:${trusted.request.network}`)
    ) fail();
    const publicKey = await verifyPersonalMessageSignature(messageBytes, text(result.signature, 8_192), { address });
    if (publicKey.toBase64() !== text(account.publicKey, 4_096) || intent.expMs <= Date.now()) fail();
    const accountId = `sui:${normalizeSuiAddress(address)}`;
    // Intent is a browser-held capability, never included in the public QR. Gateway
    // channel exchange atomically consumes this deterministic jti exactly once in its
    // shared cache. Fixed expiry prevents reissuing a longer-lived proof after that
    // cache entry expires; no Next process-memory replay store is needed.
    const token = issueGatewayPasskeyToken(accountId, `dubhe-connect:${trusted.id}`, intent.expMs, "wallet");
    return response({ status: "approved", login: {
      accountId, provider: "suiWallet", token, expiresAt: intent.expMs,
    } });
  } catch (error) {
    return errorResponse(error);
  }
}
