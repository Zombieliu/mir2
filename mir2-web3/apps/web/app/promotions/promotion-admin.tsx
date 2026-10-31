"use client";

import { useCallback, useEffect, useState } from "react";
import { PROMOTION_ADMIN_COPY, PROMOTION_EXPORT_COPY, PROMOTION_LAUNCH_COPY, PROMOTION_FUNDING_COPY } from "./promotion-admin-copy";
import { promotionReviewCommand, promotionFundingLimits, PROMOTION_DRAFT_SEASON_UTC, type PromotionReviewAction, type PromotionFundingMode } from "./promotion-admin-input";
import type { PromotionAccountCopy } from "./promotion-account-copy";
import type { PromotionCopy, PromotionLocale } from "./promotion-copy";
import { formatPromotionUsd, promotionRequest, readPromotionSettlementExport, PromotionApiError } from "./promotion-model";
import styles from "./promotion.module.css";

type AdminData = {
  roles: string[]; campaign: null | { status: string; termsVersion: string; rulesApproved: boolean; recruitmentEnabled: boolean; commissionEnabled: boolean; budgetMicroUsd: string; reservedMicroUsd: string; fundingMode?: PromotionFundingMode };
  leaders: { leaderId: string; displayName: string; status: string }[];
  settlements: { settlementId: string; leaderId: string; status: string; snapshotSha256: string; totalMicroUsd: string }[];
  sourceGaps: string[]; payoutEnabled: boolean; environment?: string;
  launchConfiguration: Record<string, unknown> | null;
  launchConfigurationSha256: string | null;
  launchApproval: null | { manifestSha256: string; manifest: Record<string, unknown>; proposedBy: string; approvedBy: string | null };
};
type AdminAction = "reviewLeader" | "reconcile" | "createSettlement" | "approveSettlement" | "approveRules" | "recruitOn" | "recruitOff" | "commissionOn" | "commissionOff" | "setInputWatermark" | "createPayoutDraft" | PromotionReviewAction;

export function PromotionLaunchReview({ data, copy, locale }: {
  data: Pick<AdminData, "payoutEnabled" | "launchConfiguration" | "launchConfigurationSha256" | "launchApproval">;
  copy: PromotionCopy; locale: PromotionLocale;
}) {
  const labels = PROMOTION_LAUNCH_COPY[locale];
  const fundingLabels = PROMOTION_FUNDING_COPY[locale];
  return <>
    <div className={styles.notice}><strong>{labels.payoutState}: {data.payoutEnabled ? labels.enabled : copy.closed}</strong>{labels.executionPolicy}</div>
    <details className={styles.details}><summary>{labels.configuration}</summary>
      {data.launchConfiguration?.fundingMode === "revenue_share" ? <p className={styles.muted} style={{ marginTop: 12 }}><strong>{fundingLabels.revenueShare}</strong> · {fundingLabels.actualRevenue} {fundingLabels.noFixedBudget}</p> : null}
      <p style={{ marginTop: 12 }}>{labels.configurationHash}: <span className={styles.code} dir="ltr">{data.launchConfigurationSha256 || copy.notConfigured}</span></p>
      {data.launchConfiguration ? <pre className={styles.code} style={{ whiteSpace: "pre-wrap", maxHeight: 360, overflow: "auto" }} dir="ltr">{JSON.stringify(data.launchConfiguration, null, 2)}</pre> : <p>{copy.notConfigured}</p>}
    </details>
    <details className={styles.details}><summary>{labels.launchReview}</summary>
      {data.launchApproval ? <>
        <p>{copy.status}: {data.launchApproval.approvedBy ? copy.approved : copy.pending}</p>
        <p>{labels.manifestHash}: <span className={styles.code} dir="ltr">{data.launchApproval.manifestSha256}</span></p>
        <pre className={styles.code} style={{ whiteSpace: "pre-wrap", maxHeight: 360, overflow: "auto" }} dir="ltr">{JSON.stringify(data.launchApproval.manifest, null, 2)}</pre>
      </> : <p style={{ marginTop: 12 }}>{copy.notConfigured}</p>}
    </details>
  </>;
}

