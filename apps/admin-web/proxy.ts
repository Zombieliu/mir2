import { NextRequest, NextResponse } from "next/server";

export function proxy(request: NextRequest) {
  const pathname = request.nextUrl.pathname;
  if (
    pathname === "/login" ||
    pathname.startsWith("/_next/") ||
    pathname === "/favicon.ico"
  ) {
    return NextResponse.next();
  }

  const expected = process.env.ADMIN_DASHBOARD_TOKEN?.trim() ?? "";
  const supplied = request.cookies.get("admin_operator_token")?.value ?? "";
  if (tokenMatches(supplied, expected)) {
    return NextResponse.next();
  }

  if (pathname.startsWith("/api/")) {
    return NextResponse.json(
      { error: expected ? "operator authentication required" : "dashboard is not configured" },
      { status: expected ? 401 : 503 }
    );
  }

  const login = request.nextUrl.clone();
  login.pathname = "/login";
  login.search = "";
  login.searchParams.set("next", `${pathname}${request.nextUrl.search}`);
  if (!expected) {
    login.searchParams.set("error", "dashboard-not-configured");
  }
  return NextResponse.redirect(login);
}

function tokenMatches(supplied: string, expected: string) {
  if (!supplied || !expected || supplied.length !== expected.length) {
    return false;
  }
  let difference = 0;
  for (let index = 0; index < expected.length; index += 1) {
    difference |= supplied.charCodeAt(index) ^ expected.charCodeAt(index);
  }
  return difference === 0;
}

export const config = {
  matcher: ["/((?!_next/static|_next/image).*)"]
};
