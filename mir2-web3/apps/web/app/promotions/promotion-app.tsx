"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { PROMOTION_ACCOUNT_COPY, promotionInvitationError } from "./promotion-account-copy";
import { PROMOTION_EARNINGS_COPY } from "./promotion-earnings-copy";
import { PROMOTION_MIGRATION_COPY, promotionMigrationError } from "./promotion-migration-copy";
import { createPromotionRefresh } from "./promotion-refresh";
import { PROMOTION_COPY, PROMOTION_LANGUAGES, normalizePromotionLocale, type PromotionCopy, type PromotionLocale, type PromotionTextKey } from "./promotion-copy";
import { canEnrollTestWallet, hasTestWalletEnrollment, testWalletApplication, formatPromotionCents, formatPromotionDate, formatPromotionRefreshTime, formatPromotionUsd, promotionRequest, promotionTimezone, PromotionApiError,
  type InvitationPreview, type PromotionBootstrap, type PromotionCampaign, type PromotionLeader, type PromotionMember, type PromotionSettlement } from "./promotion-model";
import { PromotionWalletLogin } from "./promotion-wallet-login";
import { PromotionAdmin } from "./promotion-admin";
import { PromotionBilling, PROMOTION_BILLING_LABELS } from "./promotion-billing";
import styles from "./promotion.module.css";

type View = "overview" | "characters" | "members" | "earnings" | "payout" | "rules" | "support" | "admin" | "billing";
function migrationContext(bootstrap: PromotionBootstrap | null) {
  const proof = bootstrap?.me?.migrationCandidate;
  return JSON.stringify([bootstrap?.authenticated, bootstrap?.me?.account.id, bootstrap?.binding?.credentialId,
    bootstrap?.campaign?.campaignId, bootstrap?.campaign?.termsVersion, proof?.migrationId, proof?.candidateId,
    proof?.targetTermsVersion, proof?.preparedAtMs, proof?.expiresAtMs, proof?.sourceCampaignId,
    proof?.sourceConfirmedAtMs, proof?.eligibilityAnchorMs, proof?.leaderDisplayName]);
}
const NAVIGATION: readonly { view: View; icon: string }[] = [
  { view: "overview", icon: "◈" }, { view: "characters", icon: "♙" }, { view: "members", icon: "♧" },
  { view: "earnings", icon: "▤" }, { view: "payout", icon: "◇" }, { view: "rules", icon: "≡" },
  { view: "billing", icon: "◉" },
  { view: "support", icon: "?" }, { view: "admin", icon: "⚙" },
];
const STATUS_KEYS: Record<string, PromotionTextKey> = {
  applied: "pending", pending: "pending", review: "pending", under_review: "pending", approved: "approved", verified: "confirmed", linked: "confirmed",
  rejected: "rejected", expired: "expired", paused: "paused", suspended: "paused", restricted: "paused", closed: "closed", draft: "draft",
  unknown: "unknown", result_unknown: "unknown", submitted: "pending", confirmed: "confirmed", paid: "confirmed", failed: "failed", cancelled: "cancelled",
  qualified: "qualified", excluded: "excluded", bound: "bound", attributed: "bound", conditions_pending: "conditionsPending", incomplete: "conditionsPending",
  open: "pending", unverified: "pending", unconfigured: "notConfigured", candidate: "pending", approved_pending_payment: "pending",
};

export function PromotionStatus({ value, copy }: { value: string | undefined; copy: PromotionCopy }) {
  const key = value ? STATUS_KEYS[value] : undefined;
  const good = value && ["approved", "verified", "confirmed", "qualified", "paid", "linked"].includes(value);
  return <span className={`${styles.pill} ${good ? styles.good : styles.warning}`}>{key ? copy[key] : copy.pending}</span>;
}

export function PromotionDisclosure({ copy, gaps, environment }: { copy: PromotionCopy; gaps: string[]; environment?: string }) {
  const explanations: Record<string, string> = {
    historical_credit_funding_untraceable: copy.rules1, authoritative_gameplay_events_required: copy.rules2,
    payment_tax_refund_reconciliation_required: copy.estimateNote,
    production_acceptance_and_launch_approval_required: copy.rules5,
    payment_source_reconciliation_required: copy.estimateNote,
    committed_game_events_pending: copy.rules2,
    payout_currency_regions_and_terms_unapproved: copy.payoutBody, payout_execution_not_implemented: copy.rules6,
  };
  return <>
    {environment === "test" ? <div className={styles.notice}><strong>{copy.testnet}</strong>{copy.testnetBody}</div> : null}
    <div className={styles.notice}><strong>{copy.proposal}</strong>{copy.proposalBody}</div>
    {gaps.length ? <details className={styles.details}><summary>{copy.sourceGaps} ({gaps.length})</summary>
      <ul>{gaps.map(code => <li key={code}>{explanations[code] || copy.notConfigured}</li>)}</ul>
    </details> : null}
  </>;
}

export function PromotionReceivingWallet({ bootstrap, copy, locale, busy, onVerify }: {
  bootstrap: PromotionBootstrap; copy: PromotionCopy; locale: PromotionLocale; busy: boolean; onVerify: () => void;
}) {
  const settings = bootstrap.me?.payoutSettings;
  const policy = bootstrap.campaign?.payoutPolicy;
  const environment = bootstrap.campaign?.environment === "test" || bootstrap.public?.environment === "test"
    ? "test" : bootstrap.campaign?.environment;
  const enabled = bootstrap.campaign?.payoutEnabled === true;
  const canVerify = bootstrap.binding?.status === "linked" && !!bootstrap.binding.credentialId;
  return <div className={styles.panel}>
    {environment === "test" ? <div className={styles.notice}><strong>{copy.testnet}</strong>{copy.testnetBody}</div> : null}
    <h3 style={environment === "test" ? { marginTop: 16 } : undefined}>{settings?.status === "verified" ? PROMOTION_ACCOUNT_COPY[locale].verifiedWallet : copy.noPayout}</h3>
    <dl className={styles.form}>
      <div><dt className={styles.muted}>{copy.network}</dt><dd className={styles.code}>{policy?.network || settings?.network || bootstrap.binding?.network || copy.notConfigured}</dd></div>
      <div><dt className={styles.muted}>{copy.address}</dt><dd className={styles.code}>{settings?.address || "—"}</dd></div>
      <div><dt className={styles.muted}>{copy.asset}</dt><dd>{policy ? <>
        <span className={styles.code}>{policy.symbol}</span>{" "}<PromotionStatus value={policy.status} copy={copy} />
        <div className={styles.code}>{policy.coinType}</div>
      </> : copy.notConfigured}</dd></div>
      {settings?.cooldownUntilMs ? <div><dt className={styles.muted}>{copy.cooldown}</dt><dd>{formatPromotionDate(settings.cooldownUntilMs, locale)}</dd></div> : null}
    </dl>
    <button className={styles.secondary} disabled={busy || !canVerify} onClick={onVerify}>{copy.verifyAddress}</button>
    <div className={styles.notice}>{copy.payments} · {bootstrap.campaign ? enabled ? copy.on : copy.off : copy.notConfigured}</div>
  </div>;
}

