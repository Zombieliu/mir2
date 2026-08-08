"use server";

import { timingSafeEqual } from "node:crypto";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";

export async function loginAction(formData: FormData) {
  const token = stringValue(formData, "token");
  const returnTo = safeReturnTo(stringValue(formData, "returnTo"));
  if (!dashboardTokenMatches(token)) {
    redirect(`/login?error=token-invalid&next=${encodeURIComponent(returnTo)}`);
  }
  const cookieStore = await cookies();
  cookieStore.set("admin_operator_token", token, {
    httpOnly: true,
    sameSite: "strict",
    path: "/",
    maxAge: 60 * 60 * 8
  });
  redirect(returnTo);
}

export async function logoutAction() {
  const cookieStore = await cookies();
  cookieStore.delete("admin_operator_token");
  redirect("/login");
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function dashboardTokenMatches(supplied: string) {
  const expected = process.env.ADMIN_DASHBOARD_TOKEN?.trim() ?? "";
  if (!supplied || !expected) {
    return false;
  }
  const suppliedBytes = Buffer.from(supplied);
  const expectedBytes = Buffer.from(expected);
  return (
    suppliedBytes.length === expectedBytes.length &&
    timingSafeEqual(suppliedBytes, expectedBytes)
  );
}

function safeReturnTo(value: string) {
  return value.startsWith("/") && !value.startsWith("//") ? value : "/dubhe-nodes";
}
