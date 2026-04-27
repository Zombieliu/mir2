"use server";

import { redirect } from "next/navigation";
import { revalidatePath } from "next/cache";
import { adminPost, type ApprovalRecord } from "../../lib/admin-api";

export async function createApprovalAction(formData: FormData) {
  const response = await adminPost<ApprovalRecord>("/admin/approvals", {
    approvalId: stringValue(formData, "approvalId"),
    commandId: stringValue(formData, "commandId"),
    commandType: stringValue(formData, "commandType"),
    reason: stringValue(formData, "reason")
  });
  finish(response);
}

export async function approveApprovalAction(formData: FormData) {
  const approvalId = stringValue(formData, "approvalId");
  const response = await adminPost<ApprovalRecord>(
    `/admin/approvals/${encodeURIComponent(approvalId)}/approve`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response);
}

export async function rejectApprovalAction(formData: FormData) {
  const approvalId = stringValue(formData, "approvalId");
  const response = await adminPost<ApprovalRecord>(
    `/admin/approvals/${encodeURIComponent(approvalId)}/reject`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response);
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function finish(response: Awaited<ReturnType<typeof adminPost<ApprovalRecord>>>) {
  revalidatePath("/approvals");
  if (!response.ok) {
    redirect(`/approvals?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/approvals");
}
