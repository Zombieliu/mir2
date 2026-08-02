import "server-only";

export type IdentitySession = {
  sessionId: string;
  accountId: string;
  authMethod: string;
  credentialId?: string | null;
  issuedAtMs: number;
  lastSeenAtMs: number;
  expiresAtMs: number;
  revokedAtMs?: number | null;
  revokedReason?: string | null;
  userAgentSummary: string;
  gatewayId: string;
  current: boolean;
};

export type IdentityCredential = {
  credentialId: string;
  credentialKind: string;
  credentialSubject: string;
  displayName: string;
  createdAtMs: number;
  lastUsedAtMs?: number | null;
  revokedAtMs?: number | null;
};

export type IdentityAuditEvent = {
  eventId: string;
  eventType: string;
  outcome: string;
  reasonCode: string;
  sessionId?: string | null;
  credentialId?: string | null;
  peerFingerprint?: string;
  userAgentSummary?: string;
  traceId: string;
  details?: unknown;
  occurredAtMs: number;
};

export type IdentityAccountSecurity = {
  source: string;
  accountId: string;
  sessions: IdentitySession[];
  credentials: IdentityCredential[];
  auditEvents: IdentityAuditEvent[];
};

type ApiResult<T> = { ok: true; data: T } | { ok: false; error: string; status?: number };

const configuredGatewayBase =
  process.env.MIR2_GATEWAY_ADMIN_URL?.trim() ?? process.env.MIR2_GATEWAY_HTTP_URL?.trim();
const gatewayBase = configuredGatewayBase?.replace(/\/$/, "") ?? "http://127.0.0.1:8080";

export async function identityAdminGet(accountId: string): Promise<ApiResult<IdentityAccountSecurity>> {
  return request<IdentityAccountSecurity>(
    `/admin/identity?accountId=${encodeURIComponent(accountId)}`,
  );
}

export async function identityAdminRevoke(input: {
  accountId: string;
  sessionId?: string;
  reason: string;
}): Promise<ApiResult<{ accepted: boolean; affected: number }>> {
  return request("/admin/identity/revoke", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

async function request<T>(path: string, init?: RequestInit): Promise<ApiResult<T>> {
  const token = process.env.MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN?.trim();
  if (isProductionRuntime() && !configuredGatewayBase) {
    return { ok: false, error: "MIR2_GATEWAY_ADMIN_URL is not configured" };
  }
  if (!token) {
    return { ok: false, error: "MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN is not configured" };
  }
  try {
    const response = await fetch(`${gatewayBase}${path}`, {
      ...init,
      cache: "no-store",
      headers: {
        authorization: `Bearer ${token}`,
        ...(init?.body ? { "content-type": "application/json" } : {}),
      },
      signal: AbortSignal.timeout(8_000),
    });
    const payload = (await response.json().catch(() => null)) as
      | (T & { error?: string })
      | null;
    if (!response.ok || !payload) {
      return {
        ok: false,
        status: response.status,
        error: payload?.error ?? `Gateway HTTP ${response.status}`,
      };
    }
    return { ok: true, data: payload };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Identity gateway unavailable",
    };
  }
}

function isProductionRuntime() {
  if (process.env.NODE_ENV === "production") return true;
  return ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV", "VERCEL_ENV"].some(
    (name) => {
      const value = process.env[name]?.trim().toLowerCase();
      return value === "production" || value === "prod" || value === "staging";
    },
  );
}
