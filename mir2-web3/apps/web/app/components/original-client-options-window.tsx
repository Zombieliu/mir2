"use client";

import { useRef } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { CRYSTAL_OPTION_KEYS, type CrystalOptionKey, type PlayerUiPreferences } from "../../lib/player-ui-preferences";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (key: string, params?: Array<string | number>, fallback?: string) => string;
export type OptionsWindowProps = {
  t: TranslateFn; preferences: Readonly<PlayerUiPreferences>; onClose: () => void;
  /** Immediate local application and persistence are owned by the host. */
  onChange?: (preferences: PlayerUiPreferences) => void;
  supportedOptions?: readonly CrystalOptionKey[]; audioSupported?: boolean; disabled?: boolean;
  /** Unknown remains unknown. Only an authoritative AllowObserve packet changes this. */
  observeAllowed?: boolean | null; observePending?: boolean; onRequestObserve?: (allow: boolean) => void;
};
const OPTION_ROWS: readonly { key: CrystalOptionKey; label: string; top: number }[] = [
  { key: "skillMode", label: "Skill mode", top: 68 }, { key: "skillBar", label: "Skill bar", top: 93 },
  { key: "effect", label: "Effects", top: 118 }, { key: "dropView", label: "Drop names", top: 143 },
  { key: "nameView", label: "Names", top: 168 }, { key: "hpView", label: "Health bars", top: 193 },
  { key: "newMove", label: "New movement", top: 296 },
];
function optionSprite(key: CrystalOptionKey, value: boolean, on: boolean) {
  const library = key === "newMove" ? "Title" : "Prguse2";
  const index = key === "skillMode" ? on ? value ? 452 : 450 : value ? 455 : 453
    : key === "hpView" ? on ? value ? 464 : 462 : value ? 467 : 465
    : key === "newMove" ? on ? value ? 853 : 851 : value ? 850 : 848
    : on ? value ? 458 : 456 : value ? 461 : 459;
  const pressed = key === "skillMode" ? on ? 451 : 454 : key === "hpView" ? on ? 463 : 466
    : key === "newMove" ? on ? 853 : 850 : on ? 457 : 460;
  return { base: "/original-ui/" + library + "/" + index + ".png", pressed: "/original-ui/" + library + "/" + pressed + ".png" };
}
export function OptionsWindow({ t, preferences, onClose, onChange, supportedOptions = CRYSTAL_OPTION_KEYS,
  audioSupported = true, disabled = false, observeAllowed = null, observePending = false, onRequestObserve }: OptionsWindowProps) {
  const volumeDragRef = useRef<{ channel: "sound" | "music"; pointerId: number } | null>(null);
  const change = (key: CrystalOptionKey, enabled: boolean) => {
    if (!disabled && onChange && supportedOptions.includes(key) && preferences[key] !== enabled) onChange({ ...preferences, [key]: enabled });
  };
  const changeVolume = (channel: "sound" | "music", volume: number) => {
    if (disabled || !audioSupported || !onChange || !Number.isInteger(volume) || volume < 0 || volume > 100) return;
    const key = channel === "sound" ? "soundVolume" : "musicVolume";
    if (preferences[key] !== volume) onChange({ ...preferences, [key]: volume });
  };
  return <div className="original-options-window" role="dialog" aria-label={t("ui.options", undefined, "Options")} data-ui-interactive="true"
    style={{ position: "absolute", left: 382, top: 207, width: 259, height: 354, zIndex: 45 }}>
    <img src="/original-ui/Title/411.png" alt="" draggable={false} style={{ position: "absolute", inset: 0, width: 259, height: 354, pointerEvents: "none" }} />
    <div style={{ position: "absolute", left: 233, top: 5 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.closeButton} label={t("ui.close", undefined, "Close")} onClick={onClose} /></div>
    {OPTION_ROWS.map(row => <div key={row.key} role="group" aria-label={t("ui.option." + row.key, undefined, row.label)}>
      {[true, false].map(on => <div key={String(on)} style={{ position: "absolute", left: on ? 159 : 201, top: row.top }}>
        <SpriteButton sprite={optionSprite(row.key, preferences[row.key], on)}
          label={t("ui.option." + row.key, undefined, row.label) + ": " + (row.key === "skillMode" ? on ? "~" : "Ctrl" : t(on ? "ui.on" : "ui.off", undefined, on ? "On" : "Off"))}
          disabled={disabled || !onChange || !supportedOptions.includes(row.key)} onClick={() => change(row.key, on)} />
      </div>)}
    </div>)}
    {["sound", "music"].map(channel => {
      const sound = channel === "sound";
      const enabled = sound ? preferences.soundEnabled : preferences.musicEnabled;
      const volume = sound ? preferences.soundVolume : preferences.musicVolume;
      return <div key={channel}>
        <div style={{ position: "absolute", left: 159, top: sound ? 225 : 251, width: 76, height: 19, pointerEvents: "none" }}>
          <div style={{ width: 74 * (enabled ? volume : 0) / 100, height: 19, overflow: "hidden" }}><img src="/original-ui/Prguse2/468.png" alt="" draggable={false} style={{ width: 76, height: 19, maxWidth: "none" }} /></div>
          <img src="/original-ui/Prguse/20.png" alt="" draggable={false} style={{ position: "absolute", left: 74 * (enabled ? volume : 0) / 100, top: -7, width: 8, height: 22 }} />
        </div>
        <button type="button" role="slider" aria-valuemin={0} aria-valuemax={100} aria-valuenow={enabled ? volume : 0}
          aria-label={t(sound ? "ui.soundVolume" : "ui.musicVolume", undefined, sound ? "Sound volume" : "Music volume")}
          disabled={disabled || !audioSupported || !onChange}
          onKeyDown={event => {
            const value = event.key === "Home" ? 0 : event.key === "End" ? 100
              : event.key === "ArrowLeft" || event.key === "ArrowDown" ? Math.max(0, volume - 1)
              : event.key === "ArrowRight" || event.key === "ArrowUp" ? Math.min(100, volume + 1) : null;
            if (value !== null) { event.preventDefault(); event.stopPropagation(); changeVolume(sound ? "sound" : "music", value); }
          }}
          onPointerDown={event => {
            if (event.button !== 0 || disabled || !audioSupported || !onChange) return;
            event.preventDefault(); event.stopPropagation();
            const rect = event.currentTarget.getBoundingClientRect(); if (rect.width <= 0) return;
            volumeDragRef.current = { channel: sound ? "sound" : "music", pointerId: event.pointerId };
            event.currentTarget.setPointerCapture(event.pointerId);
            changeVolume(sound ? "sound" : "music", Math.trunc(Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)) * 100));
          }}
          onPointerMove={event => {
            const drag = volumeDragRef.current; if (!drag || drag.pointerId !== event.pointerId || drag.channel !== channel) return;
            event.preventDefault(); event.stopPropagation();
            const rect = event.currentTarget.getBoundingClientRect(); if (rect.width <= 0) return;
            changeVolume(drag.channel, Math.trunc(Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)) * 100));
          }}
          onPointerUp={event => { if (volumeDragRef.current?.pointerId === event.pointerId) volumeDragRef.current = null; }}
          onPointerCancel={() => { volumeDragRef.current = null; }} onLostPointerCapture={() => { volumeDragRef.current = null; }}
          style={{ position: "absolute", left: 159, top: sound ? 225 : 251, width: 76, height: 19, padding: 0, border: 0, background: "transparent", touchAction: "none", cursor: "pointer" }} />
      </div>;
    })}
    {[true, false].map(allow => <div key={String(allow)} style={{ position: "absolute", left: allow ? 159 : 201, top: 271 }}>
      <SpriteButton sprite={{ base: "/original-ui/Prguse2/" + (allow ? observeAllowed === true ? 458 : 456 : observeAllowed === false ? 459 : 461) + ".png",
        pressed: "/original-ui/Prguse2/" + (allow ? 457 : 460) + ".png" }}
        label={t("ui.observe", undefined, "Allow observe") + ": " + t(allow ? "ui.on" : "ui.off", undefined, allow ? "On" : "Off")}
        disabled={disabled || observeAllowed === null || observePending || !onRequestObserve}
        onClick={() => { if (observeAllowed !== null && observeAllowed !== allow && !observePending && !disabled) onRequestObserve?.(allow); }} />
    </div>)}
    {observePending || observeAllowed === null ? <span role="status" style={{ position: "absolute", left: 15, top: 325, fontSize: 9, color: "#d8c47c" }}>
      {t(observePending ? "ui.pending" : "ui.stateUnknown", undefined, observePending ? "Waiting for server" : "Observe state unavailable")}
    </span> : null}
  </div>;
}
