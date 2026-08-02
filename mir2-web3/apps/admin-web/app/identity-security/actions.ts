"use server";

import { redirect } from "next/navigation";

import { identityAdminRevoke } from "../../lib/identity-api";

export async function revokeIdentitySessionAction(formData: FormData) {
  const accountId = value(formData, "accountId");
  const sessionId = value(formData, "sessionId");
  const reason = value(formData, "reason");
  const result = await identityAdminRevoke({ accountId, sessionId, reason });
  redirect(resultUrl(accountId, result.ok ? `已撤销 ${result.data.affected} 个会话` : undefined, result.ok ? undefined : result.error));
}

export async function revokeAllIdentitySessionsAction(formData: FormData) {
  const accountId = value(formData, "accountId");
  const reason = value(formData, "reason");
  const result = await identityAdminRevoke({ accountId, reason });
  redirect(resultUrl(accountId, result.ok ? `已撤销 ${result.data.affected} 个会话` : undefined, result.ok ? undefined : result.error));
}

function value(formData: FormData, key: string) {
  const candidate = formData.get(key);
  return typeof candidate === "string" ? candidate.trim() : "";
}

function resultUrl(accountId: string, success?: string, error?: string) {
  const query = new URLSearchParams();
  if (accountId) query.set("accountId", accountId);
  if (success) query.set("success", success);
  if (error) query.set("error", error);
  return `/identity-security?${query.toString()}`;
}
