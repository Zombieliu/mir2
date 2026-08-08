"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type AdminServersReadModel } from "../../lib/admin-api";

type ZoneRuntimeRecord = AdminServersReadModel["zones"][number];

export async function upsertZoneRuntimeAction(formData: FormData) {
  const response = await adminPost<ZoneRuntimeRecord>("/admin/servers/zones", {
    zoneId: optionalString(formData, "zoneId"),
    name: stringValue(formData, "name"),
    status: stringValue(formData, "status") || "Healthy",
    host: optionalString(formData, "host"),
    processId: optionalString(formData, "processId"),
    mapCount: numberValue(formData, "mapCount", 0),
    playerCount: numberValue(formData, "playerCount", 0),
    tickRate: numberValue(formData, "tickRate", 0),
    uptimeSeconds: numberValue(formData, "uptimeSeconds", 0),
    source: stringValue(formData, "source") || "operator_projection"
  });
  revalidatePath("/servers");
  if (!response.ok) {
    redirect(`/servers?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/servers");
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  return stringValue(formData, key) || undefined;
}

function numberValue(formData: FormData, key: string, fallback: number) {
  const value = Number(stringValue(formData, key));
  return Number.isFinite(value) && value >= 0 ? Math.floor(value) : fallback;
}
