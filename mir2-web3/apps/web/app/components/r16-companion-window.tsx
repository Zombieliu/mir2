"use client";

import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { CSSProperties, ReactNode } from "react";
import { R16CompanionClient, r16CompanionCanAuthorizeLife, r16CompanionModelReady, r16CompanionSupplyComplete, r16CompanionTeachingId,
  r16CompanionInstruction, r16CompanionCancelTeachingId, r16CompanionResumeLifeId, formatR16CompanionMicros,
  R16_COMPANION_ENTRY_SOURCES, R16_COMPANION_CHOICE_REASONS } from "../../lib/r16-companion-client";
import type { R16CompanionAction, R16CompanionOwnerView, R16CompanionRenderLease, R16CompanionTask } from "../../lib/r16-companion-client";

/** R16 crystal_ui/companion.rs logical 1024x768 stage, not viewport pixels. */
export const R16_COMPANION_LAYOUT = {
  entry: { left: 872, top: 174, width: 140, height: 29 },
  panel: { left: 140, top: 44, width: 744, height: 644 },
  tabs: ["培养与体验", "记忆管理", "行动与成本", "私人委托", "独立补给", "自主生活"],
} as const;
export type R16CompanionWindowProps = {
  client: R16CompanionClient; open: boolean; onOpenChange(open: boolean): void;
  entryEnabled?: boolean; entryVisible?: boolean; partyMember?: boolean;
  /** Host must call client.invitation(lease) immediately before its existing
   * party sender and use the returned current characterName. No optimistic party. */
  onInviteParty?(lease: R16CompanionRenderLease, characterName: string): boolean;
  /** Opens existing Node settings only. No key, endpoint or model SDK in this window. */
  onOpenModelSettings?(lease: R16CompanionRenderLease): void;
};
const styles = `
.r16-companion { position:absolute; inset:0; pointer-events:none; color:rgb(235,222,194); font-family:"Noto Sans SC","Microsoft YaHei",Arial,sans-serif; font-size:14px; z-index:30000; }
.r16-companion * { box-sizing:border-box; }
.r16-companion .backdrop { position:absolute; inset:0; background:rgba(0,0,0,.54); pointer-events:auto; }
.r16-companion .panel { position:absolute; border:2px solid rgb(209,176,112); background:rgb(14,11,7); overflow:hidden; pointer-events:auto; }
.r16-companion button { position:absolute; height:29px; padding:0 4px; border:1px solid rgb(209,176,112); border-radius:0; background:rgb(41,28,15); color:rgb(235,222,194); font:inherit; font-size:13px; line-height:16px; white-space:normal; cursor:pointer; }
.r16-companion button:disabled { background:rgb(20,18,15); color:rgb(166,158,138); cursor:default; }
.r16-companion button:not(:disabled):is(:hover,:focus-visible),.r16-companion button[aria-selected=true],.r16-companion button[aria-pressed=true] { border-color:rgb(255,222,115); background:rgb(71,51,26); }
.r16-companion :is(button,textarea,a):focus-visible { outline:1px solid #ffde73; outline-offset:1px; }
.r16-companion .copy { position:absolute; margin:0; white-space:pre-wrap; overflow:hidden; overflow-wrap:anywhere; font-weight:400; }
.r16-companion .gold { color:rgb(209,176,112); }
.r16-companion .muted { color:rgb(166,158,138); }
.r16-companion textarea { position:absolute; resize:none; border:1px solid rgb(166,158,138); border-radius:0; padding:8px; background:rgb(23,19,13); color:rgb(235,222,194); font:inherit; font-size:15px; line-height:20px; }
.r16-companion .download { position:absolute; left:310px; top:602px; width:164px; height:29px; display:flex; align-items:center; justify-content:center; border:1px solid rgb(209,176,112); color:rgb(235,222,194); background:rgb(41,28,15); text-decoration:none; font-size:13px; }
`;
const at = (left: number, top: number, width: number, height?: number): CSSProperties => ({ left, top, width, ...(height === undefined ? {} : { height }) });
function Copy({ x, y, w = 690, h = 28, size = 14, tone = "", children }: { x: number; y: number; w?: number; h?: number; size?: number; tone?: string; children: ReactNode }) {
  return <p className={`copy ${tone}`} style={{ ...at(x, y, w, h), fontSize: size, lineHeight: `${Math.max(size + 5, 20)}px` }}>{children}</p>;
}
function Button({ id, label, x, y, w, disabled = false, selected, onClick }: { id: string; label: string; x: number; y: number; w: number; disabled?: boolean; selected?: boolean; onClick(): void }) {
  return <button type="button" data-r16-companion-action={id} style={at(x, y, w)} disabled={disabled} role={id.startsWith("tab-") ? "tab" : undefined}
    aria-selected={id.startsWith("tab-") ? selected : undefined} aria-pressed={id.startsWith("tab-") ? undefined : selected}
    onClick={() => { if (!disabled) onClick(); }}>{label}</button>;
}
const yes = (v: boolean) => v ? "是" : "否";
const shown = (v: unknown) => typeof v === "number" && Number.isSafeInteger(v) && v >= 0 ? String(v) : "待核对";
const date = (v: number | null) => v === null ? "时间待核对" : new Date(v).toLocaleString("zh-CN");
const activeSupply = (v: R16CompanionOwnerView) => ["authorized", "running", "paused", "unknown", "returning"].includes(String(v.runtime.supplyTask.status));
const activeLife = (v: R16CompanionOwnerView) => !!v.runtime.lifeTask.status && !["idle", "ended", "completed", "cancelled", "failed", "expired"].includes(String(v.runtime.lifeTask.status));
const confirmText = (a: R16CompanionAction): string => ({
  clearGrowth: "清除全部教学和经历，并重新计算习惯。保留伙伴身份及服务器游戏角色。此操作不能撤销。",
  revokeFeedback: "撤销这条教学并重新计算习惯。记录仍可查看；撤销后不会自动恢复。",
  deleteFeedback: "删除这条教学及文字，并重新计算习惯。此操作不能撤销。",
  deleteEpisode: "删除这条经历及文字。由它产生的教学也将失效。此操作不能撤销。",
  authorizeSupply: "授权本次独立补给：去比奇杂货商鲁本处购买 1 瓶小型生命药水，最多花费 50 游戏金币，权限 5 分钟有效，买完回到授权点旁。仅本次购买；禁止拾取、出售、交易和换图。购买已提交后，撤销不会虚构退款。",
  authorizeLife: `授权本次 ${a.type === "authorizeLife" ? a.durationSeconds / 60 : ""} 分钟自主生活。单会话累计最多 50 游戏金币 / 1 瓶血药；仅在现有区域与游戏规则内行动。授权不等于已消费或已完成。`,
  resumeLife: "确认恢复原生活授权；沿用原期限与剩余额度，不生成新的采购额度。已提交或未知的购买仍只能核对原订单。",
} as Partial<Record<R16CompanionAction["type"], string>>)[a.type] ?? "请确认这次操作。";
function supplyStatus(t: R16CompanionTask): string {
  if (r16CompanionSupplyComplete(t)) return "补给完成：购买回执和实际返程均已确认。";
  return ({ authorized: "服务器已授权；购买与返程尚未完成。", running: "正在执行补给；实际扣款和入药以服务器回执为准。", paused: "补给已暂停；不会自动恢复支出。",
    unknown: "购买结果未知；仅核对原订单，不会重复下单。", returning: "正在返程；到达授权点旁前不显示完成。", completed: "完成证据不足，请刷新核对实际购买与返程。",
    cancelled: "补给已撤销；已提交购买的结果仍需核对，不会自动退款。", failed: "本次补给未完成，请核对原因和已有购买回执。" } as Record<string, string>)[String(t.status)] ?? "暂无独立补给任务。";
}

