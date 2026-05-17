import { NextResponse } from "next/server";

import {
  ensureOriginalUiLibraryExport,
  OriginalUiExportError,
} from "../../../lib/original-ui-export-server";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const library = url.searchParams.get("library");
  if (!library) {
    return NextResponse.json({ error: "missing library" }, { status: 400 });
  }

  try {
    const meta = await ensureOriginalUiLibraryExport(library);
    return NextResponse.json(meta);
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
        code: "export_failed",
      },
      { status: 500 },
    );
  }
}
