export function questIdsFromPacket(
  value: unknown,
  previous: number[] | undefined,
): number[] | undefined {
  if (!Array.isArray(value)) {
    return previous;
  }

  const questIds: number[] = [];
  for (const entry of value) {
    if (
      typeof entry === "number" &&
      Number.isInteger(entry) &&
      !questIds.includes(entry)
    ) {
      questIds.push(entry);
    }
  }
  return questIds;
}