export function PromotionAdmin({ copy, accountCopy, locale, busy, write, campaignId, onChanged }: {
  copy: PromotionCopy; accountCopy: PromotionAccountCopy; locale: PromotionLocale; busy: boolean;
  write: (path: string, body: Record<string, unknown>) => Promise<Record<string, unknown> | null>;
  campaignId: string | null; onChanged: () => void;
}) {
  const labels = PROMOTION_ADMIN_COPY[locale];
  const launchLabels = PROMOTION_LAUNCH_COPY[locale];
  const fundingLabels = PROMOTION_FUNDING_COPY[locale];
  const [data, setData] = useState<AdminData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [action, setAction] = useState<AdminAction>("reviewLeader");
  const [resourceId, setResourceId] = useState("");
  const [decision, setDecision] = useState("approved");
  const [reason, setReason] = useState("");
  const [evidenceRef, setEvidenceRef] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [watermark, setWatermark] = useState("");
  const [network, setNetwork] = useState("");
  const [coinType, setCoinType] = useState("");
  const [reviewDocument, setReviewDocument] = useState("");
  const [reviewHash, setReviewHash] = useState("");
  const [downloading, setDownloading] = useState<string | null>(null);
  const [draft, setDraft] = useState({ fundingMode: "revenue_share" as PromotionFundingMode, termsVersion: "", ...PROMOTION_DRAFT_SEASON_UTC, budget: "", memberLimit: "", leaderLimit: "", perLeaderMemberLimit: "", revenueCap: "", minimumLevel: "30", activeDays: "3", candidateDays: "7", confirmHours: "24", coverageDays: "92", reserveBps: "0", cooldownHours: "72" });
  const refresh = useCallback(async (signal?: AbortSignal) => {
    setLoading(true); setError(null);
    try {
      const next = await promotionRequest<AdminData>("admin", { signal });
      if (signal?.aborted) return;
      if (!Array.isArray(next.roles) || !Array.isArray(next.leaders) || !Array.isArray(next.settlements) || typeof next.payoutEnabled !== "boolean") throw new PromotionApiError(502, "invalid_admin_response");
      setData(next);
    } catch (cause) {
      if (!signal?.aborted) setError(cause instanceof PromotionApiError ? cause.code : "admin_unavailable");
    } finally { if (!signal?.aborted) setLoading(false); }
  }, []);
  useEffect(() => {
    const controller = new AbortController(); void refresh(controller.signal); return () => controller.abort();
  }, [refresh]);

  const actions: { value: AdminAction; label: string; role: string }[] = [
    { value: "reviewLeader", label: accountCopy.reviewApplication, role: "risk" },
    { value: "reconcile", label: accountCopy.reconcile, role: "finance" },
    { value: "createSettlement", label: accountCopy.createSettlement, role: "finance" },
    { value: "approveSettlement", label: accountCopy.approveSettlement, role: "finance" },
    { value: "approveRules", label: accountCopy.approveRules, role: "finance" },
    { value: "recruitOn", label: accountCopy.recruitOn, role: "operations" },
    { value: "recruitOff", label: accountCopy.recruitOff, role: "operations" },
    { value: "commissionOn", label: accountCopy.commissionOn, role: "finance" },
    { value: "commissionOff", label: accountCopy.commissionOff, role: "finance" },
    { value: "setInputWatermark", label: labels.watermark, role: "finance" },
    { value: "createPayoutDraft", label: labels.createPayout, role: "payoutSubmit" },
    { value: "approvePayout", label: labels.approvePayout, role: "payoutReview" },
    { value: "freezePayoutIntent", label: launchLabels.freezeIntent, role: "payoutSubmit" },
    { value: "proposeLaunch", label: launchLabels.proposeLaunch, role: "operations" },
    { value: "approveLaunch", label: launchLabels.approveLaunch, role: "finance" },
    { value: "payoutOn", label: launchLabels.payoutOn, role: "finance" },
    { value: "payoutOff", label: launchLabels.payoutOff, role: "finance" },
  ].filter(item => data?.roles.includes(item.role)) as { value: AdminAction; label: string; role: string }[];
  const selectedAction = actions.find(item => item.value === action)?.value ?? actions[0]?.value;
  const needsLeader = selectedAction === "reviewLeader" || selectedAction === "createSettlement";
  const needsSettlement = selectedAction === "approveSettlement" || selectedAction === "createPayoutDraft";
  const selectedSettlement = data?.settlements.find(row => row.settlementId === resourceId);

  async function complete(body: Record<string, unknown>) {
    if (await write("admin", body)) { setAcknowledged(false); setReason(""); setEvidenceRef(""); setReviewHash(""); setReviewDocument(""); await refresh(); onChanged(); }
  }

  async function downloadInputs(settlementId: string) {
    setDownloading(settlementId); setError(null);
    try {
      const blob = await readPromotionSettlementExport(settlementId);
      const url = URL.createObjectURL(blob), anchor = document.createElement("a");
      anchor.href = url; anchor.download = `numeron-settlement-${settlementId.replace(/[^A-Za-z0-9_-]/g, "_")}.json`;
      document.body.appendChild(anchor); anchor.click(); anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch { setError("settlement_export_unavailable"); }
    finally { setDownloading(null); }
  }

  async function submitAction(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!selectedAction || !acknowledged || !evidenceRef.trim()) return;
    let body: Record<string, unknown> = { action: selectedAction, evidenceRef: evidenceRef.trim() };
    if (selectedAction === "reviewLeader") body = { ...body, leaderId: resourceId, decision, reason: reason.trim() };
    if (selectedAction === "createSettlement") body.leaderId = resourceId;
    if (selectedAction === "approveSettlement") {
      if (!selectedSettlement) return;
      body = { ...body, settlementId: selectedSettlement.settlementId, snapshotSha256: selectedSettlement.snapshotSha256 };
    }
    if (selectedAction === "recruitOn" || selectedAction === "recruitOff") body = { ...body, action: "setRecruitment", enabled: selectedAction === "recruitOn" };
    if (selectedAction === "commissionOn" || selectedAction === "commissionOff") body = { ...body, action: "setCommission", enabled: selectedAction === "commissionOn" };
    if (selectedAction === "setInputWatermark") {
      const value = Date.parse(`${watermark}Z`);
      if (!Number.isSafeInteger(value) || value <= 0) { setError("invalid_source_cutoff"); return; }
      body.watermarkMs = value;
    }
    if (selectedAction === "createPayoutDraft") body = { ...body, settlementId: resourceId, network, coinType: coinType.trim() };
    if (["proposeLaunch", "approveLaunch", "freezePayoutIntent", "approvePayout", "payoutOn", "payoutOff"].includes(selectedAction)) {
      try {
        body = promotionReviewCommand(selectedAction as PromotionReviewAction, { document: reviewDocument, hash: reviewHash, resourceId, evidenceRef });
      } catch { setError("invalid_review_input"); return; }
    }
    await complete(body);
  }

  async function submitDraft(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!campaignId || !acknowledged || !evidenceRef.trim()) return;
    try {
      const integer = (value: string, factor = 1): number => {
        if (!/^\d+$/.test(value)) throw new Error("invalid_integer");
        const parsed = Number(value) * factor;
        if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error("invalid_integer");
        return parsed;
      };
      const startsAtMs = Date.parse(`${draft.startsAt}Z`), endsAtMs = Date.parse(`${draft.endsAt}Z`);
      if (!Number.isSafeInteger(startsAtMs) || !Number.isSafeInteger(endsAtMs) || endsAtMs <= startsAtMs) throw new Error("invalid_dates");
      await complete({ action: "configureDraft", evidenceRef: evidenceRef.trim(), draft: {
        campaignId, termsVersion: draft.termsVersion.trim(), startsAtMs, endsAtMs,
        ...promotionFundingLimits(draft.fundingMode,draft),
        rules: { minimumLevel: integer(draft.minimumLevel), activeUtcDays: integer(draft.activeDays), candidateTtlMs: integer(draft.candidateDays, 86_400_000),
          confirmationWindowMs: integer(draft.confirmHours, 3_600_000), maximumCoverageMs: integer(draft.coverageDays, 86_400_000),
          reserveBps: integer(draft.reserveBps), walletCooldownMs: integer(draft.cooldownHours, 3_600_000) },
      } });
    } catch { setError("invalid_draft_input"); }
  }

  return <section className={styles.section}>
    <div className={styles.sectionHeading}><div><h2>{copy.adminTitle}</h2><p>{copy.adminBody}</p></div><button className={styles.secondary} disabled={loading || busy} onClick={() => void refresh()}>{copy.refresh}</button></div>
    {error ? <div role="alert" className={`${styles.notice} ${styles.error}`}>{error === "invalid_review_input" ? launchLabels.invalidInput : error === "invalid_draft_input" ? copy.unavailableAction : copy.unavailable}</div> : null}
    {loading && !data ? <p role="status" className={styles.empty}>{copy.loading}</p> : null}
    {data ? <>
      <p className={styles.muted}>{copy.permission}: <span className={styles.code}>{data.roles.join(" · ")}</span></p>
      {data.campaign?.fundingMode === "revenue_share" ? <div className={styles.notice}><strong>{fundingLabels.revenueShare}</strong>{fundingLabels.actualRevenue} {fundingLabels.noFixedBudget}</div> : <div className={styles.cards} style={{ marginTop: 18 }}><div className={styles.card}><p className={styles.cardTitle}>{labels.budget}</p><p className={styles.metric}>{formatPromotionUsd(data.campaign?.budgetMicroUsd, locale)}</p></div>
        <div className={styles.card}><p className={styles.cardTitle}>{labels.reserved}</p><p className={styles.metric}>{formatPromotionUsd(data.campaign?.reservedMicroUsd, locale)}</p></div></div>}
      <PromotionLaunchReview data={data} copy={copy} locale={locale} />
      {data.roles.includes("operations") ? <details className={styles.details}><summary>{labels.configure}</summary><p style={{ marginTop: 12 }}>{copy.proposalBody}</p>
        <form className={styles.form} onSubmit={event => void submitDraft(event)}>
          <label className={styles.field}>{copy.season}<input value={campaignId || ""} readOnly required /></label>
          <label className={styles.field}>{fundingLabels.mode}<select value={draft.fundingMode} onChange={event => { setDraft(current => ({ ...current, fundingMode: event.target.value as PromotionFundingMode })); setAcknowledged(false); }}><option value="revenue_share">{fundingLabels.revenueShare}</option><option value="budget_capped">{fundingLabels.cappedBudget}</option></select></label>
          {draft.fundingMode === "revenue_share" ? <p className={styles.muted}>{fundingLabels.actualRevenue} {fundingLabels.noFixedBudget}</p> : null}
          {([
            ["termsVersion", copy.termsVersion, "text"], ["startsAt", `${copy.start} (UTC)`, "datetime-local"], ["endsAt", `${copy.end} (UTC)`, "datetime-local"],
            ["budget", labels.budget, "text"], ["memberLimit", labels.memberLimit, "number"], ["leaderLimit", labels.leaderLimit, "number"], ["perLeaderMemberLimit", labels.perLeaderLimit, "number"],
            ["revenueCap", labels.revenueCap, "text"], ["minimumLevel", labels.minimumLevel, "number"], ["activeDays", labels.activeDays, "number"],
            ["candidateDays", labels.candidateDays, "number"], ["confirmHours", labels.confirmHours, "number"], ["coverageDays", labels.coverageDays, "number"],
            ["reserveBps", labels.reserveBps, "number"], ["cooldownHours", labels.cooldownHours, "number"],
          ] as const).filter(([key]) => draft.fundingMode === "budget_capped" || !["budget", "memberLimit", "leaderLimit", "perLeaderMemberLimit", "revenueCap"].includes(key)).map(([key, label, type]) => <label key={key} className={styles.field}>{label}<input value={draft[key]} type={type} min={type === "number" ? 0 : undefined} step={type === "number" ? 1 : undefined} maxLength={type === "text" ? 128 : undefined} required onChange={event => setDraft(current => ({ ...current, [key]: event.target.value }))} /></label>)}
          <label className={styles.field}>{copy.actorEvidence}<input value={evidenceRef} onChange={event => setEvidenceRef(event.target.value)} minLength={3} maxLength={512} required /></label>
          <label className={styles.check}><input type="checkbox" checked={acknowledged} onChange={event => setAcknowledged(event.target.checked)} />{copy.acknowledge}</label>
          <button className={styles.secondary} disabled={busy || !campaignId || !acknowledged || !evidenceRef.trim()}>{labels.configure}</button>
        </form>
      </details> : null}
      {data.leaders.length ? <div className={styles.tableWrap}><table className={styles.table}><thead><tr><th>{copy.resourceId}</th><th>{copy.characters}</th><th>{copy.status}</th></tr></thead><tbody>{data.leaders.map(row => <tr key={row.leaderId}><td className={styles.code}>{row.leaderId}</td><td>{row.displayName}</td><td>{row.status}</td></tr>)}</tbody></table></div> : null}
      {data.settlements.length ? <div className={styles.tableWrap}><table className={styles.table}><thead><tr><th>{copy.settlementId}</th><th>{copy.amount}</th><th>{labels.snapshot}</th><th>{copy.status}</th>{data.roles.some(role => ["finance", "auditor"].includes(role)) ? <th>{copy.action}</th> : null}</tr></thead><tbody>{data.settlements.map(row => <tr key={row.settlementId}><td className={styles.code}>{row.settlementId}</td><td>{formatPromotionUsd(row.totalMicroUsd, locale)}</td><td className={styles.code}>{row.snapshotSha256}</td><td>{row.status}</td>{data.roles.some(role => ["finance", "auditor"].includes(role)) ? <td><button className={styles.secondary} disabled={downloading !== null} onClick={() => void downloadInputs(row.settlementId)}>{downloading === row.settlementId ? copy.loading : PROMOTION_EXPORT_COPY[locale]}</button></td> : null}</tr>)}</tbody></table></div> : null}
      {actions.length ? <form className={styles.form} onSubmit={event => void submitAction(event)}>
        <label className={styles.field}>{copy.action}<select value={selectedAction || ""} onChange={event => { setAction(event.target.value as AdminAction); setResourceId(""); setAcknowledged(false); setReviewHash(""); setReviewDocument(""); }}>{actions.map(item => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label>
        {needsLeader ? <label className={styles.field}>{copy.resourceId}<select required value={resourceId} onChange={event => setResourceId(event.target.value)}><option value="">—</option>{data.leaders.map(row => <option key={row.leaderId} value={row.leaderId}>{row.displayName} · {row.leaderId}</option>)}</select></label> : null}
        {needsSettlement ? <label className={styles.field}>{copy.settlementId}<select required value={resourceId} onChange={event => setResourceId(event.target.value)}><option value="">—</option>{data.settlements.map(row => <option key={row.settlementId} value={row.settlementId}>{row.settlementId} · {formatPromotionUsd(row.totalMicroUsd, locale)}</option>)}</select></label> : null}
        {selectedAction === "reviewLeader" ? <>
          <label className={styles.field}>{labels.decision}<select value={decision} onChange={event => setDecision(event.target.value)}><option value="approved">{copy.approved}</option><option value="restricted">{copy.recruitment} · {copy.paused}</option><option value="suspended">{copy.paused}</option><option value="closed">{copy.closed}</option></select></label>
          <label className={styles.field}>{copy.reasonRequired}<textarea value={reason} onChange={event => setReason(event.target.value)} required minLength={3} maxLength={1000} /></label>
        </> : null}
        {selectedAction === "approveSettlement" && selectedSettlement ? <p className={styles.code}>{labels.snapshot}: {selectedSettlement.snapshotSha256}</p> : null}
        {selectedAction === "setInputWatermark" ? <label className={styles.field}>{labels.watermark}<input type="datetime-local" value={watermark} onChange={event => setWatermark(event.target.value)} required /></label> : null}
        {selectedAction === "createPayoutDraft" ? <>
          <label className={styles.field}>{copy.network}<select value={network} onChange={event => setNetwork(event.target.value)} required><option value="">{copy.notConfigured}</option><option value={data.environment === "production" ? "sui:mainnet" : "sui:testnet"}>{data.environment === "production" ? "Sui Mainnet" : "Sui Testnet"}</option></select></label>
          <label className={styles.field}>{copy.asset}<input value={coinType} onChange={event => setCoinType(event.target.value)} required maxLength={256} /></label>
        </> : null}
        {selectedAction === "approvePayout" ? <label className={styles.field}>{copy.resourceId}<input value={resourceId} onChange={event => setResourceId(event.target.value)} maxLength={128} required /></label> : null}
        {selectedAction === "proposeLaunch" || selectedAction === "freezePayoutIntent" ? <>
          <p className={styles.muted}>{launchLabels.documentHelp}</p>
          {selectedAction === "proposeLaunch" ? <p className={styles.muted}>{fundingLabels.manifestRequirements}</p> : null}
          <label className={styles.field}>{selectedAction === "proposeLaunch" ? launchLabels.manifestDocument : launchLabels.intentDocument}<textarea dir="ltr" spellCheck={false} value={reviewDocument} onChange={event => { setReviewDocument(event.target.value); setAcknowledged(false); }} required maxLength={12000} rows={10} /></label>
        </> : null}
        {selectedAction === "approveLaunch" || selectedAction === "approvePayout" ? <label className={styles.field}>{selectedAction === "approveLaunch" ? launchLabels.manifestHash : launchLabels.intentHash}<input dir="ltr" spellCheck={false} autoComplete="off" value={reviewHash} onChange={event => { setReviewHash(event.target.value); setAcknowledged(false); }} pattern="[0-9a-f]{64}" minLength={64} maxLength={64} required /></label> : null}
        <label className={styles.field}>{copy.actorEvidence}<input value={evidenceRef} onChange={event => setEvidenceRef(event.target.value)} required minLength={3} maxLength={512} /></label>
        <label className={styles.check}><input type="checkbox" checked={acknowledged} onChange={event => setAcknowledged(event.target.checked)} />{copy.acknowledge}</label>
        <button className={styles.primary} disabled={busy || !acknowledged || !evidenceRef.trim()}>{busy ? copy.loading : copy.submit}</button>
      </form> : <p className={styles.empty}>{copy.unavailableAction}</p>}
    </> : null}
  </section>;
}
