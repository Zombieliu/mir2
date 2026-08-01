import { verifyPersonalMessageSignature } from "@mysten/sui/verify";
import { isValidSuiAddress, normalizeSuiAddress } from "@mysten/sui/utils";
import { NextResponse } from "next/server";

import {
  issueGatewayPasskeyToken,
  passkeyRequestOriginAllowed,
  verifyPasskeyChallenge,
} from "../../../../lib/server/passkey-auth";

export const runtime = "nodejs";

const SUI_LOGIN_PURPOSES = new Set(["mir2-sui-login", "mir2-passkey-login"]);
const MAX_LOGIN_WINDOW_MS = 120_000;

type PasskeyLoginRequest = {
  address?: string;
  message?: string;
  signature?: string;
};

type PasskeyLoginMessage = {
  purpose?: string;
  accountId?: string;
  address?: string;
  origin?: string;
  issuedAt?: number;
  expiresAt?: number;
  nonce?: string;
  challenge?: string;
  authMethod?: "passkey" | "wallet";
};

export async function POST(request: Request) {
  const body = (await request.json().catch(() => null)) as PasskeyLoginRequest | null;
  if (!body?.address || !body.message || !body.signature) {
    return errorResponse("address, message, and signature are required", 400);
  }

  let loginMessage: PasskeyLoginMessage;
  try {
    loginMessage = JSON.parse(body.message) as PasskeyLoginMessage;
  } catch {
    return errorResponse("invalid passkey message", 400);
  }

  const requestOrigin = request.headers.get("origin");
  if (!passkeyRequestOriginAllowed(requestOrigin, request.url)) {
    return errorResponse("passkey request Origin is not allowed", 403);
  }
  const validationError = validateLoginMessage(loginMessage, body.address, requestOrigin);
  if (validationError) {
    return errorResponse(validationError, 400);
  }

  let challenge;
  try {
    challenge = verifyPasskeyChallenge(loginMessage.challenge!);
  } catch (error) {
    return errorResponse(error instanceof Error ? error.message : "invalid passkey challenge", 401);
  }
  const now = Date.now();
  if (
    challenge.jti !== loginMessage.nonce ||
    normalizeSuiAddress(challenge.address) !== normalizeSuiAddress(body.address) ||
    challenge.origin !== requestOrigin ||
    challenge.iatMs !== loginMessage.issuedAt ||
    challenge.expMs !== loginMessage.expiresAt ||
    challenge.authMethod !== loginMessage.authMethod ||
    challenge.expMs <= now
  ) {
    return errorResponse("passkey challenge mismatch or expired", 401);
  }

  try {
    await verifyPersonalMessageSignature(new TextEncoder().encode(body.message), body.signature, {
      address: body.address,
    });
  } catch {
    return errorResponse("invalid passkey signature", 401);
  }

  const accountId = loginMessage.accountId!;
  const expiresAt = Math.min(loginMessage.expiresAt!, Date.now() + 60_000);
  let token: string;
  try {
    token = issueGatewayPasskeyToken(accountId, challenge.jti, expiresAt, challenge.authMethod);
  } catch (error) {
    return errorResponse(error instanceof Error ? error.message : "passkey auth is not configured", 500);
  }
  return NextResponse.json({
    accountId,
    token,
    expiresAt,
  });
}

function validateLoginMessage(
  message: PasskeyLoginMessage,
  address: string,
  requestOrigin: string | null,
) {
  const now = Date.now();
  if (!message.purpose || !SUI_LOGIN_PURPOSES.has(message.purpose)) return "invalid passkey purpose";
  if (!message.address || !isValidSuiAddress(message.address) || !isValidSuiAddress(address)) {
    return "invalid passkey address";
  }
  const normalizedAddress = normalizeSuiAddress(address);
  if (normalizeSuiAddress(message.address) !== normalizedAddress) return "passkey address mismatch";
  if (message.accountId !== `sui:${normalizedAddress}`) return "passkey account mismatch";
  if (!message.nonce) return "passkey nonce is required";
  if (!message.challenge) return "server passkey challenge is required";
  if (message.authMethod !== "passkey" && message.authMethod !== "wallet") {
    return "invalid authentication method";
  }
  if (!Number.isFinite(message.issuedAt) || !Number.isFinite(message.expiresAt)) {
    return "invalid passkey timestamp";
  }
  if (message.issuedAt! > now + 30_000 || message.expiresAt! <= now) {
    return "expired passkey message";
  }
  if (message.expiresAt! - message.issuedAt! > MAX_LOGIN_WINDOW_MS) {
    return "passkey message window is too long";
  }
  // Origin binding is mandatory: the signed message must carry an origin and it
  // must match the request's Origin header. Browser passkey logins always send
  // the Origin header, so we fail closed rather than skipping the check when it
  // is absent.
  if (!message.origin) return "passkey origin is required";
  if (!requestOrigin || message.origin !== requestOrigin) {
    return "passkey origin mismatch";
  }
  return null;
}

function errorResponse(error: string, status: number) {
  return NextResponse.json({ error }, { status });
}
