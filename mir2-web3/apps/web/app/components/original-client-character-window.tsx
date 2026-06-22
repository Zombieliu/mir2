"use client";

import { type CSSProperties } from "react";

import { ORIGINAL_UI, type CharacterTabKey } from "../../lib/original-ui";
import { OriginalItemTooltip, type ItemTooltipGrade } from "./original-client-item-tooltip";
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

type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

/**
 * The window only needs the player's identity + class for the header / paperdoll
 * badge. These fields are all optional so any compatible payload (including the
 * minimal `{ name }` the host historically passed) keeps working.
 */
type DisplayEntity = {
  name: string;
  classKey?: EntityClassKey;
  genderKey?: "male" | "female";
  level?: number;
};

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
  playerMaxMp?: number;
  playerExperience: number;
  playerMaxExperience: number;
  freeBagSlots: number;
  maxBagSlots: number;
  currentWeight: number;
  maxWeight: number;
  knownSkills: DisplayKnownSkill[];
};

type EquipmentActionRef = Pick<DisplayEquipmentItem, "slot">;

type StatRangeValue = { min?: number; max?: number };

/**
 * Rich, fully-derived character stats matching Crystal's stats pages. Every
 * field is optional: when the host does not supply this prop (the historical
 * case) the window falls back to deriving AC from equipment defence and DC
 * from equipment attack, leaving the rest of the printed slots blank — exactly
 * as before. When supplied, the stat pages render the authoritative ranges and
 * secondary attributes (accuracy/agility/luck, holy, attack speed, resists).
 */
export type DisplayCharacterStats = {
  ac?: StatRangeValue;
  mac?: StatRangeValue;
  dc?: StatRangeValue;
  mc?: StatRangeValue;
  sc?: StatRangeValue;
  accuracy?: number;
  agility?: number;
  luck?: number;
  holy?: number;
  attackSpeed?: number;
  magicResist?: number;
  poisonResist?: number;
  handWeight?: { current?: number; max?: number };
  wearWeight?: { current?: number; max?: number };
};

type CharacterWindowProps = {
  t: TranslateFn;
  activeTab: CharacterTabKey;
  onClose: () => void;
  onTabChange: (tab: CharacterTabKey) => void;
  player: DisplayEntity | null;
  world: DisplayWorld;
  onRemoveItem: (item: EquipmentActionRef) => void;
  /**
   * Retained for host compatibility but no longer surfaced here: Crystal's
   * CharacterDialog has no in-window repair buttons (repair is NPC-driven via
   * NPCRepair/NPCSRepair), and the previous buttons overlapped the bottom
   * equipment row. Kept optional so existing callers keep type-checking.
   */
  onRepairItem?: (item: EquipmentActionRef) => void;
  onSpecialRepairItem?: (item: EquipmentActionRef) => void;
  onCastSkill: (skillKey: string) => void;
  /** Optional authoritative stat ranges; derived from equipment when absent. */
  stats?: DisplayCharacterStats;
  /** Optional guild name shown on the header line under the player's name. */
  guildName?: string;
  /** Optional guild rank/title shown alongside the guild name. */
  guildRank?: string;
  /** Optional class title / honorific shown next to the level. */
  title?: string;
};

type StatCell = {
  /** Localised stat name, used for the accessible label + hover title. */
  label: string;
  /** Formatted value shown in the cell ("" renders an empty aligned slot). */
  value: string;
};

