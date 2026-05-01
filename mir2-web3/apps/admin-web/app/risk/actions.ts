"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type AdminRiskReadModel } from "../../lib/admin-api";

type TradeGraphEdge = AdminRiskReadModel["graph"][number];

export async function upsertTradeGraphEdgeAction(formData: FormData) {
  const response = await adminPost<TradeGraphEdge>("/admin/risk/trade-edges", {
    edgeId: optionalString(formData, "edgeId"),
    from: stringValue(formData, "from"),
    to: stringValue(formData, "to"),
    signal: stringValue(formData, "signal") || "manual_review",
    risk: stringValue(formData, "risk") || "Medium",
    evidence: optionalString(formData, "evidence")
  });
  revalidatePath("/risk");
  if (!response.ok) {
    redirect(`/risk?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/risk");
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  return stringValue(formData, key) || undefined;
}