type SubpageProps = { view: R16CompanionOwnerView; mutable: boolean; act(action: R16CompanionAction): void; back(): void };
function TrialPage({ view, mutable, act, back }: SubpageProps) {
  const [entrySource, setEntrySource] = useState<typeof R16_COMPANION_ENTRY_SOURCES[number] | null>(null);
  const [reminderCount, setReminderCount] = useState(0);
  const [choice, setChoice] = useState<typeof R16_COMPANION_CHOICE_REASONS[number] | null>(null);
  const active = !!view.trial?.activeVisitId, enabled = mutable && !!view.trial;
  return <>
    <Copy x={22} y={180} h={26} tone="gold">本参与者试用累计 · 体验自述</Copy>
    {R16_COMPANION_ENTRY_SOURCES.map((source, i) => <Button key={source} id={`trial-source-${source}`} label={["自己想来", "收到提醒", "约好时间", "不确定"][i]}
      x={22 + i * 174} y={216} w={162} selected={entrySource === source} disabled={!enabled || active} onClick={() => setEntrySource(source)} />)}
    <Copy x={22} y={256} w={400} h={26}>{`收到提醒次数（自述）：${reminderCount}`}</Copy>
    <Button id="trial-reminder-down" label="−" x={540} y={252} w={78} disabled={!enabled || active || reminderCount === 0} onClick={() => setReminderCount(reminderCount - 1)} />
    <Button id="trial-reminder-up" label="+" x={634} y={252} w={78} disabled={!enabled || active || reminderCount === 100} onClick={() => setReminderCount(reminderCount + 1)} />
    <Button id={active ? "trial-end" : "trial-start"} label={active ? "结束本次体验" : "开始本次体验"} x={22} y={291} w={690} disabled={!enabled}
      onClick={() => act({ type: "trialEvent", event: active ? { kind: "visitEnd" } : { kind: "visit", entrySource: entrySource ?? "unknown", reminderCount } })} />
    <Copy x={22} y={332} h={23}>选择这位伙伴的原因（自述）</Copy>
    {R16_COMPANION_CHOICE_REASONS.map((reason, i) => <Button key={reason} id={`trial-reason-${reason}`} label={["共同经历", "战力更强", "省去设置", "好奇", "其他"][i]}
      x={22 + i * 140} y={360} w={130} selected={choice === reason} disabled={!enabled || !active} onClick={() => setChoice(reason)} />)}
    <Copy x={22} y={401} h={30} size={13} tone="muted">{view.trial ? "选择与教学理解由你主动记录，仅保存所选选项。" : "此服务器尚未配置体验记录。"}</Copy>
    <Button id="trial-save-choice" label="记录选择原因" x={22} y={443} w={690} disabled={!enabled || !active || !choice}
      onClick={() => { if (choice) act({ type: "trialEvent", event: { kind: "choice", reason: choice } }); }} />
    <Button id="trial-understood" label="教学反馈：我理解了" x={22} y={484} w={335} disabled={!enabled || !active}
      onClick={() => act({ type: "trialEvent", event: { kind: "teachingCheck", understood: true } })} />
    <Button id="trial-not-understood" label="教学反馈：还没看懂" x={377} y={484} w={335} disabled={!enabled || !active}
      onClick={() => act({ type: "trialEvent", event: { kind: "teachingCheck", understood: false } })} />
    <Button id="back-trial" label="返回培养习惯" x={22} y={521} w={690} onClick={back} />
  </>;
}

