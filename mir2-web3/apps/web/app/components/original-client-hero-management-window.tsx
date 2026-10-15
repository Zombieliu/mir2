"use client";

import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import { OriginalCrystalItemTooltip } from "./original-client-crystal-item-tooltip";
import {
  captureHeroAction, heroActionCurrent, planHeroAction,
  type HeroActionDto, type HeroCell, type HeroItem, type HeroOperationPending,
  type HeroPlayerModel, type HeroUiAction,
} from "../../lib/hero-player-ui";

export type HeroCharacterPage = "equipment" | "status" | "state" | "skills";
export type HeroManagementPage = "inventory" | HeroCharacterPage;
export type HeroUiOrigin = "inventory" | "character" | "belt";
export type HeroUiLease = Readonly<{ origin: HeroUiOrigin; epoch: number }>;
export type HeroWindowPosition = Readonly<{ x: number; y: number }>;
export type HeroManagementWindows = Readonly<{
  inventoryOpen: boolean; characterOpen: boolean; characterPage: HeroCharacterPage;
  beltVisible: boolean; beltVertical: boolean;
  inventoryPosition?: HeroWindowPosition | null; characterPosition?: HeroWindowPosition | null;
  beltPosition?: HeroWindowPosition | null;
}>;
/** Already formatted by the shared Native character_stats renderer. */
export type HeroStatLine = Readonly<{ text: string; color?: string }>;
export type HeroManagementWindowProps = {
  t: (key: string, params?: Array<string | number>, fallback?: string) => string;
  model: HeroPlayerModel | null;
  pending?: HeroOperationPending | null;
  initialPage?: HeroManagementPage;
  onAction?: (dto: HeroActionDto, lease: HeroUiLease) => void;
  /** Host advances visible/page/actor lifetimes; legacy single-window management uses zero, never a belt lease. */
  windowEpochs?: Readonly<Record<HeroUiOrigin, number>>;
  onReadItemTooltip?: (item: HeroItem) => CrystalTooltipDocument | null;
  windows?: HeroManagementWindows;
  onWindowChange?: (next: HeroManagementWindows) => void;
  onReadStats?: (model: HeroPlayerModel, page: "status" | "state") => readonly HeroStatLine[] | null;
  onClose: () => void;
};

/** Current host window lease, also used by selections, confirmations and drags. */
export function heroUiLeaseCurrent(lease: HeroUiLease | null, windows?: HeroManagementWindows,
  windowEpochs?: Readonly<Record<HeroUiOrigin, number>>, legacyPage: HeroManagementPage = "inventory"): boolean {
  if (!lease || !["inventory", "character", "belt"].includes(lease.origin)
    || !Number.isSafeInteger(lease.epoch) || lease.epoch < 0 || windows && !windowEpochs) return false;
  const epoch = windowEpochs ? windowEpochs[lease.origin] : lease.origin === "belt" ? null : 0;
  if (epoch !== lease.epoch) return false;
  if (lease.origin === "belt") return !!windows?.beltVisible && !!windowEpochs;
  return windows ? (lease.origin === "inventory" ? windows.inventoryOpen : windows.characterOpen)
    : (lease.origin === "inventory" ? legacyPage === "inventory" : legacyPage !== "inventory");
}
export function captureHeroUiLease(origin: HeroUiOrigin, windows?: HeroManagementWindows,
  windowEpochs?: Readonly<Record<HeroUiOrigin, number>>, legacyPage: HeroManagementPage = "inventory"): HeroUiLease | null {
  const epoch = windowEpochs ? windowEpochs[origin] : origin === "belt" ? null : 0;
  if (typeof epoch !== "number" || !Number.isSafeInteger(epoch) || epoch < 0) return null;
  const lease = Object.freeze({ origin, epoch });
  return heroUiLeaseCurrent(lease, windows, windowEpochs, legacyPage) ? lease : null;
}
/** Presentation gesture only. Position changes never acquire or advance a window lease. */
export class HeroWindowDrag {
  private held: Readonly<{ pointerId: number; lease: HeroUiLease; dx: number; dy: number }> | null = null;
  isHeld(): boolean { return this.held !== null; }
  begin(pointerId: number, lease: HeroUiLease | null, pointer: HeroWindowPosition, position: HeroWindowPosition,
    windows?: HeroManagementWindows, windowEpochs?: Readonly<Record<HeroUiOrigin, number>>, legacyPage: HeroManagementPage = "inventory"): boolean {
    if (this.held || !Number.isSafeInteger(pointerId) || pointerId < 0 || !lease
      || ![pointer.x, pointer.y, position.x, position.y].every(Number.isFinite)
      || !heroUiLeaseCurrent(lease, windows, windowEpochs, legacyPage)) return false;
    this.held = Object.freeze({ pointerId, lease: Object.freeze({ ...lease }), dx: pointer.x - position.x, dy: pointer.y - position.y });
    return true;
  }
  validate(windows?: HeroManagementWindows, windowEpochs?: Readonly<Record<HeroUiOrigin, number>>, legacyPage: HeroManagementPage = "inventory"): boolean {
    if (!this.held) return false;
    if (!heroUiLeaseCurrent(this.held.lease, windows, windowEpochs, legacyPage)) { this.cancel(); return false; }
    return true;
  }
  move(pointerId: number, lease: HeroUiLease | null, pointer: HeroWindowPosition,
    windows?: HeroManagementWindows, windowEpochs?: Readonly<Record<HeroUiOrigin, number>>, legacyPage: HeroManagementPage = "inventory"): HeroWindowPosition | null {
    const held = this.held;
    if (!held || !lease || held.pointerId !== pointerId || held.lease.origin !== lease.origin || held.lease.epoch !== lease.epoch) return null;
    if (!this.validate(windows, windowEpochs, legacyPage) || ![pointer.x, pointer.y].every(Number.isFinite)) { this.cancel(); return null; }
    return { x: Math.max(0, pointer.x - held.dx), y: Math.max(0, pointer.y - held.dy) };
  }
  finish(pointerId: number, lease: HeroUiLease | null): boolean {
    if (!this.held || !lease || this.held.pointerId !== pointerId || this.held.lease.origin !== lease.origin || this.held.lease.epoch !== lease.epoch) return false;
    this.cancel(); return true;
  }
  cancel(): void { this.held = null; }
}

