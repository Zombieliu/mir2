"use client";

import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { Mir2Language } from "../../lib/localization";
import { r16CompanionNotice, r16CompanionPresentation, type R16CompanionCopy, type R16CompanionMessage } from "../../lib/r16-companion-localization";
import { R16CompanionClient, r16CompanionCanAuthorizeLife, r16CompanionModelReady, r16CompanionSupplyComplete, r16CompanionTeachingId,
  r16CompanionInstruction, r16CompanionCancelTeachingId, r16CompanionResumeLifeId, formatR16CompanionMicros,
  R16_COMPANION_ENTRY_SOURCES, R16_COMPANION_CHOICE_REASONS } from "../../lib/r16-companion-client";
import type { R16CompanionAction, R16CompanionOwnerView, R16CompanionRenderLease, R16CompanionTask } from "../../lib/r16-companion-client";

/** R16 crystal_ui/companion.rs logical 1024x768 stage, not viewport pixels. */
export const R16_COMPANION_LAYOUT = {
  entry: { left: 872, top: 174, width: 140, height: 29 },
  panel: { left: 140, top: 44, width: 744, height: 644 },
  tabs: ["tabCultivation", "tabMemory", "tabActions", "tabInstruction", "tabSupply", "tabLife"],
} as const;
export type R16CompanionWindowProps = {
  client: R16CompanionClient; language: Mir2Language; open: boolean; onOpenChange(open: boolean): void;
  entryEnabled?: boolean; entryVisible?: boolean; partyMember?: boolean;
  /** Host must call client.invitation(lease) immediately before its existing
   * party sender and use the returned current characterName. No optimistic party. */
  onInviteParty?(lease: R16CompanionRenderLease, characterName: string): boolean;
  /** Opens existing Node settings only. No key, endpoint or model SDK in this window. */
  onOpenModelSettings?(lease: R16CompanionRenderLease): void;
};
const styles = `
.r16-companion { position:absolute; inset:0; pointer-events:none; color:rgb(235,222,194); font-size:14px; z-index:30000; }
.r16-companion * { box-sizing:border-box; }
.r16-companion .backdrop { position:absolute; inset:0; background:rgba(0,0,0,.54); pointer-events:auto; }
.r16-companion .panel { position:absolute; border:2px solid rgb(209,176,112); background:rgb(14,11,7); overflow:hidden; pointer-events:auto; }
.r16-companion button { position:absolute; height:29px; padding:0 4px; border:1px solid rgb(209,176,112); border-radius:0; background:rgb(41,28,15); color:rgb(235,222,194); font:inherit; cursor:pointer; }
.r16-companion :is(button,.download) { display:flex; align-items:center; justify-content:center; text-align:center; font-size:12px; line-height:12px; overflow-wrap:anywhere; white-space:normal; }
.r16-companion button:disabled { background:rgb(20,18,15); color:rgb(166,158,138); cursor:default; }
.r16-companion button:not(:disabled):is(:hover,:focus-visible),.r16-companion button[aria-selected=true],.r16-companion button[aria-pressed=true] { border-color:rgb(255,222,115); background:rgb(71,51,26); }
.r16-companion :is(button,textarea,a):focus-visible { outline:1px solid #ffde73; outline-offset:1px; }
.r16-companion .copy { position:absolute; margin:0; white-space:pre-wrap; overflow:hidden; overflow-wrap:anywhere; font-weight:400; }
.r16-companion .gold { color:rgb(209,176,112); }
.r16-companion .muted { color:rgb(166,158,138); }
.r16-companion textarea { position:absolute; resize:none; border:1px solid rgb(166,158,138); border-radius:0; padding:8px; background:rgb(23,19,13); color:rgb(235,222,194); font:inherit; font-size:15px; line-height:20px; }
.r16-companion .download { position:absolute; left:310px; top:602px; width:164px; height:29px; border:1px solid rgb(209,176,112); color:rgb(235,222,194); background:rgb(41,28,15); text-decoration:none; }
`;
const at = (left: number, top: number, width: number, height?: number): CSSProperties => ({ left, top, width, ...(height === undefined ? {} : { height }) });
function Copy({ x, y, w = 690, h = 28, size = 14, lineHeight = Math.max(size + 5, 20), tone = "", dir, children }: { x: number; y: number; w?: number; h?: number; size?: number; lineHeight?: number; tone?: string; dir?: "auto"; children: ReactNode }) {
  return <p className={`copy ${tone}`} dir={dir} style={{ ...at(x, y, w, h), fontSize: size, lineHeight: `${lineHeight}px` }}>{children}</p>;
}
function Button({ id, label, x, y, w, disabled = false, selected, onClick }: { id: string; label: string; x: number; y: number; w: number; disabled?: boolean; selected?: boolean; onClick(): void }) {
  return <button type="button" data-r16-companion-action={id} style={at(x, y, w)} disabled={disabled} role={id.startsWith("tab-") ? "tab" : undefined}
    aria-selected={id.startsWith("tab-") ? selected : undefined} aria-pressed={id.startsWith("tab-") ? undefined : selected}
    onClick={() => { if (!disabled) onClick(); }}>{label}</button>;
}
const yes = (value: boolean, copy: R16CompanionCopy) => copy(value ? "yes" : "no");
const count = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const shown = (value: unknown, copy: R16CompanionCopy) => count(value) ? String(value) : copy("unknown");
const activeSupply = (v: R16CompanionOwnerView) => ["authorized", "running", "paused", "unknown", "returning"].includes(String(v.runtime.supplyTask.status));
const activeLife = (v: R16CompanionOwnerView) => !!v.runtime.lifeTask.status && !["idle", "ended", "completed", "cancelled", "failed", "expired"].includes(String(v.runtime.lifeTask.status));

