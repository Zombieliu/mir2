import "server-only";

import { createHmac, timingSafeEqual } from "node:crypto";

export const PASSKEY_CHALLENGE_AUTH = "mir2-login-challenge-v1";
export const PASSKEY_GATEWAY_TOKEN_AUTH = "sui-passkey-v2";

export type PasskeyChallengePayload = {
  auth: typeof PASSKEY_CHALLENGE_AUTH;
  jti: string;
  address: string;
  origin: string;
  iatMs: number;
  expMs: number;
  authMethod: "passkey" | "wallet";
};

export function signPasskeyChallenge(payload: PasskeyChallengePayload) {
  return signPayload(payload);
}

export function verifyPasskeyChallenge(token: string): PasskeyChallengePayload {
  const payload = verifySignedPayload(token) as Partial<PasskeyChallengePayload>;
  if (
    payload.auth !== PASSKEY_CHALLENGE_AUTH ||
    typeof payload.jti !== "string" ||
    typeof payload.address !== "string" ||
    typeof payload.origin !== "string" ||
    !Number.isFinite(payload.iatMs) ||
    !Number.isFinite(payload.expMs) ||
    (payload.authMethod !== "passkey" && payload.authMethod !== "wallet")
  ) {
    throw new Error("invalid passkey challenge");
  }
  return payload as PasskeyChallengePayload;
}

export function issueGatewayPasskeyToken(
  accountId: string,
  jti: string,
  expiresAt: number,
  authMethod: "passkey" | "wallet",
) {
  return signPayload({
    auth: PASSKEY_GATEWAY_TOKEN_AUTH,
    accountId,
    jti,
    expMs: expiresAt,
    authMethod,
  });
}

export function passkeyRequestOriginAllowed(origin: string | null, requestUrl: string) {
  if (!origin) return false;
  let normalizedOrigin: string;
  let requestOrigin: string;
  try {
    normalizedOrigin = new URL(origin).origin;
    requestOrigin = new URL(requestUrl).origin;
  } catch {
    return false;
  }
  if (normalizedOrigin !== origin) return false;

  const configured = (
    process.env.MIR2_PASSKEY_ALLOWED_ORIGINS ?? process.env.MIR2_ALLOWED_WEB_ORIGINS ?? ""
  )
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean)
    .flatMap((value) => {
      try {
        return [new URL(value).origin];
      } catch {
        return [];
      }
    });
  if (configured.length > 0) return configured.includes(normalizedOrigin);
  if (passkeySecretRequiredFromEnv()) return false;
  return normalizedOrigin === requestOrigin;
}

function signPayload(payload: object) {
  const encoded = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const signature = createHmac("sha256", passkeyGatewaySecret()).update(encoded).digest("base64url");
  return `${encoded}.${signature}`;
}

function verifySignedPayload(token: string): unknown {
  const [payload, signature, extra] = token.split(".");
  if (!payload || !signature || extra) throw new Error("invalid passkey challenge");
  const actual = Buffer.from(signature, "base64url");
  const expected = createHmac("sha256", passkeyGatewaySecret()).update(payload).digest();
  if (actual.length !== expected.length || !timingSafeEqual(actual, expected)) {
    throw new Error("invalid passkey challenge signature");
  }
  try {
    return JSON.parse(Buffer.from(payload, "base64url").toString("utf8"));
  } catch {
    throw new Error("invalid passkey challenge payload");
  }
}

function passkeyGatewaySecret() {
  const secret = process.env.MIR2_PASSKEY_AUTH_SECRET;
  if (secret && secret.length >= 32) return secret;
  if (passkeySecretRequiredFromEnv()) {
    throw new Error("MIR2_PASSKEY_AUTH_SECRET with at least 32 characters is required for production passkey login");
  }
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
