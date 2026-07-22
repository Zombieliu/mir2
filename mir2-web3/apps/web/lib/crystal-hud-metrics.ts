export const CRYSTAL_MAIN_HUD_EXPERIENCE_BAR_WIDTH = 1004;

export function crystalMainHudExperienceBarFillWidth(experienceRatio: number) {
  const normalizedRatio = Number.isFinite(experienceRatio)
    ? Math.max(0, Math.min(1, experienceRatio))
    : 0;
  return Math.floor((CRYSTAL_MAIN_HUD_EXPERIENCE_BAR_WIDTH - 3) * normalizedRatio);
}