type Selection = { source: HeroPlayerModel; cell: HeroCell; lease: HeroUiLease };
type Modal = { kind: "use"; dto: HeroActionDto; lease: HeroUiLease }
  | { kind: "magic"; source: HeroPlayerModel; spell: string; key: number; lease: HeroUiLease };
const equipmentNames = ["Weapon", "Armour", "Helmet", "Torch", "Necklace", "Bracelet", "Bracelet", "Ring", "Ring", "Amulet", "Belt", "Boots", "Stone", "Mount"];
// Row labels follow character_stats::lines; all values come from that shared renderer.
const statusLabels = ["HP", "MP", "AC", "MAC", "DC", "MC", "SC", "Critical rate", "Critical damage", "Attack speed", "Accuracy", "Agility", "Luck"];
const stateLabels = ["Experience", "Bag weight", "Wear weight", "Hand weight", "Magic resist", "Poison resist", "Health recovery", "Spell recovery", "Poison recovery", "Holy", "Freezing", "Poison attack"];
const buttonStyle: CSSProperties = {
  minWidth: 44, minHeight: 44, padding: "8px 10px", border: "1px solid #816a40", borderRadius: 3,
  color: "#ecd8ae", backgroundColor: "#2b241b", cursor: "pointer", font: "inherit",
};
const gridStyle: CSSProperties = { display: "grid", gridTemplateColumns: "repeat(8, minmax(44px, 1fr))", gap: 3 };

