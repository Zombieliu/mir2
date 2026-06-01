"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

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
  summoned?: boolean;
};

export type HeroPetWindowProps = {
  t: TranslateFn;
  hero: HeroSummary | null;
  creatures: CreatureSummary[];
  onSummonHero?: () => void;
  onDismissHero?: () => void;
  onSummonCreature?: (creatureId: string) => void;
  onReleaseCreature?: (creatureId: string) => void;
  onCyclePickupMode?: (creatureId: string) => void;
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
  onSummonCreature,
  onReleaseCreature,
  onCyclePickupMode,
  onClose,
}: HeroPetWindowProps) {
  const [tab, setTab] = useState<HeroPetTab>(hero ? "hero" : "creatures");
  const [selectedCreatureId, setSelectedCreatureId] = useState<string | null>(creatures[0]?.id ?? null);

  const selectedCreature = useMemo(() => {
    if (selectedCreatureId) {
      const match = creatures.find((creature) => creature.id === selectedCreatureId);
      if (match) return match;
    }
    return creatures[0] ?? null;
  }, [creatures, selectedCreatureId]);

  useEffect(() => {
    if (!hero && tab === "hero") {
      setTab("creatures");
    }
  }, [hero, tab]);

  const heroClassIcon = hero?.classKey ? CLASS_ICONS[hero.classKey] : undefined;

  return (
    <section
      aria-label={t("ui.heroPet", [], "Hero & Creatures")}
      data-hero-pet-tab={tab}
      data-hero-active={hero?.active ? "1" : "0"}
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
          <span style={style.tabCount}>{creatures.length}</span>
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
                  <div style={style.heroName}>{hero.name}</div>
                  <div style={style.heroMeta}>
                    {t("ui.heroLevel", [hero.level], `Level ${hero.level}`)}
                    {hero.active ? ` · ${t("ui.heroSummoned", [], "Summoned")}` : ""}
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

              <div style={style.actions}>
                <ActionButton
                  label={t("ui.heroSummon", [], "Summon")}
                  disabled={!onSummonHero || Boolean(hero.active)}
                  onClick={() => onSummonHero?.()}
                />
                <ActionButton
                  label={t("ui.heroDismiss", [], "Dismiss")}
                  disabled={!onDismissHero || !hero.active}
                  onClick={() => onDismissHero?.()}
                />
              </div>
            </>
          ) : (
            <div style={style.empty}>{t("ui.heroNone", [], "No hero recruited yet.")}</div>
          )}
        </div>
      ) : (
        <div style={style.body}>
          <div style={style.creatureList} aria-label={t("ui.creatures", [], "Creatures")}>
            {creatures.length === 0 ? (
              <div style={style.empty}>{t("ui.creatureNone", [], "No creatures tamed.")}</div>
            ) : (
              creatures.map((creature) => {
                const isSelected = selectedCreature?.id === creature.id;
                return (
                  <button
                    key={creature.id}
                    type="button"
                    data-creature-id={creature.id}
                    aria-pressed={isSelected}
                    onClick={() => setSelectedCreatureId(creature.id)}
                    style={{ ...style.creatureRow, ...(isSelected ? style.creatureRowSelected : null) }}
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

          <div style={style.creatureDetail} data-creature-detail={selectedCreature?.id ?? ""}>
            {selectedCreature ? (
              <>
                <div style={style.creatureDetailName}>{selectedCreature.name}</div>
                <div style={style.creatureDetailMeta}>
                  {selectedCreature.level
                    ? t("ui.heroLevel", [selectedCreature.level], `Level ${selectedCreature.level}`)
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
                    disabled={!onCyclePickupMode}
                    onClick={() => onCyclePickupMode?.(selectedCreature.id)}
                    style={{ ...style.creaturePickupButton, ...(!onCyclePickupMode ? style.actionButtonDisabled : null) }}
                  >
                    {selectedCreature.pickupMode ?? t("ui.creaturePickupOff", [], "Off")}
                  </button>
                </div>
                <div style={style.actions}>
                  <ActionButton
                    label={t("ui.creatureSummon", [], "Summon")}
                    disabled={!onSummonCreature || Boolean(selectedCreature.summoned)}
                    onClick={() => onSummonCreature?.(selectedCreature.id)}
                  />
                  <ActionButton
                    label={t("ui.creatureRelease", [], "Release")}
                    disabled={!onReleaseCreature}
                    onClick={() => onReleaseCreature?.(selectedCreature.id)}
                  />
                </div>
              </>
            ) : (
              <div style={style.empty}>{t("ui.creatureSelectHint", [], "Select a creature.")}</div>
            )}
          </div>
        </div>
      )}
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
      style={{ ...style.actionButton, ...(disabled ? style.actionButtonDisabled : null) }}
    >
      {label}
    </button>
  );
}

function statValue(value?: number) {
  return value && value > 0 ? String(value) : "-";
}

function creatureIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
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
    gap: 6,
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
  heroName: { color: "#f8e6bb", fontSize: 13, fontWeight: 700 },
  heroMeta: { fontSize: 10, color: "#cbb38a" },
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
    overflow: "hidden",
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
