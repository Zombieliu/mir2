export type AdminApiResponseLike = {
  status: number;
  statusText: string;
  headers: {
    get(name: string): string | null;
  };
  text(): Promise<string>;
};

export type ParsedAdminApiResponse =
  | { ok: true; data: unknown }
  | { ok: false; error: string };

const MAX_ERROR_PREVIEW = 180;

export async function parseAdminApiResponse(
  response: AdminApiResponseLike
): Promise<ParsedAdminApiResponse> {
  const body = await response.text();
  const trimmed = body.trim();
  const status = response.status;
  const statusLabel = response.statusText.trim();
  const httpLabel = `Admin API HTTP ${status}${statusLabel ? ` ${statusLabel}` : ""}`;

  if (!trimmed) {
    return { ok: false, error: `${httpLabel} returned an empty response` };
  }

  try {
    return { ok: true, data: JSON.parse(trimmed) as unknown };
  } catch {
    const contentType = response.headers.get("content-type")?.trim() || "unknown content type";
    const preview = trimmed.replace(/\s+/g, " ").slice(0, MAX_ERROR_PREVIEW);
    return {
      ok: false,
      error: `${httpLabel} returned invalid JSON (${contentType}): ${preview}`
    };
  }
}