const confirmationKeys: Partial<Record<R16CompanionAction["type"], R16CompanionMessage>> = {
  clearGrowth: "confirmClear", revokeFeedback: "confirmRevoke", deleteFeedback: "confirmDeleteFeedback",
  deleteEpisode: "confirmDeleteEpisode", authorizeSupply: "confirmSupply", resumeLife: "confirmResumeLife",
};
function confirmText(action: R16CompanionAction, copy: R16CompanionCopy) {
  return action.type === "authorizeLife" ? copy("confirmLife", action.durationSeconds / 60)
    : copy(confirmationKeys[action.type] ?? "confirmPrompt");
}

function mappedLabel(value: unknown, keys: Readonly<Record<string, R16CompanionMessage>>, fallback: R16CompanionMessage, copy: R16CompanionCopy) {
  return copy(typeof value === "string" && Object.hasOwn(keys, value) ? keys[value] : fallback);
}

const supplyStatuses = {
  authorized: "supplyAuthorized", running: "supplyRunning", paused: "supplyPaused", unknown: "supplyUnknown",
  returning: "supplyReturning", completed: "supplyUnproved", cancelled: "supplyCancelled", failed: "supplyFailed",
} as const;
function supplyStatus(task: R16CompanionTask, copy: R16CompanionCopy): string {
  return r16CompanionSupplyComplete(task) ? copy("supplyComplete") : mappedLabel(task.status, supplyStatuses, "supplyIdle", copy);
}

// Keep the Windows R16 task states separate from observations of actual purchases.
function lifePhase(task: R16CompanionTask): R16CompanionMessage {
  switch (task.status) {
    case "idle": return "idle";
    case "paused": return "paused";
    case "unknown": return "pausedUnknown";
    case "ended": case "completed": case "cancelled": case "expired": case "failed": return "ended";
  }
  if (!Object.keys(task).length) return "idle";
  switch (task.phase) {
    case "explore": case "exploring": case "exploration": case "patrolling": return "exploring";
    case "combat": case "fight": case "fighting": case "hunt": case "hunting": return "combat";
    case "return": case "returning": case "return-home": case "returning-home": case "retreat": return "returning";
    case "supply": case "purchase": case "resupply": case "buying": case "awaiting-supply": case "claiming-supply": case "supplying": return "supply";
    case "rest": case "resting": case "recovering": return "rest";
    case "awaiting-owner": return "awaitOwner";
    case "awaiting-observation": return "awaitObservation";
    case "awaiting-server-activation": return "awaitActivation";
    case "idle": case "wait": case "waiting": case "standby": return "idle";
    case "paused": return "paused";
    case "ended": case "completed": case "finished": return "ended";
    default: return "phaseUnknown";
  }
}

function lifeModel(task: R16CompanionTask): R16CompanionMessage {
  if (task.status === "paused") return "paused";
  if (task.activationPending === true) return "awaitActivation";
  switch (typeof task.modelStatus === "string" ? task.modelStatus : task.decisionMode) {
    case "rules": return "modelRules";
    case "planning": case "thinking": case "model-planning": return "planning";
    case "model": return "modelPlanned";
    case "running": case "active": return "running";
    case "paused": return "paused";
    case "disabled": case "off": return "modelOff";
    case "failed": return "unavailable";
    case "idle": return "idle";
    default: return "unknown";
  }
}

const modelTaskLabels = {
  idle: "idle", planning: "planning", running: "running", paused: "paused", completed: "ended", failed: "unavailable",
} as const;
const feedbackLabels = { active: "active", revoked: "revoked", deleted: "deleted" } as const;
const habitLabels = { assistBeforeLoot: "habitAssist", lootOnlyAfterCombat: "habitLoot", askBeforeLeaving: "habitLeave" } as const;
const habitNames = Object.keys(habitLabels) as (keyof typeof habitLabels)[];

