"use client";

import { requestChannelSessionToken, type SuiLoginToken } from "./passkey-auth";

export type DubheQrLoginState = {
  status: "creating" | "pending" | "approved" | "rejected" | "expired" | "error";
  approvalUrl?: string;
  expiresAt?: number;
  error?: string;
};

type LoginOptions = {
  signal: AbortSignal;
  isCurrent: () => boolean;
  onState: (state: DubheQrLoginState) => void;
};

const REQUEST_TIMEOUT_MS = 12_000;
const RETRY_DELAY_MS = 250;

function assertCurrent(options: LoginOptions) {
  options.signal.throwIfAborted();
  if (!options.isCurrent()) throw new DOMException("Login attempt retired", "AbortError");
}

async function readResponse(response: Response): Promise<Record<string, unknown>> {
  if (!response.ok) {
    void response.body?.cancel().catch(() => undefined);
    throw new Error("Dubhe Wallet login unavailable");
  }
  const body: unknown = await response.json();
  if (!body || typeof body !== "object" || Array.isArray(body)) {
    throw new Error("Dubhe Wallet login unavailable");
  }
  return body as Record<string, unknown>;
}

function delay(ms: number, signal: AbortSignal): Promise<void> {
  signal.throwIfAborted();
  return new Promise((resolve, reject) => {
    const abort = () => {
      clearTimeout(timer);
      reject(signal.reason);
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", abort);
      resolve();
    }, ms);
    signal.addEventListener("abort", abort, { once: true });
  });
}

async function waitForVisible(options: LoginOptions, expiresAt: number) {
  while (typeof document !== "undefined" && document.visibilityState !== "visible") {
    assertCurrent(options);
    if (Date.now() >= expiresAt) return false;
    await delay(Math.min(250, expiresAt - Date.now()), options.signal);
  }
  assertCurrent(options);
  return Date.now() < expiresAt;
}

async function requestQrJson(options: LoginOptions, session?: { intentToken: string; expiresAt: number }) {
  const path = session ? "/api/wallet/dubhe/session/status" : "/api/wallet/dubhe/session";
  const headers = session ? { authorization: `Bearer ${session.intentToken}` } : undefined;
  for (let attempt = 0; ; attempt++) {
    assertCurrent(options);
    if (session && !await waitForVisible(options, session.expiresAt)) return null;
    const remaining = session ? session.expiresAt - Date.now() : REQUEST_TIMEOUT_MS;
    if (remaining <= 0) return null;
    const deadline = AbortSignal.timeout(Math.min(REQUEST_TIMEOUT_MS, Math.ceil(remaining)));
    const signal = AbortSignal.any([options.signal, deadline]);
    let response: Response | undefined;
    try {
      response = await fetch(path, { method: "POST", cache: "no-store", credentials: "same-origin", headers, signal });
      return await readResponse(response);
    } catch (error) {
      assertCurrent(options);
      if (session && Date.now() >= session.expiresAt) return null;
      const timedOut = deadline.aborted && (error === deadline.reason || error instanceof DOMException
        && (error.name === "AbortError" || error.name === "TimeoutError"));
      // HTTP, JSON and protocol errors are terminal. Only a failed transport
      // or interrupted response stream may get one more attempt.
      if (attempt !== 0 || response?.ok === false || (!timedOut && !(error instanceof TypeError))) throw error;
    }
    await delay(session ? Math.min(RETRY_DELAY_MS, session.expiresAt - Date.now()) : RETRY_DELAY_MS, options.signal);
  }
}

/** The bearer intent lives only in this call; the QR contains only the wallet approval URL. */
export async function requestDubheWalletLoginToken(options: LoginOptions): Promise<SuiLoginToken | null> {
  const created = await requestQrJson(options);
  assertCurrent(options);
  if (!created) return null;
  const { approvalUrl, expiresAt, intentToken } = created;
  if (typeof approvalUrl !== "string" || typeof intentToken !== "string" || !intentToken
    || typeof expiresAt !== "number" || !Number.isFinite(expiresAt)
    || expiresAt <= Date.now() || expiresAt > Date.now() + 300_000) {
    throw new Error("Invalid Dubhe Wallet login response");
  }
  const approval = new URL(approvalUrl);
  if (approval.protocol !== "https:" && !(approval.protocol === "http:"
    && ["localhost", "127.0.0.1", "[::1]"].includes(approval.hostname))) {
    throw new Error("Invalid Dubhe Wallet approval URL");
  }
  options.onState({ status: "pending", approvalUrl, expiresAt });
  while (true) {
    const result = await requestQrJson(options, { intentToken, expiresAt });
    assertCurrent(options);
    if (!result || Date.now() >= expiresAt || result.status === "expired") {
      options.onState({ status: "expired", expiresAt });
      return null;
    }
    if (result.status === "rejected") {
      options.onState({ status: "rejected" });
      return null;
    }
    if (result.status === "approved") {
      const proof = result.login as Partial<SuiLoginToken> | null;
      if (!proof || proof.provider !== "suiWallet" || typeof proof.accountId !== "string"
        || !proof.accountId.startsWith("sui:") || typeof proof.token !== "string" || !proof.token
        || typeof proof.expiresAt !== "number" || proof.expiresAt <= Date.now() || proof.expiresAt > expiresAt) {
        throw new Error("Invalid Dubhe Wallet login proof");
      }
      options.onState({ status: "approved", expiresAt });
      // Opening the same-device approval link can hide the game tab. Resume
      // claiming the login when the player returns, within the signed deadline.
      if (!await waitForVisible(options, expiresAt)) {
        options.onState({ status: "expired", expiresAt });
        return null;
      }
      assertCurrent(options);
      const login = await requestChannelSessionToken("suiWallet", proof.token, options.signal);
      if (!await waitForVisible(options, login.expiresAt)) throw new Error("Dubhe Wallet login expired");
      assertCurrent(options);
      return login;
    }
    if (result.status !== "pending") throw new Error("Invalid Dubhe Wallet login status");
    await delay(Math.min(1_500, expiresAt - Date.now()), options.signal);
  }
}
