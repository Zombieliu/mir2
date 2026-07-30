"use client";

import { useEffect, useState, type CSSProperties, type FormEvent } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { DEFAULT_HOTKEY_GROUPS } from "./original-client-hotkey-window";
import type { Mir2InputProfile } from "./original-client-device-profile";
import { SpriteButton } from "./original-client-overlays";
import { SocialSystemPanel, type SystemMenuSocialPanel } from "./original-client-social-system-panel";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EquipmentItemLike = {
  slot: string;
  name: string;
  durabilityCurrent: number;
  durabilityMax: number;
};

type DisplayWorld = {
  equipmentItems: EquipmentItemLike[];
  mapTitle: string | null;
  inSafeZone: boolean;
  gold: number;
  entities: Array<{ kind: string; level?: number }>;
  activeBuffs: Array<{ name: string; remainingTicks: number }>;
  rankings?: Record<
    string,
    {
      rankType: number;
      rankIndex: number;
      onlineOnly: boolean;
      myRank: number;
      count: number;
      entries: Array<{
        rank: number;
        playerId: number;
        name: string;
        level: number;
        classKey: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
      }>;
      updatedAt: number;
    }
  >;
  rankingCurrentKey?: string | null;
  stage5Systems?: {
    group?: { members?: string[]; lootMode?: string };
    guild?: { name?: string; members?: string[]; rank?: string; permissions?: string[]; chatLog?: string[] };
    social?: { friends?: string[]; blocked?: string[] };
    relationship?: Record<string, unknown>;
    mentor?: Record<string, unknown>;
    trade?: Record<string, unknown> | null;
    auction?: Array<Record<string, unknown>>;
    hero?: Record<string, unknown> | null;
    itemRental?: Record<string, unknown>;
    intelligentCreatures?: Array<Record<string, unknown>>;
  };
};

export type SystemMenuTransferOption = {
  key: string;
  label: string;
};

type SystemMenuButtonDefinition = {
  key: keyof typeof ORIGINAL_UI.menu.buttons;
  label: string;
  panel?: SystemMenuSurfacePanel;
  onClick?: () => void;
};

export type SystemMenuFeaturePanelKey = "creature" | "mount" | "fishing";
export type SystemMenuSurfacePanel = SystemMenuFeaturePanelKey | SystemMenuSocialPanel;

