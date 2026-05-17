"use client";

import { useEffect, useState, type FormEvent } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
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
  onOpenPanel: (panel: SystemMenuSurfacePanel) => void;
  onClose: () => void;
  onLogout: () => void;
  onTransferMap: (transferKey: string) => void;
}) {
  const [qaMap, setQaMap] = useState(() => normalizeMapInput(mapFileName ?? "0"));
  const [qaX, setQaX] = useState(() => String(playerPosition?.x ?? 330));
  const [qaY, setQaY] = useState(() => String(playerPosition?.y ?? 270));
  const noop = () => undefined;
  const menuButtons: SystemMenuButtonDefinition[] = [
    { key: "exit", label: t("ui.exit"), onClick: onLogout },
    { key: "logout", label: t("ui.logout", [], "Log Out"), onClick: onLogout },
    { key: "help", label: t("ui.help", [], "Help"), onClick: noop },
    { key: "keyboard", label: t("ui.keyboard", [], "Keyboard"), onClick: noop },
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
    </>
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
