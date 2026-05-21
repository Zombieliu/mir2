"use client";

import { useEffect, useState } from "react";

import {
  getOriginalAudioSettings,
  setOriginalAudioSettings,
  subscribeOriginalAudioSettings,
  type OriginalAudioSettings,
} from "../../lib/original-audio";
import type { TranslateFn } from "./original-client-types";

type OriginalAudioSettingsControlsProps = {
  t: TranslateFn;
  className?: string;
  compact?: boolean;
};

export function OriginalAudioSettingsControls({
  t,
  className = "",
  compact = false,
}: OriginalAudioSettingsControlsProps) {
  const [settings, setSettings] = useState<OriginalAudioSettings>(() => getOriginalAudioSettings());
  const controlClassName = ["audio-settings-controls", compact ? "compact" : "", className]
    .filter(Boolean)
    .join(" ");

  useEffect(() => subscribeOriginalAudioSettings(setSettings), []);

  function updateSettings(nextSettings: Partial<OriginalAudioSettings>) {
    setSettings(setOriginalAudioSettings(nextSettings));
  }

  return (
    <section className={controlClassName} aria-label={t("ui.audio", [], "Audio")}>
      {compact ? null : <div className="audio-settings-title">{t("ui.audio", [], "Audio")}</div>}
      <AudioToggleButton
        label={t("ui.music", [], "Music")}
        enabled={settings.musicEnabled}
        onClick={() => updateSettings({ musicEnabled: !settings.musicEnabled })}
        t={t}
      />
      <AudioToggleButton
        label={t("ui.effects", [], "Effects")}
        enabled={settings.effectsEnabled}
        onClick={() => updateSettings({ effectsEnabled: !settings.effectsEnabled })}
        t={t}
      />
    </section>
  );
}

type AudioToggleButtonProps = {
  label: string;
  enabled: boolean;
  onClick: () => void;
  t: TranslateFn;
};

function AudioToggleButton({ label, enabled, onClick, t }: AudioToggleButtonProps) {
  return (
    <button
      type="button"
      className="audio-settings-toggle"
      data-audio-enabled={enabled}
      aria-pressed={enabled}
      onClick={onClick}
    >
      <span>{label}</span>
      <strong>{enabled ? t("ui.on", [], "On") : t("ui.off", [], "Off")}</strong>
    </button>
  );
}