function lifePresentation({ lifeTask: task, resources }: R16CompanionOwnerView["runtime"], now: number, copy: R16CompanionCopy) {
  const stoppedPending = ["ended", "completed", "cancelled", "expired"].includes(String(task.status))
    && (resources.medicineUnresolved !== false || task.reservedGold !== 0);
  const status = copy(stoppedPending ? "lifeStoppedPending" : lifePhase(task));
  const expiry = !count(task.expiresAtMs) ? copy("expiryUnknown") : now === 0 ? copy("clockUnknown") : task.expiresAtMs <= now ? copy("expired")
    : copy("expiryEstimate", Math.ceil((task.expiresAtMs - now) / 60_000));
  const quantity = (value: unknown) => shown(value, copy);
  const budget = !Object.keys(task).length || task.status === "idle" ? copy("lifeNoBudget")
    : copy("lifeBudget", quantity(task.budgetGold), quantity(task.remainingBudgetGold), quantity(task.reservedGold), quantity(task.chargedGold),
      quantity(task.purchasedQuantity), quantity(task.maxPurchasedQuantity), quantity(task.killsObserved), expiry);
  let resourceLines = copy("lifeResourcesUnknown");
  if (resources.known === true && count(resources.observedAtMs)) {
    const inventory = resources.inventoryCurrent === true;
    const item = (v: unknown) => inventory ? quantity(v) : copy("unknown");
    const minutes = Math.max(0, Math.floor((now - resources.observedAtMs) / 60_000));
    const observed = resources.observedAtMs > now + 60_000 ? copy("timeSkew") : minutes < 1 ? copy("justNow") : minutes < 60 ? copy("minutesAgo", minutes)
      : minutes < 1440 ? copy("hoursAgo", Math.floor(minutes / 60)) : copy("daysAgo", Math.floor(minutes / 1440));
    resourceLines = copy("lifeResources", quantity(resources.gold), quantity(resources.hp), quantity(resources.maxHp), item(resources.hpPotionCount),
      item(resources.usableHpPotionCount), item(resources.freeBagSlots), item(resources.currentWeight), item(resources.maxWeight), copy(inventory ? "checked" : "unchecked"), observed);
  }
  const reason = typeof task.pausedReason === "string" && task.pausedReason ? task.pausedReason
    : typeof task.reason === "string" ? task.reason : copy("reasonMissing");
  const boundedReason = Array.from(reason.replace(/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu, "")).slice(0, 45).join("");
  const summary = !count(task.killsObserved) && !count(task.chargedGold) ? copy("lifeNoSummary")
    : copy("lifeObserved", quantity(task.killsObserved), quantity(task.chargedGold));
  const warning = copy(resources.medicineUnresolved === true ? "lifePending" : resources.medicineUnresolved === false ? "lifeConfirmed" : "lifeConfirmPending");
  return { status, budget, resourceLines, notes: copy("lifeNotes", copy(lifeModel(task)), boundedReason, summary), warning };
}

type SubpageProps = { copy: R16CompanionCopy; view: R16CompanionOwnerView; mutable: boolean; act(action: R16CompanionAction): void; back(): void };
function TrialPage({ copy, view, mutable, act, back }: SubpageProps) {
  const [entrySource, setEntrySource] = useState<typeof R16_COMPANION_ENTRY_SOURCES[number] | null>(null);
  const [reminderCount, setReminderCount] = useState(0);
  const [choice, setChoice] = useState<typeof R16_COMPANION_CHOICE_REASONS[number] | null>(null);
  const active = !!view.trial?.activeVisitId, enabled = mutable && !!view.trial;
  const shownSource = active ? view.trial?.activeEntrySource : entrySource;
  const shownReminders = active ? view.trial?.activeReminderCount ?? copy("notRecorded") : reminderCount;
  return <>
    <Copy x={22} y={180} h={26} tone="gold">{copy("trialHeading")}</Copy>
    {R16_COMPANION_ENTRY_SOURCES.map((source, i) => <Button key={source} id={`trial-source-${source}`} label={copy((["trialSelf", "trialReminder", "trialScheduled", "trialUnknown"] as const)[i])}
      x={22 + i * 174} y={216} w={162} selected={shownSource === source} disabled={!enabled || active} onClick={() => setEntrySource(source)} />)}
    <Copy x={22} y={256} w={400} h={26}>{copy("trialReminderCount", shownReminders)}</Copy>
    <Button id="trial-reminder-down" label="−" x={540} y={252} w={78} disabled={!enabled || active || reminderCount === 0} onClick={() => setReminderCount(reminderCount - 1)} />
    <Button id="trial-reminder-up" label="+" x={634} y={252} w={78} disabled={!enabled || active || reminderCount === 100} onClick={() => setReminderCount(reminderCount + 1)} />
    <Button id={active ? "trial-end" : "trial-start"} label={copy(active ? "trialEnd" : "trialStart")} x={22} y={291} w={690} disabled={!enabled}
      onClick={() => act({ type: "trialEvent", event: active ? { kind: "visitEnd" } : { kind: "visit", entrySource: entrySource ?? "unknown", reminderCount } })} />
    <Copy x={22} y={332} h={23}>{copy("trialReasonHeading")}</Copy>
    {R16_COMPANION_CHOICE_REASONS.map((reason, i) => <Button key={reason} id={`trial-reason-${reason}`} label={copy((["trialHistory", "trialStronger", "trialConvenience", "trialCuriosity", "trialOther"] as const)[i])}
      x={22 + i * 140} y={360} w={130} selected={choice === reason} disabled={!enabled || !active} onClick={() => setChoice(reason)} />)}
    <Copy x={22} y={401} h={30} size={13} tone="muted">{copy(view.trial ? "trialNote" : "trialUnavailable")}</Copy>
    <Button id="trial-save-choice" label={copy("trialSave")} x={22} y={443} w={690} disabled={!enabled || !active || !choice}
      onClick={() => { if (choice) act({ type: "trialEvent", event: { kind: "choice", reason: choice } }); }} />
    <Button id="trial-understood" label={copy("trialUnderstand")} x={22} y={484} w={335} disabled={!enabled || !active}
      onClick={() => act({ type: "trialEvent", event: { kind: "teachingCheck", understood: true } })} />
    <Button id="trial-not-understood" label={copy("trialNotUnderstood")} x={377} y={484} w={335} disabled={!enabled || !active}
      onClick={() => act({ type: "trialEvent", event: { kind: "teachingCheck", understood: false } })} />
    <Button id="back-trial" label={copy("trialBack")} x={22} y={521} w={690} onClick={back} />
  </>;
}