function CostsPage({ view, mutable, act, back }: SubpageProps) {
  const trial = view.trial, usage = trial?.usage, costs = trial?.costs, offer = trial?.offer;
  const measure = (v: unknown) => typeof v === "number" ? String(v) : "未计量";
  const money = (v: unknown) => formatR16CompanionMicros(v, costs?.currency) ?? "未计量";
  const mode = view.runtime.modelEnabled ? "已启用模型服务" : view.runtime.provider ? "规则执行器（未启用大模型）" : "运行服务未说明";
  return <>
    <Copy x={22} y={180} h={24} size={16} tone="gold">本参与者试用累计（跨已授权伙伴）</Copy>
    <Copy x={22} y={212} h={32}>{`运行秒数：${measure(usage?.runtimeSeconds)}　模型调用：${measure(usage?.modelCalls)}　已确认冒险：${measure(usage?.confirmedAdventureCount)}`}</Copy>
    <Copy x={22} y={255} h={94}>{`模型金额记录：${money(costs?.modelMicros)}　托管：${money(costs?.hostingMicros)}\n支付费用：${money(costs?.paymentMicros)}　分成：${money(costs?.revenueShareMicros)}\n贡献：${costs?.complete === true ? money(costs.contributionMicros) : "成本未齐，暂不可计算"}　人工支持秒数：${measure(costs?.supportSeconds)}`}</Copy>
    <Copy x={22} y={350} h={24} size={12} tone="muted">{`当前伙伴：${mode}。实际付款以独立支付记录为准。`}</Copy>
    <Copy x={22} y={386} h={28} tone="gold">{offer ? `可选未来服务：${offer.price} · ${offer.durationDays}天 / ${offer.adventureCount}次冒险` : "尚未配置服务价格。"}</Copy>
    <Copy x={22} y={420} h={52} size={13}>{offer?.paymentInstructions || "角色身份、成长、记忆管理与导出均可继续使用。"}</Copy>
    <Button id="trial-purchase-intent" label="申请此服务（仅记录意向）" x={22} y={477} w={690} disabled={!mutable || !offer || !trial?.activeVisitId}
      onClick={() => act({ type: "trialEvent", event: { kind: "purchaseIntent" } })} />
    <Button id="back-costs" label="返回行动记录" x={22} y={518} w={690} onClick={back} />
  </>;
}

