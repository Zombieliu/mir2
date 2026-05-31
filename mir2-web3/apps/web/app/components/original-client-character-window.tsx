"use client";

import { useState } from "react";

import { ORIGINAL_UI, type CharacterTabKey } from "../../lib/original-ui";
import { sameItemDragSource, useItemDrag } from "./original-client-drag-item";
import type { ViewportEntitySprite } from "./original-client-scene-rendering";
import { OriginalItemTooltip } from "./original-client-item-tooltip";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EquipmentSlot =
  | "weapon"
  | "armour"
  | "helmet"
  | "mount"
  | "necklace"
  | "torch"
  | "braceletLeft"
  | "braceletRight"
  | "ringLeft"
  | "ringRight"
  | "amulet"
  | "boots"
  | "belt"
  | "stone";

type DisplayEntity = { name: string };

type DisplayEquipmentItem = {
  slot: EquipmentSlot;
  name: string;
  icon: number;
  description: string;
  durabilityCurrent: number;
  durabilityMax: number;
  attack: number;
  defence: number;
};

type DisplayKnownSkill = {
  key: string;
  name: string;
  description: string;
  spell?: string | null;
  castKind?: "passive" | "toggle" | "self" | "target" | "ground" | "direction";
  offensive?: boolean;
  hotkey?: number;
  delayMs?: number;
  castTimeMs?: number;
  cooldownRemainingTicks: number;
};

type DisplayWorld = {
  equipmentItems: DisplayEquipmentItem[];
  playerHp?: number;
  playerMaxHp?: number;
  playerMp?: number;
  playerExperience: number;
  playerMaxExperience: number;
  freeBagSlots: number;
  maxBagSlots: number;
  currentWeight: number;
  maxWeight: number;
  knownSkills: DisplayKnownSkill[];
};

type EquipmentActionRef = Pick<DisplayEquipmentItem, "slot">;

type CharacterWindowProps = {
  t: TranslateFn;
  activeTab: CharacterTabKey;
  onClose: () => void;
  onTabChange: (tab: CharacterTabKey) => void;
  player: DisplayEntity | null;
  paperdollSprite?: ViewportEntitySprite | null;
  world: DisplayWorld;
  onRemoveItem: (item: EquipmentActionRef) => void;
  onRepairItem: (item: EquipmentActionRef) => void;
  onSpecialRepairItem: (item: EquipmentActionRef) => void;
  onCastSkill: (skillKey: string) => void;
};

