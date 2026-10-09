"use client";

import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { SpriteButton } from "./original-client-overlays";
import { CREATURE_FILTER_KEYS, creaturePlayerCommand, creaturePlayerSourceCurrent, creaturePlayerSourceKey,
  type CreatureItemFilter, type CreaturePlayerIntent, type CreaturePlayerSource } from "../../lib/creature-player-ui";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

/** Hero AI behaviour, mirroring Crystal's `HeroBehaviour` enum. */
export type HeroBehaviourKey = "attack" | "counterAttack" | "follow" | "custom";

/** Hero spawn state, mirroring Crystal's `HeroSpawnState` enum. */
export type HeroSpawnStateKey = "none" | "unsummoned" | "summoned" | "dead";

/** A summoned battle hero (the "hero" slot of stage-5 systems). */
export type HeroSummary = {
  name: string;
  classKey?: EntityClassKey;
  level: number;
  hp: number;
  maxHp: number;
  mp?: number;
  maxMp?: number;
  experience?: number;
  maxExperience?: number;
  loyalty?: number;
  maxLoyalty?: number;
  attack?: number;
  defence?: number;
  /** Magic defence (Crystal MAC stat). */
  magicDefence?: number;
  /** Magic attack (Crystal MC stat). */
  magicAttack?: number;
  /** Spell power / Tao attack (Crystal SC stat). */
  spellPower?: number;
  /** Hero grade/tier label (e.g. "Common" / "Elite"). */
  grade?: string;
  /** Gender, used for the avatar fallback. */
  gender?: "male" | "female";
  /** Current AI behaviour. Drives the highlighted mode in the selector. */
  behaviour?: HeroBehaviourKey;
  /** Spawn state; when present it supersedes `active` for the summon buttons. */
  spawnState?: HeroSpawnStateKey;
  active?: boolean;
};

/** A tamed pet / intelligent creature row. */
export type CreatureSummary = {
  id: string;
  name: string;
  /** Icon index into `/original-ui/Items/<icon>.png` if known. */
  icon?: number;
  level?: number;
  hp?: number;
  maxHp?: number;
  /** Remaining lifespan in ticks/seconds — rendered as a bar when maxLifespan is set. */
  lifespan?: number;
  maxLifespan?: number;
  /** Pickup/autopick mode label, e.g. "Auto" / "Semi" / "Off". */
  pickupMode?: string;
  /** Creature type label (Crystal IntelligentCreatureType). */
  typeLabel?: string;
  summoned?: boolean;
};

/** AI-mode selector option. */
type HeroBehaviourOption = { key: HeroBehaviourKey; labelKey: string; fallback: string };

const HERO_BEHAVIOURS: HeroBehaviourOption[] = [
  { key: "attack", labelKey: "ui.heroAiAttack", fallback: "Attack" },
  { key: "counterAttack", labelKey: "ui.heroAiCounter", fallback: "Counter" },
  { key: "follow", labelKey: "ui.heroAiFollow", fallback: "Follow" },
  { key: "custom", labelKey: "ui.heroAiCustom", fallback: "Custom" },
];

export type HeroPetWindowProps = {
  t: TranslateFn;
  hero: HeroSummary | null;
  creatures: CreatureSummary[];
  onSummonHero?: () => void;
  onDismissHero?: () => void;
  /** Recall a dead/away hero (Crystal recall). */
  onRecallHero?: () => void;
  /** Change the hero's AI behaviour (Crystal SetHeroBehaviour). */
  onSetHeroBehaviour?: (behaviour: HeroBehaviourKey) => void;
  onOpenHeroManagement?: (page: "inventory" | "equipment" | "skills") => void;
  onSummonCreature?: (creatureId: string) => void;
  onReleaseCreature?: (creatureId: string) => void;
  onCyclePickupMode?: (creatureId: string) => void;
  creatureSource?: CreaturePlayerSource | null;
  petActionPending?: boolean;
  onPetAction?: (intent: CreaturePlayerIntent, renderSource: CreaturePlayerSource) => void;
  onClose: () => void;
};

type HeroPetTab = "hero" | "creatures";

const FRAME = ORIGINAL_UI.character;

const CLASS_ICONS = FRAME.classIcons;

