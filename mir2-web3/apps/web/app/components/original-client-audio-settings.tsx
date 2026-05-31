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
      <AudioVolumeSlider
        label={t("ui.musicVolume", [], "Music Volume")}
        value={settings.musicVolume}
        disabled={!settings.musicEnabled}
        onChange={(value) => updateSettings({ musicVolume: value })}
      />
      <AudioToggleButton
        label={t("ui.effects", [], "Effects")}
        enabled={settings.effectsEnabled}
        onClick={() => updateSettings({ effectsEnabled: !settings.effectsEnabled })}
        t={t}
      />
      <AudioVolumeSlider
        label={t("ui.effectsVolume", [], "Effects Volume")}
        value={settings.effectsVolume}
        disabled={!settings.effectsEnabled}
        onChange={(value) => updateSettings({ effectsVolume: value })}
      />
    </section>
  );
}

type AudioVolumeSliderProps = {
  label: string;
  value: number;
  disabled: boolean;
  onChange: (value: number) => void;
};

function AudioVolumeSlider({ label, value, disabled, onChange }: AudioVolumeSliderProps) {
  const percent = Math.round(Math.min(1, Math.max(0, value)) * 100);
  return (
    <label className="audio-settings-slider" data-audio-disabled={disabled}>
      <span>{label}</span>
      <input
        type="range"
        min={0}
        max={100}
        step={1}
        value={percent}
        disabled={disabled}
        aria-label={label}
        onChange={(event) => onChange(Number(event.target.value) / 100)}
      />
      <strong>{percent}%</strong>
    </label>
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
