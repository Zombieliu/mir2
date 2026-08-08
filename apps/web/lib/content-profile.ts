export type Mir2ContentProfileName = "platinum_176" | "crystal_full";

function configuredContentProfile(): Mir2ContentProfileName {
  const configured = process.env.NEXT_PUBLIC_MIR2_CONTENT_PROFILE?.trim().toLowerCase();
  return configured === "crystal_full" ? "crystal_full" : "platinum_176";
}

export const MIR2_CONTENT_PROFILE = configuredContentProfile();
export const IS_PLATINUM_176_PROFILE = MIR2_CONTENT_PROFILE === "platinum_176";
