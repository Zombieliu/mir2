import { NextResponse } from "next/server";

import {
  ensureOriginalUiLibraryExport,
  OriginalUiExportError,
  readDeployedOriginalUiLibraryMeta,
} from "../../../lib/original-ui-export-server";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const library = url.searchParams.get("library");
  if (!library) {
    return NextResponse.json({ error: "missing library" }, { status: 400 });
  }

  const normalizedLibrary = normalizeLibraryKey(library);
  if (!normalizedLibrary || normalizedLibrary.startsWith("Map/")) {
    return NextResponse.json(
      {
        error: `Unsupported original UI library: ${normalizedLibrary}`,
        code: "unsupported_library",
      },
      { status: 400 },
    );
  }

  try {
    const existing = await readDeployedOriginalUiLibraryMeta(normalizedLibrary);
    if (existing) {
      return NextResponse.json(existing);
    }

    const staticMeta = await readStaticOriginalUiLibraryMeta(request, normalizedLibrary);
    return NextResponse.json(staticMeta ?? (await ensureOriginalUiLibraryExport(normalizedLibrary)));
  } catch (error) {
    if (error instanceof OriginalUiExportError) {
      return NextResponse.json(
        {
          error: error.message,
          code: error.code,
        },
        { status: error.status },
      );
    }

    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : String(error),
        code: "original_ui_export_failed",
      },
      { status: 500 },
    );
  }
}

async function readStaticOriginalUiLibraryMeta(request: Request, normalizedLibrary: string) {
  const staticPath = normalizedLibrary
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
  const staticUrl = new URL(`/original-ui/${staticPath}/meta.json`, request.url);

  try {
    const response = await fetch(staticUrl, { cache: "force-cache" });
    if (!response.ok) {
      return null;
    }
    return (await response.json()) as unknown;
  } catch {
    return null;
  }
}

function normalizeLibraryKey(libraryKey: string) {
  const normalized = libraryKey
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");

  if (
    normalized.startsWith("/") ||
    normalized.split("/").some((segment) => segment === "." || segment === "..")
  ) {
    return "";
  }

  return normalized;
}