export function HeroPetWindow({
  t,
  hero,
  creatures,
  onSummonHero,
  onDismissHero,
  onRecallHero,
  onSetHeroBehaviour,
  onOpenHeroManagement,
  creatureSource = null,
  petActionPending = false,
  onPetAction,
  onClose,
}: HeroPetWindowProps) {
  const [tab, setTab] = useState<HeroPetTab>(hero ? "hero" : "creatures");
  const [selectedCreatureId, setSelectedCreatureId] = useState<string | null>(creatures[0]?.id ?? null);
  const visibleCreatures = useMemo<CreatureSummary[]>(() => creatureSource ? creatureSource.records.map(pet => ({
    id: `pet-${pet.petType}`, name: pet.customName, icon: pet.icon >= 0 ? pet.icon : undefined,
    summoned: creatureSource.summonedPetType === pet.petType, typeLabel: `Pet ${pet.petType}`,
    pickupMode: pet.petMode === 0 ? "Auto" : "Semi-auto",
  })) : creatures, [creatures, creatureSource]);
  const [petEditor, setPetEditor] = useState<{
    kind: "rename" | "release" | "options"; source: CreaturePlayerSource; petType: number;
    text: string; filter: CreatureItemFilter; grade: number;
  } | null>(null);
  const submittedPetAction = useRef<string | null>(null);

  const selectedCreature = useMemo(() => {
    if (selectedCreatureId) {
      const match = visibleCreatures.find((creature) => creature.id === selectedCreatureId);
      if (match) return match;
    }
    return visibleCreatures[0] ?? null;
  }, [visibleCreatures, selectedCreatureId]);
  const selectedPet = creatureSource?.records.find(pet => `pet-${pet.petType}` === selectedCreature?.id) ?? null;
  const petEditorCurrent = petEditor !== null && creaturePlayerSourceCurrent(petEditor.source, creatureSource);
  const canPetAction = (intent: CreaturePlayerIntent) => !!creatureSource && !!onPetAction && !petActionPending
    && petEditor === null && creaturePlayerCommand(intent, creatureSource) !== null;
  const submitPetAction = (intent: CreaturePlayerIntent, captured: CreaturePlayerSource | null = creatureSource) => {
    if (!captured || !onPetAction || petActionPending || !creaturePlayerSourceCurrent(captured, creatureSource)
      || !creaturePlayerCommand(intent, captured)) return;
    const key = JSON.stringify([captured.owner.connectionGeneration, captured.owner.sessionGeneration,
      captured.owner.sceneRevision, captured.owner.playerObjectId, captured.owner.mapFileName, creaturePlayerSourceKey(captured), intent]);
    if (submittedPetAction.current === key) return;
    submittedPetAction.current = key;
    onPetAction(intent, captured);
  };
  const openPetEditor = (kind: "rename" | "release" | "options") => {
    if (!selectedPet || !creatureSource || !onPetAction || petActionPending) return;
    if (kind === "rename" && !creatureSource.renameEnabled
      || kind === "release" && creatureSource.summonedPetType === selectedPet.petType) return;
    setPetEditor({ kind, source: creatureSource, petType: selectedPet.petType,
      text: kind === "rename" ? selectedPet.customName : "", filter: selectedPet.filter, grade: selectedPet.pickupGrade });
  };

  useEffect(() => {
    if (!hero && tab === "hero") {
      setTab("creatures");
    }
  }, [hero, tab]);

  const heroClassIcon = hero?.classKey ? CLASS_ICONS[hero.classKey] : undefined;
  // Spawn state supersedes the legacy `active` flag when present.
  const heroSummoned = hero ? hero.spawnState === "summoned" || (hero.spawnState === undefined && Boolean(hero.active)) : false;
  const heroDead = hero?.spawnState === "dead";

  return (
    <section
      aria-label={t("ui.heroPet", [], "Hero & Creatures")}
      data-hero-pet-tab={tab}
      data-hero-active={heroSummoned ? "1" : "0"}
      data-hero-state={hero?.spawnState ?? ""}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.page} src={FRAME.pages.char} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.heroPet", [], "Hero & Creatures")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.heroPet", [], "Hero & Creatures")}>
        <button
          type="button"
          role="tab"
          data-hero-pet-tab="hero"
          aria-selected={tab === "hero"}
          onClick={() => setTab("hero")}
          style={{ ...style.tab, ...(tab === "hero" ? style.tabActive : null) }}
        >
          {t("ui.hero", [], "Hero")}
        </button>
        <button
          type="button"
          role="tab"
          data-hero-pet-tab="creatures"
          aria-selected={tab === "creatures"}
          onClick={() => setTab("creatures")}
          style={{ ...style.tab, ...(tab === "creatures" ? style.tabActive : null) }}
        >
          {t("ui.creatures", [], "Creatures")}
          <span style={style.tabCount}>{visibleCreatures.length}</span>
        </button>
      </div>

      {tab === "hero" ? (
        <div style={style.body} data-hero-name={hero?.name ?? ""}>
          {hero ? (
            <>
              <div style={style.heroHeader}>
                {heroClassIcon ? (
                  <img style={style.heroClassIcon} src={heroClassIcon} alt="" draggable={false} />
                ) : (
                  <span style={style.heroClassFallback} aria-hidden="true" />
                )}
                <div style={style.heroHeaderText}>
                  <div style={style.heroNameRow}>
                    <span style={style.heroName}>{hero.name}</span>
                    {hero.grade ? <span style={style.heroGrade}>{hero.grade}</span> : null}
                  </div>
                  <div style={style.heroMeta}>
                    {t("ui.heroLevel", [hero.level], `Level ${hero.level}`)}
                    {heroDead
                      ? ` · ${t("ui.heroDead", [], "Dead")}`
                      : heroSummoned
                        ? ` · ${t("ui.heroSummoned", [], "Summoned")}`
                        : ` · ${t("ui.heroAway", [], "Away")}`}
                  </div>
                </div>
              </div>

              <Gauge label={t("ui.hp", [], "HP")} current={hero.hp} max={hero.maxHp} color="#d8552f" />
              {hero.maxMp ? (
                <Gauge label={t("ui.mp", [], "MP")} current={hero.mp ?? 0} max={hero.maxMp} color="#3f7fd8" />
              ) : null}
              {hero.maxExperience ? (
                <Gauge
                  label={t("ui.experience", [], "EXP")}
                  current={hero.experience ?? 0}
                  max={hero.maxExperience}
                  color="#caa64a"
                />
              ) : null}
              {hero.maxLoyalty ? (
                <Gauge
                  label={t("ui.heroLoyalty", [], "Loyalty")}
                  current={hero.loyalty ?? 0}
                  max={hero.maxLoyalty}
                  color="#8be07a"
                />
              ) : null}

              <div style={style.statGrid}>
                <Stat label={t("ui.attack", [], "AC")} value={statValue(hero.attack)} />
                <Stat label={t("ui.defence", [], "DC")} value={statValue(hero.defence)} />
              </div>
              {hero.magicAttack !== undefined ||
              hero.magicDefence !== undefined ||
              hero.spellPower !== undefined ? (
                <div style={style.statGrid}>
                  <Stat label={t("ui.magicAttack", [], "MC")} value={statValue(hero.magicAttack)} />
                  <Stat label={t("ui.magicDefence", [], "MAC")} value={statValue(hero.magicDefence)} />
                  <Stat label={t("ui.spellPower", [], "SC")} value={statValue(hero.spellPower)} />
                </div>
              ) : null}

              <div style={style.aiSection}>
                <div style={style.aiLabel}>{t("ui.heroAiMode", [], "AI Mode")}</div>
                <div style={style.aiButtons} role="group" aria-label={t("ui.heroAiMode", [], "AI Mode")}>
                  {HERO_BEHAVIOURS.map((option) => {
                    const isActive = hero.behaviour === option.key;
                    return (
                      <button
                        key={option.key}
                        type="button"
                        data-hero-ai={option.key}
                        aria-pressed={isActive}
                        disabled={!onSetHeroBehaviour || isActive}
                        onClick={() => onSetHeroBehaviour?.(option.key)}
                        style={{
                          ...style.aiButton,
                          ...(isActive ? style.aiButtonActive : null),
                          ...(!onSetHeroBehaviour && !isActive ? style.actionButtonDisabled : null),
                        }}
                      >
                        {t(option.labelKey, [], option.fallback)}
                      </button>
                    );
                  })}
                </div>
              </div>

              <div style={style.actions}>
                <ActionButton
                  label={t("ui.heroSummon", [], "Summon")}
                  disabled={!onSummonHero || heroSummoned || heroDead}
                  onClick={() => onSummonHero?.()}
                />
                <ActionButton
                  label={t("ui.heroDismiss", [], "Dismiss")}
                  disabled={!onDismissHero || !heroSummoned}
                  onClick={() => onDismissHero?.()}
                />
                {onRecallHero ? (
                  <ActionButton
                    label={t("ui.heroRecall", [], "Recall")}
                    disabled={!heroSummoned}
                    onClick={() => onRecallHero()}
                  />
                ) : null}
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 6 }}>
                <ActionButton label={t("ui.heroBag", [], "Hero bag")} disabled={!onOpenHeroManagement}
                  onClick={() => onOpenHeroManagement?.("inventory")} />
                <ActionButton label={t("ui.heroEquipment", [], "Equipment")} disabled={!onOpenHeroManagement}
                  onClick={() => onOpenHeroManagement?.("equipment")} />
                <ActionButton label={t("ui.heroSkills", [], "Skills")} disabled={!onOpenHeroManagement}
                  onClick={() => onOpenHeroManagement?.("skills")} />
              </div>
            </>
          ) : (
            <div style={style.empty}>{t("ui.heroNone", [], "No hero recruited yet.")}</div>
          )}
        </div>
      ) : (
        <div style={style.body}>
          <div style={style.creatureList} aria-label={t("ui.creatures", [], "Creatures")}>
            {visibleCreatures.length === 0 ? (
              <div style={style.empty}>{t("ui.creatureNone", [], "No creatures tamed.")}</div>
            ) : (
               visibleCreatures.map((creature) => {
                const isSelected = selectedCreature?.id === creature.id;
                return (
                  <button
                    key={creature.id}
                    type="button"
                    data-creature-id={creature.id}
                    aria-pressed={isSelected}
                    onClick={() => { setSelectedCreatureId(creature.id); setPetEditor(null); }}
                    style={{ ...style.creatureRow, minHeight: 44, flexShrink: 0, ...(isSelected ? style.creatureRowSelected : null) }}
                  >
                    {typeof creature.icon === "number" ? (
                      <img style={style.creatureIcon} src={creatureIconPath(creature.icon)} alt="" draggable={false} />
                    ) : (
                      <span style={style.creatureIconFallback} aria-hidden="true" />
                    )}
                    <span style={style.creatureRowName}>{creature.name}</span>
                    {creature.summoned ? <span style={style.creatureRowFlag}>{t("ui.creatureOut", [], "Out")}</span> : null}
                  </button>
                );
              })
            )}
          </div>

          <div style={{ ...style.creatureDetail, flex: "0 0 auto", overflow: "visible" }} data-creature-detail={selectedCreature?.id ?? ""}>
            {selectedCreature ? (
              <>
                <div style={style.creatureDetailName}>{selectedCreature.name}</div>
                <div style={style.creatureDetailMeta}>
                  {selectedCreature.typeLabel ? <span>{selectedCreature.typeLabel}</span> : null}
                  {selectedCreature.typeLabel && selectedCreature.level ? <span> · </span> : null}
                  {selectedCreature.level
                    ? t("ui.heroLevel", [selectedCreature.level], `Level ${selectedCreature.level}`)
                    : selectedCreature.typeLabel
                      ? null
                      : t("ui.creature", [], "Creature")}
                </div>
                {selectedCreature.maxHp ? (
                  <Gauge label={t("ui.hp", [], "HP")} current={selectedCreature.hp ?? 0} max={selectedCreature.maxHp} color="#d8552f" />
                ) : null}
                {selectedCreature.maxLifespan ? (
                  <Gauge
                    label={t("ui.creatureLifespan", [], "Lifespan")}
                    current={selectedCreature.lifespan ?? 0}
                    max={selectedCreature.maxLifespan}
                    color="#8be07a"
                  />
                ) : null}
                <div style={style.creaturePickup}>
                  <span style={style.creaturePickupLabel}>{t("ui.creaturePickup", [], "Pickup")}</span>
                  <button
                    type="button"
                    disabled={!selectedPet || !canPetAction({ kind: "mode", petType: selectedPet.petType, mode: selectedPet.petMode === 0 ? 1 : 0 })}
                    onClick={() => { if (selectedPet) submitPetAction({ kind: "mode", petType: selectedPet.petType,
                      mode: selectedPet.petMode === 0 ? 1 : 0 }); }}
                    style={{ ...style.creaturePickupButton, minHeight: 44 }}
                  >
                    {selectedCreature.pickupMode ?? t("ui.creaturePickupOff", [], "Off")}
                  </button>
                </div>
                <div style={{ ...style.actions, display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))" }}>
                  <ActionButton
                    label={t("ui.creatureSummon", [], "Summon")}
                    disabled={!selectedPet || !canPetAction({ kind: "summon", petType: selectedPet.petType })}
                    onClick={() => { if (selectedPet) submitPetAction({ kind: "summon", petType: selectedPet.petType }); }}
                  />
                  <ActionButton label={t("ui.creatureDismiss", [], "Dismiss")}
                    disabled={!selectedPet || !canPetAction({ kind: "dismiss", petType: selectedPet.petType })}
                    onClick={() => { if (selectedPet) submitPetAction({ kind: "dismiss", petType: selectedPet.petType }); }} />
                  <ActionButton
                    label={t("ui.creatureRelease", [], "Release")}
                    disabled={!selectedPet || !onPetAction || petActionPending || petEditor !== null
                      || creatureSource?.summonedPetType === selectedPet.petType}
                    onClick={() => openPetEditor("release")}
                  />
                  <ActionButton label={t("ui.creatureRename", [], "Rename")}
                    disabled={!selectedPet || !onPetAction || petActionPending || petEditor !== null || !creatureSource?.renameEnabled}
                    onClick={() => openPetEditor("rename")} />
                  <ActionButton label={t("ui.creatureOptions", [], "Options")}
                    disabled={!selectedPet || !onPetAction || petActionPending || petEditor !== null}
                    onClick={() => openPetEditor("options")} />
                </div>
                {selectedPet ? <label style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 8 }}>
                  {t("ui.creatureSlot", [], "Slot")}
                  <select aria-label={t("ui.creatureSlot", [], "Slot")} value={selectedPet.slotIndex}
                    style={{ minHeight: 44 }} disabled={!onPetAction || petActionPending || petEditor !== null}
                    onChange={event => { if (selectedPet) submitPetAction({ kind: "slot", petType: selectedPet.petType, slotIndex: Number(event.currentTarget.value) }); }}>
                    {Array.from({ length: 10 }, (_, slot) => <option key={slot} value={slot}
                      disabled={creatureSource?.records.some(pet => pet.petType !== selectedPet.petType && pet.slotIndex === slot)}>{slot}</option>)}
                  </select>
                </label> : null}
                {!creatureSource ? <p role="status">{t("ui.creatureSourceUnknown", [], "Waiting for the complete current creature record.")}</p> : null}
                {petActionPending ? <p role="status">{t("ui.creaturePending", [], "Waiting for authoritative creature state.")}</p> : null}
              </>
            ) : (
              <div style={style.empty}>{t("ui.creatureSelectHint", [], "Select a creature.")}</div>
            )}
          </div>
        </div>
      )}
      {petEditor ? <div role="dialog" aria-modal="true"
        aria-label={petEditor.kind === "options" ? t("ui.creatureOptions", [], "Creature options") : t("ui.confirm", [], "Confirm")}
        style={{ position: "absolute", inset: 16, zIndex: 1000, background: "#21190f", border: "1px solid #987443",
          padding: 16, color: "#eadbb4", overflow: "auto" }} onKeyDown={event => {
          if (event.key === "Escape") { event.stopPropagation(); setPetEditor(null); }
        }}>
        {petEditor.kind === "options" ? <>
          <p>{t("ui.creatureOptions", [], "Creature options")}</p>
          {CREATURE_FILTER_KEYS.map(key => <label key={key} style={{ display: "flex", minHeight: 44, alignItems: "center", gap: 10 }}>
            <input type="checkbox" checked={petEditor.filter[key]} disabled={!petEditorCurrent || petActionPending}
              onChange={event => { const checked = event.currentTarget.checked; setPetEditor(current => current ? {
                ...current, filter: { ...current.filter, [key]: checked } } : null); }} />
            {t(`ui.${key}`, [], key.replace(/^petPickup/, "Pickup "))}
          </label>)}
          <label style={{ display: "flex", minHeight: 44, alignItems: "center", gap: 10 }}>
            {t("ui.creaturePickupGrade", [], "Pickup grade")}
            <select value={petEditor.grade} style={{ minHeight: 44 }} disabled={!petEditorCurrent || petActionPending}
              onChange={event => { const grade = Number(event.currentTarget.value); setPetEditor(current => current ? { ...current, grade } : null); }}>
              {[0, 1, 2, 3, 4, 5].map(grade => <option key={grade} value={grade}>{grade}</option>)}
            </select>
          </label>
        </> : <>
          <p>{petEditor.kind === "release"
            ? t("ui.creatureReleaseVerify", [petEditor.source.records.find(pet => pet.petType === petEditor.petType)?.customName ?? ""],
              `Type the current creature name: ${petEditor.source.records.find(pet => pet.petType === petEditor.petType)?.customName ?? ""}`)
            : t("ui.creatureRenameRule", [], "Use 3–15 ASCII letters or digits.")}</p>
          <input aria-label={petEditor.kind === "release" ? t("ui.creatureName", [], "Current creature name") : t("ui.creatureNewName", [], "New creature name")}
            value={petEditor.text} maxLength={petEditor.kind === "rename" ? 15 : 256} style={{ minHeight: 44, width: "100%" }}
            disabled={!petEditorCurrent || petActionPending} onChange={event => {
              const text = event.currentTarget.value; setPetEditor(current => current ? { ...current, text } : null);
            }} />
        </>}
        {!petEditorCurrent ? <p role="status">{t("ui.creatureChanged", [], "Creature state changed. Close this draft and review it again.")}</p> : null}
        <div style={{ display: "flex", gap: 12, marginTop: 12 }}>
          <button type="button" autoFocus style={{ minHeight: 44, minWidth: 88 }} onClick={() => setPetEditor(null)}>{t("ui.cancel", [], "Cancel")}</button>
          <button type="button" style={{ minHeight: 44, minWidth: 88 }} disabled={!petEditorCurrent || petActionPending || !onPetAction
            || !creaturePlayerCommand(petEditor.kind === "options"
              ? { kind: "options", petType: petEditor.petType, filter: petEditor.filter, pickupGrade: petEditor.grade }
              : petEditor.kind === "rename" ? { kind: "rename", petType: petEditor.petType, text: petEditor.text }
                : { kind: "release", petType: petEditor.petType, confirmationName: petEditor.text }, petEditor.source)}
            onClick={() => {
              const captured = petEditor;
              if (!captured || !creaturePlayerSourceCurrent(captured.source, creatureSource)) return;
              const intent: CreaturePlayerIntent = captured.kind === "options"
                ? { kind: "options", petType: captured.petType, filter: captured.filter, pickupGrade: captured.grade }
                : captured.kind === "rename" ? { kind: "rename", petType: captured.petType, text: captured.text }
                  : { kind: "release", petType: captured.petType, confirmationName: captured.text };
              if (!creaturePlayerCommand(intent, captured.source) || petActionPending || !onPetAction) return;
              submitPetAction(intent, captured.source); setPetEditor(null);
            }}>{petEditor.kind === "options" ? t("ui.save", [], "Save") : t("ui.confirm", [], "Confirm")}</button>
        </div>
      </div> : null}
    </section>
  );
}

