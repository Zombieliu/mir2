export type ApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: string; status?: number };

export type AdminCommandRecord = {
  envelope: {
    commandId: string;
    reason: string;
    operator: { email: string; role: string };
    target: { targetType: string; targetId: string };
    traceId: string;
  };
  status: string;
  resultMessage?: string;
  errorCode?: string;
  updatedAtMs: number;
};

export type AuditRecord = {
  auditId: string;
  commandId: string;
  operatorEmail: string;
  permission: string;
  target: { targetType: string; targetId: string };
  reason: string;
  status: string;
  errorCode?: string;
  traceId: string;
  completedAtMs?: number;
};

export type SystemMailReceipt = {
  outboxId: string;
  targetKind: string;
  targetId: string;
  attachmentCount: number;
  acceptedAtMs: number;
  deliveryMode: string;
  deliveredCount: number;
  mailIds: number[];
};

const adminApiBase = process.env.ADMIN_API_BASE_URL ?? "http://127.0.0.1:7420";

export function operatorHeaders() {
  return {
    "x-operator-id": process.env.ADMIN_OPERATOR_ID ?? "local-gm",
    "x-operator-email": process.env.ADMIN_OPERATOR_EMAIL ?? "gm.local@mir2.dev",
    "x-operator-role": process.env.ADMIN_OPERATOR_ROLE ?? "ops_admin",
    "x-operator-permissions":
      process.env.ADMIN_OPERATOR_PERMISSIONS ??
      "account_read,character_read,inventory_read,mail_send_system,audit_read"
  };
}

export async function adminGet<T>(path: string): Promise<ApiResult<T>> {
  try {
    const response = await fetch(`${adminApiBase}${path}`, {
      cache: "no-store",
      headers: operatorHeaders()
    });
    const data = (await response.json()) as unknown;
    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        error: errorMessage(data) ?? `HTTP ${response.status}`
      };
    }
    return { ok: true, data: data as T };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Admin API unavailable"
    };
  }
}

export async function adminPost<T>(
  path: string,
  body: unknown
): Promise<ApiResult<T>> {
  try {
    const response = await fetch(`${adminApiBase}${path}`, {
      method: "POST",
      cache: "no-store",
      headers: {
        "content-type": "application/json",
        ...operatorHeaders()
      },
      body: JSON.stringify(body)
    });
    const data = (await response.json()) as unknown;
    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        error: errorMessage(data) ?? `HTTP ${response.status}`
      };
    }
    return { ok: true, data: data as T };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Admin API unavailable"
    };
  }
}

function errorMessage(data: unknown): string | undefined {
  if (
    data &&
    typeof data === "object" &&
    "error" in data &&
    typeof data.error === "string"
  ) {
    return data.error;
  }
  return undefined;
}
