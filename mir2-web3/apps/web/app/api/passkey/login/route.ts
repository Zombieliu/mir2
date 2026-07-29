import { createHmac } from "node:crypto";

import { SIGNATURE_SCHEME_TO_FLAG } from "@mysten/sui/cryptography";
import { verifyPersonalMessageSignature } from "@mysten/sui/verify";
import { NextResponse } from "next/server";

export const runtime = "nodejs";

const SUI_LOGIN_PURPOSES = new Set(["mir2-sui-login", "mir2-passkey-login"]);
const TOKEN_AUTH = "sui-passkey-v1";
const MAX_LOGIN_WINDOW_MS = 120_000;

type PasskeyLoginRequest = {
  address?: string;
  message?: string;
  signature?: string;
  provider?: "suiPasskey" | "suiWallet";
};

type PasskeyLoginMessage = {
  purpose?: string;
  accountId?: string;
  address?: string;
  origin?: string;
  issuedAt?: number;
  expiresAt?: number;
  nonce?: string;
};

export async function POST(request: Request) {
  const body = (await request.json().catch(() => null)) as PasskeyLoginRequest | null;
  if (!body?.address || !body.message || !body.signature) {
    return errorResponse("address, message, and signature are required", 400);
  }
  const provider = body.provider ?? "suiPasskey";
  if (provider !== "suiPasskey" && provider !== "suiWallet") {
    return errorResponse("invalid Sui login provider", 400);
  }

  let loginMessage: PasskeyLoginMessage;
  try {
    loginMessage = JSON.parse(body.message) as PasskeyLoginMessage;
  } catch {
    return errorResponse("invalid passkey message", 400);
  }

  const validationError = validateLoginMessage(loginMessage, body.address, request.headers.get("origin"));
  if (validationError) {
    return errorResponse(validationError, 400);
  }

  try {
    const publicKey = await verifyPersonalMessageSignature(
      new TextEncoder().encode(body.message),
      body.signature,
      {
        address: body.address,
      },
    );
    if (provider === "suiPasskey" && publicKey.flag() !== SIGNATURE_SCHEME_TO_FLAG.Passkey) {
      return errorResponse("suiPasskey requires a Sui Passkey signature", 401);
    }
  } catch {
    return errorResponse("invalid passkey signature", 401);
  }

  const accountId = loginMessage.accountId!;
  const expiresAt = Math.min(loginMessage.expiresAt!, Date.now() + 60_000);
  let token: string;
  try {
    token = issueGatewayToken(accountId, provider, expiresAt);
  } catch (error) {
    return errorResponse(error instanceof Error ? error.message : "passkey auth is not configured", 500);
  }
  return NextResponse.json({
    accountId,
    provider,
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
  if (message.address !== address) return "passkey address mismatch";
  if (message.accountId !== `sui:${address}`) return "passkey account mismatch";
  if (!message.nonce) return "passkey nonce is required";
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

function issueGatewayToken(
  accountId: string,
  provider: "suiPasskey" | "suiWallet",
  expiresAt: number,
) {
  const payload = base64Url(
    Buffer.from(
      JSON.stringify({
        auth: TOKEN_AUTH,
        accountId,
        provider,
        expMs: expiresAt,
      }),
    ),
  );
  const signature = createHmac("sha256", passkeyGatewaySecret()).update(payload).digest("base64url");
  return `${payload}.${signature}`;
}

function passkeyGatewaySecret() {
  const secret = process.env.MIR2_PASSKEY_AUTH_SECRET;
  if (secret && secret.length > 0) return secret;
  if (passkeySecretRequiredFromEnv()) {
    throw new Error("MIR2_PASSKEY_AUTH_SECRET is required for production passkey login");
  }
  // Fail closed by default: only use the insecure local secret when explicitly
  // opted in, so a misconfigured deployment cannot silently sign tokens with a
  // publicly known key.
  if (!devPasskeySecretAllowed()) {
    throw new Error(
      "MIR2_PASSKEY_AUTH_SECRET is not set; set it, or set MIR2_ALLOW_DEV_PASSKEY_SECRET=1 to use the insecure local development secret",
    );
  }
  return "mir2-web3-local-passkey-auth-secret";
}

function devPasskeySecretAllowed() {
  const value = process.env.MIR2_ALLOW_DEV_PASSKEY_SECRET?.trim().toLowerCase();
  return value === "1" || value === "true" || value === "yes";
}

function passkeySecretRequiredFromEnv() {
  return ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV", "VERCEL_ENV"].some((name) => {
    const value = process.env[name]?.trim().toLowerCase();
    return value === "production" || value === "prod" || value === "staging";
  });
}

function base64Url(bytes: Buffer) {
  return bytes.toString("base64url");
}

function errorResponse(error: string, status: number) {
  return NextResponse.json({ error }, { status });
}