export function SystemMenuPanel({
  t,
  playerName,
  playerPosition,
  mapTitle,
  mapFileName,
  inSafeZone,
  transferOptions,
  inputProfile,
  onStartTutorial,
  onOpenPanel,
  onClose,
  onLogout,
  onTransferMap,
}: {
  t: TranslateFn;
  playerName: string | null;
  playerPosition: { x: number; y: number } | null;
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  transferOptions: SystemMenuTransferOption[];
  inputProfile: Mir2InputProfile;
  onStartTutorial: () => void;
  onOpenPanel: (panel: SystemMenuSurfacePanel) => void;
  onClose: () => void;
  onLogout: () => void;
  onTransferMap: (transferKey: string) => void;
}) {
  const [qaMap, setQaMap] = useState(() => normalizeMapInput(mapFileName ?? "0"));
  const [qaX, setQaX] = useState(() => String(playerPosition?.x ?? 330));
  const [qaY, setQaY] = useState(() => String(playerPosition?.y ?? 270));
  // Local reference overlay. The original options screen exposes a key list +
  // quick help next to the menu; this version also links back to the active
  // input profile's interactive tutorial.
  const [infoOverlay, setInfoOverlay] = useState<"help" | "keyboard" | null>(null);
  const noop = () => undefined;
  const toggleInfoOverlay = (next: "help" | "keyboard") =>
    setInfoOverlay((current) => (current === next ? null : next));
  const menuButtons: SystemMenuButtonDefinition[] = [
    { key: "exit", label: t("ui.exit"), onClick: onLogout },
    { key: "logout", label: t("ui.logout", [], "Log Out"), onClick: onLogout },
    { key: "help", label: t("ui.help", [], "Help"), onClick: () => toggleInfoOverlay("help") },
    { key: "keyboard", label: t("ui.keyboard", [], "Keyboard"), onClick: () => toggleInfoOverlay("keyboard") },
    { key: "ranking", label: t("ui.ranking", [], "Ranking"), panel: "ranking" as const },
    { key: "creature", label: t("ui.creature", [], "Creature"), panel: "creature" as const },
    { key: "ride", label: t("ui.mount", [], "Mount"), panel: "mount" as const },
    { key: "fishing", label: t("ui.fishing", [], "Fishing"), panel: "fishing" as const },
    { key: "friend", label: t("ui.friends", [], "Friends"), panel: "friend" as const },
    { key: "mentor", label: t("ui.mentor", [], "Mentor"), panel: "mentor" as const },
    { key: "relationship", label: t("ui.relationship", [], "Relationship"), panel: "relationship" as const },
    { key: "group", label: t("ui.group", [], "Group"), panel: "group" as const },
    { key: "guild", label: t("ui.guild", [], "Guild"), panel: "guild" as const },
  ];
  const lateSystemButtons: Array<{ panel: SystemMenuSocialPanel; label: string }> = [
    { panel: "hero", label: t("ui.hero", [], "Hero") },
    { panel: "trade", label: t("ui.trade", [], "Trade") },
    { panel: "market", label: t("ui.market", [], "Market") },
    { panel: "marriage", label: t("ui.marriage", [], "Marriage") },
    { panel: "itemRental", label: t("ui.itemRental", [], "Item Rental") },
  ];

  function submitQaTransfer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const map = normalizeMapInput(qaMap);
    const x = parseFiniteInteger(qaX);
    const y = parseFiniteInteger(qaY);
    if (!map || x === null || y === null) {
      return;
    }

    onTransferMap(`crystal:${map}:${x}:${y}`);
  }

  return (
    <>
      <section className="system-menu-panel" aria-label={t("ui.menu")}>
        <img className="system-menu-frame" src={ORIGINAL_UI.menu.frame} alt="" draggable={false} />
        <section className="system-menu-actions" aria-label={t("ui.menu") + " actions"}>
          {menuButtons.map((button) => {
            const definition = ORIGINAL_UI.menu.buttons[button.key];
            const panel = button.panel;
            const handleClick = panel ? () => onOpenPanel(panel) : button.onClick ?? noop;

            return (
              <div
                key={button.key}
                className="system-menu-icon"
                data-system-menu-action={button.key}
                style={{ left: `${definition.x}px`, top: `${definition.y}px` }}
              >
                <SpriteButton sprite={definition.sprite} label={button.label} onClick={handleClick} />
              </div>
            );
          })}
        </section>
        <button type="button" className="system-menu-close-hit" onClick={onClose} aria-label={t("ui.close")} />
      </section>
      <section className="system-menu-qa-panel" aria-label={t("ui.transfer", [], "Transfer controls")}>
        <div className="system-menu-meta">
          <span>{playerName ?? "-"}</span>
          <span>{mapTitle ?? t("ui.map")}{mapFileName ? ` [${mapFileName}]` : ""}</span>
          <span>{inSafeZone ? t("ui.safeZone", [], "Safe Zone") : t("ui.combatZone", [], "Combat Zone")}</span>
        </div>
        <div className="system-menu-transfer-list">
          <div className="system-menu-transfer-title">{t("ui.transfer", [], "Transfer")}</div>
          {transferOptions.map((option) => (
            <button key={option.key} type="button" onClick={() => onTransferMap(option.key)}>
              {option.label}
            </button>
          ))}
        </div>
        <form className="system-menu-qa-transfer" onSubmit={submitQaTransfer}>
          <div className="system-menu-transfer-title">{t("ui.quickJump", [], "Quick Jump")}</div>
          <label>
            <span>{t("ui.map")}</span>
            <input
              value={qaMap}
              onChange={(event) => setQaMap(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          <div className="system-menu-qa-transfer-coords">
            <label>
              <span>X</span>
              <input
                type="number"
                value={qaX}
                onChange={(event) => setQaX(event.target.value)}
                inputMode="numeric"
              />
            </label>
            <label>
              <span>Y</span>
              <input
                type="number"
                value={qaY}
                onChange={(event) => setQaY(event.target.value)}
                inputMode="numeric"
              />
            </label>
            <button type="submit">{t("ui.go", [], "Go")}</button>
          </div>
        </form>
        <div className="system-menu-late-actions" aria-label="Late systems">
          <div className="system-menu-transfer-title">{t("ui.lateSystems", [], "Systems")}</div>
          {lateSystemButtons.map((button) => (
            <button
              key={button.panel}
              type="button"
              data-system-menu-action={button.panel}
              onClick={() => onOpenPanel(button.panel)}
            >
              {button.label}
            </button>
          ))}
        </div>
      </section>
      {infoOverlay ? (
        <SystemMenuInfoOverlay
          t={t}
          mode={infoOverlay}
          inputProfile={inputProfile}
          onStartTutorial={onStartTutorial}
          onClose={() => setInfoOverlay(null)}
        />
      ) : null}
    </>
  );
}

function SystemMenuInfoOverlay({
  t,
  mode,
  inputProfile,
  onStartTutorial,
  onClose,
}: {
  t: TranslateFn;
  mode: "help" | "keyboard";
  inputProfile: Mir2InputProfile;
  onStartTutorial: () => void;
  onClose: () => void;
}) {
  const title = mode === "keyboard" ? t("ui.keyboard", [], "Keyboard") : t("ui.help", [], "Help");
  return (
    <section
      className="system-menu-info-overlay"
      data-system-menu-info={mode}
      aria-label={title}
      style={infoStyle.panel}
    >
      <div style={infoStyle.head}>
        <span style={infoStyle.title}>{title}</span>
        <button type="button" aria-label={t("ui.close", [], "Close")} onClick={onClose} style={infoStyle.close}>
          ×
        </button>
      </div>
      <div style={infoStyle.body}>
        {mode === "keyboard" ? (
          DEFAULT_HOTKEY_GROUPS.map((group) => (
            <div key={group.titleKey} style={infoStyle.group}>
              <div style={infoStyle.groupTitle}>{t(group.titleKey, [], group.titleFallback)}</div>
              {group.bindings.map((binding) => (
                <div key={binding.id} style={infoStyle.row}>
                  <span style={infoStyle.rowLabel}>{t(binding.labelKey, [], binding.labelFallback)}</span>
                  <kbd style={infoStyle.keycap}>{binding.keys}</kbd>
                </div>
              ))}
            </div>
          ))
        ) : (
          <div style={infoStyle.group}>
            <div style={infoStyle.groupTitle}>{t("ui.helpSubtitle", [], "Legend of Mir 2 quick reference")}</div>
            {inputProfile === "touch" ? (
              <>
                <p style={infoStyle.helpLine}>{t("ui.helpTouchMoveBody", [], "Drag the left joystick to move. Tap Run to switch between running and walking.")}</p>
                <p style={infoStyle.helpLine}>{t("ui.helpTouchInteractBody", [], "Use Attack, Approach and Pick for targets and nearby loot. S and number buttons are skills and belt items.")}</p>
                <p style={infoStyle.helpLine}>{t("ui.helpTouchWindowsBody", [], "Char opens equipment and stats. Bag opens inventory, quests and storage.")}</p>
              </>
            ) : inputProfile === "gamepad" ? (
              <>
                <p style={infoStyle.helpLine}>{t("ui.helpGamepadMoveBody", [], "Move with the left stick or D-pad. A is primary action, Y approaches and X picks up loot.")}</p>
                <p style={infoStyle.helpLine}>{t("ui.helpGamepadQuickBody", [], "LT/RT cast skills, LB/RB use belt items, View opens Character and Menu opens Bag.")}</p>
                <p style={infoStyle.helpLine}>{t("ui.helpGamepadUiBody", [], "In menus, use the D-pad to move focus, A to activate and B to go back.")}</p>
              </>
            ) : (
              <>
                <p style={infoStyle.helpLine}>{t("ui.helpMoveBody", [], "Left-click the ground to walk. Hold the button to keep moving.")}</p>
                <p style={infoStyle.helpLine}>{t("ui.helpInteractBody", [], "Left-click an NPC to talk, a monster to attack, or a dropped item to pick it up.")}</p>
                <p style={infoStyle.helpLine}>{t("ui.helpWindowsBody", [], "Use the toolbar or hotkeys to open Character, Inventory, Skills and this menu.")}</p>
              </>
            )}
            <button
              type="button"
              data-system-menu-action="tutorial"
              onClick={onStartTutorial}
              style={infoStyle.tutorialButton}
            >
              {t("ui.replayControlsTutorial", [], "Replay controls tutorial")}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}

export function SystemMenuFeaturePanel({
  t,
  feature,
  playerName,
  world,
  onRunStage5Command,
  onSendClientCommand,
  onClose,
}: {
  t: TranslateFn;
  feature: SystemMenuSurfacePanel;
  playerName: string | null;
  world: DisplayWorld;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  onClose: () => void;
}) {
  const featureTitle =
    feature === "creature"
      ? t("ui.creature", [], "Creature")
      : feature === "mount"
        ? t("ui.mount", [], "Mount")
        : feature === "fishing"
          ? t("ui.fishing", [], "Fishing")
          : feature === "ranking"
            ? t("ui.ranking", [], "Ranking")
            : feature === "friend"
              ? t("ui.friends", [], "Friends")
              : feature === "mentor"
                ? t("ui.mentor", [], "Mentor")
                : feature === "relationship"
                  ? t("ui.relationship", [], "Relationship")
                  : feature === "group"
                    ? t("ui.group", [], "Group")
                    : feature === "guild"
                      ? t("ui.guild", [], "Guild")
                      : feature === "trade"
                        ? t("ui.trade", [], "Trade")
                        : feature === "market"
                          ? t("ui.market", [], "Market")
                          : t("ui.marriage", [], "Marriage");
  const isSocialPanel = feature !== "creature" && feature !== "mount" && feature !== "fishing";

  return (
    <section
      className={`system-feature-panel system-feature-panel-${feature} ${isSocialPanel ? "system-feature-panel-social" : ""}`}
      aria-label={featureTitle}
      data-system-feature-panel={feature}
    >
      <button type="button" className="system-feature-close" onClick={onClose} aria-label={t("ui.close")} />
      {feature === "creature" ? (
        <CreatureSystemPanel t={t} world={world} onSendClientCommand={onSendClientCommand} />
      ) : feature === "mount" ? (
        <MountSystemPanel t={t} world={world} onSendClientCommand={onSendClientCommand} />
      ) : feature === "fishing" ? (
        <FishingSystemPanel t={t} onSendClientCommand={onSendClientCommand} />
      ) : (
        <SocialSystemPanel
          t={t}
          panel={feature}
          playerName={playerName}
          world={world}
          onRunStage5Command={onRunStage5Command}
          onSendClientCommand={onSendClientCommand}
        />
      )}
    </section>
  );
}

function defaultIntelligentCreaturePayload(source: Record<string, unknown> | null, summoned: boolean) {
  const rules = recordObjectValue(source, "creatureRules") ?? {};
  const filter = recordObjectValue(source, "filter") ?? {};
  return {
    petType: numberRecordValue(source, ["petType"], 1),
    icon: numberRecordValue(source, ["icon"], 44),
    customName: stringRecordValue(source, ["customName", "name"]) ?? "Buddy",
    fullness: numberRecordValue(source, ["fullness"], 50),
    slotIndex: numberRecordValue(source, ["slotIndex"], 0),
    expireBinaryDatetime: numberRecordValue(source, ["expireBinaryDatetime"], 638000000000000000),
    blackstoneTime: numberRecordValue(source, ["blackstoneTime"], 12000),
    petMode: summoned ? 1 : 0,
    creatureRules: {
      minimalFullness: numberRecordValue(rules, ["minimalFullness"], 1),
      mousePickupEnabled: boolRecordValue(rules, ["mousePickupEnabled"], true),
      mousePickupRange: numberRecordValue(rules, ["mousePickupRange"], 6),
      autoPickupEnabled: boolRecordValue(rules, ["autoPickupEnabled"], false),
      autoPickupRange: numberRecordValue(rules, ["autoPickupRange"], 0),
      semiAutoPickupEnabled: boolRecordValue(rules, ["semiAutoPickupEnabled"], true),
      semiAutoPickupRange: numberRecordValue(rules, ["semiAutoPickupRange"], 4),
      canProduceBlackstone: boolRecordValue(rules, ["canProduceBlackstone"], true),
    },
    filter: {
      petPickupAll: boolRecordValue(filter, ["petPickupAll"], false),
      petPickupGold: boolRecordValue(filter, ["petPickupGold"], true),
      petPickupWeapons: boolRecordValue(filter, ["petPickupWeapons"], false),
      petPickupArmours: boolRecordValue(filter, ["petPickupArmours"], false),
      petPickupHelmets: boolRecordValue(filter, ["petPickupHelmets"], false),
      petPickupBoots: boolRecordValue(filter, ["petPickupBoots"], false),
      petPickupBelts: boolRecordValue(filter, ["petPickupBelts"], false),
      petPickupAccessories: boolRecordValue(filter, ["petPickupAccessories"], false),
      petPickupOthers: boolRecordValue(filter, ["petPickupOthers"], true),
    },
    pickupGrade: numberRecordValue(source, ["pickupGrade"], 2),
    maintainFoodTime: numberRecordValue(source, ["maintainFoodTime"], 24000),
  };
}

function CreatureSystemPanel({
  t,
  world,
  onSendClientCommand,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const creature = world.stage5Systems?.intelligentCreatures?.[0] ?? null;
  const fullness = clampNumber(numberRecordValue(creature, ["fullness"], 0), 0, 100);
  const blackstone = clampNumber(Math.round(numberRecordValue(creature, ["blackstoneTime"], 0) / 240), 0, 100);
  const creatureName = stringRecordValue(creature, ["customName", "name"]) ?? "-";
  const updateCreature = (summoned: boolean, releaseMe = false) => {
    onSendClientCommand({
      type: "updateIntelligentCreature",
      creature: defaultIntelligentCreaturePayload(creature, summoned),
      summonMe: summoned && !releaseMe,
      unsummonMe: !summoned && !releaseMe,
      releaseMe,
    });
  };
  return (
    <>
      <div className="system-feature-title">{t("ui.creature", [], "Creature")}</div>
      <div className="creature-feature-name">{creatureName}</div>
      <div className="creature-feature-gauge creature-feature-fullness">
        <span className="creature-feature-fill" style={{ width: `${fullness}%` }} />
      </div>
      <div className="creature-feature-gauge creature-feature-minimum">
        <span className="creature-feature-marker" style={{ left: "10%" }} />
      </div>
      <div className="creature-feature-blackstone">
        <span className="creature-feature-fill" style={{ width: `${blackstone}%` }} />
      </div>
      <div className="creature-feature-actions">
        <button type="button" onClick={() => updateCreature(true)}>{t("ui.summon", [], "Summon")}</button>
        <button type="button" onClick={() => updateCreature(false)}>{t("ui.dismiss", [], "Dismiss")}</button>
        <button type="button" onClick={() => updateCreature(false, true)}>{t("ui.release", [], "Release")}</button>
      </div>
      <div className="creature-feature-slots">
        {Array.from({ length: 10 }, (_, index) => (
          <span key={`creature-slot-${index}`} />
        ))}
      </div>
    </>
  );
}

function MountSystemPanel({
  t,
  world,
  onSendClientCommand,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const slots = ["Reins", "Bells", "Saddle", "Ribbon", "Mask"];
  const mountItem = world.equipmentItems.find((item) => item.slot === "mount");
  return (
    <>
      <div className="system-feature-title">{t("ui.mount", [], "Mount")}</div>
      <div className="mount-feature-name">{mountItem?.name ?? "-"}</div>
      <div className="mount-feature-loyalty">
        {mountItem ? `${mountItem.durabilityCurrent} / ${mountItem.durabilityMax}` : "0 / 0"}
      </div>
      <div className="mount-feature-preview" />
      <button
        type="button"
        className="mount-feature-ride"
        onClick={() => onSendClientCommand({ type: "useItem", slot: 13, grid: "equipment" })}
      >
        {t("ui.mount", [], "Mount")}
      </button>
      <div className="mount-feature-slots">
        {slots.map((slot) => (
          <span key={slot} aria-label={slot} />
        ))}
      </div>
    </>
  );
}

function FishingSystemPanel({
  t,
  onSendClientCommand,
}: {
  t: TranslateFn;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const slots = ["Hook", "Float", "Bait", "Finder", "Reel"];
  return (
    <>
      <div className="system-feature-title">{t("ui.fishing", [], "Fishing")}</div>
      <div className="fishing-feature-water" />
      <div className="fishing-feature-slots">
        {slots.map((slot) => (
          <span key={slot} aria-label={slot} />
        ))}
      </div>
      <div className="fishing-feature-status">
        <button type="button" onClick={() => onSendClientCommand({ type: "fishingCast", castOut: true })}>
          {t("ui.cast", [], "Cast")}
        </button>
        <button type="button" onClick={() => onSendClientCommand({ type: "fishingChangeAutocast", autoCast: true })}>
          {t("ui.auto", [], "Auto")}
        </button>
        <label>
          <input
            type="checkbox"
            readOnly
            onClick={() => onSendClientCommand({ type: "fishingChangeAutocast", autoCast: true })}
          />
        </label>
      </div>
    </>
  );
}

const infoStyle: Record<string, CSSProperties> = {
  panel: {
    position: "absolute",
    right: 56,
    top: 60,
    width: 280,
    maxHeight: 420,
    zIndex: 40,
    display: "flex",
    flexDirection: "column",
    border: "1px solid rgba(190, 157, 99, 0.6)",
    background:
      "linear-gradient(180deg, rgba(40, 28, 15, 0.97), rgba(12, 8, 5, 0.97))",
    boxShadow: "0 16px 36px rgba(0, 0, 0, 0.5)",
    color: "#f0eee8",
    font: "11px Georgia, 'Times New Roman', serif",
    textShadow: "1px 1px 0 #000",
  },
  head: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "5px 8px",
    borderBottom: "1px solid rgba(190, 157, 99, 0.36)",
  },
  title: { fontSize: 12, fontWeight: 700, color: "#f4dcaf", letterSpacing: 0.5 },
  close: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(30, 20, 12, 0.86)",
    color: "#e3d3af",
    width: 18,
    height: 18,
    lineHeight: "14px",
    fontSize: 13,
    cursor: "pointer",
  },
  body: { overflowY: "auto", padding: "6px 8px", display: "flex", flexDirection: "column", gap: 8 },
  group: { display: "flex", flexDirection: "column", gap: 3 },
  groupTitle: {
    fontSize: 10,
    fontWeight: 700,
    color: "#f4dcaf",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    borderBottom: "1px solid rgba(190, 157, 99, 0.24)",
    paddingBottom: 3,
    marginBottom: 1,
  },
  row: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 },
  rowLabel: { color: "#d6c6a5", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  keycap: {
    flex: "0 0 auto",
    fontFamily: "inherit",
    fontSize: 10,
    color: "#f8e6bb",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.95), rgba(28, 17, 9, 0.95))",
    border: "1px solid rgba(214, 180, 110, 0.6)",
    borderRadius: 3,
    padding: "0 6px",
    minWidth: 44,
    textAlign: "center",
  },
  helpLine: { margin: 0, color: "#d6c6a5", lineHeight: 1.4 },
  tutorialButton: {
    marginTop: 8,
    minHeight: 36,
    padding: "7px 10px",
    border: "1px solid rgba(214, 180, 110, 0.72)",
    borderRadius: 3,
    background: "linear-gradient(180deg, #755226, #3d2914)",
    color: "#ffe3a8",
    font: "700 12px Georgia, 'Times New Roman', serif",
    cursor: "pointer",
    textShadow: "1px 1px 0 #000",
  },
};

function normalizeMapInput(value: string) {
  return value.trim().replace(/\.map$/i, "") || "0";
}

function parseFiniteInteger(value: string) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function stringRecordValue(record: Record<string, unknown> | null | undefined, keys: string[]) {
  if (!record) return null;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      return String(value);
    }
  }
  return null;
}

function numberRecordValue(record: Record<string, unknown> | null | undefined, keys: string[], fallback = 0) {
  const value = stringRecordValue(record, keys);
  if (value === null) return fallback;
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function boolRecordValue(record: Record<string, unknown> | null | undefined, keys: string[], fallback = false) {
  if (!record) return fallback;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "boolean") return value;
  }
  return fallback;
}

function recordObjectValue(record: Record<string, unknown> | null | undefined, key: string) {
  const value = record?.[key];
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function clampNumber(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