export function PromotionTestWalletEnrollment({ campaign, copy, locale, bound, acceptedTerms, busy, leader, onAccept, onSubmit }: {
  campaign: PromotionCampaign; copy: PromotionCopy; locale: PromotionLocale; bound: boolean; acceptedTerms: boolean; busy: boolean;
  leader: PromotionLeader | null | undefined; onAccept: (accepted: boolean) => void; onSubmit: () => void;
}) {
  if (!hasTestWalletEnrollment(campaign)) return null;
  return <section className={styles.panel}>
    <h3>{copy.testEnrollmentTitle}</h3><p className={styles.muted}>{copy.testEnrollmentBody}</p>
    <dl className={styles.testEnrollmentDates}><div><dt>{copy.start}</dt><dd>{formatPromotionDate(campaign.startsAtMs, locale)}</dd></div>
      <div><dt>{copy.end}</dt><dd>{formatPromotionDate(campaign.endsAtMs, locale)}</dd></div></dl>
    <form className={styles.form} onSubmit={event => { event.preventDefault(); if (!busy && bound && acceptedTerms && !leader && canEnrollTestWallet(campaign)) onSubmit(); }}>
      <label className={styles.check}><input type="checkbox" checked={acceptedTerms} onChange={event => onAccept(event.target.checked)} disabled={busy || !!leader} />{copy.acceptTerms} ({campaign.termsVersion || copy.notConfigured})</label>
      <button className={styles.primary} disabled={busy || !bound || !acceptedTerms || !!leader || !canEnrollTestWallet(campaign)}>{busy ? copy.loading : copy.testEnrollmentAction}</button>
      {!canEnrollTestWallet(campaign) ? <p className={styles.muted}>{copy.unavailableAction}</p> : null}
    </form>
  </section>;
}

function RulePanel({ copy, campaign }: { copy: PromotionCopy; campaign?: PromotionCampaign | null }) {
  const level = campaign?.rules?.minimumLevel, days = campaign?.rules?.activeUtcDays;
  const eligibility = campaign?.termsStatus === "approved" && typeof level === "number" && Number.isSafeInteger(level) && level > 0 &&
    typeof days === "number" && Number.isSafeInteger(days) && days > 0
    ? copy.configuredEligibility.replace("{level}", String(level)).replace("{days}", String(days)) : copy.rules2;
  return <section className={styles.section}>
    <div className={styles.sectionHeading}><div><h2>{copy.rules}</h2><p>{copy.termsBody}</p></div></div>
    <div className={styles.cards}>
      {(["tier1", "tier2", "tier3"] as const).map((key, index) => <div key={key} className={`${styles.card} ${styles.tier}`}>
        <span className={styles.cardTitle}>{copy[key]}</span><strong>{["10%", "20%", "30%"][index]}</strong>
      </div>)}
    </div>
    <p className={styles.muted} style={{ marginTop: 13 }}>{copy.rateProposal}</p>
    <ol className={styles.rulesList}>{(["rules1", "rules2", "rules3", "rules4", "rules5", "rules6", "rules7"] as const).map(key => <li key={key}>{key === "rules2" ? eligibility : copy[key]}</li>)}</ol>
  </section>;
}

function InvitationQr({ link, alt }: { link: string; alt: string }) {
  const [image, setImage] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void import("qrcode").then(({ default: QRCode }) => QRCode.toDataURL(link, { width: 184, margin: 3 }))
      .then(value => { if (active) setImage(value); }).catch(() => { if (active) setImage(null); });
    return () => { active = false; };
  }, [link]);
  return image ? <img className={styles.inviteQr} src={image} width={164} height={164} alt={alt} /> : null;
}

