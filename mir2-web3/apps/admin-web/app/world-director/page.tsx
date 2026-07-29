import { AdminShell } from "../../components/admin-shell";
import { MetricCard } from "../../components/metric-card";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import {
  adminGet,
  type DirectorApprovalRecord,
  type DirectorPressureScores,
  type WorldDirectorDashboard
} from "../../lib/admin-api";
import { formatCompactNumber, formatDateTime } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import {
  approveDirectorProposalAction,
  cancelDirectorProposalAction,
  editDirectorProposalAction,
  generateDirectorProposalAction,
  pauseDirectorAction,
  rejectDirectorProposalAction,
  retryDirectorDeliveryAction,
  resumeDirectorAction
} from "./actions";

export const dynamic = "force-dynamic";

export default async function WorldDirectorPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const notice = firstParam(params.notice);
  const response = await adminGet<WorldDirectorDashboard>("/admin/world-director");

  return (
    <AdminShell active="/world-director">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("director.eyebrow")}</p>
          <h2>{t("director.title")}</h2>
          <p className="muted">{t("director.subtitle")}</p>
        </div>
        <StatusBadge
          tone={!response.ok ? "danger" : response.data.paused ? "warn" : "success"}
        >
          {!response.ok
            ? t("common.apiOffline")
            : response.data.paused
              ? t("director.paused")
              : t("director.running")}
        </StatusBadge>
      </div>

      {error ? <p className="notice">{error}</p> : null}
      {notice ? <p className="notice success">{directorNotice(t, notice)}</p> : null}
      {!response.ok ? (
        <p className="notice">{response.error}</p>
      ) : (
        <DirectorDashboard dashboard={response.data} t={t} />
      )}
    </AdminShell>
  );
}

