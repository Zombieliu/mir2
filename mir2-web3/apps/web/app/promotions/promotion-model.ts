import type { PromotionLocale } from "./promotion-copy";

export type PromotionCampaign = {
  campaignId: string; environment: string; status: string; termsVersion?: string;
  termsStatus: "draft" | "approved"; startsAtMs?: number; endsAtMs?: number;
  recruitmentEnabled: boolean; commissionEnabled: boolean; payoutEnabled: boolean;
  fundingMode?: "revenue_share" | "budget_capped";
  payoutPolicy?: PromotionPayoutPolicy | null;
  rules: Record<string, unknown> | null; sourceGaps?: string[];
};
export type PromotionPayoutPolicy = {
  status: "proposed" | "approved"; network: string; coinType: string;
  coinDecimals: number; symbol: string;
};
export type PromotionCharacter = { index: number; name: string; level: number; class: string | number; gender: string | number };
export type PromotionLeader = { id: string; status: string; displayName: string; characterIndex: number; inviteCode: string | null };
export type PromotionDashboard = {
  clicks: number; registrations: number; confirmed: number; qualified: number;
  tiers: { rateBps: number; count: number }[]; eligibleRevenueMicroUsd: string;
  estimatedCommissionMicroUsd: string; commissionEnabled: boolean; status?: string;
};
export type PromotionMember = { memberRef: string; status: string; reasonCode?: string; estimatedCommissionMicroUsd?: string; qualifiedAtMs?: number | null; rank?: number | null; rateBps?: number | null };
export type PromotionSettlement = { id: string; status: string; totalMicroUsd: string; payableCents: number; payableMicroUsd?: string; paidMicroUsd?: string; reserveMicroUsd: string; reversalMicroUsd: string; payoutId?: string | null; payoutStatus?: string | null; txDigest?: string | null; checkpoint?: string | null; createdAtMs?: number; snapshotSha256: string };
export type PromotionPayoutSettings = { status: string; address?: string; network?: string; version?: number; cooldownUntilMs?: number | null; payoutEnabled: boolean };
export type PromotionAppeal = { id: string; status: string; category?: string; message?: string; createdAtMs?: number };
export type PromotionMe = {
  account: { id: string; registeredAtMs: number; banned?: boolean };
  characters: PromotionCharacter[]; leader: PromotionLeader | null;
  dashboard: PromotionDashboard | null; sourceGaps: string[];
  attribution: { leaderId: string; confirmedAtMs: number; status: string; reasonCode?: string } | null;
  qualification?: { levelReached: boolean; activeUtcDays: number; qualifiedAtMs: number | null; status: string };
  settlements: PromotionSettlement[]; payoutSettings: PromotionPayoutSettings;
  appeals: PromotionAppeal[]; permissions: { isLeader: boolean; isAdmin: boolean; roles?: string[] };
};
export type PromotionBootstrap = {
  authenticated: boolean; csrfToken: string | null; campaign: PromotionCampaign | null;
  characters: PromotionCharacter[]; leader: PromotionLeader | null; dashboard: PromotionDashboard | null;
  sourceGaps: string[]; permissions: string[] | { isLeader?: boolean; isAdmin?: boolean; roles?: string[] };
  binding: { status: string; reason?: string; credentialId?: string; address?: string; network?: string };
  me?: PromotionMe;
  public?: { campaignId?: string; environment?: string; [key: string]: unknown };
};
export type InvitationPreview = {
  leaderDisplayName: string; termsVersion: string; termsStatus: string;
  recruitmentEnabled: boolean; rewardedPlacesAvailable: boolean; unrewardedPlayAvailable: boolean;
};

export function hasTestWalletEnrollment(campaign: PromotionCampaign | null | undefined): boolean {
  return campaign?.environment === "test" && campaign.rules?.testWalletEnrollment === true;
}

export function canEnrollTestWallet(campaign: PromotionCampaign | null | undefined, now = Date.now()): boolean {
  return hasTestWalletEnrollment(campaign) && campaign?.termsStatus === "approved" && !!campaign.termsVersion &&
    campaign.recruitmentEnabled === true && Number.isSafeInteger(campaign.startsAtMs) && Number.isSafeInteger(campaign.endsAtMs) &&
    campaign.startsAtMs! > 0 && campaign.startsAtMs! <= now && campaign.endsAtMs! > now;
}

export function testWalletApplication(campaign: PromotionCampaign | null | undefined,
  { bound, acceptedTerms, now = Date.now() }: { bound: boolean; acceptedTerms: boolean; now?: number }) {
  if (!bound || !acceptedTerms || !canEnrollTestWallet(campaign, now)) return null;
  return { characterIndex: -1, termsVersion: campaign!.termsVersion! };
}

export class PromotionApiError extends Error {
  constructor(public readonly status: number, public readonly code: string) { super(code); }
}

const CAMPAIGN_READS = new Set(["bootstrap", "public", "invite", "me", "members", "settlements", "export", "admin"]);
const CAMPAIGN_WRITES = new Set(["click", "apply", "candidate", "confirm", "appeals", "admin"]);

