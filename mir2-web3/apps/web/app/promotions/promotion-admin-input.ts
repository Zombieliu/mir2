export type PromotionReviewAction = "proposeLaunch" | "approveLaunch" | "freezePayoutIntent" | "approvePayout" | "payoutOn" | "payoutOff";
export type PromotionFundingMode = "revenue_share" | "budget_capped";
export const PROMOTION_DRAFT_SEASON_UTC = { startsAt: "2026-10-31T16:00", endsAt: "2027-01-31T16:00" } as const;

export function promotionFundingLimits(mode: PromotionFundingMode, input: {
  budget: string; memberLimit: string; leaderLimit: string; perLeaderMemberLimit: string; revenueCap: string;
}): { fundingMode: PromotionFundingMode; budgetMicroUsd: number; memberLimit: number; leaderLimit: number; perLeaderMemberLimit: number; perMemberRevenueCapMicroUsd: number } {
  if (mode === "revenue_share") return {fundingMode:mode,budgetMicroUsd:0,memberLimit:0,leaderLimit:0,perLeaderMemberLimit:0,perMemberRevenueCapMicroUsd:0};
  if (mode !== "budget_capped") throw new Error("invalid_funding_mode");
  const integer = (value: string) => {
    if (!/^\d+$/.test(value) || !Number.isSafeInteger(Number(value))) throw new Error("invalid_funding_limit");
    return Number(value);
  };
  const money = (value: string) => {
    if (!/^\d+(?:\.\d{1,6})?$/.test(value)) throw new Error("invalid_funding_limit");
    const [whole,fraction=""] = value.split(".");
    const parsed=BigInt(whole)*1_000_000n+BigInt(fraction.padEnd(6,"0"));
    if (parsed>BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("invalid_funding_limit");
    return Number(parsed);
  };
  return {fundingMode:mode,budgetMicroUsd:money(input.budget),memberLimit:integer(input.memberLimit),leaderLimit:integer(input.leaderLimit),perLeaderMemberLimit:integer(input.perLeaderMemberLimit),perMemberRevenueCapMicroUsd:money(input.revenueCap)};
}

/** Review documents preserve monetary integers as strings. Reject JSON numbers
 * whose precision the browser cannot retain before anything reaches the API. */
export function parsePromotionReviewJson(value: string): Record<string, unknown> {
  if (new TextEncoder().encode(value).length > 12_000) throw new Error("invalid_review_json");
  const result: unknown = JSON.parse(value, (key, item: unknown) => {
    if (["__proto__", "prototype", "constructor"].includes(key)) throw new Error("invalid_review_json");
    if (typeof item === "number" && !Number.isSafeInteger(item)) throw new Error("invalid_review_json");
    return item;
  });
  if (!result || typeof result !== "object" || Array.isArray(result)) throw new Error("invalid_review_json");
  return result as Record<string, unknown>;
}

export function promotionReviewHash(value: string): string {
  const trimmed = value.trim();
  if (!/^[0-9a-f]{64}$/.test(trimmed)) throw new Error("invalid_review_hash");
  return trimmed;
}

export function promotionReviewCommand(action: PromotionReviewAction, input: {
  document: string; hash: string; resourceId: string; evidenceRef: string;
}): Record<string, unknown> {
  const evidenceRef = input.evidenceRef.trim();
  if (evidenceRef.length < 3 || evidenceRef.length > 512) throw new Error("invalid_review_evidence");
  if (action === "proposeLaunch") return { action, manifest: parsePromotionReviewJson(input.document), evidenceRef };
  if (action === "freezePayoutIntent") return { action, intent: parsePromotionReviewJson(input.document), evidenceRef };
  if (action === "approveLaunch") return { action, manifestSha256: promotionReviewHash(input.hash), evidenceRef };
  if (action === "approvePayout") {
    const payoutId = input.resourceId.trim();
    if (!/^[A-Za-z0-9:_-]{1,128}$/.test(payoutId)) throw new Error("invalid_review_resource");
    return { action, payoutId, intentSha256: promotionReviewHash(input.hash), evidenceRef };
  }
  if (action === "payoutOn" || action === "payoutOff") return { action: "setPayout", enabled: action === "payoutOn", evidenceRef };
  throw new Error("invalid_review_action");
}
