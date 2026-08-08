"use client";

import { useEffect, useState, type CSSProperties } from "react";

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

  const anyEnabled = settings.musicEnabled || settings.effectsEnabled;
  // A single "master" level reflects the loudest active channel and, when
  // dragged, scales both channels together — mirroring the original options
  // screen's combined volume control without needing a separate lib field.
  const masterVolume = Math.max(
    settings.musicEnabled ? settings.musicVolume : 0,
    settings.effectsEnabled ? settings.effectsVolume : 0,
  );

  function setMasterVolume(value: number) {
    updateSettings({ musicVolume: value, effectsVolume: value });
  }

  function toggleMuteAll() {
    const nextEnabled = !anyEnabled;
    updateSettings({ musicEnabled: nextEnabled, effectsEnabled: nextEnabled });
  }

  return (
    <section className={controlClassName} aria-label={t("ui.audio", [], "Audio")}>
      {compact ? null : (
        <div className="audio-settings-title" style={TITLE_ROW_STYLE}>
          <span>{t("ui.audio", [], "Audio")}</span>
          <button
            type="button"
            className="audio-settings-mute"
            data-audio-enabled={anyEnabled}
            aria-pressed={!anyEnabled}
            onClick={toggleMuteAll}
            style={MUTE_BUTTON_STYLE}
          >
            {anyEnabled ? t("ui.muteAll", [], "Mute All") : t("ui.unmuteAll", [], "Unmute All")}
          </button>
        </div>
      )}
      {compact ? null : (
        <AudioVolumeSlider
          label={t("ui.masterVolume", [], "Master Volume")}
          value={masterVolume}
          disabled={!anyEnabled}
          onChange={setMasterVolume}
        />
      )}
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

const TITLE_ROW_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 8,
};

const MUTE_BUTTON_STYLE: CSSProperties = {
  border: "1px solid rgba(190, 157, 99, 0.5)",
  background: "rgba(30, 20, 12, 0.86)",
  color: "#dfc58f",
  font: "10px Georgia, 'Times New Roman', serif",
  letterSpacing: 0,
  textTransform: "uppercase",
  textShadow: "1px 1px 0 #000",
  padding: "1px 7px",
  cursor: "pointer",
};

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
        aria-valuetext={`${percent}%`}
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