function DirectorDashboard({
  dashboard,
  t
}: {
  dashboard: WorldDirectorDashboard;
  t: (key: string) => string;
}) {
  const liveRuntime = dashboard.runtimeStatuses.filter(
    (runtime) => runtime.status === "live"
  ).length;
  return (
    <div className="director-console">
      <div className="grid metrics">
        <MetricCard
          title={t("director.pending")}
          value={String(dashboard.pendingCount)}
          delta={dashboard.paused ? t("director.paused") : t("director.running")}
        />
        <MetricCard
          title={t("director.active")}
          value={String(dashboard.activeCount)}
          delta={`${liveRuntime}/${dashboard.configuration.zoneHostCount} ${t("director.zoneHosts")}`}
        />
        <MetricCard
          title={t("director.committee")}
          value={`${dashboard.configuration.committeeSize}`}
          delta={
            dashboard.configuration.executionConfigured
              ? `${t("director.configured")} · ${t("director.remoteFinality")}`
              : dashboard.configuration.remoteCommonwareRequired &&
                  !dashboard.configuration.remoteCommonwareConfigured
                ? `${t("director.notConfigured")} · ${t("director.remoteFinality")}`
                : t("director.notConfigured")
          }
          negative={!dashboard.configuration.executionConfigured}
        />
        <MetricCard
          title={t("director.autoGeneration")}
          value={dashboard.configuration.automaticGenerationEnabled ? "ON" : "OFF"}
          delta={`${dashboard.configuration.generationIntervalSeconds}s`}
        />
      </div>

      <section className="card director-control-card">
        <div>
          <p className="eyebrow">{t("director.control")}</p>
          <p className="muted">
            {dashboard.paused
              ? dashboard.pauseReason ?? t("director.paused")
              : `${dashboard.configuration.persistence} · ${dashboard.configuration.directorPublicKey?.slice(0, 18) ?? "-"}…`}
          </p>
          <p className="muted">
            {dashboard.configuration.aiConfigured
              ? `${t("director.generatorAi")} · ${dashboard.configuration.aiProvider}/${dashboard.configuration.aiModel}`
              : t("director.aiNotConfigured")}
          </p>
        </div>
        <div className="director-control-actions">
          <form action={generateDirectorProposalAction}>
            <SubmitButton
              disabled={!dashboard.configuration.aiConfigured}
              idle={
                dashboard.configuration.aiConfigured
                  ? t("director.generate")
                  : t("director.aiNotConfigured")
              }
              pending={t("director.generating")}
            />
          </form>
          <form
            action={dashboard.paused ? resumeDirectorAction : pauseDirectorAction}
            className="director-inline-form"
          >
            <input
              className="control"
              name="reason"
              defaultValue={
                dashboard.paused
                  ? t("director.defaultResumeReason")
                  : t("director.defaultPauseReason")
              }
              minLength={8}
              maxLength={512}
              required
            />
            <SubmitButton
              className="button secondary"
              confirmMessage={t("common.confirmDangerous")}
              idle={dashboard.paused ? t("director.resume") : t("director.pause")}
              pending={
                dashboard.paused ? t("director.resuming") : t("director.pausing")
              }
            />
          </form>
        </div>
      </section>

      <section>
        <div className="section-head">
          <div>
            <p className="eyebrow">{t("director.queue")}</p>
          </div>
        </div>
        {dashboard.proposals.length ? (
          <div className="director-proposal-list">
            {dashboard.proposals.map((record) => (
              <DirectorProposalCard key={record.proposalId} record={record} t={t} />
            ))}
          </div>
        ) : (
          <section className="card">
            <p className="muted">{t("director.empty")}</p>
          </section>
        )}
      </section>

      <section className="grid two">
        <div className="card">
          <p className="eyebrow">{t("director.runtime")}</p>
          {dashboard.runtimeStatuses.length ? (
            <div className="director-runtime-list">
              {dashboard.runtimeStatuses.map((target) => (
                <article key={target.endpoint}>
                  <div>
                    <strong>{target.endpoint}</strong>
                    <p className="muted">
                      {target.runtime
                        ? `Finality #${target.runtime.finalizedHeight} · ${target.runtime.spawnedMonstersTotal} monsters`
                        : target.error}
                    </p>
                  </div>
                  <StatusBadge tone={target.status === "live" ? "success" : "danger"}>
                    {translateAdminStatus(t, target.status)}
                  </StatusBadge>
                </article>
              ))}
            </div>
          ) : (
            <p className="muted">{t("director.noRuntime")}</p>
          )}
        </div>
        <div className="card">
          <p className="eyebrow">{t("director.audit")}</p>
          <div className="director-audit-list">
            {dashboard.audit.slice(0, 12).map((audit) => (
              <article key={audit.auditId}>
                <div>
                  <strong>{audit.action}</strong>
                  <p className="muted">
                    {audit.actorId} · {formatDateTime(audit.occurredAtMs)}
                  </p>
                </div>
                <code>{audit.recordHash.slice(0, 12)}</code>
              </article>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}

function DirectorProposalCard({
  record,
  t
}: {
  record: DirectorApprovalRecord;
  t: (key: string) => string;
}) {
  const pending = record.status === "pending_approval";
  const statusTone =
    record.status === "executing" || record.status === "completed"
      ? "success"
      : record.status === "failed"
        ? "danger"
        : "warn";
  return (
    <article className="card director-proposal-card">
      <header>
        <div>
          <div className="director-title-line">
            <h3>{record.proposal.templateId}</h3>
            <StatusBadge tone={statusTone}>
              {translateAdminStatus(t, record.status)}
            </StatusBadge>
            <StatusBadge tone={record.riskLevel === "high" ? "danger" : "default"}>
              {t("director.risk")}: {translateAdminStatus(t, record.riskLevel)}
            </StatusBadge>
          </div>
          <p>{record.proposal.rationale}</p>
        </div>
        <code>{record.proposalId}</code>
      </header>

      <div className="director-facts">
        <div>
          <span>{t("director.zones")}</span>
          <strong>{record.proposal.targetZones.join(" · ")}</strong>
        </div>
        <div>
          <span>{t("director.duration")}</span>
          <strong>{Math.round(record.proposal.durationMs / 60_000)} min</strong>
        </div>
        <div>
          <span>{t("director.budget")}</span>
          <strong>{formatCompactNumber(record.proposal.rewardBudget)}</strong>
        </div>
        <div>
          <span>{t("director.snapshot")}</span>
          <strong>{record.snapshot.snapshotId}</strong>
        </div>
      </div>

      <PressureBars scores={record.pressureScores} t={t} />

      {record.finalizedHeight ? (
        <div className="director-finality">
          {record.commonwareNetworkHeight ? (
            <>
              <span>{t("director.remoteFinality")}</span>
              <strong>#{record.commonwareNetworkHeight}</strong>
              <code>{record.commonwareNetworkStateRoot?.slice(0, 24)}…</code>
            </>
          ) : null}
          <span>{t("director.zoneFinality")}</span>
          <strong>#{record.finalizedHeight}</strong>
          <code>{record.finalizedDigest?.slice(0, 24)}…</code>
          <span>{t("director.command")}</span>
          <code>{record.commandId?.slice(0, 24)}…</code>
        </div>
      ) : null}
      {record.lastError ? (
        <p className="notice">
          {t("director.deliveryError")}: {record.lastError}
        </p>
      ) : null}

      {pending ? (
        <>
          <div className="director-decisions">
            <DecisionForm
              action={approveDirectorProposalAction}
              proposalId={record.proposalId}
              reason={t("director.defaultApproveReason")}
              label={t("director.approve")}
              pendingLabel={t("director.approving")}
              confirmMessage={t("common.confirmDangerous")}
            />
            <DecisionForm
              action={rejectDirectorProposalAction}
              proposalId={record.proposalId}
              reason={t("director.defaultRejectReason")}
              label={t("director.reject")}
              pendingLabel={t("director.rejecting")}
              confirmMessage={t("common.confirmDangerous")}
              secondary
            />
            <DecisionForm
              action={cancelDirectorProposalAction}
              proposalId={record.proposalId}
              reason={t("director.defaultCancelReason")}
              label={t("director.cancel")}
              pendingLabel={t("director.cancelling")}
              confirmMessage={t("common.confirmDangerous")}
              secondary
            />
          </div>
          <form action={editDirectorProposalAction} className="director-edit-form">
            <input name="proposalId" type="hidden" value={record.proposalId} />
            <label>
              <span>{t("director.duration")}</span>
              <input
                className="control"
                name="durationMinutes"
                type="number"
                min={1}
                defaultValue={Math.round(record.proposal.durationMs / 60_000)}
                required
              />
            </label>
            <label>
              <span>{t("director.budget")}</span>
              <input
                className="control"
                name="rewardBudget"
                type="number"
                min={1}
                defaultValue={record.proposal.rewardBudget}
                required
              />
            </label>
            <label>
              <span>{t("director.zones")}</span>
              <input
                className="control"
                name="targetZones"
                defaultValue={record.proposal.targetZones.join(",")}
                required
              />
            </label>
            <label className="director-edit-reason">
              <span>{t("director.reason")}</span>
              <input
                className="control"
                name="reason"
                defaultValue={t("director.defaultEditReason")}
                minLength={8}
                maxLength={512}
                required
              />
            </label>
            <SubmitButton
              className="button secondary"
              confirmMessage={t("common.confirmDangerous")}
              idle={t("director.edit")}
              pending={t("director.editing")}
            />
          </form>
        </>
      ) : record.status === "failed" ? (
        <DecisionForm
          action={retryDirectorDeliveryAction}
          proposalId={record.proposalId}
          reason={t("director.defaultRetryReason")}
          label={t("director.retry")}
          pendingLabel={t("director.retrying")}
          confirmMessage={t("common.confirmDangerous")}
        />
      ) : (
        <p className="muted">
          {record.decidedBy ?? record.requestedBy} · {record.decisionReason ?? "-"}
        </p>
      )}
    </article>
  );
}

function DecisionForm({
  action,
  proposalId,
  reason,
  label,
  pendingLabel,
  confirmMessage,
  secondary
}: {
  action: (formData: FormData) => Promise<void>;
  proposalId: string;
  reason: string;
  label: string;
  pendingLabel: string;
  confirmMessage?: string;
  secondary?: boolean;
}) {
  return (
    <form action={action}>
      <input name="proposalId" type="hidden" value={proposalId} />
      <textarea
        className="control"
        name="reason"
        defaultValue={reason}
        minLength={8}
        maxLength={512}
        required
      />
      <SubmitButton
        className={secondary ? "button secondary" : "button"}
        confirmMessage={confirmMessage}
        idle={label}
        pending={pendingLabel}
      />
    </form>
  );
}

function PressureBars({
  scores,
  t
}: {
  scores: DirectorPressureScores;
  t: (key: string) => string;
}) {
  const rows = [
    [t("director.population"), scores.populationImbalanceBps],
    [t("director.fatigue"), scores.contentFatigueBps],
    [t("director.progression"), scores.progressionGapBps],
    [t("director.economy"), scores.economyInflationBps],
    [t("director.guild"), scores.guildDominanceBps]
  ] as const;
  return (
    <div className="director-pressure">
      {rows.map(([label, value]) => (
        <div key={label}>
          <span>{label}</span>
          <div>
            <i style={{ width: `${Math.min(100, value / 100)}%` }} />
          </div>
          <strong>{(value / 100).toFixed(0)}%</strong>
        </div>
      ))}
    </div>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}

function directorNotice(t: (key: string) => string, notice: string) {
  if (notice === "proposal-generated") return t("director.proposalGenerated");
  if (notice === "no-proposal") return t("director.noProposal");
  return t("director.operationUpdated");
}
