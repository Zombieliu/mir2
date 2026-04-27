"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type SubmitCommandResponse } from "../../lib/admin-api";

export async function sendSystemMailAction(formData: FormData) {
  const response = await adminPost<SubmitCommandResponse>(
    "/admin/commands/send-system-mail",
    {
      commandId: optionalString(formData, "commandId"),
      traceId: optionalString(formData, "traceId"),
      approvalId: optionalString(formData, "approvalId"),
      targetKind: stringValue(formData, "targetKind") || "character",
      targetId: stringValue(formData, "targetId"),
      subject: stringValue(formData, "subject"),
      body: stringValue(formData, "body"),
      reason: stringValue(formData, "reason"),
      attachments: [
        {
          itemId: stringValue(formData, "itemId") || "gold",
          count: numberValue(formData, "count", 1)
        }
      ]
    }
  );
  finish(response);
}

export async function grantItemAction(formData: FormData) {
  const response = await adminPost<SubmitCommandResponse>(
    "/admin/commands/grant-item",
    {
      commandId: optionalString(formData, "commandId"),
      traceId: optionalString(formData, "traceId"),
      approvalId: optionalString(formData, "approvalId"),
      characterId: stringValue(formData, "characterId"),
      itemId: stringValue(formData, "itemId"),
      count: numberValue(formData, "count", 1),
      reason: stringValue(formData, "reason")
    }
  );
  finish(response);
}

export async function grantCurrencyAction(formData: FormData) {
  const response = await adminPost<SubmitCommandResponse>(
    "/admin/commands/grant-currency",
    {
      commandId: optionalString(formData, "commandId"),
      traceId: optionalString(formData, "traceId"),
      approvalId: optionalString(formData, "approvalId"),
      characterId: stringValue(formData, "characterId"),
      currency: stringValue(formData, "currency") || "gold",
      amount: numberValue(formData, "amount", 1),
      reason: stringValue(formData, "reason")
    }
  );
  finish(response);
}

export async function kickPlayerAction(formData: FormData) {
  const response = await adminPost<SubmitCommandResponse>(
    "/admin/commands/kick-player",
    {
      commandId: optionalString(formData, "commandId"),
      traceId: optionalString(formData, "traceId"),
      characterId: stringValue(formData, "characterId"),
      reason: stringValue(formData, "reason")
    }
  );
  finish(response);
}

export async function banAccountAction(formData: FormData) {
  const durationSeconds = stringValue(formData, "durationSeconds");
  const response = await adminPost<SubmitCommandResponse>(
    "/admin/commands/ban-account",
    {
      commandId: optionalString(formData, "commandId"),
      traceId: optionalString(formData, "traceId"),
      approvalId: optionalString(formData, "approvalId"),
      accountId: stringValue(formData, "accountId"),
      durationSeconds: durationSeconds ? Number(durationSeconds) : undefined,
      reason: stringValue(formData, "reason")
    }
  );
  finish(response);
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  const value = stringValue(formData, key);
  return value || undefined;
}

function numberValue(formData: FormData, key: string, fallback: number) {
  const value = Number(stringValue(formData, key));
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function finish(response: Awaited<ReturnType<typeof adminPost<SubmitCommandResponse>>>) {
  revalidatePath("/gm-tools");
  revalidatePath("/audit");
  revalidatePath("/timeline");
  if (!response.ok) {
    redirect(`/gm-tools?error=${encodeURIComponent(response.error)}`);
  }
  redirect(`/gm-tools?commandId=${encodeURIComponent(response.data.commandId)}`);
}