/** Keep a selected season in invitation links and every related request. Identity stays in the cookie. */
export function promotionSeasonRequest(path: string, body?: Record<string, unknown>, search = typeof window === "undefined" ? "" : window.location.search) {
  const values = new URL(search || "/", "https://promotion.invalid").searchParams.getAll("campaignId");
  if (!values.length) return { path, body };
  if (values.length !== 1 || !/^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/.test(values[0])) throw new PromotionApiError(400, "invalid_promotion_campaign");
  const selected = values[0];
  const [route] = path.split("?");
  if (body !== undefined && CAMPAIGN_WRITES.has(route)) {
    if (body.campaignId !== undefined && body.campaignId !== selected) throw new PromotionApiError(400, "promotion_campaign_mismatch");
    return { path, body: { ...body, campaignId: selected } };
  }
  if (body === undefined && CAMPAIGN_READS.has(route)) {
    const url = new URL(`/api/promotions/${path}`, "https://promotion.invalid");
    if (url.searchParams.has("campaignId") && url.searchParams.get("campaignId") !== selected) throw new PromotionApiError(400, "promotion_campaign_mismatch");
    url.searchParams.set("campaignId", selected);
    return { path: `${route}?${url.searchParams}`, body };
  }
  return { path, body };
}

export async function promotionRequest<T>(path: string, options: { body?: Record<string, unknown>; csrfToken?: string; signal?: AbortSignal; idempotencyKey?: string } = {}): Promise<T> {
  const selected = promotionSeasonRequest(path, options.body);
  const writing = selected.body !== undefined;
  const response = await fetch(`/api/promotions/${selected.path}`, {
    method: writing ? "POST" : "GET", credentials: "same-origin", cache: "no-store",
    headers: writing ? {
      "content-type": "application/json", "x-promotion-csrf": options.csrfToken || "",
      "Idempotency-Key": options.idempotencyKey || crypto.randomUUID(),
    } : undefined,
    body: writing ? JSON.stringify(selected.body) : undefined,
    signal: options.signal ? AbortSignal.any([options.signal, AbortSignal.timeout(15_000)]) : AbortSignal.timeout(15_000),
  });
  let body: unknown;
  try { body = await response.json(); } catch { throw new PromotionApiError(response.status || 502, "invalid_response"); }
  if (!response.ok) {
    const code = body && typeof body === "object" && "error" in body && typeof body.error === "string"
      && /^[a-z0-9_.-]{1,100}$/i.test(body.error) ? body.error : `http_${response.status}`;
    throw new PromotionApiError(response.status, code);
  }
  if (body === null || typeof body !== "object" || Array.isArray(body) || Object.keys(body).length === 0) throw new PromotionApiError(502, "invalid_response");
  return body as T;
}

/** Do not parse/re-serialize canonical settlement input: doing so could change integers or hashes. */
export async function readPromotionSettlementExport(settlementId: string): Promise<Blob> {
  const selected = promotionSeasonRequest(`export?settlementId=${encodeURIComponent(settlementId)}`);
  const response = await fetch(`/api/promotions/${selected.path}`, {
    credentials: "same-origin", cache: "no-store", signal: AbortSignal.timeout(30_000),
  });
  if (!response.ok || !response.headers.get("content-type")?.toLowerCase().includes("application/json")) {
    throw new PromotionApiError(response.status || 502, "settlement_export_unavailable");
  }
  const envelope: unknown = await response.json();
  if (!envelope || typeof envelope !== "object" || !("snapshotCanonicalJson" in envelope)
    || typeof envelope.snapshotCanonicalJson !== "string" || !("snapshotSha256" in envelope)
    || typeof envelope.snapshotSha256 !== "string" || !/^[a-f0-9]{64}$/i.test(envelope.snapshotSha256)) {
    throw new PromotionApiError(502, "settlement_export_invalid");
  }
  const bytes = new TextEncoder().encode(envelope.snapshotCanonicalJson);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const actual = Array.from(new Uint8Array(digest), value => value.toString(16).padStart(2, "0")).join("");
  if (actual !== envelope.snapshotSha256.toLowerCase()) throw new PromotionApiError(502, "settlement_export_hash_mismatch");
  return new Blob([envelope.snapshotCanonicalJson], { type: "application/json;charset=utf-8" });
}

/** Exact string formatting: financial amounts never pass through floating-point dollars. */
export function formatPromotionUsd(value: string | null | undefined, locale: PromotionLocale): string {
  if (typeof value !== "string" || !/^-?\d{1,40}$/.test(value)) return "—";
  const raw = BigInt(value), absolute = raw < 0n ? -raw : raw;
  const whole = absolute / 1_000_000n;
  const digits = (absolute % 1_000_000n).toString().padStart(6, "0");
  const fraction = digits.replace(/0+$/, "").padEnd(2, "0");
  const formatter = new Intl.NumberFormat(locale);
  const decimal = formatter.formatToParts(1.1).find(part => part.type === "decimal")?.value || ".";
  const localFraction = [...fraction].map(digit => new Intl.NumberFormat(locale, { useGrouping: false }).format(Number(digit))).join("");
  return `${raw < 0n ? "−" : ""}${formatter.format(whole)}${decimal}${localFraction} USD`;
}

export function formatPromotionCents(value: number | null | undefined, locale: PromotionLocale): string {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? formatPromotionUsd((BigInt(value) * 10_000n).toString(), locale) : "—";
}

export function formatPromotionDate(value: number | null | undefined, locale: PromotionLocale): string {
  return typeof value === "number" && Number.isFinite(value) && value > 0 && Number.isFinite(new Date(value).getTime())
    ? new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value))
    : "—";
}

export function validatePromotionApprovalUrl(value: unknown): string | null {
  if (typeof value !== "string") return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" || (url.protocol === "http:" &&
      ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)) ? url.href : null;
  } catch { return null; }
}

export function promotionTimezone(): string {
  try { return new Intl.DateTimeFormat().resolvedOptions().timeZone; } catch { return "UTC"; }
}