function Gauge({ label, current, max, color }: { label: string; current: number; max: number; color: string }) {
  const pct = Math.min(100, Math.max(0, (current / Math.max(max, 1)) * 100));
  return (
    <div style={style.gauge}>
      <div style={style.gaugeHead}>
        <span style={style.gaugeLabel}>{label}</span>
        <span style={style.gaugeValue}>{`${current}/${max}`}</span>
      </div>
      <div style={style.gaugeTrack} role="progressbar" aria-valuenow={current} aria-valuemax={max} aria-label={label}>
        <span style={{ ...style.gaugeFill, width: `${pct}%`, background: color }} />
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div style={style.stat}>
      <span style={style.statLabel}>{label}</span>
      <span style={style.statValue}>{value}</span>
    </div>
  );
}

function ActionButton({ label, disabled, onClick }: { label: string; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      style={{ ...style.actionButton, minHeight: 44, ...(disabled ? style.actionButtonDisabled : null) }}
    >
      {label}
    </button>
  );
}

function statValue(value?: number) {
  return value && value > 0 ? String(value) : "-";
}

function creatureIconPath(icon: number) {
  return originalItemIconPath(icon);
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 380,
    top: 120,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 29,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  page: { position: "absolute", left: 8, top: 90, pointerEvents: "none", opacity: 0.35 },
  titleText: {
    position: "absolute",
    left: 16,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  close: { position: "absolute", left: 238, top: 4 },
  tabs: {
    position: "absolute",
    left: 12,
    top: 28,
    width: 240,
    display: "flex",
    gap: 3,
  },
  tab: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "3px 0",
    fontSize: 11,
    cursor: "pointer",
    display: "flex",
    justifyContent: "center",
    alignItems: "center",
    gap: 5,
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  tabCount: { fontSize: 9, opacity: 0.85 },
  body: {
    position: "absolute",
    left: 12,
    top: 54,
    width: 240,
    height: 314,
    display: "flex",
    flexDirection: "column",
    gap: 5,
    overflowY: "auto",
  },
  empty: { color: "#cbb38a", padding: "10px 4px", fontSize: 11 },
  heroHeader: { display: "flex", alignItems: "center", gap: 8 },
  heroClassIcon: { width: 40, height: 40, imageRendering: "pixelated" },
  heroClassFallback: {
    width: 40,
    height: 40,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(20, 13, 7, 0.6)",
  },
  heroHeaderText: { display: "flex", flexDirection: "column", gap: 2, minWidth: 0 },
  heroNameRow: { display: "flex", alignItems: "baseline", gap: 6, minWidth: 0 },
  heroName: { color: "#f8e6bb", fontSize: 13, fontWeight: 700, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  heroGrade: {
    flex: "0 0 auto",
    fontSize: 9,
    color: "#f0d69b",
    border: "1px solid rgba(214, 180, 110, 0.5)",
    borderRadius: 2,
    padding: "0 4px",
  },
  heroMeta: { fontSize: 10, color: "#cbb38a" },
  aiSection: { display: "flex", flexDirection: "column", gap: 3 },
  aiLabel: { fontSize: 9, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  aiButtons: { display: "flex", gap: 3 },
  aiButton: {
    flex: 1,
    minWidth: 0,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "3px 2px",
    fontSize: 10,
    cursor: "pointer",
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  aiButtonActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
    cursor: "default",
  },
  statGrid: { display: "flex", gap: 6 },
  stat: {
    flex: 1,
    display: "flex",
    justifyContent: "space-between",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(20, 13, 7, 0.45)",
    padding: "2px 6px",
    fontSize: 11,
  },
  statLabel: { color: "#a89568" },
  statValue: { color: "#f0d69b" },
  gauge: { display: "flex", flexDirection: "column", gap: 2 },
  gaugeHead: { display: "flex", justifyContent: "space-between", fontSize: 10, color: "#cbb38a" },
  gaugeLabel: { color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  gaugeValue: { color: "#e3d3af" },
  gaugeTrack: {
    position: "relative",
    height: 7,
    background: "rgba(0, 0, 0, 0.55)",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    overflow: "hidden",
  },
  gaugeFill: { position: "absolute", left: 0, top: 0, bottom: 0, display: "block" },
  creatureList: {
    flex: "0 0 132px",
    display: "flex",
    flexDirection: "column",
    gap: 2,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: 3,
  },
  creatureRow: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    width: "100%",
    height: 24,
    padding: "0 5px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  creatureRowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  creatureIcon: { width: 18, height: 18, imageRendering: "pixelated", flex: "0 0 auto" },
  creatureIconFallback: {
    width: 18,
    height: 18,
    flex: "0 0 auto",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "rgba(0, 0, 0, 0.4)",
  },
  creatureRowName: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  creatureRowFlag: { fontSize: 9, color: "#8be07a", flex: "0 0 auto" },
  creatureDetail: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    gap: 5,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
    overflow: "hidden",
  },
  creatureDetailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700 },
  creatureDetailMeta: { fontSize: 10, color: "#cbb38a" },
  creaturePickup: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 6 },
  creaturePickupLabel: { fontSize: 10, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  creaturePickupButton: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "2px 10px",
    fontSize: 11,
    cursor: "pointer",
  },
  actions: { display: "flex", gap: 6, marginTop: "auto" },
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "4px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
