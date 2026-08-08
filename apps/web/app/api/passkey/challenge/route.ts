import { randomUUID } from "node:crypto";

import { isValidSuiAddress, normalizeSuiAddress } from "@mysten/sui/utils";
import { NextResponse } from "next/server";

import {
  PASSKEY_CHALLENGE_AUTH,
  passkeyRequestOriginAllowed,
  signPasskeyChallenge,
} from "../../../../lib/server/passkey-auth";

export const runtime = "nodejs";

const CHALLENGE_TTL_MS = 60_000;

export async function POST(request: Request) {
  const body = (await request.json().catch(() => null)) as {
    address?: string;
    authMethod?: "passkey" | "wallet";
  } | null;
  const requestOrigin = request.headers.get("origin");
  if (
    !body?.address ||
    !requestOrigin ||
    !passkeyRequestOriginAllowed(requestOrigin, request.url) ||
    !isValidSuiAddress(body.address) ||
    (body.authMethod !== "passkey" && body.authMethod !== "wallet")
  ) {
    return NextResponse.json(
      { error: "valid address, authMethod, and an allowed Origin are required" },
      { status: 400 },
    );
  }

  const address = normalizeSuiAddress(body.address);
  const issuedAt = Date.now();
  const expiresAt = issuedAt + CHALLENGE_TTL_MS;
  const challengeId = randomUUID();
  try {
    const challenge = signPasskeyChallenge({
      auth: PASSKEY_CHALLENGE_AUTH,
      jti: challengeId,
      address,
      origin: requestOrigin,
      iatMs: issuedAt,
      expMs: expiresAt,
      authMethod: body.authMethod,
    });
    return NextResponse.json(
      { challenge, challengeId, issuedAt, expiresAt },
      { headers: { "Cache-Control": "no-store" } },
    );
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "passkey auth is not configured" },
      { status: 500 },
    );
  }
}
