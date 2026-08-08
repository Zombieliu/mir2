import type { ClientScreen } from "../../lib/original-ui";
import type { DisplayEntity, EntityKind, TranslateFn } from "./original-client-types";

export const LOGIN_STATIC_BACKGROUND_FRAME = 0;
export const LOGIN_TRANSITION_FRAME_MS = 180;
export const ORIGINAL_AUDIO = {
  loginMusic: "/original-ui/Sound/Login2.wav",
  loginEffect: "/original-ui/Sound/100.wav",
  selectMusic: "/original-ui/Sound/Select2.wav",
} as const;
export const ORIGINAL_MUSIC_VOLUME = 0.72;
export const ORIGINAL_EFFECT_VOLUME = 0.86;

/** Builds the compact target prompt without owning target-selection state. */
export function selectedTargetActionLabel(
  t: TranslateFn,
  entity: DisplayEntity,
  targetDistance: number | null,
): string {
  const actionLabel =
    entity.kind === "monster"
      ? t("ui.attack", [], "Attack")
      : entity.kind === "npc"
        ? t("ui.talk", [], "Talk")
        : t("ui.approach", [], "Approach");

  if (targetDistance === null) {
    return `${t("ui.target", [], "Target")} · ${actionLabel}`;
  }

  return t("ui.targetDistance", [actionLabel, targetDistance], `${actionLabel} · ${targetDistance} tiles`);
}

export function desiredMusicForScreen(screen: ClientScreen, loginTransitionActive: boolean) {
  // The transition retains Login2.wav until the select screen is actually
  // visible, matching the original client's audio boundary.
  if (loginTransitionActive) {
    return ORIGINAL_AUDIO.loginMusic;
  }

  if (screen === "login") {
    return ORIGINAL_AUDIO.loginMusic;
  }

  if (screen === "select") {
    return ORIGINAL_AUDIO.selectMusic;
  }

  return null;
}

export function entityKindLabelKey(kind: EntityKind) {
  // Keep this exhaustive: adding an EntityKind should fail type checking until
  // its original-client label is intentionally selected.
  switch (kind) {
    case "selfPlayer":
      return "ui.self";
    case "player":
      return "ui.player";
    case "monster":
      return "ui.monster";
    case "npc":
      return "ui.npc";
  }
}
