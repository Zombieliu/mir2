export type BevyEntityAtlasPolicy = "disabled" | "stable" | "dynamic";

export function resolveBevyEntityAtlasPolicy(input: {
  queryValue?: string | null;
  storedValue?: string | null;
}): BevyEntityAtlasPolicy {
  const value = input.queryValue ?? input.storedValue ?? null;
  if (value === "0") return "disabled";
  if (value === "1" || value === "dynamic") return "dynamic";
  return "stable";
}

export function bevyEntityAtlasCandidateHasCoverage(
  candidateRectKeys: Iterable<string>,
  sourceKeys: ReadonlySet<string>,
) {
  const rectKeys = candidateRectKeys instanceof Set
    ? candidateRectKeys
    : new Set(candidateRectKeys);
  for (const sourceKey of sourceKeys) {
    if (rectKeys.has(sourceKey)) return true;
  }
  return false;
}
