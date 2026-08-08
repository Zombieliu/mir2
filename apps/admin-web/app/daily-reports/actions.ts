"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import {
  adminPost,
  type DailyReport,
  type DailyReportDetailResponse
} from "../../lib/admin-api";

export async function generateDailyReportAction(formData: FormData) {
  const response = await adminPost<DailyReport>("/admin/daily-reports/generate", {
    reportDate: optionalString(formData, "reportDate"),
    force: formData.get("force") === "on",
    trigger: "operator_console"
  });
  finish(response, response.ok ? response.data.reportId : undefined);
}

export async function approveDailyReportAction(formData: FormData) {
  const reportId = stringValue(formData, "reportId");
  const response = await adminPost<DailyReport>(
    `/admin/daily-reports/${encodeURIComponent(reportId)}/approve`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response, reportId);
}

export async function publishDailyReportAction(formData: FormData) {
  const reportId = stringValue(formData, "reportId");
  const response = await adminPost<DailyReportDetailResponse>(
    `/admin/daily-reports/${encodeURIComponent(reportId)}/publish`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response, reportId);
}

export async function retryDiscordAction(formData: FormData) {
  const reportId = stringValue(formData, "reportId");
  const response = await adminPost<DailyReportDetailResponse>(
    `/admin/daily-reports/${encodeURIComponent(reportId)}/retry-discord`,
    { reason: stringValue(formData, "reason") }
  );
  finish(response, reportId);
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  return stringValue(formData, key) || undefined;
}

function finish(
  response:
    | Awaited<ReturnType<typeof adminPost<DailyReport>>>
    | Awaited<ReturnType<typeof adminPost<DailyReportDetailResponse>>>,
  reportId?: string
) {
  revalidatePath("/daily-reports");
  if (!response.ok) {
    redirect(`/daily-reports?error=${encodeURIComponent(response.error)}`);
  }
  redirect(`/daily-reports${reportId ? `?report=${encodeURIComponent(reportId)}` : ""}`);
}