function CostsPage({ copy, view, mutable, act, back }: SubpageProps) {
  const trial = view.trial, usage = trial?.usage, costs = trial?.costs, offer = trial?.offer;
  const measure = (v: unknown) => typeof v === "number" ? String(v) : copy("costMeasure");
  const money = (v: unknown) => formatR16CompanionMicros(v, costs?.currency) ?? copy("costMeasure");
  const mode = copy(view.runtime.modelEnabled ? "costModeEnabled" : view.runtime.provider ? "costModeRules" : "costModeUnknown");
  return <>
    <Copy x={22} y={180} h={28} size={16} lineHeight={20} tone="gold">{copy("costHeading")}</Copy>
    <Copy x={22} y={212} h={32}>{copy("costUsage", measure(usage?.runtimeSeconds), measure(usage?.modelCalls), measure(usage?.confirmedAdventureCount))}</Copy>
    <Copy x={22} y={255} h={94}>{copy("costValues", money(costs?.modelMicros), money(costs?.hostingMicros),
      money(costs?.paymentMicros), money(costs?.revenueShareMicros), costs?.complete === true ? money(costs.contributionMicros) : copy("costContributionUnknown"), measure(costs?.supportSeconds))}</Copy>
    <Copy x={22} y={350} h={24} size={12} tone="muted">{copy("costRuntime", mode)}</Copy>
    <Copy x={22} y={386} h={28} tone="gold">{offer ? copy("costOffer", offer.price, offer.durationDays, offer.adventureCount) : copy("costNoOffer")}</Copy>
    <Copy x={22} y={420} h={52} size={13} dir="auto">{offer?.paymentInstructions || copy("costService")}</Copy>
    <Button id="trial-purchase-intent" label={copy("costIntent")} x={22} y={477} w={690} disabled={!mutable || !offer || !trial?.activeVisitId}
      onClick={() => act({ type: "trialEvent", event: { kind: "purchaseIntent" } })} />
    <Button id="back-costs" label={copy("costBack")} x={22} y={518} w={690} onClick={back} />
  </>;
}

