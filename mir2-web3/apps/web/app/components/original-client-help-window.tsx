"use client";

import { useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type HelpWindowProps = {
  t: TranslateFn;
  onClose: () => void;
};

type HelpTopicKey = "basics" | "combat" | "social" | "economy" | "advanced";

type HelpEntry = { titleKey: string; titleFallback: string; bodyKey: string; bodyFallback: string };

const FRAME = ORIGINAL_UI.bigMap;

const HELP_TABS: { key: HelpTopicKey; labelKey: string; fallback: string }[] = [
  { key: "basics", labelKey: "ui.helpTabBasics", fallback: "Basics" },
  { key: "combat", labelKey: "ui.helpTabCombat", fallback: "Combat" },
  { key: "social", labelKey: "ui.helpTabSocial", fallback: "Social" },
  { key: "economy", labelKey: "ui.helpTabEconomy", fallback: "Economy" },
  { key: "advanced", labelKey: "ui.helpTabAdvanced", fallback: "Advanced" },
];

const HELP_CONTENT: Record<HelpTopicKey, HelpEntry[]> = {
  basics: [
    {
      titleKey: "ui.helpMoveTitle",
      titleFallback: "Movement",
      bodyKey: "ui.helpMoveBody",
      bodyFallback: "Left-click the ground to walk. Hold the button to keep moving. Click an exit tile to change maps.",
    },
    {
      titleKey: "ui.helpInteractTitle",
      titleFallback: "Interacting",
      bodyKey: "ui.helpInteractBody",
      bodyFallback: "Left-click an NPC to talk, a monster to attack, or a dropped item to pick it up.",
    },
    {
      titleKey: "ui.helpWindowsTitle",
      titleFallback: "Windows",
      bodyKey: "ui.helpWindowsBody",
      bodyFallback: "Use the bottom toolbar or hotkeys to open Character, Inventory, Skills and the system menu.",
    },
  ],
  combat: [
    {
      titleKey: "ui.helpAttackTitle",
      titleFallback: "Attacking",
      bodyKey: "ui.helpAttackBody",
      bodyFallback: "Hold the action key and click an enemy to keep attacking. Equip a better weapon to raise damage.",
    },
    {
      titleKey: "ui.helpSkillTitle",
      titleFallback: "Skills",
      bodyKey: "ui.helpSkillBody",
      bodyFallback: "Assign learned skills to the belt slots, then press the matching key or click the slot to cast.",
    },
    {
      titleKey: "ui.helpReviveTitle",
      titleFallback: "Recovery",
      bodyKey: "ui.helpReviveBody",
      bodyFallback: "Use potions from the belt to restore HP/MP. Rest in a safe zone to recover faster.",
    },
  ],
  social: [
    {
      titleKey: "ui.helpGroupTitle",
      titleFallback: "Groups",
      bodyKey: "ui.helpGroupBody",
      bodyFallback: "Invite nearby players to a group to share experience and loot. The leader manages the loot mode.",
    },
    {
      titleKey: "ui.helpGuildTitle",
      titleFallback: "Guilds",
      bodyKey: "ui.helpGuildBody",
      bodyFallback: "Join or found a guild for shared chat, ranks and territory wars at the conquest castle.",
    },
    {
      titleKey: "ui.helpFriendTitle",
      titleFallback: "Friends & Whisper",
      bodyKey: "ui.helpFriendBody",
      bodyFallback: "Add friends to see who is online, and whisper a player by name from the Friends window.",
    },
  ],
  economy: [
    {
      titleKey: "ui.helpTradeTitle",
      titleFallback: "Trading",
      bodyKey: "ui.helpTradeBody",
      bodyFallback: "Request a trade with a nearby player, place gold and items, then both sides confirm to complete it.",
    },
    {
      titleKey: "ui.helpShopTitle",
      titleFallback: "Shops & Market",
      bodyKey: "ui.helpShopBody",
      bodyFallback: "Buy and sell at NPC shops, or list items on the player market for other players to purchase.",
    },
    {
      titleKey: "ui.helpStorageTitle",
      titleFallback: "Storage",
      bodyKey: "ui.helpStorageBody",
      bodyFallback: "Deposit valuables and gold in storage at a warehouse NPC. Set a password to keep it secure.",
    },
  ],
  advanced: [
    {
      titleKey: "ui.helpHeroTitle",
      titleFallback: "Hero & Pets",
      bodyKey: "ui.helpHeroBody",
      bodyFallback: "Summon a hero companion or tamed creatures to fight alongside you and gather drops.",
    },
    {
      titleKey: "ui.helpMapTitle",
      titleFallback: "World Map",
      bodyKey: "ui.helpMapBody",
      bodyFallback: "Open the world map to see your location, map links and any teleport destinations you have unlocked.",
    },
    {
      titleKey: "ui.helpBondsTitle",
      titleFallback: "Bonds",
      bodyKey: "ui.helpBondsBody",
      bodyFallback: "Form a marriage or mentor bond for shared buffs and bonus experience while playing together.",
    },
  ],
};

export function HelpWindow({ t, onClose }: HelpWindowProps) {
  const [tab, setTab] = useState<HelpTopicKey>("basics");
  const entries = HELP_CONTENT[tab];

  return (
    <section aria-label={t("ui.help", [], "Help")} data-help-tab={tab} style={style.window}>
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.help", [], "Help")}</div>
      <div style={style.subtitle}>{t("ui.helpSubtitle", [], "Legend of Mir 2 quick reference")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.help", [], "Help")}>
        {HELP_TABS.map((entry) => {
          const active = entry.key === tab;
          return (
            <button
              key={entry.key}
              type="button"
              role="tab"
              aria-selected={active}
              data-help-tab={entry.key}
              onClick={() => setTab(entry.key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(entry.labelKey, [], entry.fallback)}
            </button>
          );
        })}
      </div>

      <div style={style.body} role="tabpanel">
        {entries.map((entry) => (
          <article key={entry.titleKey} style={style.entry}>
            <h3 style={style.entryTitle}>{t(entry.titleKey, [], entry.titleFallback)}</h3>
            <p style={style.entryBody}>{t(entry.bodyKey, [], entry.bodyFallback)}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 132,
    top: 134,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 33,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  titleText: {
    position: "absolute",
    left: 24,
    top: 12,
    fontSize: 14,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  subtitle: { position: "absolute", left: 24, top: 32, fontSize: 11, color: "#cbb38a" },
  close: { position: "absolute", left: 724, top: 8 },
  tabs: { position: "absolute", left: 24, top: 56, display: "flex", gap: 4 },
  tab: {
    minWidth: 96,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "4px 8px",
    fontSize: 12,
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  body: {
    position: "absolute",
    left: 24,
    top: 88,
    width: 712,
    height: 392,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: "10px 14px",
    display: "flex",
    flexDirection: "column",
    gap: 12,
  },
  entry: {
    borderBottom: "1px solid rgba(190, 157, 99, 0.18)",
    paddingBottom: 10,
  },
  entryTitle: { margin: "0 0 4px", fontSize: 13, fontWeight: 700, color: "#f4dcaf" },
  entryBody: { margin: 0, fontSize: 12, color: "#d6c6a5", lineHeight: 1.45 },
};