/** Render-only window: every mutation leaves as a leased source DTO. */
export function HeroManagementWindow({ t, model, pending = null, initialPage = "inventory", onAction, windowEpochs, onReadItemTooltip, windows, onWindowChange, onReadStats, onClose }: HeroManagementWindowProps) {
  const [page, setPage] = useState<HeroManagementPage>(initialPage);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [modal, setModal] = useState<Modal | null>(null);
  const [personalPage, setPersonalPage] = useState(0);
  const [hpPercent, setHpPercent] = useState("0");
  const [mpPercent, setMpPercent] = useState("0");
  const [activeTooltip, setActiveTooltip] = useState<{ cell: HeroCell; origin: HeroUiOrigin; sourceKey: string; generation: number } | null>(null);
  const managerRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);
  const drag = useRef(new HeroWindowDrag());
  const shownPage = windows ? windows.characterPage : page;
  const locked = !model || !!pending || !onAction;
  const liveUiRef = useRef({ windows, windowEpochs, page });
  liveUiRef.current = { windows, windowEpochs, page };
  const leaseCurrent = (lease: HeroUiLease): boolean => {
    const current = liveUiRef.current;
    return heroUiLeaseCurrent(lease, current.windows, current.windowEpochs, current.page);
  };
  const captureLease = (origin: HeroUiOrigin) => captureHeroUiLease(origin, windows, windowEpochs, page);
  // Each render captures the actual surface; later visibility never upgrades its origin.
  const inventoryLease = captureLease("inventory"), characterLease = captureLease("character");
  const liveSelection = selection && leaseCurrent(selection.lease) && model && selection.source.sourceKey === model.sourceKey
    && selection.source.actor.generation === model.actor.generation ? selection : null;
  const selectedItem = liveSelection ? readItem(liveSelection.source, liveSelection.cell) : null;

  useEffect(() => { setPage(initialPage); }, [initialPage]);
  useEffect(() => { setActiveTooltip(null); }, [page, personalPage, windows?.characterPage,
    windows?.inventoryOpen, windows?.characterOpen, windows?.beltVisible]);
  useEffect(() => { dialogRef.current?.focus(); }, []);
  useEffect(() => {
    const cancel = () => drag.current.cancel();
    const hidden = () => { if (document.visibilityState !== "visible") cancel(); };
    window.addEventListener("blur", cancel); document.addEventListener("visibilitychange", hidden);
    return () => { window.removeEventListener("blur", cancel); document.removeEventListener("visibilitychange", hidden); cancel(); };
  }, []);
  useEffect(() => {
    drag.current.validate(windows, windowEpochs, page);
  }, [windows?.inventoryOpen, windows?.characterOpen, windows?.beltVisible, windows?.characterPage, page,
    windowEpochs?.inventory, windowEpochs?.character, windowEpochs?.belt]);
  useEffect(() => {
    if (!windows) return;
    const kind = windows.characterOpen ? "character" : windows.inventoryOpen ? "inventory" : null;
    if (kind) managerRef.current?.querySelector<HTMLElement>(`[data-hero-window="${kind}"]`)?.focus();
  }, [windows?.inventoryOpen, windows?.characterOpen]);
  useEffect(() => {
    setHpPercent(String(model?.hpPercent ?? 0)); setMpPercent(String(model?.mpPercent ?? 0));
  }, [model?.hpPercent, model?.mpPercent, model?.actor.generation]);
  useEffect(() => {
    if (modal) {
      returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      modalRef.current?.querySelector<HTMLElement>("button:not(:disabled),input:not(:disabled)")?.focus();
    } else { returnFocus.current?.focus(); returnFocus.current = null; }
  }, [!!modal]);

  const send = (action: HeroUiAction, lease: HeroUiLease | null, source = model) => {
    if (locked || !source || !model || !lease || !leaseCurrent(lease)) return;
    if (lease.origin === "belt" && (action.kind !== "use" || ![0, 1].includes(action.slot))) return;
    const dto = captureHeroAction(source, action);
    if (!dto || !heroActionCurrent(dto, model)) return;
    const plan = planHeroAction(source, action);
    if (plan?.confirmationRequired) { setModal({ kind: "use", dto, lease }); return; }
    onAction?.(dto, lease);
  };
  const activate = (cell: HeroCell, lease: HeroUiLease | null) => {
    if (!model || locked || !lease || !leaseCurrent(lease)) return;
    if (lease.origin === "belt") {
      if (cell.grid === "HeroInventory" && [0, 1].includes(cell.slot)) send({ kind: "use", slot: cell.slot }, lease);
      return;
    }
    const item = readItem(model, cell);
    if (!liveSelection || liveSelection.lease.origin !== lease.origin) { if (item) setSelection({ source: model, cell, lease }); return; }
    const from = liveSelection.cell;
    if (from.grid === cell.grid && from.slot === cell.slot) { setSelection(null); return; }
    let action: HeroUiAction | null = null;
    if (item) {
      const merge: HeroUiAction = { kind: "merge", from, to: cell };
      if (planHeroAction(model, merge)) action = merge;
    }
    if (!action && from.grid === "HeroInventory" && cell.grid === "HeroInventory") action = { kind: "move", from: from.slot, to: cell.slot };
    if (!action && from.grid === "HeroInventory" && cell.grid === "HeroEquipment") action = { kind: "equip", from: from.slot, to: cell.slot };
    if (!action && from.grid === "HeroEquipment" && cell.grid === "HeroInventory") action = { kind: "remove", from: from.slot, to: cell.slot };
    if (!action && from.grid === "Inventory" && cell.grid === "HeroInventory") action = { kind: "transfer", from: from.slot, to: cell.slot };
    if (!action && from.grid === "HeroInventory" && cell.grid === "Inventory") action = { kind: "takeBack", from: from.slot, to: cell.slot };
    if (action && planHeroAction(model, action)) send(action, liveSelection.lease, liveSelection.source);
    else if (item) setSelection({ source: model, cell, lease });
  };
  const useSelected = (origin: "inventory" | "character") => {
    if (!liveSelection || liveSelection.lease.origin !== origin) return;
    if (liveSelection.cell.grid === "HeroInventory") send({ kind: "use", slot: liveSelection.cell.slot }, liveSelection.lease, liveSelection.source);
    if (liveSelection.cell.grid === "HeroEquipment") send({ kind: "remove", from: liveSelection.cell.slot }, liveSelection.lease, liveSelection.source);
  };
  const closeModal = () => setModal(null);
  const confirmModal = () => {
    if (!modal || !model || locked || !leaseCurrent(modal.lease)) return;
    if (modal.kind === "use") {
      if (!heroActionCurrent(modal.dto, model) || modal.dto.action.kind !== "use") return;
      send({ ...modal.dto.action, confirmed: true }, modal.lease, model);
    } else send({ kind: "magicKey", spell: modal.spell, key: modal.key }, modal.lease, modal.source);
    setModal(null);
  };
  const onDialogKey = (event: KeyboardEvent<HTMLDivElement>) => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); if (modal) closeModal(); else {
      const kind = event.currentTarget.dataset.heroWindow;
      if (windows && kind) closeWindow(kind as "inventory" | "character" | "belt"); else onClose();
    } }
    if (modal && event.key === "Enter" && !(event.target instanceof HTMLButtonElement)) { event.preventDefault(); confirmModal(); }
    if (event.key === "Tab") trapFocus(event, modal ? modalRef.current : event.currentTarget);
  };
  const cell = (origin: HeroUiOrigin, grid: HeroCell["grid"], slot: number, enabled: boolean, label?: string) => {
    const lease = captureLease(origin);
    const item = model ? readItem(model, { grid, slot }) : null;
    const selected = !!liveSelection && liveSelection.lease.origin === origin && liveSelection.cell.grid === grid && liveSelection.cell.slot === slot;
    const active = !!model && activeTooltip?.sourceKey === model.sourceKey && activeTooltip.generation === model.actor.generation
      && activeTooltip.origin === origin && activeTooltip.cell.grid === grid && activeTooltip.cell.slot === slot;
    const document = active && item ? onReadItemTooltip?.(item) : null;
    const showTooltip = () => { if (model && item) setActiveTooltip({cell:{grid,slot},origin,sourceKey:model.sourceKey,generation:model.actor.generation}); };
    return <div key={`${grid}:${slot}`} data-hero-tooltip-cell="true" style={{position:"relative",minWidth:44}}
      tabIndex={item && (!enabled || locked) ? 0 : undefined} aria-label={item && (!enabled || locked) ? `${item.name} ×${item.count}` : undefined}
      onMouseEnter={showTooltip} onMouseLeave={() => setActiveTooltip(null)}
      onFocus={showTooltip} onBlur={() => setActiveTooltip(null)}>
      <button type="button" data-hero-grid={grid} data-hero-slot={slot}
      aria-label={`${label ?? t("ui.itemSlot", [slot + 1], `Slot ${slot + 1}`)}${item ? `: ${item.name} (${item.count})` : ""}`}
      aria-pressed={selected} disabled={!enabled || locked || !lease} title={document ? undefined : item ? `${item.name} ×${item.count}` : label}
      onClick={() => activate({ grid, slot }, lease)}
      onContextMenu={event => { event.preventDefault(); if (grid === "HeroInventory") send({ kind: "use", slot }, lease); }}
      onDoubleClick={() => grid === "HeroInventory" ? send({ kind: "use", slot }, lease) : grid === "HeroEquipment" ? send({ kind: "remove", from: slot }, lease) : undefined}
      style={{ ...buttonStyle, position: "relative", padding: 2, height: 48, width:"100%",
        opacity: enabled ? 1 : .35, borderColor: selected ? "#f6d373" : "#816a40", backgroundColor: selected ? "#514123" : "#16140f" }}>
      {item && <img src={`/original-ui/Items/${item.icon}.png`} alt="" width={32} height={32} draggable={false} />}
      {item && item.count > 1 && <span style={{ position: "absolute", right: 2, bottom: 0, fontSize: 11 }}>{item.count}</span>}
      {!item && label && <span style={{ fontSize: 10 }}>{label}</span>}
      </button>
      {document ? <OriginalCrystalItemTooltip document={document} align={slot % 8 > 3 ? "left" : "right"} /> : null}
    </div>;
  };
  const modalCurrent = modal && leaseCurrent(modal.lease) && model && (modal.kind === "use" ? heroActionCurrent(modal.dto, model)
    : modal.source.sourceKey === model.sourceKey && modal.source.actor.generation === model.actor.generation);

  const closeWindow = (kind: "inventory" | "character" | "belt") => {
    setModal(null); setSelection(null); setActiveTooltip(null);
    if (!windows) { onClose(); return; }
    onWindowChange?.({ ...windows, ...(kind === "inventory" ? { inventoryOpen: false }
      : kind === "character" ? { characterOpen: false } : { beltVisible: false }) });
  };
  const changePage = (next: HeroManagementPage) => {
    setSelection(null); setActiveTooltip(null);
    if (windows && next !== "inventory") onWindowChange?.({ ...windows, characterPage: next });
    else setPage(next);
  };
  const tabs = (pages: readonly HeroManagementPage[]) => <div role="tablist" aria-label={t("ui.hero", [], "Hero")}
    style={{ display: "flex", flexWrap: "wrap", gap: 4, marginBottom: 8 }} onKeyDown={event => {
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      const index = event.key === "Home" ? 0 : event.key === "End" ? pages.length - 1
        : (pages.indexOf(shownPage) + (event.key === "ArrowRight" ? 1 : pages.length - 1)) % pages.length;
      changePage(pages[index]); event.currentTarget.querySelectorAll<HTMLButtonElement>("button")[index]?.focus();
    }}>
    {pages.map(value => <button key={value} type="button" role="tab" aria-selected={shownPage === value}
      aria-controls={`hero-panel-${value}`} onClick={() => changePage(value)}
      style={{ ...buttonStyle, backgroundColor: shownPage === value ? "#514123" : "#2b241b" }}>
      {t(`ui.${value}`, [], value[0].toUpperCase() + value.slice(1))}
    </button>)}
  </div>;
  const inventoryContent = <div id="hero-panel-inventory" role="tabpanel">
      <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
        <section style={{ flex: "1 1 355px" }} aria-label={t("ui.heroInventory", [], "Hero inventory")}>
          <div style={{ display: "flex", gap: 4, marginBottom: 8 }}>{cell("inventory", "HeroInventory", 0, !!model, "7")}{cell("inventory", "HeroInventory", 1, !!model, "8")}</div>
          <div style={gridStyle}>{Array.from({ length: 40 }, (_, n) => cell("inventory", "HeroInventory", n + 2, !!model && n + 2 < model.inventoryCapacity))}</div>
          <div style={{ display: "flex", gap: 4, marginTop: 8 }}>
            <button type="button" disabled={locked || liveSelection?.lease.origin !== "inventory" || liveSelection?.cell.grid !== "HeroInventory" || !selectedItem} style={buttonStyle} onClick={() => useSelected("inventory")}>{t("ui.use", [], "Use")}</button>
            <button type="button" style={buttonStyle} disabled={!selection} onClick={() => setSelection(null)}>{t("ui.cancel", [], "Cancel")}</button>
          </div>
        </section>
        {model?.personalInventory && <section style={{ flex: "1 1 355px" }} aria-label={t("ui.inventory", [], "Inventory")}>
          <div style={{ display: "flex", gap: 4, marginBottom: 8 }}>{[0, 1].map(n => <button key={n} type="button" disabled={n * 40 >= model.personalInventory!.length}
            style={buttonStyle} aria-pressed={personalPage === n} onClick={() => setPersonalPage(n)}>{t(`ui.bag${n + 1}`, [], `Bag ${n + 1}`)}</button>)}</div>
          <div style={gridStyle}>{Array.from({ length: 40 }, (_, n) => cell("inventory", "Inventory", personalPage * 40 + n, personalPage * 40 + n < model.personalInventory!.length))}</div>
        </section>}
      </div>
      <fieldset disabled={locked || !inventoryLease || !model?.autoPot} style={{ marginTop: 12, borderColor: "#816a40" }}>
        <legend>{t("ui.autoPot", [], "Auto potion")}</legend>
        {([12, 13] as const).map(stat => <form key={stat} onSubmit={event => { event.preventDefault(); send({ kind: "autoPotValue", stat, value: Number(stat === 12 ? hpPercent : mpPercent) }, inventoryLease); }}
          style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 4, marginBottom: 4 }}>
          <label>{stat === 12 ? "HP" : "MP"} <input aria-label={stat === 12 ? "HP %" : "MP %"} type="number" min={0} max={99} step={1}
            value={stat === 12 ? hpPercent : mpPercent} onChange={e => stat === 12 ? setHpPercent(e.target.value) : setMpPercent(e.target.value)} style={{ ...buttonStyle, width: 68 }} /> %</label>
          <button type="submit" style={buttonStyle}>{t("ui.save", [], "Save")}</button>
          <button type="button" style={buttonStyle} disabled={locked || liveSelection?.lease.origin !== "inventory" || liveSelection?.cell.grid !== "HeroInventory" || !selectedItem || !model || !planHeroAction(model, { kind: "autoPotItem", grid: stat === 12 ? "HeroHpItem" : "HeroMpItem", slot: liveSelection.cell.slot })}
            onClick={() => send({ kind: "autoPotItem", grid: stat === 12 ? "HeroHpItem" : "HeroMpItem", slot: liveSelection!.cell.slot }, liveSelection!.lease, liveSelection!.source)}>{t("ui.select", [], "Select potion")}</button>
          <button type="button" style={buttonStyle} onClick={() => send({ kind: "autoPotItem", grid: stat === 12 ? "HeroHpItem" : "HeroMpItem", slot: null }, inventoryLease)}>{t("ui.clear", [], "Clear")}</button>
        </form>)}
      </fieldset>
    </div>;
  const equipmentContent = <div id="hero-panel-equipment" role="tabpanel">
      <div style={{ display: "grid", gridTemplateColumns: "repeat(7, minmax(44px, 1fr))", gap: 4 }}>
        {equipmentNames.map((name, n) => cell("character", "HeroEquipment", n, !!model, t(`ui.equipment${name}`, [], name)))}
      </div>
      <button type="button" disabled={locked || liveSelection?.lease.origin !== "character" || liveSelection?.cell.grid !== "HeroEquipment" || !selectedItem} style={{ ...buttonStyle, marginTop: 8 }} onClick={() => useSelected("character")}>{t("ui.remove", [], "Remove")}</button>
      <div style={{ ...gridStyle, marginTop: 8 }}>{Array.from({ length: 40 }, (_, n) => cell("character", "HeroInventory", n + 2, !!model && n + 2 < model.inventoryCapacity))}</div>
    </div>;
  const skillsContent = <div id="hero-panel-skills" role="tabpanel">
      {model?.magics.map(m => <div key={m.spell} style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
        <img alt="" src={`/original-ui/MagIcon/${m.icon}.png`} width={32} height={32} />
        <span style={{ flex: 1 }}>{m.name} · {t("ui.heroLevel", [m.level], `Level ${m.level}`)}</span>
        <button type="button" disabled={locked || !characterLease || model.hp <= 0} style={buttonStyle} data-hero-spell={m.spell}
          onClick={() => { if (characterLease && leaseCurrent(characterLease)) setModal({ kind: "magic", source: model, spell: m.spell, key: m.key, lease: characterLease }); }}>{m.key ? `Shift+F${m.key - 16}` : t("ui.assignKey", [], "Assign key")}</button>
      </div>)}
    </div>;
  const statsPage = shownPage === "status" || shownPage === "state" ? shownPage : null;
  const statsLines = model && statsPage && (!windows || windows.characterOpen) ? onReadStats?.(model, statsPage) : null;
  const characterContent = <>
    {shownPage === "equipment" && equipmentContent}
    {shownPage === "skills" && skillsContent}
    {statsPage && <div id={`hero-panel-${statsPage}`} role="tabpanel" data-hero-stats-page={statsPage}>
      {statsLines ? <dl>{statsLines.map((line, n) => <div key={n} style={{ display: "flex", justifyContent: "space-between", gap: 12, margin: "6px 0", color: line.color }}>
        <dt>{t(`ui.hero${statsPage}Row${n}`, [], (statsPage === "status" ? statusLabels : stateLabels)[n] ?? "")}</dt>
        <dd style={{ margin: 0 }}>{line.text}</dd>
      </div>)}</dl>
        : <p role="status">{t("ui.heroStatsUnavailable", [], "Hero statistics are unavailable.")}</p>}
    </div>}
  </>;
  const summary = <>{!model && <p role="status">{t("ui.heroUnavailable", [], "Hero information is unavailable.")}</p>}
    {model && <p>{t("ui.heroLevel", [model.level], `Level ${model.level}`)} · HP {model.hp} · MP {model.mp}</p>}
    {pending && <p role="status">{t("ui.waiting", [], "Waiting for the server…")}</p>}</>;
  const frame = (kind: "inventory" | "character" | "belt", content: ReactNode,
    position: HeroWindowPosition | null | undefined, fallback: HeroWindowPosition, width: number) => {
    const location = position ?? fallback;
    const frameLease = captureLease(kind);
    return <div key={kind} role="dialog" aria-label={t(kind === "belt" ? "ui.heroBelt" : "ui.hero", [], kind === "belt" ? "Hero belt" : "Hero")}
      tabIndex={-1} data-hero-window={kind} data-hero-management-window={kind !== "belt" ? "true" : undefined}
      data-hero-belt-window={kind === "belt" ? "true" : undefined} onKeyDown={onDialogKey}
      onPointerDown={event => event.stopPropagation()} onPointerUp={event => event.stopPropagation()}
      style={{ color: "#ecd8ae", background: "#211c14", border: "2px solid #8c713e", padding: 8,
        width: `min(${width}px, calc(100vw - 24px))`, maxHeight: "calc(100vh - 24px)", overflow: "auto",
        position: "fixed", left: location.x, top: location.y, pointerEvents: "auto" }}>
      <div style={{ display: "flex", flexWrap: kind === "belt" ? "wrap" : undefined, alignItems: "center", justifyContent: "space-between", gap: 4 }}
        onPointerDown={event => {
          if (!windows || !onWindowChange || event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
          const current = liveUiRef.current;
          if (!drag.current.begin(event.pointerId, frameLease, { x: event.clientX, y: event.clientY }, location,
            current.windows, current.windowEpochs, current.page)) return;
          event.currentTarget.setPointerCapture(event.pointerId); event.preventDefault();
        }} onPointerMove={event => {
          const current = liveUiRef.current;
          const next = drag.current.move(event.pointerId, frameLease, { x: event.clientX, y: event.clientY },
            current.windows, current.windowEpochs, current.page);
          if (!next || !current.windows) return;
          onWindowChange?.({ ...current.windows, [`${kind}Position`]: next });
        }} onPointerUp={event => { drag.current.finish(event.pointerId, frameLease); }}
        onPointerCancel={event => { drag.current.finish(event.pointerId, frameLease); }}
        onLostPointerCapture={event => { drag.current.finish(event.pointerId, frameLease); }}>
        <strong>{kind === "belt" ? t("ui.heroBelt", [], "Hero belt") : model?.actor.name ?? t("ui.hero", [], "Hero")}</strong>
        {kind === "belt" && <button type="button" style={buttonStyle} aria-label={t("ui.rotate", [], "Rotate")}
          onClick={() => windows && onWindowChange?.({ ...windows, beltVertical: !windows.beltVertical, beltPosition: null })}>↻</button>}
        <button type="button" style={buttonStyle} aria-label={t("ui.close", [], "Close")} onClick={() => closeWindow(kind)}>×</button>
      </div>
      {content}
    </div>;
  };
  const modalContent = modal && <div style={{ position: "fixed", inset: 0, pointerEvents: "auto", zIndex: 2, background: "#000a", display: "grid", placeItems: "center", padding: 12 }}>
      <div ref={modalRef} role="alertdialog" aria-modal="true" onKeyDown={onDialogKey}
        onPointerDown={event => event.stopPropagation()} onPointerUp={event => event.stopPropagation()} aria-label={modal.kind === "use" ? t("ui.confirm", [], "Confirm") : t("ui.assignKey", [], "Assign key")}
        style={{ background: "#211c14", border: "2px solid #8c713e", padding: 16, maxWidth: 460 }}>
        <p>{modal.kind === "use" ? t("client.ConfirmItemUse", [], "Use this potion?") : modal.source.magics.find(m => m.spell === modal.spell)?.name}</p>
        {modal.kind === "magic" && <div role="group" aria-label={t("ui.assignKey", [], "Assign key")} style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
          {[0, 17, 18, 19, 20, 21, 22, 23, 24].map(k => <button key={k} type="button" style={buttonStyle} aria-pressed={modal.key === k}
            onClick={() => setModal({ ...modal, key: k })}>{k ? `Shift+F${k - 16}` : t("ui.none", [], "None")}</button>)}
        </div>}
        <div style={{ display: "flex", gap: 4, marginTop: 8 }}>
          <button type="button" style={buttonStyle} disabled={locked || !modalCurrent || (modal.kind === "magic" && !planHeroAction(modal.source, { kind: "magicKey", spell: modal.spell, key: modal.key }))} onClick={confirmModal}>{t("ui.confirm", [], "Confirm")}</button>
          <button type="button" style={buttonStyle} onClick={closeModal}>{t("ui.cancel", [], "Cancel")}</button>
        </div>
      </div>
    </div>;
  const tooltipStyle = <style>{'[data-hero-tooltip-cell] > .original-item-tooltip { opacity: 1; visibility: visible; transform: translateY(0); }'}</style>;
  if (windows) return <div ref={managerRef} data-hero-window-manager="true" style={{ position: "fixed", inset: 0, pointerEvents: "none" }}>
    {tooltipStyle}
    {windows.inventoryOpen && frame("inventory", <>{summary}{inventoryContent}</>, windows.inventoryPosition, { x: 12, y: 80 }, 780)}
    {windows.characterOpen && frame("character", <>{summary}{tabs(["equipment", "status", "state", "skills"])}{characterContent}</>,
      windows.characterPosition, { x: 24, y: 100 }, 520)}
    {windows.beltVisible && model && frame("belt", <div style={{ display: "flex", gap: 4, flexDirection: windows.beltVertical ? "column" : "row" }}>
      {cell("belt", "HeroInventory", 0, true, "7")}{cell("belt", "HeroInventory", 1, true, "8")}
    </div>, windows.beltPosition, windows.beltVertical ? { x: 0, y: 446 } : { x: 475, y: 618 }, windows.beltVertical ? 108 : 160)}
    {modalContent}
  </div>;
  return <div ref={dialogRef} role="dialog" aria-modal="true" aria-label={t("ui.hero", [], "Hero")}
    tabIndex={-1} data-hero-management-window="true" onKeyDown={onDialogKey}
    onPointerDown={event => event.stopPropagation()} onPointerUp={event => event.stopPropagation()}
    style={{ color: "#ecd8ae", background: "#211c14", border: "2px solid #8c713e", padding: 12,
      width: "min(780px, calc(100vw - 24px))", maxHeight: "calc(100vh - 24px)", overflow: "auto", position: "relative" }}>
    {tooltipStyle}
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
      <strong>{model?.actor.name ?? t("ui.hero", [], "Hero")}</strong>
      <button type="button" onClick={onClose} aria-label={t("ui.close", [], "Close")}
        style={{ ...buttonStyle, backgroundImage: `url(${ORIGINAL_UI.character.closeButton.base})`, backgroundRepeat: "no-repeat", backgroundPosition: "center" }}>×</button>
    </div>
    {summary}{tabs(["inventory", "equipment", "status", "state", "skills"])}
    {page === "inventory" ? inventoryContent : characterContent}
    {modalContent}
  </div>;
}

function readItem(model: HeroPlayerModel, cell: HeroCell): HeroItem | null {
  return (cell.grid === "HeroInventory" ? model.inventory : cell.grid === "HeroEquipment" ? model.equipment : model.personalInventory)?.[cell.slot] ?? null;
}
function trapFocus(event: KeyboardEvent<HTMLDivElement>, container: HTMLDivElement | null) {
  const elements = container?.querySelectorAll<HTMLElement>("button:not(:disabled),input:not(:disabled),[tabindex='0']");
  if (!elements?.length) { event.preventDefault(); container?.focus(); return; }
  const first = elements[0], last = elements[elements.length - 1];
  if (event.shiftKey && (document.activeElement === first || !container?.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
  else if (!event.shiftKey && (document.activeElement === last || !container?.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
}
