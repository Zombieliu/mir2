export type BevyMapImageResidencyResult = {
  presented: Set<string>;
  presentedChanged: boolean;
  releasedUploadKeys: string[];
};

export function shouldUploadBevyMapImage(uploaded: ReadonlySet<string>, key: string): boolean {
  return !uploaded.has(key);
}

export function isCompleteBevyMapImageFamilyResident(
  presented: ReadonlySet<string>,
  requiredImageKeys: readonly string[],
): boolean {
  return (
    requiredImageKeys.length > 0 &&
    requiredImageKeys.every((key) => presented.has(key))
  );
}

export function reconcileBevyMapImageResidency(
  uploaded: Set<string>,
  presented: ReadonlySet<string>,
  residentImageKeys: readonly string[],
): BevyMapImageResidencyResult {
  const nextPresented = new Set(residentImageKeys);
  const releasedUploadKeys: string[] = [];

  // Rust prunes inactive standalone frames after each committed map transaction.
  // Forget the matching upload marker so a recurring animation frame is sent again.
  for (const key of uploaded) {
    if (!nextPresented.has(key)) {
      uploaded.delete(key);
      releasedUploadKeys.push(key);
    }
  }

  const presentedChanged =
    nextPresented.size !== presented.size ||
    Array.from(nextPresented).some((key) => !presented.has(key));

  return {
    presented: nextPresented,
    presentedChanged,
    releasedUploadKeys,
  };
}