export function R16CompanionWindow({ client, open, onOpenChange, entryEnabled = true, entryVisible = true, partyMember = false, onInviteParty, onOpenModelSettings }: R16CompanionWindowProps) {
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
  const changeTab = (value: number) => { editing.current = false; setTab(value); setPage(0); setSubpage(null); setDraft(""); setConfirmation(null); };
  const sendText = () => { if (!composing.current) act({ type: teaching ? "proposeTeaching" : "instruct", text: draft }); };
  const rows = tab === 1 ? view?.feedback ?? [] : view?.episodes ?? [];
  const pageCount = Math.max(1, Math.ceil(rows.length / 4)), safePage = Math.min(page, pageCount - 1);
  const pageButtons = () => <>
    <Button id="previous-page" label="上一页" x={360} y={482} w={166} disabled={safePage === 0} onClick={() => setPage(safePage - 1)} />
    <Button id="next-page" label={safePage + 1 < pageCount ? "下一页" : "读取更早记录"} x={540} y={482} w={174}
      disabled={snapshot.busy || safePage + 1 >= pageCount && !view?.nextCursor}
      onClick={() => { if (safePage + 1 < pageCount) setPage(safePage + 1); else if (view?.nextCursor) client.refresh(lease, view.nextCursor); }} />
  </>;
  return <section className="r16-companion" data-r16-companion-open={open} onPointerDown={event => event.stopPropagation()} onClick={event => event.stopPropagation()}>
    <style>{styles}</style>
    {!open ? entryVisible && <button type="button" style={{ ...R16_COMPANION_LAYOUT.entry, pointerEvents: "auto" }} data-r16-companion-action="open" disabled={!entryEnabled || !snapshot.available}
      onClick={() => { if (entryEnabled && client.isCurrentLease(lease)) onOpenChange(true); }}>伙伴 Companion</button> : <>
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
        <h2 id="r16-companion-title" className="copy gold" style={{ ...at(18, 12, 470, 30), fontSize: 20 }}>伙伴 · 身份与成长</h2>
        <Button id="close" label="关闭 Esc" x={638} y={10} w={88} onClick={close} />
        <Button id="model-settings" label="模型设置" x={506} y={10} w={118} disabled={!onOpenModelSettings || !snapshot.available}
          onClick={() => { if (client.isCurrentLease(lease)) onOpenModelSettings?.(lease); }} />
        <Copy x={18} y={50} w={530} h={25} tone="muted">{selected ? `${selected.displayName} · ${selected.online ? "在线" : "离线"} · 已授权 ${snapshot.enrollments.indexOf(selected) + 1}/${snapshot.enrollments.length}` : "仅列出当前登录角色有权管理的伙伴"}</Copy>
        <Button id="previous-resident" label="上一位" x={558} y={47} w={78} disabled={snapshot.busy || snapshot.enrollments.length < 2}
          onClick={() => { const index = snapshot.enrollments.indexOf(selected!); client.choose(lease, snapshot.enrollments[(index + snapshot.enrollments.length - 1) % snapshot.enrollments.length].residentId); }} />
        <Button id="next-resident" label="下一位" x={646} y={47} w={78} disabled={snapshot.busy || snapshot.enrollments.length < 2}
          onClick={() => { const index = snapshot.enrollments.indexOf(selected!); client.choose(lease, snapshot.enrollments[(index + 1) % snapshot.enrollments.length].residentId); }} />
        <Button id="refresh" label="刷新记录" x={18} y={87} w={112} disabled={snapshot.busy || !snapshot.available} onClick={() => client.refresh(lease)} />
        <Button id="invite" label={partyMember ? "已在队伍" : "邀请组队"} x={142} y={87} w={112} disabled={!mutable || partyMember || !onInviteParty}
          onClick={() => { const invite = client.invitation(lease); if (invite) onInviteParty?.(lease, invite.characterName); }} />
        <Button id="recall" label="召回" x={266} y={87} w={92} disabled={!mutable} onClick={() => act({ type: "recall" })} />
        <Button id="resume" label={tab === 5 ? "恢复生活" : "恢复行动"} x={370} y={87} w={112}
          disabled={!mutable || tab === 5 && (!view || !r16CompanionResumeLifeId(view))}
          onClick={() => { const lifePermitId = view ? r16CompanionResumeLifeId(view) : null; if (tab === 5 && lifePermitId) prepare({ type: "resumeLife", lifePermitId }); else if (tab !== 5) act({ type: "resume" }); }} />
        <Copy x={496} y={91} w={225} h={26} size={12} tone="muted">组队以服务器名单为准</Copy>
        <div role="tablist" aria-label="伙伴资料">{R16_COMPANION_LAYOUT.tabs.map((label, index) => <Button key={index} id={`tab-${index}`} label={label} x={18 + index * 120} y={132} w={110} selected={tab === index} onClick={() => changeTab(index)} />)}</div>
        {confirmation ? <div data-r16-companion-confirm>
          <Copy x={24} y={205} w={680} h={32} size={19} tone="gold">请确认这次操作</Copy>
          <Copy x={24} y={252} w={680} h={130} size={16}>{confirmText(confirmation.action)}</Copy>
          <Button id="cancel-confirm" label="取消" x={130} y={390} w={210} onClick={() => setConfirmation(null)} />
          <Button id="confirm" label="确认执行" x={370} y={390} w={210} disabled={!mutable || confirmation.lease !== lease}
            onClick={() => { client.act(confirmation.lease, confirmation.action); setConfirmation(null); }} />
        </div> : !view ? <Copy x={22} y={215} h={90} size={16} tone="muted">{snapshot.busy ? "正在读取已授权的伙伴资料…" : "请刷新伙伴列表。没有授权时不会显示私人记忆。"}</Copy>
          : subpage === "trial" ? <TrialPage key={snapshot.contentEpoch} view={view} mutable={mutable} act={act} back={() => setSubpage(null)} />
          : subpage === "costs" ? <CostsPage view={view} mutable={mutable} act={act} back={() => setSubpage(null)} />
          : tab === 0 && !teaching ? <>
            <Copy x={22} y={181} size={17} tone="gold">{`${view.archive.displayName} · 培养 v${view.profile.policyVersion} · 成长修订 ${view.growthRevision}`}</Copy>
            <Copy x={22} y={222} h={92} size={16}>{`组队先支援：${yes(view.profile.party.assistBeforeLoot)}\n战斗后再拾取：${yes(view.profile.party.lootOnlyAfterCombat)}\n离队先询问：${yes(view.profile.party.askBeforeLeaving)}`}</Copy>
            <Button id="teach" label="教它：组队先支援，战后再拾取，离队先问" x={22} y={326} w={690} disabled={!mutable} onClick={() => act({ type: "teach" })} />
            <Copy x={22} y={371} h={62} tone="muted">这是明确的习惯教学。保存成功后显示新版本；伙伴实际行动仍受血量、距离和游戏规则约束。</Copy>
            <Button id="open-teaching" label="用自己的话教阿洛" x={22} y={435} w={334} disabled={!mutable} onClick={() => { setSubpage("teaching"); setDraft(""); }} />
            <Button id="open-trial" label="体验自述与选择原因" x={368} y={435} w={344} disabled={snapshot.busy} onClick={() => setSubpage("trial")} />
            <Copy x={22} y={478} h={61} size={13} tone="muted">{`固定身份：${view.archive.archiveId}\n${view.archive.baseSetting}`}</Copy>
          </> : tab === 1 ? <>
            <Copy x={22} y={180} size={15} tone="gold">{`教学记录 · 当前读取 ${view.feedback.length} 条 · 第 ${safePage + 1} 页`}</Copy>
            {view.feedback.slice(safePage * 4, safePage * 4 + 4).map((row, index) => <div key={row.feedbackId}>
              <Copy x={22} y={218 + index * 65} w={470} h={20} size={12} tone="muted">{`${({ active: "生效中", revoked: "已撤销", deleted: "已删除" })[row.status] ?? "记录"} · ${date(row.recordedAt)}`}</Copy>
              <Copy x={22} y={238 + index * 65} w={462} h={42}>{row.summary || "文字已删除"}</Copy>
              <Button id={`revoke-${row.feedbackId}`} label="撤销" x={500} y={228 + index * 65} w={95} disabled={!mutable || row.status !== "active"} onClick={() => prepare({ type: "revokeFeedback", feedbackId: row.feedbackId })} />
              <Button id={`delete-feedback-${row.feedbackId}`} label="删除文字" x={605} y={228 + index * 65} w={108} disabled={!mutable || row.status === "deleted"} onClick={() => prepare({ type: "deleteFeedback", feedbackId: row.feedbackId })} />
            </div>)}
            {!view.feedback.length ? <Copy x={22} y={245} size={16} tone="muted">还没有教学记录。</Copy> : null}{pageButtons()}
            <Button id="clear-growth" label="清除全部成长…" x={22} y={513} w={220} disabled={!mutable} onClick={() => prepare({ type: "clearGrowth" })} />
            <Copy x={258} y={518} w={450} size={13} tone="muted">完整历史可导出；不受购买服务限制。</Copy>
          </> : tab === 2 ? <>
            <Copy x={22} y={180} tone="gold">行动记录（选择只是计划，完成须服务器确认）</Copy>
            {view.episodes.slice(safePage * 4, safePage * 4 + 4).map((row, index) => <div key={row.episodeId}>
              <Copy x={22} y={215 + index * 64} w={560} h={22} size={12} tone="muted">{`${date(row.recordedAt)} · ${row.kind} · ${row.outcome}`}</Copy>
              <Copy x={22} y={237 + index * 64} w={560} h={40}>{row.summary || row.action || "文字已删除"}</Copy>
              <Button id={`delete-episode-${row.episodeId}`} label="删除" x={610} y={225 + index * 64} w={104} disabled={!mutable} onClick={() => prepare({ type: "deleteEpisode", episodeId: row.episodeId })} />
            </div>)}
            {!view.episodes.length ? <Copy x={22} y={245} size={16} tone="muted">还没有已记录的行动经历。</Copy> : null}{pageButtons()}
            <Button id="open-costs" label="查看服务状态、费用与可选服务" x={22} y={513} w={690} disabled={snapshot.busy} onClick={() => setSubpage("costs")} />
          </> : tab === 3 || teaching ? <>
            <Copy x={22} y={181} w={teaching ? 470 : 690} h={44} size={teaching ? 17 : 14} tone="gold">{teaching ? "用自己的话教阿洛" : `${view.runtime.provider || "规则"} · ${view.runtime.model || "规则"} · ${r16CompanionModelReady(view) ? "模型已启用" : "模型未启用"}`}</Copy>
            {teaching ? <Button id="close-teaching" label="返回培养" x={520} y={180} w={192} onClick={() => setSubpage(null)} /> : null}
            <Copy x={22} y={teaching ? 216 : 226} h={27} size={12} tone="muted">{teaching ? "支持支援、拾取与离队习惯。生成会调用模型；确认前不生效，也不会扩大花费权限。" : "私人委托 · 仅发送给你有权管理的伙伴，最多 1000 字符"}</Copy>
            <textarea aria-label={teaching ? "自然语言教学输入" : "私人委托输入"} data-r16-companion-input="instruction" style={at(22, teaching ? 249 : 255, 690, teaching ? 65 : 84)}
              value={draft} maxLength={2000} disabled={!mutable} autoComplete="off" spellCheck={false}
              onFocus={() => { editing.current = true; }} onBlur={() => { editing.current = false; }}
              onChange={event => setDraft(event.target.value)} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
              onKeyDown={event => { if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229 && !composing.current && !event.repeat) { event.preventDefault(); event.stopPropagation(); sendText(); } }} />
            <Button id={teaching ? "propose-teaching" : "instruct"} label={teaching ? "生成教学提案 · Enter" : "发送私人委托"} x={22} y={teaching ? 323 : 349} w={teaching ? 334 : 218}
              disabled={!mutable || !r16CompanionModelReady(view) || !r16CompanionInstruction(draft) || (teaching ? view.runtime.teachingProposal.status === "planning" : activeLife(view))} onClick={sendText} />
            {teaching ? <>
              <Button id="cancel-teaching" label="取消当前提案" x={368} y={323} w={344} disabled={!mutable || !r16CompanionCancelTeachingId(view)}
                onClick={() => { const proposalId = r16CompanionCancelTeachingId(view); if (proposalId) act({ type: "cancelTeaching", proposalId }); }} />
              <Copy x={22} y={365} h={30} tone="gold">{r16CompanionModelReady(view) ? "请核对下方改动；确认之前不会改变习惯。" : "请在 Node 设置中启用模型；原有固定教学仍可使用。"}</Copy>
              <Copy x={22} y={398} h={40} size={13}>{String(view.runtime.teachingProposal.reply ?? "")}</Copy>
              {(["assistBeforeLoot", "lootOnlyAfterCombat", "askBeforeLeaving"] as const).map((key, index) => typeof view.runtime.teachingChanges[key] === "boolean" ? <Copy key={key} x={22} y={439 + index * 23} h={23}>
                {`${["组队先支援", "战斗后再拾取", "离队先询问"][index]}：${yes(view.profile.party[key])} → ${yes(view.runtime.teachingChanges[key]!)}`}</Copy> : null)}
              <Button id="confirm-teaching" label="确认这些习惯改动" x={22} y={515} w={334} disabled={!mutable || !r16CompanionTeachingId(view)} onClick={() => { const proposalId = r16CompanionTeachingId(view); if (proposalId) act({ type: "confirmTeaching", proposalId }); }} />
              <Copy x={368} y={515} w={344} h={29} size={12} tone="muted">{`${Array.from(draft).length} / 1000 字符 · 中文候选词确认后再提交。`}</Copy>
            </> : <>
              <Button id="refresh-task" label="刷新模型状态" x={254} y={349} w={218} disabled={snapshot.busy} onClick={() => client.refresh(lease)} />
              <Copy x={22} y={395} h={135} size={13} tone="muted">{`任务状态：${view.runtime.modelTask.status ?? "待命"}\n${view.runtime.modelTask.reply ?? "模型计划不代表游戏行动已完成。"}\n已观察击杀 ${shown(view.runtime.modelTask.completedKills)} / ${shown(view.runtime.modelTask.requestedKills)}`}</Copy>
            </>}
          </> : tab === 4 ? <>
            <Copy x={22} y={181} size={17} tone="gold">独立补给 · 单次受限采购</Copy>
            <Copy x={22} y={217} h={60}>目的地：比奇杂货商鲁本。购买 1 瓶小型生命药水，再回到授权点旁。{"\n"}最多 50 游戏金币，权限 5 分钟有效。禁止拾取、出售、交易和换图。</Copy>
            <Button id="prepare-supply" label="查看并确认这次授权" x={22} y={286} w={334} disabled={!mutable || activeSupply(view) || activeLife(view)} onClick={() => prepare({ type: "authorizeSupply" })} />
            <Button id="cancel-supply" label="撤销本次补给" x={368} y={286} w={344} disabled={!mutable || !activeSupply(view)} onClick={() => act({ type: "cancelSupply" })} />
            <Copy x={22} y={330} h={44} size={15} tone="gold">{supplyStatus(view.runtime.supplyTask)}</Copy>
            <Copy x={22} y={377} h={25}>{`当前步骤：${view.runtime.supplyTask.step ?? "等待实际行动记录"}`}</Copy>
            <Copy x={22} y={409} h={49}>{`实际扣款：${view.runtime.supplyTask.receiptKnown === true ? shown(view.runtime.supplyTask.chargedGold) : "尚未确认"} 游戏金币 · 实际入药：${view.runtime.supplyTask.receiptKnown === true ? shown(view.runtime.supplyTask.acquiredQuantity) : "尚未确认"}\n返程到达：${view.runtime.supplyTask.returnedToAnchor === true ? "已确认到达授权点旁" : "尚未确认"}`}</Copy>
            <Copy x={22} y={466} h={39} size={13} tone="muted">{String(view.runtime.supplyTask.lastError ?? view.runtime.supplyTask.reason ?? "")}</Copy>
            <Button id="refresh-supply" label="刷新服务器补给记录" x={22} y={515} w={690} disabled={snapshot.busy} onClick={() => client.refresh(lease)} />
          </> : <>
            <Copy x={22} y={180} size={17} tone="gold">{`自主生活 · ${view.runtime.lifeTask.status ?? "待命"} · ${view.runtime.lifeTask.phase ?? "阶段待核对"}`}</Copy>
            <Button id="duration-20" label="20分钟" x={22} y={214} w={100} selected={duration === 1200} onClick={() => setDuration(1200)} />
            <Button id="duration-30" label="30分钟" x={132} y={214} w={100} selected={duration === 1800} onClick={() => setDuration(1800)} />
            <Button id="prepare-life" label="查看并确认授权" x={242} y={214} w={232} disabled={!mutable || !r16CompanionCanAuthorizeLife(view)} onClick={() => prepare({ type: "authorizeLife", durationSeconds: duration })} />
            <Button id="cancel-life" label="停止生活" x={486} y={214} w={108} disabled={!mutable || !activeLife(view)} onClick={() => act({ type: "cancelLife" })} />
            <Button id="life-recall" label="召回" x={604} y={214} w={108} disabled={!mutable} onClick={() => act({ type: "recall" })} />
            <Copy x={22} y={251} h={38} size={13} tone="muted">单会话累计最多 50 游戏金币 / 1 瓶血药；可独立于主人视野行动。{"\n"}范围：地图0 x270–310、y595–645；回城点（288,616）。</Copy>
            <Copy x={22} y={292} h={40}>{`采购额度 ${shown(view.runtime.lifeTask.budgetGold)} · 剩余 ${shown(view.runtime.lifeTask.remainingBudgetGold)} · 预留 ${shown(view.runtime.lifeTask.reservedGold)} · 实扣 ${shown(view.runtime.lifeTask.chargedGold)} 游戏金币\n已购 ${shown(view.runtime.lifeTask.purchasedQuantity)} / ${shown(view.runtime.lifeTask.maxPurchasedQuantity)} 瓶 · 观察击杀 ${shown(view.runtime.lifeTask.killsObserved)}`}</Copy>
            <Copy x={22} y={335} h={51} size={13}>{view.runtime.resources.known === true ? `阿洛自己的金币：${shown(view.runtime.resources.gold)} · 血量 ${shown(view.runtime.resources.hp)}/${shown(view.runtime.resources.maxHp)}\n血药 ${view.runtime.resources.inventoryCurrent === true ? shown(view.runtime.resources.hpPotionCount) : "待核对"} · 背包空位 ${view.runtime.resources.inventoryCurrent === true ? shown(view.runtime.resources.freeBagSlots) : "待核对"}` : "阿洛自己的金币、血量、药品、背包与负重：待核对。未知数量不会显示为 0。"}</Copy>
            <Copy x={22} y={391} h={35} size={12} tone="muted">靠近并面对阿洛，用原交易窗口赠送补给；阿洛只接收，不付出。</Copy>
            <Copy x={22} y={431} h={57} size={12} tone="muted">{`模型：${view.runtime.lifeTask.modelStatus ?? view.runtime.lifeTask.decisionMode ?? "待核对"}\n${view.runtime.lifeTask.pausedReason ?? view.runtime.lifeTask.reason ?? "暂无已确认的行动说明。"}`}</Copy>
            <Button id="refresh-life" label="刷新生活与资源" x={368} y={494} w={344} disabled={snapshot.busy} onClick={() => client.refresh(lease)} />
            <Copy x={22} y={529} h={23} size={12} tone="gold">{view.runtime.resources.medicineUnresolved === true ? "购药结果未知：仅核对原订单，不算作完成。" : "行动、击杀与消费均来自实际观察；授权不等于完成。"}</Copy>
          </>}
        <div role="status" aria-live="polite"><Copy x={20} y={556} w={700} h={45} size={13} tone="gold">{snapshot.notice}</Copy></div>
        {!confirmation ? <>
          <Button id="export" label="导出角色与完整成长" x={20} y={602} w={272} disabled={!view || snapshot.busy || snapshot.uncertain || !selected?.online} onClick={() => client.exportArchive(lease)} />
          {snapshot.download ? <a className="download" data-r16-companion-action="download" href={snapshot.download.url} download={snapshot.download.fileName}
            onClick={event => { if (!client.isCurrentLease(lease, true)) event.preventDefault(); }}>下载已校验档案</a> : null}
          <Copy x={490} y={601} w={236} h={38} size={12} tone="muted">查看与导出无需购买。{"\n"}Tab 选控件，Enter 确认。</Copy>
        </> : null}
      </div>
    </>}
  </section>;
}
