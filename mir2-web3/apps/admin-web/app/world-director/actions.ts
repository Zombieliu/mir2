"use server";

import { redirect } from "next/navigation";
import { revalidatePath } from "next/cache";
import {
  adminPost,
  type DirectorApprovalRecord,
  type WorldDirectorDashboard
} from "../../lib/admin-api";

export async function generateDirectorProposalAction() {
  const response = await adminPost<DirectorApprovalRecord | null>(
    "/admin/world-director/proposals/generate",
    {}
  );
  if (!response.ok) {
    finish(response);
  }
  revalidatePath("/world-director");
  redirect(
    response.data
      ? "/world-director?notice=proposal-generated"
      : "/world-director?notice=no-proposal"
  );
}

export async function approveDirectorProposalAction(formData: FormData) {
  await decide(formData, "approve");
}

export async function rejectDirectorProposalAction(formData: FormData) {
  await decide(formData, "reject");
}

export async function cancelDirectorProposalAction(formData: FormData) {
  await decide(formData, "cancel");
}

export async function retryDirectorDeliveryAction(formData: FormData) {
  await decide(formData, "retry");
}

export async function editDirectorProposalAction(formData: FormData) {
  const proposalId = stringValue(formData, "proposalId");
  const durationMinutes = numberValue(formData, "durationMinutes");
  const rewardBudget = numberValue(formData, "rewardBudget");
  const targetZones = stringValue(formData, "targetZones")
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
  const response = await adminPost<DirectorApprovalRecord>(
    `/admin/world-director/proposals/${encodeURIComponent(proposalId)}/edit`,
    {
      reason: stringValue(formData, "reason"),
      durationMs: durationMinutes ? durationMinutes * 60_000 : undefined,
      rewardBudget: rewardBudget || undefined,
      targetZones: targetZones.length ? targetZones : undefined
    }
  );
  finish(response);
}

export async function pauseDirectorAction(formData: FormData) {
  await control(formData, "pause");
}

export async function resumeDirectorAction(formData: FormData) {
  await control(formData, "resume");
}

async function decide(
  formData: FormData,
  action: "approve" | "reject" | "cancel" | "retry"
) {
  const proposalId = stringValue(formData, "proposalId");
  const response = await adminPost<DirectorApprovalRecord>(
    `/admin/world-director/proposals/${encodeURIComponent(proposalId)}/${action}`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response);
}

async function control(formData: FormData, action: "pause" | "resume") {
  const response = await adminPost<WorldDirectorDashboard>(
    `/admin/world-director/control/${action}`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response);
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function numberValue(formData: FormData, key: string) {
  const value = Number(stringValue(formData, key));
  return Number.isFinite(value) && value > 0 ? Math.floor(value) : 0;
}

function finish(response: { ok: true; data: unknown } | { ok: false; error: string }): never {
  revalidatePath("/world-director");
  if (!response.ok) {
    redirect(`/world-director?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/world-director?notice=updated");
}