export function PromotionApp({ invitationCode }: { invitationCode?: string }) {
  const [locale, setLocale] = useState<PromotionLocale>("en");
  const [timeZone, setTimeZone] = useState("—");
  const [view, setView] = useState<View>("overview");
  const [bootstrap, setBootstrap] = useState<PromotionBootstrap | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Invitation writes have their own feedback: a successful background read
  // must not dismiss a rejected or uncertain attribution request.
  const [invitationError, setInvitationError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [walletOpen, setWalletOpen] = useState(false);
  const [characterIndex, setCharacterIndex] = useState<number | null>(null);
  const [acceptedTerms, setAcceptedTerms] = useState(false);
  // Consent belongs to this exact identity, activity and migration proof. A
  // previous render's checkbox can never authorize a replacement proof.
  const [acceptedMigrationTerms, setAcceptedMigrationTerms] = useState<string | null>(null);
  const [migrationFeedback, setMigrationFeedback] = useState<{ context: string; error?: string; submitted?: boolean } | null>(null);
  const [, setMigrationExpiryTick] = useState(0);
  const [account, setAccount] = useState("");
  const [password, setPassword] = useState("");
  const [preview, setPreview] = useState<InvitationPreview | null>(null);
  const [inviteCode, setInviteCode] = useState(invitationCode || "");
  const [candidate, setCandidate] = useState<{ candidateId: string; leaderDisplayName: string; expiresAtMs: number } | null>(null);
  const [members, setMembers] = useState<PromotionMember[] | null>(null);
  const [memberOffset, setMemberOffset] = useState(0);
  const [nextMemberOffset, setNextMemberOffset] = useState<number | null>(null);
  const [settlements, setSettlements] = useState<PromotionSettlement[] | null>(null);
  const [listLoading, setListLoading] = useState(false);
  const [lastUpdatedMs, setLastUpdatedMs] = useState<number | null>(null);
  const [appealCategory, setAppealCategory] = useState("other");
  const [appealMessage, setAppealMessage] = useState("");
  const [invitationLink, setInvitationLink] = useState<string | null>(null);
  const [clickRecorded, setClickRecorded] = useState(false);
  const mutationKeys = useRef(new Map<string, string>());
  const refreshGeneration = useRef(0);
  const refreshAbort = useRef<AbortController | null>(null);
  const refreshContext = useRef({ view, memberOffset, bootstrap, loading, busy, walletOpen });
  refreshContext.current = { view, memberOffset, bootstrap, loading, busy, walletOpen };
  const clickKey = useRef<{ code: string; id: string } | null>(null);
  const app = useRef<HTMLDivElement>(null);
  const existingAccountSection = useRef<HTMLDetailsElement>(null);
  const copy = PROMOTION_COPY[locale];
  const earningsCopy = PROMOTION_EARNINGS_COPY[locale];
  const homeLocale = locale === "zh-Hans" ? "zh-CN" : locale === "zh-Hant" ? "zh-TW" : locale === "hi" ? "en" : locale;
  const accountCopy = PROMOTION_ACCOUNT_COPY[locale];
  const migrationCopy = PROMOTION_MIGRATION_COPY[locale];
  const authenticated = bootstrap?.authenticated === true;
  const bound = bootstrap?.binding?.status === "linked";
  const campaign = bootstrap?.campaign;
  const testWalletEnrollment = hasTestWalletEnrollment(campaign);
  const me = bootstrap?.me;
  const migrationCandidate = !me?.attribution ? me?.migrationCandidate : undefined;
  const currentMigrationContext = migrationContext(bootstrap);
  const migrationTermsAccepted = acceptedMigrationTerms === currentMigrationContext;
  const currentMigrationFeedback = migrationFeedback?.context === currentMigrationContext ? migrationFeedback : null;
  const migrationProofValid = !!migrationCandidate && [migrationCandidate.migrationId, migrationCandidate.candidateId,
    migrationCandidate.leaderDisplayName, migrationCandidate.sourceCampaignId, migrationCandidate.targetTermsVersion]
    .every(value => typeof value === "string" && value.trim().length > 0) &&
    [migrationCandidate.sourceConfirmedAtMs, migrationCandidate.eligibilityAnchorMs, migrationCandidate.preparedAtMs, migrationCandidate.expiresAtMs]
      .every(value => Number.isSafeInteger(value) && value > 0 && Number.isFinite(new Date(value).getTime())) &&
    migrationCandidate.sourceConfirmedAtMs <= migrationCandidate.preparedAtMs &&
    migrationCandidate.eligibilityAnchorMs <= migrationCandidate.preparedAtMs && migrationCandidate.preparedAtMs < migrationCandidate.expiresAtMs;
  const migrationExpired = !!migrationCandidate && migrationCandidate.expiresAtMs <= Date.now();
  const migrationTermsCurrent = !!migrationCandidate && !!campaign?.termsVersion && campaign.termsStatus === "approved" &&
    migrationCandidate.targetTermsVersion === campaign.termsVersion;
  const canConfirmMigration = authenticated && bound && !!bootstrap?.csrfToken && migrationProofValid && !migrationExpired &&
    migrationTermsCurrent && campaign?.status === "active" && campaign.recruitmentEnabled && migrationTermsAccepted &&
    !busy && !currentMigrationFeedback?.submitted;
  const characters = bootstrap?.characters || [];
  const leader = bootstrap?.leader;
  const dashboard = bootstrap?.dashboard;
  const liveUpdatesEnabled = authenticated && !invitationCode && ["overview", "members", "earnings"].includes(view);
  const commissionTitle = campaign?.status === "locked" ? earningsCopy.finalizedCommission : earningsCopy.accruedCommission;
  const settlementNote = campaign?.status === "locked" ? earningsCopy.finalizedNote : earningsCopy.settlementTiming;
  const canAdmin = Array.isArray(bootstrap?.permissions) ? bootstrap.permissions.includes("isAdmin") : bootstrap?.permissions?.isAdmin === true;
  const errorMessage = (code: string | null) => code === "session_expired" ? copy.sessionExpired : code && (promotionMigrationError(code, locale) || promotionInvitationError(code, locale) || ({
    promotionBudgetFull: copy.inviteClosed, promotionRecruitmentClosed: copy.inviteClosed,
    promotionConfirmationExpired: copy.expired, promotionCandidateExpired: copy.expired,
    promotionSelfReferral: copy.rules3, promotionExistingAccountExcluded: copy.rules3,
    promotionAlreadyPaid: copy.rules4, promotionAttributionFrozen: accountCopy.finalAttributionBody,
    promotionCharacterMissing: copy.noCharacters, promotionLevelRequired: copy.rules2,
    promotionTermsUnapproved: copy.awaitingApproval, promotionTermsChanged: copy.awaitingApproval,
    promotionDistinctReviewerRequired: copy.adminBody, promotionPermissionDenied: copy.unavailableAction,
    promotionPayoutExecutionDisabled: copy.rules6,
  } as Record<string, string>)[code]) || copy.unavailable;
  const errorText = errorMessage(error);

  const refresh = useCallback(async (signal?: AbortSignal, background = false) => {
    const generation = ++refreshGeneration.current;
    refreshAbort.current?.abort();
    const controller = new AbortController();
    refreshAbort.current = controller;
    const requestSignal = signal ? AbortSignal.any([signal, controller.signal]) : controller.signal;
    const context = refreshContext.current;
    if (!background) setLoading(true);
    try {
      const next = await promotionRequest<PromotionBootstrap>("bootstrap", { signal: requestSignal });
      if (requestSignal.aborted || refreshGeneration.current !== generation) return;
      if (typeof next.authenticated !== "boolean" || !Array.isArray(next.characters)) throw new PromotionApiError(502, "invalid_bootstrap");
      const identityChanged = !next.authenticated || context.bootstrap?.me?.account.id !== next.me?.account.id ||
        context.bootstrap?.binding?.credentialId !== next.binding?.credentialId;
      const invitationContextChanged = identityChanged || context.bootstrap?.campaign?.campaignId !== next.campaign?.campaignId ||
        context.bootstrap?.campaign?.termsVersion !== next.campaign?.termsVersion;
      let nextMembers: { items: PromotionMember[]; nextOffset: number | null } | undefined;
      let nextSettlements = Array.isArray(next.me?.settlements) ? next.me.settlements : undefined;
      if (next.authenticated && context.view === "members") {
        nextMembers = await promotionRequest<{ items: PromotionMember[]; nextOffset: number | null }>(`members?offset=${identityChanged ? 0 : context.memberOffset}&limit=20`, { signal: requestSignal });
        if (!Array.isArray(nextMembers.items)) throw new PromotionApiError(502, "invalid_members_response");
      } else if (next.authenticated && context.view === "earnings" && !nextSettlements) {
        const records = await promotionRequest<{ items: PromotionSettlement[] }>("settlements", { signal: requestSignal });
        if (!Array.isArray(records.items)) throw new PromotionApiError(502, "invalid_settlements_response");
        nextSettlements = records.items;
      }
      if (requestSignal.aborted || refreshGeneration.current !== generation) return;
      if (identityChanged) {
        setMembers(null); setSettlements(null); setNextMemberOffset(null); setMemberOffset(0);
      }
      if (invitationContextChanged) {
        setInvitationError(null); setCandidate(null); setAcceptedTerms(false); setNotice(null);
        setAcceptedMigrationTerms(null); setMigrationFeedback(null);
      }
      setBootstrap(next); setError(null);
      if (nextMembers) { setMembers(nextMembers.items); setNextMemberOffset(nextMembers.nextOffset); }
      if (nextSettlements) setSettlements(nextSettlements);
      const asOf = next.dashboard?.asOfMs;
      setLastUpdatedMs(next.authenticated ? typeof asOf === "number" && Number.isSafeInteger(asOf) && asOf > 0 && Number.isFinite(new Date(asOf).getTime()) ? asOf : Date.now() : null);
      setCharacterIndex(current => next.characters.some(character => character.index === current) ? current : next.characters[0]?.index ?? null);
      if (next.leader?.inviteCode) {
        const invitation = new URL(`/promotions/invite/${encodeURIComponent(next.leader.inviteCode)}`, window.location.origin);
        if (next.campaign?.campaignId) invitation.searchParams.set("campaignId", next.campaign.campaignId);
        setInvitationLink(invitation.href);
      } else setInvitationLink(null);
    } catch (cause) {
      if (requestSignal.aborted || refreshGeneration.current !== generation) return;
      if (cause instanceof PromotionApiError && cause.status === 401) { setBootstrap(null); setMembers(null); setSettlements(null); }
      setError(cause instanceof PromotionApiError && cause.status === 401 ? "session_expired" : cause instanceof PromotionApiError ? cause.code : "network_unavailable");
    } finally {
      if (refreshGeneration.current === generation) { setLoading(false); setListLoading(false); }
      if (refreshAbort.current === controller) refreshAbort.current = null;
    }
  }, []);

  useEffect(() => {
    let initial = "en";
    try { initial = localStorage.getItem("numeron-language") || navigator.language; } catch { initial = navigator.language; }
    const requestedLanguage = new URLSearchParams(location.search).get("lang");
    const initialLocale = normalizePromotionLocale(requestedLanguage || initial);
    setLocale(initialLocale);
    setTimeZone(promotionTimezone());
    if (requestedLanguage) {
      try { localStorage.setItem("numeron-language", initialLocale); } catch { /* Optional language preference. */ }
    }
    const hash = location.hash.slice(1);
    if (NAVIGATION.some(item => item.view === hash)) setView(hash as View);
    const controller = new AbortController();
    void refresh(controller.signal);
    return () => { controller.abort(); refreshAbort.current?.abort(); ++refreshGeneration.current; };
  }, [refresh]);

  useEffect(() => {
    if (!liveUpdatesEnabled) return;
    const updates = createPromotionRefresh({
      refresh: signal => refresh(signal, true),
      canRefresh: () => {
        const current = refreshContext.current;
        return document.visibilityState === "visible" && current.bootstrap?.authenticated === true &&
          !current.busy && !current.loading && !current.walletOpen && refreshAbort.current === null &&
          ["overview", "members", "earnings"].includes(current.view);
      },
    });
    window.addEventListener("focus", updates.request);
    document.addEventListener("visibilitychange", updates.request);
    return () => {
      updates.stop();
      window.removeEventListener("focus", updates.request);
      document.removeEventListener("visibilitychange", updates.request);
    };
  }, [liveUpdatesEnabled, view, bootstrap?.binding?.credentialId, refresh]);

  useEffect(() => {
    const previous = { language: document.documentElement.lang, direction: document.documentElement.dir, title: document.title };
    document.documentElement.lang = locale;
    document.documentElement.dir = locale === "ar" ? "rtl" : "ltr";
    document.title = `${copy.program} · Numeron`;
    return () => {
      document.documentElement.lang = previous.language;
      document.documentElement.dir = previous.direction;
      document.title = previous.title;
    };
  }, [locale, copy.program]);

  useEffect(() => {
    if (!invitationCode) return;
    setClickRecorded(false); setInviteCode(invitationCode); setCandidate(null); setPreview(null);
    setAcceptedTerms(false); setInvitationError(null); setNotice(null);
    const controller = new AbortController();
    if (clickKey.current?.code !== invitationCode) clickKey.current = { code: invitationCode, id: crypto.randomUUID() };
    void promotionRequest<InvitationPreview>(`invite?code=${encodeURIComponent(invitationCode)}`, { signal: controller.signal })
      .then(async value => {
        if (controller.signal.aborted) return;
        setPreview(value);
        await promotionRequest("click", { body: { code: invitationCode }, signal: controller.signal, idempotencyKey: clickKey.current!.id });
        if (!controller.signal.aborted) setClickRecorded(true);
      })
      .catch(cause => { if (!controller.signal.aborted) {
        const code = cause instanceof PromotionApiError ? cause.code : "invitation_unavailable";
        setError(code); setInvitationError(code);
      } });
    return () => controller.abort();
  }, [invitationCode]);

  useEffect(() => {
    if (!authenticated || !["members", "earnings"].includes(view)) return;
    const controller = new AbortController();
    setListLoading(true); setError(null);
    void refresh(controller.signal, true);
    return () => controller.abort();
  }, [authenticated, view, memberOffset, refresh]);

  useEffect(() => {
    setAcceptedTerms(false); setCandidate(null); setInvitationError(null); setNotice(null);
  }, [bootstrap?.me?.account.id, bootstrap?.binding?.credentialId, campaign?.campaignId, campaign?.termsVersion, authenticated]);

  useEffect(() => {
    setAcceptedMigrationTerms(null); setMigrationFeedback(null);
  }, [currentMigrationContext]);

  useEffect(() => {
    if (!migrationCandidate || !Number.isSafeInteger(migrationCandidate.expiresAtMs)) return;
    const timer = window.setTimeout(() => setMigrationExpiryTick(current => current + 1),
      Math.min(2_147_483_647, Math.max(0, migrationCandidate.expiresAtMs - Date.now() + 1)));
    return () => window.clearTimeout(timer);
  }, [currentMigrationContext, migrationCandidate?.expiresAtMs]);

  async function write(path: string, body: Record<string, unknown>, feedback?: "migration"): Promise<Record<string, unknown> | null> {
    const migrationWrite = feedback === "migration";
    const invitationWrite = path === "candidate" || path === "confirm";
    if (busy) return null;
    if (!bootstrap?.csrfToken) {
      if (migrationWrite) setMigrationFeedback({ context: currentMigrationContext, error: "session_expired" });
      else (invitationWrite ? setInvitationError : setError)("session_expired");
      return null;
    }
    cancelRefresh();
    setBusy(true); setError(null); setNotice(null);
    if (migrationWrite) setMigrationFeedback(null);
    else if (invitationWrite) setInvitationError(null);
    // Retry an uncertain operation with its original key, never with a new payment/ledger intent.
    const intent = `${path}:${JSON.stringify(body)}`;
    const key = mutationKeys.current.get(intent) || crypto.randomUUID();
    mutationKeys.current.set(intent, key);
    try {
      const result = await promotionRequest<Record<string, unknown>>(path, { body, csrfToken: bootstrap.csrfToken, idempotencyKey: key });
      mutationKeys.current.delete(intent);
      const id = result.appealId || result.leaderId || result.settlementId || result.id;
      if (migrationWrite) setMigrationFeedback({ context: currentMigrationContext, submitted: true });
      else if (path === "confirm") setNotice("invitation_confirmed");
      else if (!invitationWrite) setNotice(typeof id === "string" ? id : typeof result.status === "string" ? result.status : "recorded");
      return result;
    } catch (cause) {
      // A retired migration request cannot expire a replacement identity or
      // proof, nor replace feedback belonging to that newer context.
      if (migrationWrite && migrationContext(refreshContext.current.bootstrap) !== currentMigrationContext) return null;
      if (cause instanceof PromotionApiError && cause.status === 401) { setBootstrap(null); setMembers(null); setSettlements(null); setError("session_expired"); }
      const code = cause instanceof PromotionApiError && cause.status === 401 ? "session_expired" : cause instanceof PromotionApiError ? cause.code : "network_unavailable";
      if (migrationWrite) setMigrationFeedback({ context: currentMigrationContext, error: code });
      else (invitationWrite ? setInvitationError : setError)(code);
      return null;
    } finally { setBusy(false); }
  }

  function cancelRefresh() {
    ++refreshGeneration.current;
    refreshAbort.current?.abort(); refreshAbort.current = null;
    setLoading(false); setListLoading(false);
  }

  function changeView(next: View) {
    if (next !== view) cancelRefresh();
    setView(next); setError(null); setNotice(null);
    history.replaceState(null, "", `#${next}`);
    app.current?.scrollTo({ behavior: "auto", top: 0 });
  }

  async function passwordLogin(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (busy) return;
    setBusy(true); setError(null);
    try {
      await promotionRequest("auth/password", { body: { accountId: account.trim(), password } });
      setPassword(""); setAccount(""); await refresh(); setWalletOpen(true);
    } catch (cause) {
      setPassword(""); setError(cause instanceof PromotionApiError ? cause.code : "login_unavailable");
    } finally { setBusy(false); }
  }

  async function copyLink() {
    if (!invitationLink) return;
    try { await navigator.clipboard.writeText(invitationLink); setNotice("copied"); }
    catch { setError("clipboard_unavailable"); }
  }

  async function confirmCandidate() {
    if (!candidate || !acceptedTerms || !campaign?.termsVersion) return;
    const result = await write("confirm", { candidateId: candidate.candidateId, termsVersion: campaign.termsVersion });
    if (result) { setCandidate(null); setAcceptedTerms(false); await refresh(); }
  }

  async function confirmMigration() {
    if (!canConfirmMigration || !migrationCandidate || !campaign?.termsVersion || migrationCandidate.expiresAtMs <= Date.now() ||
      migrationContext(refreshContext.current.bootstrap) !== currentMigrationContext) return;
    const result = await write("confirm", { candidateId: migrationCandidate.candidateId, termsVersion: campaign.termsVersion }, "migration");
    if (result) { setAcceptedMigrationTerms(null); await refresh(); }
  }

  function connectButton(label = copy.login) {
    return <button className={styles.primary} onClick={() => setWalletOpen(true)} disabled={busy}>{label} <span aria-hidden="true">↗</span></button>;
  }

  const updateNotice = <p className={styles.muted} style={{ marginTop: 13 }}>
    {earningsCopy.autoUpdate} · {earningsCopy.updatedAt}: <time dateTime={lastUpdatedMs ? new Date(lastUpdatedMs).toISOString() : undefined}>{formatPromotionRefreshTime(lastUpdatedMs, locale)}</time>
  </p>;

  return <div ref={app} className={styles.app} lang={locale} dir={locale === "ar" ? "rtl" : "ltr"} data-promotion-live-updates={liveUpdatesEnabled ? "true" : "false"}>
    <header className={styles.header}>
      <a href="/promotions" className={styles.brand}><span className={styles.mark} aria-hidden="true">N</span><span>NUMERON<small>{copy.program}</small></span></a>
      <div className={styles.headerActions}><a href={`/${homeLocale}`} className={styles.home}>{copy.home} ↗</a>
        <label><span className={styles.srOnly}>{copy.language}</span><select className={styles.language} value={locale} onChange={event => {
          const next = normalizePromotionLocale(event.target.value); setLocale(next);
          try { localStorage.setItem("numeron-language", next); } catch { /* Optional language preference. */ }
          const url = new URL(location.href); url.searchParams.set("lang", next);
          history.replaceState(history.state, "", url);
        }}>{PROMOTION_LANGUAGES.map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label>
      </div>
    </header>
    <div className={styles.layout}>
      <nav className={styles.sidebar} aria-label={copy.program}>
        {NAVIGATION.filter(item => (item.view !== "admin" || canAdmin) && (item.view !== "billing" || campaign?.environment === "test")).map(item => <button key={item.view} className={`${styles.navButton} ${view === item.view ? styles.active : ""}`}
          aria-current={view === item.view ? "page" : undefined} onClick={() => changeView(item.view)}><span className={styles.navIcon} aria-hidden="true">{item.icon}</span>{item.view === "billing" ? PROMOTION_BILLING_LABELS[locale][0] : item.view === "characters" && testWalletEnrollment ? copy.testEnrollmentTitle : copy[item.view]}</button>)}
        <p className={styles.sidebarNote}>{copy.programIdentity}</p>
      </nav>
      <main className={styles.main}>
        <div className={styles.topline}><span className={styles.eyebrow}>{copy.heroEyebrow}</span><div className={styles.actions}>
          {authenticated ? <button className={styles.secondary} disabled={busy} onClick={() => void (async () => {
            if (await write("auth/logout", {})) { setBootstrap(null); setMembers(null); setSettlements(null); setWalletOpen(false); await refresh(); }
          })()}>{copy.logout}</button> : null}
          <button className={styles.secondary} disabled={loading || busy} onClick={() => void refresh()}>{loading ? copy.loading : copy.refresh}</button>
        </div></div>
        {error ? <div className={`${styles.notice} ${styles.error}`} role="alert"><strong>{error === "clipboard_unavailable" ? copy.copyFailed : errorText}</strong></div> : null}
        {notice ? <div className={`${styles.notice} ${styles.success}`} role="status">{notice === "invitation_confirmed" ? accountCopy.attributionConfirmed : notice === "copied" ? copy.copied : STATUS_KEYS[notice] ? copy[STATUS_KEYS[notice]] : <>{copy.resultId}: <code>{notice === "recorded" ? copy.confirmed : notice}</code></>}</div> : null}

        {view === "overview" ? <>
          <section className={styles.hero}><div><h1>{copy.heroTitle}</h1><p>{testWalletEnrollment ? copy.testEnrollmentBody : copy.heroBody}</p><div className={styles.heroActions}>
            {!authenticated || !bound ? connectButton() : <button className={styles.primary} onClick={() => changeView("characters")}>{testWalletEnrollment ? copy.testEnrollmentAction : copy.characters} →</button>}
            {!authenticated ? <button className={styles.secondary} onClick={() => {
              if (!existingAccountSection.current) return;
              existingAccountSection.current.open = true;
              const container = app.current;
              if (container) container.scrollTo({ behavior: "auto", top: container.scrollTop + existingAccountSection.current.getBoundingClientRect().top - container.getBoundingClientRect().top - 110 });
              existingAccountSection.current.querySelector("input")?.focus({ preventScroll: true });
            }}>{accountCopy.existingTitle}</button> : null}
            <button className={styles.secondary} onClick={() => changeView("rules")}>{copy.rules}</button>
          </div>{!authenticated ? <p className={styles.muted} style={{ marginTop: 16, fontSize: 11 }}>{accountCopy.existingBody}</p> : null}</div><div className={styles.heroArt} aria-hidden="true"><span className={styles.orbit} /><span className={styles.rune}>N</span><span className={styles.dot} /></div></section>
          <div className={styles.metadata}>
            <span>{copy.season}<strong>{campaign?.campaignId || copy.notConfigured}</strong></span>
            <span>{copy.termsVersion}<strong>{campaign?.termsVersion || copy.awaitingApproval}</strong></span>
            <span>{copy.start}<strong>{formatPromotionDate(campaign?.startsAtMs, locale)}</strong></span>
            <span>{copy.end}<strong>{formatPromotionDate(campaign?.endsAtMs, locale)}</strong></span>
          </div>
          <div className={styles.switches}>{(["recruitment", "commission", "payments"] as const).map((key, index) => <span key={key} className={`${styles.pill} ${styles.warning}`}>
            {copy[key]} · {campaign ? [campaign.recruitmentEnabled, campaign.commissionEnabled, campaign.payoutEnabled][index] ? copy.on : copy.off : copy.notConfigured}
          </span>)}</div>
          <PromotionDisclosure copy={copy} gaps={bootstrap?.sourceGaps || []} environment={campaign?.environment === "test" || bootstrap?.public?.environment === "test" ? "test" : campaign?.environment} />
          {invitationCode ? <section className={styles.panel}><h2>{copy.inviteTitle}</h2><p className={styles.muted} style={{ marginTop: 12 }}>{copy.inviteDisclosure}</p>
            {preview ? <p style={{ marginTop: 15 }}>{preview.leaderDisplayName} <span className={styles.pill}>{preview.termsVersion}</span></p> : null}
            {preview && !preview.rewardedPlacesAvailable ? <div className={styles.notice}>{copy.inviteClosed}</div> : null}
          </section> : null}
          <section className={styles.section}><h2>{testWalletEnrollment ? copy.testStepsTitle : copy.stepsTitle}</h2><div className={`${styles.steps} ${testWalletEnrollment ? styles.testSteps : ""}`}>
            {(testWalletEnrollment ? ["stepWallet", "testStepJoin", "testStepShare"] as const : ["stepWallet", "stepCharacter", "stepReview", "stepShare"] as const).map((key, index) => <div className={styles.step} key={key}><span className={styles.stepNumber}>0{index + 1}</span>{copy[key]}</div>)}
          </div>{testWalletEnrollment ? <p className={styles.muted} style={{ marginTop: 13 }}>{copy.testEnrollmentBody}</p> : null}</section>
          {authenticated ? <>
            <section className={styles.section}><div className={styles.sectionHeading}><h2>{copy.overview}</h2>{leader ? <PromotionStatus value={leader.status} copy={copy} /> : null}</div>
              {dashboard ? <div className={styles.cards}>
                {([[copy.clicks, dashboard.clicks], [copy.registered, dashboard.registrations], [copy.attributed, dashboard.confirmed], [copy.qualifiedMembers, dashboard.qualified]] as const).map(([label, value]) => <div className={styles.card} key={label}><p className={styles.cardTitle}>{label}</p><p className={styles.metric}>{Number.isSafeInteger(value) && value >= 0 ? new Intl.NumberFormat(locale).format(value) : "—"}</p></div>)}
                <div className={styles.card}><p className={styles.cardTitle}>{commissionTitle}</p><p className={styles.metric} style={{ fontSize: 25 }}>{formatPromotionUsd(dashboard.estimatedCommissionMicroUsd, locale)}</p></div>
              </div> : <p className={styles.empty}>{copy.noDashboard}</p>}
              {updateNotice}
              <p className={styles.muted} style={{ marginTop: 13 }}>{settlementNote} {copy.estimateNote}</p>
            </section>
            {leader?.status === "approved" && invitationLink ? <section className={styles.panel}><h3>{copy.inviteLink}</h3>
              <div className={styles.linkRow}><input readOnly value={invitationLink} aria-label={copy.inviteLink} onFocus={event => event.currentTarget.select()} /><button className={styles.secondary} onClick={() => void copyLink()}>{copy.copy}</button></div><InvitationQr link={invitationLink} alt={copy.inviteLink} />
            </section> : null}
          </> : <details className={styles.details} ref={existingAccountSection}><summary>{accountCopy.existingTitle}</summary><p style={{ marginTop: 12 }}>{accountCopy.existingBody}</p>
            <form className={styles.form} onSubmit={event => void passwordLogin(event)}>
              <label className={styles.field}>{accountCopy.account}<input value={account} onChange={event => setAccount(event.target.value)} autoComplete="username" required maxLength={128} /></label>
              <label className={styles.field}>{accountCopy.password}<input type="password" value={password} onChange={event => setPassword(event.target.value)} autoComplete="current-password" required maxLength={256} /></label>
              <button className={styles.secondary} disabled={busy || !account.trim() || !password}>{busy ? copy.loading : accountCopy.verifyAccount}</button>
            </form>
          </details>}
          {authenticated && migrationCandidate ? <section id="promotion-migration-pending" className={styles.panel} aria-label={migrationCopy.pendingTitle}>
            <h3>{migrationCopy.pendingTitle}</h3><p className={styles.muted}>{migrationCopy.pendingBody}</p>
            <dl className={styles.form}>
              <div><dt>{migrationCopy.leader}</dt><dd>{migrationCandidate.leaderDisplayName}</dd></div>
              <div><dt>{migrationCopy.sourceActivity}</dt><dd className={styles.code}>{migrationCandidate.sourceCampaignId}</dd></div>
              <div><dt>{migrationCopy.sourceConfirmedAt}</dt><dd>{formatPromotionDate(migrationCandidate.sourceConfirmedAtMs, locale)}</dd></div>
              <div><dt>{migrationCopy.preparedAt}</dt><dd>{formatPromotionDate(migrationCandidate.preparedAtMs, locale)}</dd></div>
              <div><dt>{copy.termsVersion}</dt><dd className={styles.code}>{campaign?.termsVersion || copy.notConfigured}</dd></div>
              <div><dt>{copy.end}</dt><dd>{formatPromotionDate(migrationCandidate.expiresAtMs, locale)}</dd></div>
              <div><dt>{migrationCopy.auditId}</dt><dd className={styles.code}>{migrationCandidate.migrationId}</dd></div>
            </dl>
            <p id="promotion-migration-finance-note" className={styles.muted}>{migrationCopy.financeNote}</p>
            {!bound ? <p className={styles.muted}>{accountCopy.bindingBody}</p> : null}
            {!migrationProofValid ? <p className={styles.error} role="alert">{migrationCopy.proofInvalid}</p> : migrationExpired ?
              <p className={styles.error} role="alert">{migrationCopy.expired}</p> : !migrationTermsCurrent ?
                <p className={styles.error} role="alert">{migrationCopy.termsRequired}</p> : null}
            <label className={styles.check} style={{ marginTop: 12 }}><input id="promotion-migration-terms" type="checkbox" checked={migrationTermsAccepted}
              disabled={busy || !bound || !migrationProofValid || migrationExpired || !migrationTermsCurrent || !!currentMigrationFeedback?.submitted}
              aria-describedby="promotion-migration-finance-note"
              onChange={event => setAcceptedMigrationTerms(event.target.checked ? currentMigrationContext : null)} />{copy.acceptTerms} ({campaign?.termsVersion || copy.notConfigured})</label>
            {currentMigrationFeedback?.error ? <div id="promotion-migration-error" className={`${styles.notice} ${styles.error}`} role="alert">{errorMessage(currentMigrationFeedback.error)}</div> : null}
            {currentMigrationFeedback?.submitted ? <p id="promotion-migration-submitted" role="status">{migrationCopy.submitted}</p> : null}
            <button id="promotion-migration-confirm" className={styles.primary} style={{ marginTop: 14 }} disabled={!canConfirmMigration}
              onClick={() => void confirmMigration()}>{busy ? copy.loading : migrationCopy.confirmAction}</button>
          </section> : null}
          {authenticated && !me?.attribution && !migrationCandidate ? <section className={styles.panel} aria-label={copy.invitationCode}><h3>{copy.inviteTitle}</h3><p className={styles.muted}>{copy.inviteDisclosure}</p>
            <p className={styles.muted} id="promotion-invitation-steps">{accountCopy.invitationSteps}</p>
            <form className={styles.form} onSubmit={event => { event.preventDefault(); void (async () => {
              const result = await write("candidate", { code: inviteCode.trim(), source: invitationCode && inviteCode.trim() === invitationCode ? "invite_link" : "manual_code" });
              if (result && typeof result.candidateId === "string" && typeof result.leaderDisplayName === "string" && typeof result.expiresAtMs === "number") {
                setCandidate({ candidateId: result.candidateId, leaderDisplayName: result.leaderDisplayName, expiresAtMs: result.expiresAtMs }); setAcceptedTerms(false);
              } else if (result) setInvitationError("invalid_response");
            })(); }}>
              <label className={styles.field}>{copy.invitationCode}<input value={inviteCode} onChange={event => { setInviteCode(event.target.value); setCandidate(null); setAcceptedTerms(false); setInvitationError(null); setNotice(null); }} aria-describedby={invitationError ? "promotion-invitation-steps promotion-invitation-error" : "promotion-invitation-steps"} aria-invalid={!!invitationError} maxLength={128} autoComplete="off" required disabled={busy} /></label>
              <button className={styles.secondary} disabled={busy || !bound || !inviteCode.trim() || Boolean(invitationCode && inviteCode.trim() === invitationCode && !clickRecorded)}>{busy ? copy.loading : accountCopy.findLeader}</button>
            </form>
            {!bound ? <p className={styles.muted}>{accountCopy.bindingBody}</p> : null}
            {invitationError ? <div id="promotion-invitation-error" className={`${styles.notice} ${styles.error}`} role="alert">{errorMessage(invitationError)}</div> : null}
            {candidate ? <div className={styles.notice}><p role="status">{accountCopy.candidatePending}</p><strong>{candidate.leaderDisplayName}</strong>{accountCopy.finalAttributionBody}<p className={styles.muted}>{copy.end}: {formatPromotionDate(candidate.expiresAtMs, locale)}</p>
              <label className={styles.check} style={{ marginTop: 12 }}><input type="checkbox" checked={acceptedTerms} onChange={event => setAcceptedTerms(event.target.checked)} />{copy.acceptTerms} ({campaign?.termsVersion || copy.notConfigured})</label>
              <button className={styles.primary} style={{ marginTop: 14 }} disabled={busy || !acceptedTerms || !bound || !campaign?.recruitmentEnabled || !campaign?.termsVersion} onClick={() => void confirmCandidate()}>{copy.confirmAttribution}</button>
            </div> : null}
          </section> : null}
          {me?.attribution ? <section className={styles.panel}><h3>{accountCopy.finalAttribution}</h3><PromotionStatus value={me.attribution.status} copy={copy} /><p className={styles.muted} style={{ marginTop: 10 }}>{accountCopy.finalAttributionBody}</p>
            {me.attribution.migration ? <div id="promotion-migration-completed">
              <h3>{migrationCopy.completedTitle}</h3><p className={styles.muted}>{migrationCopy.completedBody}</p>
              <dl className={styles.form}>
                <div><dt>{migrationCopy.leader}</dt><dd>{me.attribution.migration.leaderDisplayName}</dd></div>
                <div><dt>{migrationCopy.sourceActivity}</dt><dd className={styles.code}>{me.attribution.migration.sourceCampaignId}</dd></div>
                <div><dt>{migrationCopy.sourceConfirmedAt}</dt><dd>{formatPromotionDate(me.attribution.migration.sourceConfirmedAtMs, locale)}</dd></div>
                <div><dt>{migrationCopy.confirmedAt}</dt><dd>{formatPromotionDate(me.attribution.migration.confirmedAtMs, locale)}</dd></div>
                <div><dt>{migrationCopy.auditId}</dt><dd className={styles.code}>{me.attribution.migration.migrationId}</dd></div>
                <div><dt>{migrationCopy.evidenceHash}</dt><dd className={styles.code}>{me.attribution.migration.snapshotSha256}</dd></div>
              </dl>
              <p className={styles.muted}>{migrationCopy.financeNote}</p>
            </div> : null}
          </section> : null}
        </> : null}

        {authenticated && !bound && view !== "rules" ? <div className={styles.notice}><strong>{accountCopy.bindingRequired}</strong>{accountCopy.bindingBody}<div className={styles.heroActions}>{connectButton()}</div></div> : null}
        {view === "characters" ? <section className={styles.section}><div className={styles.sectionHeading}><div><h2>{testWalletEnrollment ? copy.testEnrollmentTitle : copy.charactersTitle}</h2><p>{testWalletEnrollment ? copy.testEnrollmentBody : copy.charactersBody}</p></div></div>
          {!authenticated ? <div className={styles.empty}>{copy.notAuthenticated}<div className={styles.heroActions}>{connectButton()}</div></div> : <>
            {testWalletEnrollment && campaign ? <PromotionTestWalletEnrollment campaign={campaign} copy={copy} locale={locale} bound={bound} acceptedTerms={acceptedTerms} busy={busy} leader={leader} onAccept={setAcceptedTerms} onSubmit={() => void (async () => {
              const application = testWalletApplication(campaign, { bound, acceptedTerms });
              if (!application) return;
              if (await write("apply", application)) { setAcceptedTerms(false); await refresh(); changeView("overview"); }
            })()} /> : characters.length ? <form className={styles.form} onSubmit={event => { event.preventDefault(); void (async () => {
              if (characterIndex === null || !campaign?.termsVersion) return;
              if (await write("apply", { characterIndex, termsVersion: campaign.termsVersion })) { setAcceptedTerms(false); await refresh(); }
            })(); }}>
              <label className={styles.field}>{copy.selectCharacter}<select value={characterIndex ?? ""} onChange={event => setCharacterIndex(Number(event.target.value))}>
                {characters.map(character => <option key={character.index} value={character.index}>{character.name} · {copy.level} {character.level}</option>)}
              </select></label>
              <label className={styles.check}><input type="checkbox" checked={acceptedTerms} onChange={event => setAcceptedTerms(event.target.checked)} />{copy.acceptTerms} ({campaign?.termsVersion || copy.notConfigured})</label>
              <button className={styles.primary} disabled={busy || !bound || !acceptedTerms || !campaign?.recruitmentEnabled || campaign.termsStatus !== "approved" || leader !== null && leader !== undefined}>{busy ? copy.loading : copy.apply}</button>
              {!campaign?.recruitmentEnabled ? <p className={styles.muted}>{copy.unavailableAction}</p> : null}
            </form> : <p className={styles.empty}>{copy.noCharacters}</p>}
            <div className={styles.panel}><h3>{copy.applicationState}</h3>{leader ? <><p>{leader.displayName}</p><PromotionStatus value={leader.status} copy={copy} />{leader.status === "approved" && invitationLink ? <div className={styles.actions} style={{ marginTop: 14 }}><button className={styles.secondary} onClick={() => changeView("overview")}>{copy.inviteLink} →</button></div> : null}</> : <p className={styles.muted}>{copy.noLeader}</p>}</div>
          </>}
        </section> : null}

        {view === "members" ? <section className={styles.section}><div className={styles.sectionHeading}><div><h2>{copy.membersTitle}</h2><p>{copy.privacy}</p></div></div>
          {!authenticated ? <p className={styles.empty}>{copy.notAuthenticated}</p> : listLoading ? <p role="status" className={styles.empty}>{copy.loading}</p> : members && members.length ? <>
            <div className={styles.tableWrap}><table className={styles.table}><thead><tr><th>{copy.member}</th><th>{copy.qualification}</th><th>{copy.reason}</th><th>{copy.estimatedCommission}</th></tr></thead><tbody>
              {members.map(member => <tr key={member.memberRef}><td className={styles.code}>{member.memberRef}</td><td><PromotionStatus value={member.status} copy={copy} /></td><td>{member.reasonCode === "requirements_complete" ? copy.qualified : ["requirements_pending", "authoritative_sources_pending"].includes(member.reasonCode || "") ? copy.conditionsPending : member.status === "excluded" ? copy.excluded : copy.pending}</td><td>{formatPromotionUsd(member.estimatedCommissionMicroUsd, locale)}</td></tr>)}
            </tbody></table></div><div className={styles.actions} style={{ marginTop: 15 }}><button className={styles.secondary} disabled={!memberOffset} onClick={() => setMemberOffset(offset => Math.max(0, offset - 20))}>{copy.previousPage}</button><button className={styles.secondary} disabled={nextMemberOffset === null} onClick={() => { if (nextMemberOffset !== null) setMemberOffset(nextMemberOffset); }}>{copy.nextPage}</button></div>
          </> : members ? <p className={styles.empty}>{copy.empty}</p> : null}
        </section> : null}

        {view === "earnings" ? <section className={styles.section}><div className={styles.sectionHeading}><div><h2>{copy.earnings}</h2><p>{settlementNote}</p></div></div>
          {!authenticated ? <p className={styles.empty}>{copy.notAuthenticated}</p> : <>
            {dashboard ? <div className={styles.card}><p className={styles.cardTitle}>{commissionTitle}</p><p className={styles.metric}>{formatPromotionUsd(dashboard.estimatedCommissionMicroUsd, locale)}</p>
              <p className={styles.muted}>{earningsCopy.seasonEnd}: {formatPromotionDate(campaign?.endsAtMs, locale)}</p></div> : <p className={styles.empty}>{copy.totalsNoData}</p>}
            {updateNotice}
            <p className={styles.muted} style={{ marginTop: 13 }}>{copy.estimateNote}</p>
            <div className={styles.sectionHeading} style={{ marginTop: 30 }}><h3>{copy.settlements}</h3></div>
            {listLoading ? <p role="status" className={styles.empty}>{copy.loading}</p> : settlements?.length ? <div className={styles.tableWrap}><table className={styles.table}><thead><tr><th>{copy.settlementId}</th><th>{copy.amount}</th><th>{accountCopy.reserve}</th><th>{accountCopy.reversal}</th><th>{accountCopy.payable}</th><th>{copy.status}</th></tr></thead><tbody>
              {settlements.map(row => <tr key={row.id}><td className={styles.code}>{row.id}</td><td>{formatPromotionUsd(row.totalMicroUsd, locale)}</td><td>{formatPromotionUsd(row.reserveMicroUsd, locale)}</td><td>{formatPromotionUsd(row.reversalMicroUsd, locale)}</td><td>{row.payableMicroUsd !== undefined ? formatPromotionUsd(row.payableMicroUsd, locale) : formatPromotionCents(row.payableCents, locale)}{row.paidMicroUsd && row.paidMicroUsd !== "0" ? <p>{copy.confirmed}: {formatPromotionUsd(row.paidMicroUsd, locale)}</p> : null}</td><td><PromotionStatus value={row.payoutStatus || row.status} copy={copy} />{row.txDigest ? <p className={styles.code} dir="ltr">{row.txDigest}{row.checkpoint ? ` · ${row.checkpoint}` : ""}</p> : null}{["unknown", "result_unknown"].includes(row.payoutStatus || "") ? <p className={styles.muted}>{copy.unknownPayment}</p> : null}</td></tr>)}
            </tbody></table></div> : settlements ? <p className={styles.empty}>{copy.noSettlements}</p> : null}
          </>}
        </section> : null}

        {view === "payout" ? <section className={styles.section}><div className={styles.sectionHeading}><div><h2>{copy.payout}</h2><p>{copy.payoutBody}</p></div></div>
          {!authenticated || !bootstrap ? <p className={styles.empty}>{copy.notAuthenticated}</p> : <PromotionReceivingWallet bootstrap={bootstrap} copy={copy} locale={locale} busy={busy} onVerify={() => void (async () => {
              const network = bootstrap?.binding?.network?.replace(/^sui:/, "");
              if (!bootstrap?.binding?.credentialId || !["testnet", "mainnet"].includes(network || "")) { setError("wallet_network_unconfigured"); return; }
              if (await write("payout-address", { credentialId: bootstrap.binding.credentialId, network: `sui:${network}` })) await refresh();
            })()} />}
        </section> : null}

        {view === "billing" && campaign?.environment === "test" ? authenticated && bootstrap ? <PromotionBilling key={bootstrap.csrfToken} bootstrap={bootstrap} copy={copy} locale={locale} /> : <p className={styles.empty}>{copy.notAuthenticated}</p> : null}
        {view === "rules" ? <><PromotionDisclosure copy={copy} gaps={bootstrap?.sourceGaps || []} environment={campaign?.environment === "test" || bootstrap?.public?.environment === "test" ? "test" : campaign?.environment} /><RulePanel copy={copy} campaign={campaign} /></> : null}

        {view === "support" ? <section className={styles.section}><div className={styles.sectionHeading}><div><h2>{copy.support}</h2><p>{copy.supportBody}</p></div></div>
          {!authenticated ? <p className={styles.empty}>{copy.notAuthenticated}</p> : <>
            <form className={styles.form} onSubmit={event => { event.preventDefault(); void (async () => { if (await write("appeals", { category: appealCategory, message: appealMessage.trim() })) { setAppealMessage(""); await refresh(); } })(); }}>
              <label className={styles.field}>{accountCopy.category}<select value={appealCategory} onChange={event => setAppealCategory(event.target.value)}>
                {(["attribution", "qualification", "revenue", "settlement", "wallet", "other"] as const).map((category, index) => <option key={category} value={category}>{[copy.attributed, copy.qualification, copy.earnings, copy.settlements, copy.payout, copy.support][index]}</option>)}
              </select></label>
              <label className={styles.field}>{copy.message}<textarea value={appealMessage} onChange={event => setAppealMessage(event.target.value)} minLength={5} maxLength={4000} required /></label>
              <button className={styles.primary} disabled={busy || appealMessage.trim().length < 5 || !campaign}>{copy.appeal}</button>
            </form>
            {me?.appeals?.length ? <div className={styles.tableWrap}><table className={styles.table}><thead><tr><th>ID</th><th>{accountCopy.category}</th><th>{copy.status}</th></tr></thead><tbody>{me.appeals.map(row => <tr key={row.id}><td className={styles.code}>{row.id}</td><td>{({ attribution: copy.attributed, qualification: copy.qualification, revenue: copy.earnings, settlement: copy.settlements, wallet: copy.payout } as Record<string, string>)[row.category || ""] || copy.support}</td><td><PromotionStatus value={row.status} copy={copy} /></td></tr>)}</tbody></table></div> : <p className={styles.empty}>{copy.noAppeals}</p>}
          </>}
        </section> : null}

        {view === "admin" ? canAdmin && bootstrap ? <PromotionAdmin copy={copy} accountCopy={accountCopy} locale={locale} busy={busy} write={write}
          campaignId={campaign?.campaignId || (typeof bootstrap.public?.campaignId === "string" ? bootstrap.public.campaignId : null)} onChanged={() => void refresh()} /> : <p className={styles.empty}>{copy.unavailableAction}</p> : null}
        <footer className={styles.footer}><span>© NUMERON · {copy.program}</span><a href={`/${homeLocale}`}>{copy.home} ↗</a><span>{copy.timeZone} · {timeZone}</span></footer>
      </main>
    </div>
    {walletOpen ? <PromotionWalletLogin copy={copy} onClose={() => setWalletOpen(false)} onAuthenticated={() => { setWalletOpen(false); void refresh(); }} /> : null}
  </div>;
}
