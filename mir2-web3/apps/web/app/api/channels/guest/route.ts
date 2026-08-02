import { createHmac, randomUUID } from "node:crypto";
import { cookies } from "next/headers";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const COOKIE_NAME = "mir2_channel_guest";
const ALLOWED_PROVIDERS = new Set(["directGuest", "itch", "crazyGamesGuest"]);

type GuestRequest = {
  provider?: string;
};

function signingSecret() {
  const secret = process.env.MIR2_PASSKEY_AUTH_SECRET?.trim();
  if (secret) return secret;
  if (
    process.env.NODE_ENV !== "production" &&
    /^(?:1|true|yes)$/iu.test(process.env.MIR2_ALLOW_DEV_PASSKEY_SECRET?.trim() ?? "")
  ) {
    return "mir2-web3-local-passkey-auth-secret";
  }
  throw new Error("MIR2_PASSKEY_AUTH_SECRET is not configured");
}

export async function POST(request: Request) {
  let body: GuestRequest;
  try {
    body = (await request.json()) as GuestRequest;
  } catch {
    return Response.json({ error: "invalid guest session request" }, { status: 400 });
  }
  const provider = body.provider?.trim() ?? "";
  if (!ALLOWED_PROVIDERS.has(provider)) {
    return Response.json({ error: "unsupported guest channel" }, { status: 400 });
  }

  try {
    const cookieStore = await cookies();
    let guestId = cookieStore.get(COOKIE_NAME)?.value;
    if (!guestId || !/^[0-9a-f-]{36}$/iu.test(guestId)) {
      guestId = randomUUID();
      cookieStore.set(COOKIE_NAME, guestId, {
        httpOnly: true,
        secure: process.env.NODE_ENV === "production",
        sameSite: process.env.NODE_ENV === "production" ? "none" : "lax",
        path: "/",
        maxAge: 60 * 60 * 24 * 365,
      });
    }

    const expiresAt = Date.now() + 5 * 60_000;
    const payload = Buffer.from(
      JSON.stringify({
        auth: "mir2-channel-guest-v1",
        subject: `guest:${guestId}`,
        provider,
        expMs: expiresAt,
      }),
    ).toString("base64url");
    const signature = createHmac("sha256", signingSecret())
      .update(payload)
      .digest("base64url");
    return Response.json(
      { provider, credential: `${payload}.${signature}`, expiresAt },
      { headers: { "cache-control": "no-store" } },
    );
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : "guest session failed" },
      { status: 503 },
    );
  }
}