export function CharacterWindow({
  t,
  activeTab,
  onClose,
  onTabChange,
  player,
  paperdollSprite,
  world,
  onRemoveItem,
  onRepairItem,
  onSpecialRepairItem,
  onCastSkill,
}: CharacterWindowProps) {
  const itemDrag = useItemDrag();
  const activePage = ORIGINAL_UI.character.pages[activeTab];
  const equipmentBySlot = new Map(world.equipmentItems.map((item) => [item.slot, item]));
  const totalAttack = world.equipmentItems.reduce((sum, item) => sum + item.attack, 0);
  const totalDefence = world.equipmentItems.reduce((sum, item) => sum + item.defence, 0);
  const [repairMode, setRepairMode] = useState<"normal" | "special" | null>(null);
  const repairModeLabel =
    repairMode === "normal"
      ? t("ui.repairItem", [], "Repair Item")
      : repairMode === "special"
        ? t("ui.specialRepairItem", [], "Special Repair")
        : "";
  const stats1Values = [
    displayFieldValue(world.playerHp, world.playerMaxHp),
    displayFieldValue(world.playerMp, 100),
    statNumber(totalDefence),
    "",
    statNumber(totalAttack),
    "",
    "",
    "",
    "",
    "",
    "",
  ];
  const stats2Values = [
    `${(ratio(world.playerExperience, world.playerMaxExperience) * 100).toFixed(2)}%`,
    statPair(world.freeBagSlots, world.maxBagSlots),
    statPair(world.currentWeight, world.maxWeight),
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
  ];
  const spellValues = [
    ...world.knownSkills.slice(0, 7).map((skill) => {
      const cooldownSuffix =
        skill.cooldownRemainingTicks > 0 ? ` (${skill.cooldownRemainingTicks})` : "";
      return `${skill.name}${cooldownSuffix}`;
    }),
  ];
  while (spellValues.length < 7) {
    spellValues.push("");
  }

  return (
    <div className="window-shell character-window">
      <img className="window-frame" src={ORIGINAL_UI.character.frame} alt="" draggable={false} />
      <img className="character-page" src={activePage} alt="" draggable={false} />
      <div className="character-close">
        <SpriteButton sprite={ORIGINAL_UI.character.closeButton} label={t("ui.closeCharacter")} onClick={onClose} />
      </div>

      <div className="character-tab char"><button type="button" className="window-tab-button" onClick={() => onTabChange("char")}><img src={ORIGINAL_UI.character.tabs.char} alt={t("ui.character")} draggable={false} /></button></div>
      <div className="character-tab stats1"><button type="button" className="window-tab-button" onClick={() => onTabChange("stats1")}><img src={ORIGINAL_UI.character.tabs.stats1} alt={t("ui.statsPrimary")} draggable={false} /></button></div>
      <div className="character-tab stats2"><button type="button" className="window-tab-button" onClick={() => onTabChange("stats2")}><img src={ORIGINAL_UI.character.tabs.stats2} alt={t("ui.statsSecondary")} draggable={false} /></button></div>
      <div className="character-tab spells"><button type="button" className="window-tab-button" onClick={() => onTabChange("spells")}><img src={ORIGINAL_UI.character.tabs.spells} alt={t("ui.spells")} draggable={false} /></button></div>

      <div className="character-name">{player?.name ?? ""}</div>
      <div className="character-guild" />

      {activeTab === "char" ? (
        <>
          {paperdollSprite ? <CharacterPaperdoll sprite={paperdollSprite} /> : null}
          {ORIGINAL_UI.character.equipmentSlots.map((slot) => {
            const item = equipmentBySlot.get(equipmentSlotFromLabel(slot.label));

            return (
              <div
                key={slot.label}
                className="character-slot" data-item-drop-kind="equipment" data-item-drop-equip={equipmentSlotFromLabel(slot.label)}
                style={{ left: slot.x + 8, top: slot.y + 90 }}
                aria-label={slot.label}
              >
	                {item ? (
	                  <button
	                    type="button"
	                    className={`character-slot-card ${itemDrag && sameItemDragSource(itemDrag.draggingSource, { kind: "equipment", equipmentSlot: item.slot }) ? "item-dragging" : ""}`}
	                    aria-label={item.name}
                    onPointerDown={(event) =>
                      itemDrag?.beginDrag(event, {
                        uniqueId: -1,
                        key: `equip:${item.slot}`,
                        name: item.name,
                        icon: item.icon,
                        quantity: 1,
                        source: { kind: "equipment", equipmentSlot: item.slot },
                      })
                    }
	                    onClick={() => {
	                      if (repairMode === "normal") {
	                        onRepairItem({ slot: item.slot });
	                        setRepairMode(null);
	                        return;
	                      }
	                      if (repairMode === "special") {
	                        onSpecialRepairItem({ slot: item.slot });
	                        setRepairMode(null);
	                        return;
	                      }
	                      onRemoveItem({ slot: item.slot });
	                    }}
	                  >
                    <img
                      className="original-item-icon character-item-icon"
                      src={originalItemIconPath(item.icon)}
                      alt=""
                      draggable={false}
                    />
                    <OriginalItemTooltip
                      t={t}
                      name={item.name}
                      description={item.description}
                      durabilityCurrent={item.durabilityCurrent}
                      durabilityMax={item.durabilityMax}
                      attack={item.attack}
                      defence={item.defence}
                      align={slot.x > 110 ? "left" : "right"}
                    />
                  </button>
                ) : null}
              </div>
            );
          })}
	        </>
	      ) : null}

	      {activeTab === "char" ? (
	        <>
	          {repairModeLabel ? <div className="inventory-delete-hint">{repairModeLabel}</div> : null}
	          <div className="character-repair-actions">
	            <button
	              type="button"
	              className={repairMode === "normal" ? "active" : ""}
	              onClick={() => setRepairMode((current) => (current === "normal" ? null : "normal"))}
	            >
	              {t("ui.repairItem", [], "Repair Item")}
	            </button>
	            <button
	              type="button"
	              className={repairMode === "special" ? "active" : ""}
	              onClick={() => setRepairMode((current) => (current === "special" ? null : "special"))}
	            >
	              {t("ui.specialRepairItem", [], "Special Repair")}
	            </button>
	          </div>
	        </>
	      ) : null}

      {activeTab === "stats1" ? (
        <div className="character-field-values stats1">
          {stats1Values.map((value, index) => (
            <div key={`stats1-${index}`} className="character-field-value">
              {value}
            </div>
          ))}
        </div>
      ) : null}

      {activeTab === "stats2" ? (
        <div className="character-field-values stats2">
          {stats2Values.map((value, index) => (
            <div key={`stats2-${index}`} className="character-field-value">
              {value}
            </div>
          ))}
        </div>
      ) : null}

      {activeTab === "spells" ? (
        <div className="character-spell-values">
          {spellValues.map((value, index) => {
            const skill = world.knownSkills[index] ?? null;
            if (skill) {
              return (
                <button
                  key={`spells-${skill.key}`}
                  type="button"
                  className="character-spell-value"
                  title={skill.description}
                  onClick={() => onCastSkill(skill.key)}
                >
                  {value}
                </button>
              );
            }

            return (
              <div key={`spells-${index}`} className="character-spell-value">
                {value}
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

const PAPERDOLL_BOX_WIDTH = 92;
const PAPERDOLL_BOX_HEIGHT = 116;

// Renders the standing self sprite (body + hair + weapon layers from the scene
// sprite pipeline) centered/scaled to fit the character-dialog paperdoll box.
function CharacterPaperdoll({ sprite }: { sprite: ViewportEntitySprite }) {
  const layers = [...sprite.rearWeapons, sprite.body, sprite.hair, ...sprite.frontWeapons].filter(
    (layer): layer is NonNullable<typeof layer> => Boolean(layer),
  );
  if (!layers.length) {
    return null;
  }

  const minX = Math.min(...layers.map((layer) => layer.x));
  const minY = Math.min(...layers.map((layer) => layer.y));
  const maxX = Math.max(...layers.map((layer) => layer.x + layer.width));
  const maxY = Math.max(...layers.map((layer) => layer.y + layer.height));
  const figureWidth = Math.max(1, maxX - minX);
  const figureHeight = Math.max(1, maxY - minY);
  const scale = Math.min(1, PAPERDOLL_BOX_WIDTH / figureWidth, PAPERDOLL_BOX_HEIGHT / figureHeight);
  const offsetX = (PAPERDOLL_BOX_WIDTH - figureWidth * scale) / 2 - minX * scale;
  const offsetY = (PAPERDOLL_BOX_HEIGHT - figureHeight * scale) / 2 - minY * scale;

  return (
    <div className="character-paperdoll" aria-hidden>
      {layers.map((layer, index) => (
        <img
          key={`paperdoll-${index}-${layer.path}`}
          className="character-paperdoll-layer"
          src={layer.path}
          alt=""
          draggable={false}
          style={{
            left: layer.x * scale + offsetX,
            top: layer.y * scale + offsetY,
            width: layer.width * scale,
            height: layer.height * scale,
          }}
        />
      ))}
    </div>
  );
}

function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

function ratio(value?: number, max?: number) {
  if (value === undefined || max === undefined || max <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(1, value / max));
}

function statNumber(value: number) {
  return value > 0 ? String(value) : "";
}

function statPair(current?: number, max?: number) {
  if (current === undefined || max === undefined) {
    return "";
  }

  return `${current}/${max}`;
}

function displayFieldValue(current?: number, max?: number) {
  if (current === undefined || max === undefined) {
    return "";
  }

  return `${current}/${max}`;
}

function equipmentSlotFromLabel(label: string): EquipmentSlot {
  switch (label) {
    case "Weapon":
      return "weapon";
    case "Armour":
      return "armour";
    case "Helmet":
      return "helmet";
    case "Mount":
      return "mount";
    case "Necklace":
      return "necklace";
    case "Torch":
      return "torch";
    case "BraceletL":
      return "braceletLeft";
    case "BraceletR":
      return "braceletRight";
    case "RingL":
      return "ringLeft";
    case "RingR":
      return "ringRight";
    case "Amulet":
      return "amulet";
    case "Boots":
      return "boots";
    case "Belt":
      return "belt";
    default:
      return "stone";
  }
}
