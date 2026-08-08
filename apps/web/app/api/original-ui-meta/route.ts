import { NextResponse } from "next/server";

import {
  normalizeOriginalUiLibraryKey,
  OriginalUiMetaError,
  readStaticOriginalUiLibraryMeta,
} from "../../../lib/original-ui-meta-server";
import {
  ensureOriginalUiLibraryExport,
  OriginalUiExportError,
} from "../../../lib/original-ui-export-server";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const library = url.searchParams.get("library");
  if (!library) {
    return NextResponse.json({ error: "missing library" }, { status: 400 });
  }

  try {
    const normalizedLibrary = normalizeOriginalUiLibraryKey(library);
    const staticMeta = await readStaticOriginalUiLibraryMeta(request, normalizedLibrary);
    if (staticMeta) return NextResponse.json(staticMeta);

    if (process.env.NODE_ENV !== "production") {
      try {
        return NextResponse.json(await ensureOriginalUiLibraryExport(normalizedLibrary));
      } catch (error) {
        if (error instanceof OriginalUiExportError) {
          return NextResponse.json(
            { error: error.message, code: error.code },
            { status: error.status },
          );
        }
        throw error;
      }
    }

    return NextResponse.json(
      {
        error: `Original UI library metadata is not deployed: ${normalizedLibrary}`,
        code: "library_not_deployed",
      },
      { status: 404 },
    );
  } catch (error) {
    if (error instanceof OriginalUiMetaError) {
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
        code: "original_ui_meta_failed",
      },
      { status: 500 },
    );
  }
}
