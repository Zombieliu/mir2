"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type AdminEconomyReadModel } from "../../lib/admin-api";

type PriceFeedRecord = AdminEconomyReadModel["priceFeeds"][number];

export async function upsertPriceFeedAction(formData: FormData) {
  const response = await adminPost<PriceFeedRecord>("/admin/economy/price-feeds", {
    item: stringValue(formData, "item"),
    latestPrice: numberValue(formData, "latestPrice", 0),
    sampleCount: numberValue(formData, "sampleCount", 1),
    source: stringValue(formData, "source") || "operator_projection"
  });
  revalidatePath("/economy");
  if (!response.ok) {
    redirect(`/economy?error=${encodeURIComponent(response.error)}`);
  }
  redirect("/economy");
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function numberValue(formData: FormData, key: string, fallback: number) {
  const value = Number(stringValue(formData, key));
  return Number.isFinite(value) && value >= 0 ? Math.floor(value) : fallback;
}
