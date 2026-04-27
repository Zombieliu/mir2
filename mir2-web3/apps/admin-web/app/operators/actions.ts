"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type AdminOperatorsReadModel } from "../../lib/admin-api";

type OperatorRecord = AdminOperatorsReadModel["operators"][number];

export async function upsertOperatorAction(formData: FormData) {
  const response = await adminPost<OperatorRecord>("/admin/operators", {
    operatorId: optionalString(formData, "operatorId"),
    email: stringValue(formData, "email"),
    role: stringValue(formData, "role") || "ops_admin",
    status: stringValue(formData, "status") || "Active",
    permissions: listValue(formData, "permissions")
  });
  revalidatePath("/operators");
  if (!response.ok) {
    redirect(`/operators?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/operators");
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  return stringValue(formData, key) || undefined;
}

function listValue(formData: FormData, key: string) {
  return stringValue(formData, key)
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}
