/** Keep wallet, game and billing requests in the same selected season as the dashboard. */
export function promotionSeasonHeaders(search = typeof window === "undefined" ? "" : window.location.search): Record<string, string> {
  const selected = new URL(search || "/", "https://promotion.invalid").searchParams.getAll("campaignId");
  if (!selected.length) return {};
  if (selected.length !== 1 || !/^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/.test(selected[0])) throw new Error("invalid_promotion_campaign");
  return { "x-mir2-promotion-season": selected[0] };
}