export function R16CompanionWindow({ client, language, open, onOpenChange, entryEnabled = true, entryVisible = true, partyMember = false, onInviteParty, onOpenModelSettings }: R16CompanionWindowProps) {
  const presentation = r16CompanionPresentation(language);
  const { copy } = presentation;
  const date = (value: number | null) => value === null ? copy("timeUnknown") : new Date(value).toLocaleString(presentation.locale);
  const snapshot = useSyncExternalStore(client.subscribe, client.snapshot, client.snapshot);
  const [tab, setTab] = useState(0), [page, setPage] = useState(0);
  const [subpage, setSubpage] = useState<"trial" | "teaching" | "costs" | null>(null);
  const teaching = subpage === "teaching";
  const [draftValue, setDraftValue] = useState<{ client: R16CompanionClient; epoch: number; text: string } | null>(null);
  // A new owner/view cannot briefly paint the previous owner's input before a passive effect runs.
  const draft = draftValue?.client === client && draftValue.epoch === snapshot.contentEpoch ? draftValue.text : "";
  const [duration, setDuration] = useState<1200 | 1800>(1200);
  const [confirmation, setConfirmationValue] = useState<{ lease: R16CompanionRenderLease; action: R16CompanionAction } | null>(null);
  const panelRef = useRef<HTMLDivElement>(null), composing = useRef(false), editing = useRef(false);
  const activityRef = useRef({ open, draft: false, confirming: false });
  activityRef.current = { open, draft: !!draft, confirming: !!confirmation };
  const setDraft = (text: string) => { activityRef.current.draft = !!text; setDraftValue({ client, epoch: snapshot.contentEpoch, text }); };
  const setConfirmation = (next: typeof confirmation) => { activityRef.current.confirming = !!next; setConfirmationValue(next); };
  const lease = snapshot.renderLease, view = snapshot.view;
  const selected = snapshot.enrollments.find(e => e.residentId === snapshot.selected);
  const mutable = !!view && !!selected?.online && !snapshot.busy && !snapshot.uncertain;
  const life = tab === 5 && view ? lifePresentation(view.runtime, Date.now(), copy) : null;
  const canResumeLife = !!life && !!view && r16CompanionResumeLifeId(view) !== null;
  useEffect(() => { const timer = setInterval(() => client.tick(), 1000); return () => clearInterval(timer); }, [client]);
  useEffect(() => {
    if (!open) return;
    const timer = setInterval(() => {
      const activity = activityRef.current, current = client.snapshot();
      if (!activity.open || activity.draft || activity.confirming || composing.current || editing.current || current.busy || !current.available) return;
      client.poll(current.renderLease);
    }, 2500);
    return () => clearInterval(timer);
  }, [client, open]);
  useEffect(() => {
    client.syncSource();
    if (open && client.snapshot().available && !client.snapshot().busy) client.refresh(client.snapshot().renderLease);
  }, [client, open, snapshot.available]);
  useEffect(() => {
    setDraftValue(null); setConfirmationValue(null); composing.current = false; editing.current = false; setPage(0);
  }, [client, snapshot.contentEpoch, open]);
  useEffect(() => {
    if (!open) return;
    panelRef.current?.querySelector<HTMLElement>(confirmation ? '[data-r16-companion-confirm] button:not(:disabled)' : 'button:not(:disabled)')?.focus({ preventScroll: true });
  }, [open, !!confirmation]);
  const close = () => { activityRef.current.open = false; setDraft(""); setConfirmation(null); composing.current = false; editing.current = false; onOpenChange(false); };
  const act = (action: R16CompanionAction) => { client.act(lease, action); };
  const prepare = (action: R16CompanionAction) => { if (mutable && client.isCurrentLease(lease, true)) setConfirmation({ lease, action }); };
  const prepareResumeLife = () => {
    const lifePermitId = view && r16CompanionResumeLifeId(view);
    if (lifePermitId) prepare({ type: "resumeLife", lifePermitId });
  };
  const changeTab = (value: number) => { editing.current = false; setTab(value); setPage(0); setSubpage(null); setDraft(""); setConfirmation(null); };
  const sendText = () => { if (!composing.current) act({ type: teaching ? "proposeTeaching" : "instruct", text: draft }); };
  const rows = tab === 1 ? view?.feedback ?? [] : view?.episodes ?? [];
  const pageCount = Math.max(1, Math.ceil(rows.length / 4)), safePage = Math.min(page, pageCount - 1);
  const pageButtons = () => <>
    <Button id="previous-page" label={copy("previousPage")} x={360} y={482} w={166} disabled={safePage === 0} onClick={() => setPage(safePage - 1)} />
    <Button id="next-page" label={copy(safePage + 1 < pageCount ? "nextPage" : "older")} x={540} y={482} w={174}
      disabled={snapshot.busy || safePage + 1 >= pageCount && !view?.nextCursor}
      onClick={() => { if (safePage + 1 < pageCount) setPage(safePage + 1); else if (view?.nextCursor) client.refresh(lease, view.nextCursor); }} />
  </>;
  return <section className="r16-companion" lang={presentation.language} dir={presentation.dir}
    style={{ fontFamily: `"${presentation.fontFamily}", "Noto Sans SC", sans-serif` }} data-r16-companion-open={open} onPointerDown={event => event.stopPropagation()} onClick={event => event.stopPropagation()}>
    <style>{styles}</style>
    {!open ? entryVisible && <button type="button" style={{ ...R16_COMPANION_LAYOUT.entry, pointerEvents: "auto" }} data-r16-companion-action="open" disabled={!entryEnabled || !snapshot.available}
      onClick={() => { if (entryEnabled && client.isCurrentLease(lease)) onOpenChange(true); }}>{copy("entry")}</button> : <>
      <div className="backdrop" />
      <div className="panel" style={R16_COMPANION_LAYOUT.panel} ref={panelRef} role="dialog" aria-modal="true" aria-labelledby="r16-companion-title" aria-busy={snapshot.busy}
        onKeyDown={event => {
          event.stopPropagation();
          if (event.nativeEvent.isComposing || event.keyCode === 229 || composing.current) return;
          if (event.key === "Escape") { event.preventDefault(); if (confirmation) setConfirmation(null); else if (subpage) setSubpage(null); else close(); }
          if (event.key === "Tab") {
            const root = confirmation ? panelRef.current?.querySelector<HTMLElement>("[data-r16-companion-confirm]") : panelRef.current;
            const fields = Array.from(root?.querySelectorAll<HTMLElement>('button:not(:disabled),textarea:not(:disabled),a[href]') ?? []);
            const first = fields[0], last = fields.at(-1);
            if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
            else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
          }
        }}>
        <h2 id="r16-companion-title" className="copy gold" style={{ ...at(18, 12, 470, 32), fontSize: 20, lineHeight: "24px" }}>{copy("title")}</h2>
        <Button id="close" label={copy("close")} x={638} y={10} w={88} onClick={close} />
        <Button id="model-settings" label={copy("modelSettings")} x={506} y={10} w={118} disabled={!onOpenModelSettings || !snapshot.available}
          onClick={() => { if (client.isCurrentLease(lease)) onOpenModelSettings?.(lease); }} />
        <Copy x={18} y={50} w={530} h={25} tone="muted">{selected
          ? copy("enrollment", selected.displayName, copy(selected.online ? "online" : "offline"), snapshot.enrollments.indexOf(selected) + 1, snapshot.enrollments.length)
          : copy("noEnrollments")}</Copy>
        <Button id="previous-resident" label={copy("previousResident")} x={558} y={47} w={78} disabled={snapshot.busy || snapshot.enrollments.length < 2}
          onClick={() => { const index = snapshot.enrollments.indexOf(selected!); client.choose(lease, snapshot.enrollments[(index + snapshot.enrollments.length - 1) % snapshot.enrollments.length].residentId); }} />
        <Button id="next-resident" label={copy("nextResident")} x={646} y={47} w={78} disabled={snapshot.busy || snapshot.enrollments.length < 2}
          onClick={() => { const index = snapshot.enrollments.indexOf(selected!); client.choose(lease, snapshot.enrollments[(index + 1) % snapshot.enrollments.length].residentId); }} />
        <Button id="refresh" label={copy("refresh")} x={18} y={87} w={112} disabled={snapshot.busy || !snapshot.available} onClick={() => client.refresh(lease)} />
        <Button id="invite" label={copy(partyMember ? "inParty" : "invite")} x={142} y={87} w={112} disabled={!mutable || partyMember || !onInviteParty}
          onClick={() => { const invite = client.invitation(lease); if (invite) onInviteParty?.(lease, invite.characterName); }} />
        <Button id="recall" label={copy("recall")} x={266} y={87} w={92} disabled={!mutable} onClick={() => act({ type: "recall" })} />
        <Button id="resume" label={copy(tab === 5 ? "resumeLife" : "resumeActions")} x={370} y={87} w={112}
          disabled={!mutable || tab === 5 && !canResumeLife}
          onClick={() => { if (tab === 5) prepareResumeLife(); else act({ type: "resume" }); }} />
        <Copy x={496} y={91} w={225} h={32} size={12} lineHeight={14} tone="muted">{copy("rosterAuthority")}</Copy>
        <div role="tablist" aria-label={copy("tabsLabel")}>{R16_COMPANION_LAYOUT.tabs.map((label, index) => <Button key={index} id={`tab-${index}`} label={copy(label)} x={18 + index * 120} y={132} w={110} selected={tab === index} onClick={() => changeTab(index)} />)}</div>
        {confirmation ? <div data-r16-companion-confirm>
          <Copy x={24} y={205} w={680} h={32} size={19} tone="gold">{copy("confirmTitle")}</Copy>
          <Copy x={24} y={252} w={680} h={130} size={16}>{confirmText(confirmation.action, copy)}</Copy>
          <Button id="cancel-confirm" label={copy("cancel")} x={130} y={390} w={210} onClick={() => setConfirmation(null)} />
          <Button id="confirm" label={copy("confirm")} x={370} y={390} w={210} disabled={!mutable || confirmation.lease !== lease}
            onClick={() => { client.act(confirmation.lease, confirmation.action); setConfirmation(null); }} />
        </div> : !view ? <Copy x={22} y={215} h={90} size={16} tone="muted">{copy(snapshot.busy ? "loading" : "noPrivate")}</Copy>
          : subpage === "trial" ? <TrialPage copy={copy} key={snapshot.contentEpoch} view={view} mutable={mutable} act={act} back={() => setSubpage(null)} />
          : subpage === "costs" ? <CostsPage copy={copy} view={view} mutable={mutable} act={act} back={() => setSubpage(null)} />
          : tab === 0 && !teaching ? <>
            <Copy x={22} y={181} size={17} tone="gold">{copy("policyLine", view.archive.displayName, view.profile.policyVersion, view.growthRevision)}</Copy>
            <Copy x={22} y={222} h={92} size={16}>{habitNames.map(key =>
              copy("habitLine", copy(habitLabels[key]), yes(view.profile.party[key], copy))).join("\n")}</Copy>
            <Button id="teach" label={copy("teach")} x={22} y={326} w={690} disabled={!mutable} onClick={() => act({ type: "teach" })} />
            <Copy x={22} y={371} h={62} tone="muted">{copy("teachNote")}</Copy>
            <Button id="open-teaching" label={copy("naturalTeach")} x={22} y={435} w={334} disabled={!mutable} onClick={() => { setSubpage("teaching"); setDraft(""); }} />
            <Button id="open-trial" label={copy("trialOpen")} x={368} y={435} w={344} disabled={snapshot.busy} onClick={() => setSubpage("trial")} />
            <Copy x={22} y={478} h={61} size={13} tone="muted">{copy("identity", view.archive.archiveId, view.archive.baseSetting)}</Copy>
          </> : tab === 1 ? <>
            <Copy x={22} y={180} size={15} tone="gold">{copy("feedbackTitle", view.feedback.length, safePage + 1)}</Copy>
            {view.feedback.slice(safePage * 4, safePage * 4 + 4).map((row, index) => <div key={row.feedbackId}>
              <Copy x={22} y={218 + index * 65} w={470} h={20} size={12} lineHeight={16} tone="muted">{`${mappedLabel(row.status, feedbackLabels, "entryRecord", copy)} · ${date(row.recordedAt)}`}</Copy>
              <Copy x={22} y={238 + index * 65} w={462} h={42} dir="auto">{row.summary || copy("deletedText")}</Copy>
              <Button id={`revoke-${row.feedbackId}`} label={copy("revoke")} x={500} y={228 + index * 65} w={95} disabled={!mutable || row.status !== "active"} onClick={() => prepare({ type: "revokeFeedback", feedbackId: row.feedbackId })} />
              <Button id={`delete-feedback-${row.feedbackId}`} label={copy("deleteText")} x={605} y={228 + index * 65} w={108} disabled={!mutable || row.status === "deleted"} onClick={() => prepare({ type: "deleteFeedback", feedbackId: row.feedbackId })} />
            </div>)}
            {!view.feedback.length ? <Copy x={22} y={245} size={16} tone="muted">{copy("noFeedback")}</Copy> : null}{pageButtons()}
            <Button id="clear-growth" label={copy("clear")} x={22} y={513} w={220} disabled={!mutable} onClick={() => prepare({ type: "clearGrowth" })} />
            <Copy x={258} y={518} w={450} size={13} tone="muted">{copy("historyExport")}</Copy>
          </> : tab === 2 ? <>
            <Copy x={22} y={180} tone="gold">{copy("actionTitle")}</Copy>
            {view.episodes.slice(safePage * 4, safePage * 4 + 4).map((row, index) => <div key={row.episodeId}>
              <Copy x={22} y={215 + index * 64} w={560} h={22} size={12} tone="muted">{`${date(row.recordedAt)} · ${row.kind} · ${row.outcome}`}</Copy>
              <Copy x={22} y={237 + index * 64} w={560} h={40} dir="auto">{row.summary || row.action || copy("deletedText")}</Copy>
              <Button id={`delete-episode-${row.episodeId}`} label={copy("delete")} x={610} y={225 + index * 64} w={104} disabled={!mutable} onClick={() => prepare({ type: "deleteEpisode", episodeId: row.episodeId })} />
            </div>)}
            {!view.episodes.length ? <Copy x={22} y={245} size={16} tone="muted">{copy("noEpisodes")}</Copy> : null}{pageButtons()}
            <Button id="open-costs" label={copy("costOpen")} x={22} y={513} w={690} disabled={snapshot.busy} onClick={() => setSubpage("costs")} />
          </> : tab === 3 || teaching ? <>
            <Copy x={22} y={181} w={teaching ? 470 : 690} h={44} size={teaching ? 17 : 14} tone="gold">{teaching ? copy("naturalTeach") : [
              !view.runtime.provider || view.runtime.provider === "rules" ? copy("rules") : view.runtime.provider,
              !view.runtime.model || view.runtime.model === "rules" ? copy("rules") : view.runtime.model,
              copy(r16CompanionModelReady(view) ? "modelOn" : "modelOff"),
            ].join(" · ")}</Copy>
            {teaching ? <Button id="close-teaching" label={copy("teachingBack")} x={520} y={180} w={192} onClick={() => setSubpage(null)} /> : null}
            <Copy x={22} y={teaching ? 216 : 226} h={27} size={12} tone="muted">{copy(teaching ? "teachingScope" : "instructionScope")}</Copy>
            <textarea dir="auto" aria-label={copy(teaching ? "teachingInput" : "instructionInput")} data-r16-companion-input="instruction" style={at(22, teaching ? 249 : 255, 690, teaching ? 65 : 84)}
              value={draft} maxLength={2000} disabled={!mutable} autoComplete="off" spellCheck={false}
              onFocus={() => { editing.current = true; }} onBlur={() => { editing.current = false; }}
              onChange={event => setDraft(event.target.value)} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
              onKeyDown={event => { if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229 && !composing.current && !event.repeat) { event.preventDefault(); event.stopPropagation(); sendText(); } }} />
            <Button id={teaching ? "propose-teaching" : "instruct"} label={copy(teaching ? "propose" : "instruct")} x={22} y={teaching ? 323 : 349} w={teaching ? 334 : 218}
              disabled={!mutable || !r16CompanionModelReady(view) || !r16CompanionInstruction(draft) || (teaching ? view.runtime.teachingProposal.status === "planning" : activeLife(view))} onClick={sendText} />
            {teaching ? <>
              <Button id="cancel-teaching" label={copy("cancelProposal")} x={368} y={323} w={344} disabled={!mutable || !r16CompanionCancelTeachingId(view)}
                onClick={() => { const proposalId = r16CompanionCancelTeachingId(view); if (proposalId) act({ type: "cancelTeaching", proposalId }); }} />
              <Copy x={22} y={365} h={30} tone="gold">{copy(r16CompanionModelReady(view) ? "proposalReview" : "proposalDisabled")}</Copy>
              <Copy x={22} y={398} h={40} size={13} dir="auto">{String(view.runtime.teachingProposal.reply ?? "")}</Copy>
              {habitNames.map((key, index) => typeof view.runtime.teachingChanges[key] === "boolean" ? <Copy key={key} x={22} y={439 + index * 23} h={23}>
                {copy("habitLine", copy(habitLabels[key]), `${yes(view.profile.party[key], copy)} → ${yes(view.runtime.teachingChanges[key]!, copy)}`)}</Copy> : null)}
              <Button id="confirm-teaching" label={copy("confirmTeach")} x={22} y={515} w={334} disabled={!mutable || !r16CompanionTeachingId(view)} onClick={() => { const proposalId = r16CompanionTeachingId(view); if (proposalId) act({ type: "confirmTeaching", proposalId }); }} />
              <Copy x={368} y={515} w={344} h={29} size={12} tone="muted">{copy("proposalCounter", Array.from(draft).length)}</Copy>
            </> : <>
              <Button id="refresh-task" label={copy("refreshModel")} x={254} y={349} w={218} disabled={snapshot.busy} onClick={() => client.refresh(lease)} />
              <Copy x={22} y={395} h={135} size={13} tone="muted">{copy("modelTaskBlock",
                view.runtime.modelTask.status == null ? copy("idle") : mappedLabel(view.runtime.modelTask.status, modelTaskLabels, "unknown", copy),
                String(view.runtime.modelTask.reply ?? copy("taskDefault")), shown(view.runtime.modelTask.completedKills, copy), shown(view.runtime.modelTask.requestedKills, copy))}</Copy>
            </>}
          </> : tab === 4 ? <>
            <Copy x={22} y={181} size={17} tone="gold">{copy("supplyTitle")}</Copy>
            <Copy x={22} y={217} h={60}>{copy("supplyScope")}</Copy>
            <Button id="prepare-supply" label={copy("supplyAuthorize")} x={22} y={286} w={334} disabled={!mutable || activeSupply(view) || activeLife(view)} onClick={() => prepare({ type: "authorizeSupply" })} />
            <Button id="cancel-supply" label={copy("supplyCancel")} x={368} y={286} w={344} disabled={!mutable || !activeSupply(view)} onClick={() => act({ type: "cancelSupply" })} />
            <Copy x={22} y={330} h={44} size={15} tone="gold">{supplyStatus(view.runtime.supplyTask, copy)}</Copy>
            <Copy x={22} y={377} h={25}>{copy("supplyStep", String(view.runtime.supplyTask.step ?? copy("supplyStepMissing")))}</Copy>
            <Copy x={22} y={409} h={49}>{copy("supplyOutcome",
              view.runtime.supplyTask.receiptKnown === true ? shown(view.runtime.supplyTask.chargedGold, copy) : copy("unconfirmed"),
              view.runtime.supplyTask.receiptKnown === true ? shown(view.runtime.supplyTask.acquiredQuantity, copy) : copy("unconfirmed"),
              copy(view.runtime.supplyTask.returnedToAnchor === true ? "arrived" : "unconfirmed"))}</Copy>
            <Copy x={22} y={466} h={39} size={13} tone="muted" dir="auto">{String(view.runtime.supplyTask.lastError ?? view.runtime.supplyTask.reason ?? "")}</Copy>
            <Button id="refresh-supply" label={copy("supplyRefresh")} x={22} y={515} w={690} disabled={snapshot.busy} onClick={() => client.refresh(lease)} />
          </> : life ? <>
            <Copy x={22} y={180} size={17} tone="gold">{copy("lifeHeading", life.status)}</Copy>
            <Button id="duration-20" label={copy("durationOption", 20, duration === 1200 ? copy("chosen") : "")} x={22} y={214} w={110} disabled={!mutable} selected={duration === 1200} onClick={() => setDuration(1200)} />
            <Button id="duration-30" label={copy("durationOption", 30, duration === 1800 ? copy("chosen") : "")} x={142} y={214} w={110} disabled={!mutable} selected={duration === 1800} onClick={() => setDuration(1800)} />
            <Button id="prepare-life" label={copy("lifeAuthorize")} x={262} y={214} w={212} disabled={!mutable || !r16CompanionCanAuthorizeLife(view)} onClick={() => prepare({ type: "authorizeLife", durationSeconds: duration })} />
            <Button id="cancel-life" label={copy("lifeCancel")} x={486} y={214} w={108} disabled={!mutable || !activeLife(view)} onClick={() => act({ type: "cancelLife" })} />
            <Button id="life-recall" label={copy("recall")} x={604} y={214} w={108} disabled={!mutable} onClick={() => act({ type: "recall" })} />
            <Copy x={22} y={251} h={40} size={13} lineHeight={17} tone="muted">{copy("lifeScope")}</Copy>
            <Copy x={22} y={292} h={40}>{life.budget}</Copy>
            <Copy x={22} y={335} h={56} size={13} lineHeight={16}>{life.resourceLines}</Copy>
            <Copy x={22} y={391} h={39} size={12} lineHeight={16} tone="muted">{copy("lifeGift")}</Copy>
            <Copy x={22} y={431} h={57} size={12} lineHeight={18} tone="muted">{life.notes}</Copy>
            <Button id="prepare-resume-life" label={copy("lifeResumeReview")} x={22} y={494} w={334} disabled={!mutable || !canResumeLife} onClick={prepareResumeLife} />
            <Button id="refresh-life" label={copy("lifeRefresh")} x={368} y={494} w={344} disabled={snapshot.busy} onClick={() => client.refresh(lease)} />
            <Copy x={22} y={529} h={23} size={12} tone="gold">{life.warning}</Copy>
          </> : null}
        <div role="status" aria-live="polite"><Copy x={20} y={556} w={700} h={45} size={13} tone="gold">{r16CompanionNotice(snapshot.notice, copy)}</Copy></div>
        {!confirmation ? <>
          <Button id="export" label={copy("export")} x={20} y={602} w={272} disabled={!view || snapshot.busy || snapshot.uncertain || !selected?.online} onClick={() => client.exportArchive(lease)} />
          {snapshot.download ? <a className="download" data-r16-companion-action="download" href={snapshot.download.url} download={snapshot.download.fileName}
            onClick={event => { if (!client.isCurrentLease(lease, true)) event.preventDefault(); }}>{copy("download")}</a> : null}
          <Copy x={490} y={601} w={236} h={40} size={12} lineHeight={16} tone="muted">{copy("footnote")}</Copy>
        </> : null}
      </div>
    </>}
  </section>;
}
