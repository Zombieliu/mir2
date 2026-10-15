import { NextResponse, type NextRequest } from "next/server";
import runtimeManifest from "./lib/bevy-runtime-build-manifest";
import { bevyRuntimeLocalRewrite } from "./lib/bevy-runtime-local-route";

export function proxy(request: NextRequest) {
  const destination = bevyRuntimeLocalRewrite(runtimeManifest, request.nextUrl.pathname);
  if (!destination) {
    return new NextResponse("Runtime version or artifact unavailable", {
      status: 404,
      headers: { "Cache-Control": "no-store" },
    });
  }
  const url = request.nextUrl.clone();
  url.pathname = destination;
  return NextResponse.rewrite(url, {
    headers: { "X-Mir2-Runtime-Version": runtimeManifest.version },
  });
}

export const config = { matcher: "/bevy-runtime/v/:path*" };
