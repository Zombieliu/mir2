/** Optional server scalars belong to one self and one connection/session generation. */
export type SelfAppearance = { owner: string; hair?: number; wingEffect?: number };
const scalar = (v: unknown): number | undefined => Number.isInteger(v) && Number(v) >= 0 && Number(v) <= 255 ? Number(v) : undefined;
export function projectSelfAppearance(previous: SelfAppearance | null, owner: string,
  entities: readonly { objectId: unknown; kind?: string; hair?: unknown; wingEffect?: unknown; wing_effect?: unknown }[], playerObjectId: unknown): SelfAppearance {
  const hasId = playerObjectId !== null && playerObjectId !== undefined && String(playerObjectId).length > 0;
  const entity = entities.find(e => hasId && e.objectId !== null && e.objectId !== undefined && String(e.objectId) === String(playerObjectId))
    ?? entities.find(e => e.kind === "selfPlayer");
  const result: SelfAppearance = previous?.owner === owner ? { ...previous } : { owner };
  if (!entity) return result;
  const hair = scalar(entity.hair), wing = scalar(entity.wingEffect ?? entity.wing_effect);
  if (hair !== undefined) result.hair = hair;
  if (wing !== undefined) result.wingEffect = wing;
  return result;
}
export function sameSelfAppearance(a: Pick<SelfAppearance, "hair" | "wingEffect"> | null | undefined,
  b: Pick<SelfAppearance, "hair" | "wingEffect"> | null | undefined) {
  return a?.hair === b?.hair && a?.wingEffect === b?.wingEffect;
}