export function CharacterWindow({
  t,
  activeTab,
  onClose,
  onTabChange,
  player,
  world,
  onRemoveItem,
  onCastSkill,
  stats,
  guildName,
  guildRank,
  title,
}: CharacterWindowProps) {
  const activePage = ORIGINAL_UI.character.pages[activeTab];
  const equipmentBySlot = new Map(world.equipmentItems.map((item) => [item.slot, item]));
  const totalAttack = world.equipmentItems.reduce((sum, item) => sum + item.attack, 0);
  const totalDefence = world.equipmentItems.reduce((sum, item) => sum + item.defence, 0);
  const weaponAttack = equipmentBySlot.get("weapon")?.attack ?? 0;

  // Crystal stats page 1: AC / MAC / DC / MC / SC / Accuracy / Agility / Light /
  // ... then HP / MP at the bottom. We surface what the world payload provides
  // (HP/MP plus derived AC from defence and DC from attack) and leave the rest
  // as empty aligned slots so the panel still lines up with the printed labels.
  const stats1Cells: StatCell[] = [
    rangeCellFrom(t("ui.statAc", [], "AC"), stats?.ac, 0, totalDefence),
    rangeCellFrom(t("ui.statMac", [], "MAC"), stats?.mac, 0, 0),
    rangeCellFrom(t("ui.statDc", [], "DC"), stats?.dc, weaponAttack, totalAttack),
    rangeCellFrom(t("ui.statMc", [], "MC"), stats?.mc, 0, 0),
    rangeCellFrom(t("ui.statSc", [], "SC"), stats?.sc, 0, 0),
    statCell(t("ui.statAccuracy", [], "Accuracy"), flatValue(stats?.accuracy)),
    statCell(t("ui.statAgility", [], "Agility"), flatValue(stats?.agility)),
    statCell(t("ui.statLuck", [], "Luck"), flatValue(stats?.luck)),
    statCell(t("ui.statAttackSpeed", [], "A.Speed"), flatValue(stats?.attackSpeed)),
    statCell(t("ui.hp", [], "HP"), pairValue(world.playerHp, world.playerMaxHp)),
    statCell(t("ui.mp", [], "MP"), pairValue(world.playerMp, world.playerMaxMp)),
  ];

  // Crystal stats page 2: Experience / Weight (bag) / Wear weight / Hand weight /
  // Magic resist / Poison resist / Holy / Luck / Bag space.
  const expPercent = ratio(world.playerExperience, world.playerMaxExperience) * 100;
  const stats2Cells: StatCell[] = [
    statCell(t("ui.experience", [], "Experience"), `${expPercent.toFixed(2)}%`),
    statCell(t("ui.statBagSpace", [], "Bag Space"), pairValue(world.freeBagSlots, world.maxBagSlots)),
    statCell(t("ui.statWeight", [], "Weight"), pairValue(world.currentWeight, world.maxWeight)),
    statCell(t("ui.statHandWeight", [], "Hand Weight"), pairValue(stats?.handWeight?.current, stats?.handWeight?.max)),
    statCell(t("ui.statWearWeight", [], "Wear Weight"), pairValue(stats?.wearWeight?.current, stats?.wearWeight?.max)),
    statCell(t("ui.statMagicResist", [], "Magic Resist"), flatValue(stats?.magicResist)),
    statCell(t("ui.statPoisonResist", [], "Poison Resist"), flatValue(stats?.poisonResist)),
    statCell(t("ui.statHoly", [], "Holy"), flatValue(stats?.holy)),
    statCell("", ""),
    statCell("", ""),
    statCell("", ""),
  ];

  const spellValues = world.knownSkills.slice(0, 7).map((skill) => {
    const cooldownSuffix = skill.cooldownRemainingTicks > 0 ? ` (${skill.cooldownRemainingTicks})` : "";
    return `${skill.name}${cooldownSuffix}`;
  });
  while (spellValues.length < 7) {
    spellValues.push("");
  }

  const classIcon = player?.classKey ? ORIGINAL_UI.character.classIcons[player.classKey] : null;
  const levelLabel = player?.level ? t("ui.heroLevel", [player.level], `Level ${player.level}`) : "";

  return (
    <div className="window-shell character-window">
      <img className="window-frame" src={ORIGINAL_UI.character.frame} alt="" draggable={false} />
      <img className="character-page" src={activePage} alt="" draggable={false} />
      <div className="character-close">
        <SpriteButton sprite={ORIGINAL_UI.character.closeButton} label={t("ui.closeCharacter")} onClick={onClose} />
      </div>

      {/* Crystal CharacterDialog: the frame (Title/504) bakes all four dim tab
          labels; only the selected tab overlays its bright sprite (500-503),
          the others draw nothing (Index = -1). Match that: render a tab's
          <img> only when active; the button stays as the always-clickable
          hotspot over the frame's dim label. */}
      <div className="character-tab char"><button type="button" className="window-tab-button" onClick={() => onTabChange("char")}>{activeTab === "char" ? <img src={ORIGINAL_UI.character.tabs.char} alt={t("ui.character")} draggable={false} /> : null}</button></div>
      <div className="character-tab stats1"><button type="button" className="window-tab-button" onClick={() => onTabChange("stats1")}>{activeTab === "stats1" ? <img src={ORIGINAL_UI.character.tabs.stats1} alt={t("ui.statsPrimary")} draggable={false} /> : null}</button></div>
      <div className="character-tab stats2"><button type="button" className="window-tab-button" onClick={() => onTabChange("stats2")}>{activeTab === "stats2" ? <img src={ORIGINAL_UI.character.tabs.stats2} alt={t("ui.statsSecondary")} draggable={false} /> : null}</button></div>
      <div className="character-tab spells"><button type="button" className="window-tab-button" onClick={() => onTabChange("spells")}>{activeTab === "spells" ? <img src={ORIGINAL_UI.character.tabs.spells} alt={t("ui.spells")} draggable={false} /> : null}</button></div>

      <div className="character-name">{player?.name ?? ""}</div>
      <div className="character-guild">
        {classIcon ? (
          <img
            src={classIcon}
            alt={player?.classKey ?? ""}
            title={`${classLabel(t, player?.classKey)}${levelLabel ? ` · ${levelLabel}` : ""}`}
            draggable={false}
            style={CLASS_BADGE_STYLE}
          />
        ) : null}
        <span>{[levelLabel, title].filter(Boolean).join(" · ")}</span>
      </div>

      {guildName ? (
        <div style={HEADER_GUILD_STYLE} title={guildName}>
          {`< ${guildName}${guildRank ? ` · ${guildRank}` : ""} >`}
        </div>
      ) : null}

      {activeTab === "char" ? (
        <>
          {ORIGINAL_UI.character.equipmentSlots.map((slot) => {
            const equipSlot = equipmentSlotFromLabel(slot.label);
            const item = equipmentBySlot.get(equipSlot);

            return (
              <div
                key={slot.label}
                className="character-slot"
                style={{ left: slot.x + 8, top: slot.y + 90 }}
                aria-label={equipmentSlotLabel(t, equipSlot)}
                title={equipmentSlotLabel(t, equipSlot)}
              >
                {item ? (
                  <button
                    type="button"
                    className="character-slot-card"
                    aria-label={item.name}
                    onClick={() => onRemoveItem({ slot: item.slot })}
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
                      itemType={equipmentSlotLabel(t, equipSlot)}
                      grade={gradeForDurability(item)}
                      durabilityCurrent={item.durabilityCurrent}
                      durabilityMax={item.durabilityMax}
                      dc={item.attack > 0 ? { min: 0, max: item.attack } : undefined}
                      ac={item.defence > 0 ? { min: 0, max: item.defence } : undefined}
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

      {activeTab === "stats1" ? <StatColumn page="stats1" cells={stats1Cells} /> : null}
      {activeTab === "stats2" ? <StatColumn page="stats2" cells={stats2Cells} /> : null}

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

function StatColumn({ page, cells }: { page: "stats1" | "stats2"; cells: StatCell[] }) {
  return (
    <div className={`character-field-values ${page}`}>
      {cells.map((cell, index) => (
        <div
          key={`${page}-${index}`}
          className="character-field-value"
          aria-label={cell.label || undefined}
          title={cell.label || undefined}
        >
          {cell.value}
        </div>
      ))}
    </div>
  );
}

const CLASS_BADGE_STYLE: CSSProperties = {
  width: 16,
  height: 16,
  marginRight: 5,
  verticalAlign: "middle",
  imageRendering: "pixelated",
};

const HEADER_GUILD_STYLE: CSSProperties = {
  position: "absolute",
  left: 0,
  top: 50,
  width: 264,
  textAlign: "center",
  fontSize: 10,
  color: "#caa64a",
  textShadow: "1px 1px 0 #000",
  overflow: "hidden",
  whiteSpace: "nowrap",
  textOverflow: "ellipsis",
};

function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

function statCell(label: string, value: string): StatCell {
  return { label, value };
}

function statRangeCell(label: string, min: number, max: number): StatCell {
  if (min <= 0 && max <= 0) {
    return { label, value: "" };
  }
  const safeMin = Math.max(0, Math.min(min, max));
  return { label, value: `${safeMin}-${max}` };
}

/**
 * Prefer an authoritative stat range when supplied; otherwise derive from the
 * equipment-based fallback (min/max). Renders an empty aligned slot when no
 * data is available, keeping the printed labels aligned.
 */
function rangeCellFrom(label: string, range: StatRangeValue | undefined, fallbackMin: number, fallbackMax: number): StatCell {
  if (range && (range.min !== undefined || range.max !== undefined)) {
    const min = Math.max(0, range.min ?? 0);
    const max = Math.max(min, range.max ?? min);
    return { label, value: `${min}-${max}` };
  }
  return statRangeCell(label, fallbackMin, fallbackMax);
}

function flatValue(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return "";
  return String(value);
}

function pairValue(current?: number, max?: number) {
  if (current === undefined || max === undefined) {
    return "";
  }
  return `${current}/${max}`;
}

function ratio(value?: number, max?: number) {
  if (value === undefined || max === undefined || max <= 0) {
    return 0;
  }
  return Math.max(0, Math.min(1, value / max));
}

/**
 * Derive a display grade from how well-preserved the item is so the tooltip
 * header gets a sensible colour even when the payload has no explicit grade.
 */
function gradeForDurability(item: DisplayEquipmentItem): ItemTooltipGrade | undefined {
  if (item.durabilityMax <= 0) return undefined;
  const combined = item.attack + item.defence;
  if (combined >= 12) return "legendary";
  if (combined >= 8) return "heroic";
  if (combined >= 4) return "rare";
  return "common";
}

function classLabel(t: TranslateFn, classKey?: EntityClassKey) {
  switch (classKey) {
    case "wizard":
      return t("ui.classWizard", [], "Wizard");
    case "taoist":
      return t("ui.classTaoist", [], "Taoist");
    case "assassin":
      return t("ui.classAssassin", [], "Assassin");
    case "archer":
      return t("ui.classArcher", [], "Archer");
    case "warrior":
      return t("ui.classWarrior", [], "Warrior");
    default:
      return "";
  }
}

function equipmentSlotLabel(t: TranslateFn, slot: EquipmentSlot): string {
  switch (slot) {
    case "weapon":
      return t("ui.slotWeapon", [], "Weapon");
    case "armour":
      return t("ui.slotArmour", [], "Armour");
    case "helmet":
      return t("ui.slotHelmet", [], "Helmet");
    case "mount":
      return t("ui.slotMount", [], "Mount");
    case "necklace":
      return t("ui.slotNecklace", [], "Necklace");
    case "torch":
      return t("ui.slotTorch", [], "Torch");
    case "braceletLeft":
    case "braceletRight":
      return t("ui.slotBracelet", [], "Bracelet");
    case "ringLeft":
    case "ringRight":
      return t("ui.slotRing", [], "Ring");
    case "amulet":
      return t("ui.slotAmulet", [], "Amulet");
    case "boots":
      return t("ui.slotBoots", [], "Boots");
    case "belt":
      return t("ui.slotBelt", [], "Belt");
    case "stone":
    default:
      return t("ui.slotStone", [], "Stone");
  }
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
