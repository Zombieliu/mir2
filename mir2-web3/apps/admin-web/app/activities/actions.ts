"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type AdminActivitiesReadModel } from "../../lib/admin-api";

type ActivityRecord = AdminActivitiesReadModel["activities"][number];

export async function upsertActivityAction(formData: FormData) {
  const response = await adminPost<ActivityRecord>("/admin/activities", {
    activityId: optionalString(formData, "activityId"),
    name: stringValue(formData, "name"),
    activityType: stringValue(formData, "activityType") || "seasonal",
    status: stringValue(formData, "status") || "Draft",
    signal: optionalString(formData, "signal"),
    startAtMs: optionalNumber(formData, "startAtMs"),
    reward: optionalString(formData, "reward"),
    condition: optionalString(formData, "condition"),
    realms: listValue(formData, "realms")
  });
  revalidatePath("/activities");
  if (!response.ok) {
    redirect(`/activities?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/activities");
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  return stringValue(formData, key) || undefined;
}

function optionalNumber(formData: FormData, key: string) {
  const raw = stringValue(formData, key);
  if (!raw) {
    return undefined;
  }
  const value = Number(raw);
  return Number.isFinite(value) && value >= 0 ? Math.floor(value) : undefined;
}

function listValue(formData: FormData, key: string) {
  return stringValue(formData, key)
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}
