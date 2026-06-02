"use client";

import { useEffect, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type ChatChannelKey =
  | "normal"
  | "shout"
  | "whisper"
  | "group"
  | "guild"
  | "lover"
  | "mentor"
  | "system";

export type ChatFontSize = "small" | "medium" | "large";

/** The chat display settings this window edits. */
export type ChatSettings = {
  /** Channels that are currently visible in the chat box. */
  channels: Record<ChatChannelKey, boolean>;
  fontSize: ChatFontSize;
  /** Whether to prefix each line with a timestamp. */
  showTimestamps: boolean;
  /** Whether the chat box keeps a translucent background. */
  transparentBackground: boolean;
};

export type ChatSettingsWindowProps = {
  t: TranslateFn;
  /** Controlled settings; when omitted the window manages a local default copy. */
  settings?: ChatSettings;
  /** Persist edited settings. When omitted the window still toggles locally. */
  onApply?: (settings: ChatSettings) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.mail;

const CHANNEL_ROWS: { key: ChatChannelKey; labelKey: string; fallback: string; dot: string }[] = [
  { key: "normal", labelKey: "ui.chatChannelNormal", fallback: "Normal", dot: "#e3d3af" },
  { key: "shout", labelKey: "ui.chatChannelShout", fallback: "Shout", dot: "#e8c45a" },
  { key: "whisper", labelKey: "ui.chatChannelWhisper", fallback: "Whisper", dot: "#74b6e8" },
  { key: "group", labelKey: "ui.chatChannelGroup", fallback: "Group", dot: "#8be07a" },
  { key: "guild", labelKey: "ui.chatChannelGuild", fallback: "Guild", dot: "#caa64a" },
  { key: "lover", labelKey: "ui.chatChannelLover", fallback: "Lover", dot: "#e87ab0" },
  { key: "mentor", labelKey: "ui.chatChannelMentor", fallback: "Mentor", dot: "#b07ae8" },
  { key: "system", labelKey: "ui.chatChannelSystem", fallback: "System", dot: "#d8552f" },
];

const FONT_SIZES: { key: ChatFontSize; labelKey: string; fallback: string }[] = [
  { key: "small", labelKey: "ui.chatFontSmall", fallback: "Small" },
  { key: "medium", labelKey: "ui.chatFontMedium", fallback: "Medium" },
  { key: "large", labelKey: "ui.chatFontLarge", fallback: "Large" },
];

const DEFAULT_SETTINGS: ChatSettings = {
  channels: {
    normal: true,
    shout: true,
    whisper: true,
    group: true,
    guild: true,
    lover: true,
    mentor: true,
    system: true,
  },
  fontSize: "medium",
  showTimestamps: false,
  transparentBackground: true,
};

export function ChatSettingsWindow({ t, settings, onApply, onClose }: ChatSettingsWindowProps) {
  const [draft, setDraft] = useState<ChatSettings>(() => settings ?? DEFAULT_SETTINGS);

  // Re-sync when the host pushes a new controlled value.
  useEffect(() => {
    if (settings) {
      setDraft(settings);
    }
  }, [settings]);

  const update = (next: ChatSettings) => {
    setDraft(next);
    onApply?.(next);
  };

  const toggleChannel = (key: ChatChannelKey) => {
    update({ ...draft, channels: { ...draft.channels, [key]: !draft.channels[key] } });
  };

  const visibleCount = CHANNEL_ROWS.filter((row) => draft.channels[row.key]).length;

  return (
    <section
      aria-label={t("ui.chatSettings", [], "Chat Settings")}
      data-chat-font={draft.fontSize}
      data-chat-visible={visibleCount}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.title} src={FRAME.title} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.chatSettings", [], "Chat Settings")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={{ ...style.sectionLabel, top: 36 }}>{t("ui.chatChannels", [], "Channels")}</div>
      <div style={style.channels} aria-label={t("ui.chatChannels", [], "Channels")}>
        {CHANNEL_ROWS.map((row) => {
          const on = draft.channels[row.key];
          return (
            <button
              key={row.key}
              type="button"
              role="switch"
              aria-checked={on}
              data-chat-channel={row.key}
              onClick={() => toggleChannel(row.key)}
              style={{ ...style.channelRow, ...(on ? style.channelRowOn : null) }}
            >
              <span style={{ ...style.channelDot, background: row.dot, opacity: on ? 1 : 0.3 }} aria-hidden="true" />
              <span style={style.channelName}>{t(row.labelKey, [], row.fallback)}</span>
              <span style={{ ...style.channelState, color: on ? "#8be07a" : "#9c8d6f" }}>
                {on ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
              </span>
            </button>
          );
        })}
      </div>

      <div style={{ ...style.sectionLabel, top: 184 }}>{t("ui.chatFontSize", [], "Font Size")}</div>
      <div style={style.fontRow}>
        {FONT_SIZES.map((size) => {
          const active = draft.fontSize === size.key;
          return (
            <button
              key={size.key}
              type="button"
              aria-pressed={active}
              data-chat-font={size.key}
              onClick={() => update({ ...draft, fontSize: size.key })}
              style={{ ...style.fontButton, ...(active ? style.fontButtonActive : null) }}
            >
              {t(size.labelKey, [], size.fallback)}
            </button>
          );
        })}
      </div>

      <div style={style.optionRow}>
        <ToggleOption
          label={t("ui.chatTimestamps", [], "Show timestamps")}
          on={draft.showTimestamps}
          onToggle={() => update({ ...draft, showTimestamps: !draft.showTimestamps })}
          t={t}
        />
        <ToggleOption
          label={t("ui.chatTransparent", [], "Transparent box")}
          on={draft.transparentBackground}
          onToggle={() => update({ ...draft, transparentBackground: !draft.transparentBackground })}
          t={t}
        />
      </div>

      <div style={style.footer}>
        <span style={style.footerInfo}>{t("ui.chatVisibleCount", [visibleCount, CHANNEL_ROWS.length], `${visibleCount}/${CHANNEL_ROWS.length} channels shown`)}</span>
      </div>
    </section>
  );
}

function ToggleOption({
  label,
  on,
  onToggle,
  t,
}: {
  label: string;
  on: boolean;
  onToggle: () => void;
  t: TranslateFn;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      onClick={onToggle}
      style={{ ...style.toggle, ...(on ? style.toggleOn : null) }}
    >
      <span style={style.toggleLabel}>{label}</span>
      <span style={{ ...style.toggleState, color: on ? "#8be07a" : "#9c8d6f" }}>
        {on ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
      </span>
    </button>
  );
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 356,
    top: 60,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 33,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  title: { position: "absolute", left: 18, top: 9 },
  titleText: {
    position: "absolute",
    left: 18,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  close: { position: "absolute", left: 288, top: 3 },
  sectionLabel: {
    position: "absolute",
    left: 14,
    fontSize: 10,
    color: "#a89568",
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  channels: {
    position: "absolute",
    left: 12,
    top: 52,
    width: 288,
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gap: 4,
  },
  channelRow: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: "4px 8px",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "rgba(20, 13, 7, 0.5)",
    color: "#cbb38a",
    cursor: "pointer",
  },
  channelRowOn: {
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.8), rgba(45, 23, 12, 0.8))",
    borderColor: "rgba(214, 180, 110, 0.7)",
    color: "#f4dcaf",
  },
  channelDot: { width: 8, height: 8, borderRadius: "50%", flex: "0 0 auto" },
  channelName: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 11 },
  channelState: { fontSize: 9, flex: "0 0 auto" },
  fontRow: { position: "absolute", left: 12, top: 200, width: 288, display: "flex", gap: 6 },
  fontButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "5px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  fontButtonActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  optionRow: { position: "absolute", left: 12, top: 244, width: 288, display: "flex", flexDirection: "column", gap: 6 },
  toggle: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
    width: "100%",
    padding: "6px 10px",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "rgba(20, 13, 7, 0.5)",
    color: "#cbb38a",
    cursor: "pointer",
    fontSize: 11,
  },
  toggleOn: {
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.8), rgba(45, 23, 12, 0.8))",
    borderColor: "rgba(214, 180, 110, 0.7)",
    color: "#f4dcaf",
  },
  toggleLabel: { flex: 1, textAlign: "left" },
  toggleState: { fontSize: 10, flex: "0 0 auto" },
  footer: {
    position: "absolute",
    left: 12,
    top: 410,
    width: 288,
    display: "flex",
    alignItems: "center",
  },
  footerInfo: { fontSize: 11, color: "#cbb38a" },
};
