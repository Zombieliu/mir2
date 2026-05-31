"use client";

import { OriginalAudioSettingsControls } from "./original-client-audio-settings";
import { SpriteButton } from "./original-client-overlays";
import { ORIGINAL_UI } from "../../lib/original-ui";
import type { TranslateFn } from "./original-client-types";

export type OptionsDialogProps = {
  t: TranslateFn;
  chatTransparent: boolean;
  onToggleChatTransparent: () => void;
  onClose: () => void;
};

// Crystal's Option dialog groups Sound / Game toggles. The HUD Option button
// now opens this in-game panel instead of jumping to the character stats tab.
// Audio (music/effects) is fully wired; the remaining video/game rows are shown
// as the toggles the web client actually owns.
export function OptionsDialog({ t, chatTransparent, onToggleChatTransparent, onClose }: OptionsDialogProps) {
  return (
    <section className="options-dialog" role="dialog" aria-label={t("ui.options", [], "Options")}>
      <div className="options-dialog-head">
        <span className="options-dialog-title">{t("ui.options", [], "Options")}</span>
        <div className="options-dialog-close">
          <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.close")} onClick={onClose} />
        </div>
      </div>

      <div className="options-dialog-section">
        <div className="options-dialog-section-title">{t("ui.sound", [], "Sound")}</div>
        <OriginalAudioSettingsControls t={t} className="options-audio-settings" />
      </div>

      <div className="options-dialog-section">
        <div className="options-dialog-section-title">{t("ui.game", [], "Game")}</div>
        <button
          type="button"
          className="options-dialog-toggle"
          data-options-toggle="chat-transparent"
          data-options-toggle-on={chatTransparent}
          onClick={onToggleChatTransparent}
        >
          <span>{t("ui.chatTransparency", [], "Chat Transparency")}</span>
          <span className="options-dialog-toggle-state">
            {chatTransparent ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
          </span>
        </button>
      </div>
    </section>
  );
}
