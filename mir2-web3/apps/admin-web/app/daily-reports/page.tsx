import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import {
  adminGet,
  type DailyReport,
  type DailyReportDetailResponse,
  type DailyReportListResponse
} from "../../lib/admin-api";
import { getAdminI18n } from "../../lib/i18n";
import {
  approveDailyReportAction,
  generateDailyReportAction,
  publishDailyReportAction,
  retryDiscordAction
} from "./actions";

export default async function DailyReportsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const requestedReport = firstParam(params.report);
  const list = await adminGet<DailyReportListResponse>("/admin/daily-reports?limit=45");
  const reports = list.ok ? list.data.reports : [];
  const selectedId = requestedReport || reports[0]?.reportId;
  const detail = selectedId
    ? await adminGet<DailyReportDetailResponse>(
        `/admin/daily-reports/${encodeURIComponent(selectedId)}`
      )
    : undefined;
  const selected = detail?.ok ? detail.data.report : undefined;
  const deliveries = detail?.ok ? detail.data.deliveries : [];

  return (
    <AdminShell active="/daily-reports">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("daily.eyebrow")}</p>
          <h2>{t("daily.title")}</h2>
          <p className="muted">{t("daily.subtitle")}</p>
        </div>
        <div className="daily-status-row">
          <StatusBadge tone={list.ok && list.data.configured ? "success" : "warn"}>
            {list.ok && list.data.configured ? t("daily.storageReady") : t("daily.storageMissing")}
          </StatusBadge>
          <StatusBadge tone={list.ok && list.data.discordConfigured ? "success" : "warn"}>
            {list.ok && list.data.discordConfigured
              ? t("daily.discordReady")
              : t("daily.discordMissing")}
          </StatusBadge>
        </div>
      </div>

      {error ? <p className="notice">{error}</p> : null}
      {!list.ok ? <p className="notice">{list.error}</p> : null}

      <section className="card daily-control-card">
        <div>
          <p className="eyebrow">{t("daily.schedule")}</p>
          <h3>
            {list.ok ? `${list.data.timezone} · ${list.data.schedule}` : t("common.unavailable")}
          </h3>
          <p className="muted">
            {list.ok && list.data.schedulerEnabled
              ? t("daily.schedulerOn")
              : t("daily.schedulerOff")}
          </p>
        </div>
        <form action={generateDailyReportAction} className="daily-generate-form">
          <label className="field">
            <span>{t("daily.reportDate")}</span>
            <input className="control" name="reportDate" type="date" />
          </label>
          <label className="daily-checkbox">
            <input name="force" type="checkbox" />
            <span>{t("daily.force")}</span>
          </label>
          <SubmitButton idle={t("daily.generate")} pending={t("daily.generating")} />
        </form>
      </section>

      <div className="daily-layout">
        <section className="card daily-list">
          <p className="eyebrow">{t("daily.history")}</p>
          {reports.length ? (
            reports.map((report) => (
              <a
                className={report.reportId === selected?.reportId ? "daily-list-item active" : "daily-list-item"}
                href={`/daily-reports?report=${encodeURIComponent(report.reportId)}`}
                key={report.reportId}
              >
                <div>
                  <strong>{report.reportDate}</strong>
                  <span>{report.generationSource}</span>
                </div>
                <StatusBadge tone={statusTone(report.status)}>{report.status}</StatusBadge>
              </a>
            ))
          ) : (
            <p className="notice">{t("daily.empty")}</p>
          )}
        </section>

        <div className="daily-detail-stack">
          {selected ? (
            <>
              <DailyReportSummary report={selected} />
              <section className="card">
                <div className="daily-section-head">
                  <div>
                    <p className="eyebrow">{t("daily.operationsReport")}</p>
                    <h3>{selected.reportDate}</h3>
                  </div>
                  <StatusBadge tone={statusTone(selected.status)}>{selected.status}</StatusBadge>
                </div>
                <article className="daily-markdown">
                  <SafeMarkdown source={selected.operationsMarkdown} />
                </article>
              </section>
              <section className="card">
                <p className="eyebrow">{t("daily.playerReport")}</p>
                <article className="daily-markdown player">
                  <SafeMarkdown source={selected.playerMarkdown} />
                </article>
              </section>
              <section className="card">
                <p className="eyebrow">{t("daily.evidence")}</p>
                <div className="daily-evidence-grid">
                  {selected.evidence.sources.map((source) => (
                    <div className="daily-evidence" key={source.source}>
                      <StatusBadge tone={source.status === "ok" ? "success" : "warn"}>
                        {source.status}
                      </StatusBadge>
                      <strong>{source.source}</strong>
                      <span>{source.detail}</span>
                    </div>
                  ))}
                </div>
                <p className="muted">{selected.evidence.privacy}</p>
                {selected.evidence.warnings.map((warning) => (
                  <p className="notice" key={warning}>{warning}</p>
                ))}
                <div className="daily-hashes">
                  <code>input {selected.inputSha256}</code>
                  <code>content {selected.contentSha256}</code>
                </div>
              </section>
              <section className="card">
                <p className="eyebrow">{t("daily.reviewPublish")}</p>
                <div className="daily-actions">
                  {selected.status === "draft" ? (
                    <DailyActionForm
                      action={approveDailyReportAction}
                      button={t("daily.approve")}
                      pending={t("common.submitting")}
                      reportId={selected.reportId}
                    />
                  ) : null}
                  {selected.status === "approved" ? (
                    <DailyActionForm
                      action={publishDailyReportAction}
                      button={t("daily.publish")}
                      pending={t("daily.publishing")}
                      reportId={selected.reportId}
                    />
                  ) : null}
                  {selected.status === "published" && list.ok && list.data.discordConfigured ? (
                    <DailyActionForm
                      action={retryDiscordAction}
                      button={t("daily.retryDiscord")}
                      pending={t("common.submitting")}
                      reportId={selected.reportId}
                    />
                  ) : null}
                </div>
                <table className="table">
                  <thead>
                    <tr>
                      <th>{t("daily.channel")}</th>
                      <th>{t("table.status")}</th>
                      <th>{t("daily.attempts")}</th>
                      <th>{t("daily.destination")}</th>
                      <th>{t("daily.deliveryError")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {deliveries.map((delivery) => (
                      <tr key={delivery.deliveryId}>
                        <td>{delivery.channel}</td>
                        <td><StatusBadge tone={statusTone(delivery.status)}>{delivery.status}</StatusBadge></td>
                        <td>{delivery.attempts}</td>
                        <td>{delivery.destinationLabel}</td>
                        <td>{delivery.lastError ?? "-"}</td>
                      </tr>
                    ))}
                    {!deliveries.length ? (
                      <tr><td colSpan={5}>{t("daily.noDeliveries")}</td></tr>
                    ) : null}
                  </tbody>
                </table>
              </section>
            </>
          ) : (
            <section className="card"><p className="notice">{t("daily.selectReport")}</p></section>
          )}
        </div>
      </div>
    </AdminShell>
  );
}

function DailyReportSummary({ report }: { report: DailyReport }) {
  const metrics = report.metrics;
  return (
    <section className="daily-metric-grid">
      <DailyMetric label="DAU" value={metrics.dailyActiveAccounts} />
      <DailyMetric label="EVENTS" value={metrics.gameplayEventCount} />
      <DailyMetric label="ZONES" value={metrics.activeZones} />
      <DailyMetric label="ONLINE" value={metrics.onlineAtGeneration} />
      <DailyMetric label="GOLD" value={metrics.totalGoldStock} />
      <DailyMetric
        label="SERVICES"
        value={`${metrics.healthyServices}/${metrics.configuredServices}`}
      />
    </section>
  );
}

function DailyMetric({ label, value }: { label: string; value: number | string }) {
  const display = typeof value === "number" ? value.toLocaleString() : value;
  return (
    <div className="card daily-metric">
      <span>{label}</span>
      <strong>{display}</strong>
    </div>
  );
}

function SafeMarkdown({ source }: { source: string }) {
  return source.split(/\r?\n/).map((line, index) => {
    const value = line.trim();
    if (!value) return <div className="daily-markdown-gap" key={`gap-${index}`} />;
    if (value.startsWith("### ")) {
      return <h4 key={`h4-${index}`}>{inlineMarkdown(value.slice(4))}</h4>;
    }
    if (value.startsWith("## ")) {
      return <h3 key={`h3-${index}`}>{inlineMarkdown(value.slice(3))}</h3>;
    }
    if (value.startsWith("# ")) {
      return <h2 key={`h2-${index}`}>{inlineMarkdown(value.slice(2))}</h2>;
    }
    if (value.startsWith("- ")) {
      return <p className="daily-markdown-list" key={`li-${index}`}>• {inlineMarkdown(value.slice(2))}</p>;
    }
    if (value.startsWith("> ")) {
      return <blockquote key={`quote-${index}`}>{inlineMarkdown(value.slice(2))}</blockquote>;
    }
    return <p key={`p-${index}`}>{inlineMarkdown(value)}</p>;
  });
}

function inlineMarkdown(value: string) {
  return value.split(/(\*\*[^*]+\*\*)/).map((part, index) =>
    part.startsWith("**") && part.endsWith("**") ? (
      <strong key={`${part}-${index}`}>{part.slice(2, -2)}</strong>
    ) : (
      part
    )
  );
}

function DailyActionForm({
  action,
  button,
  pending,
  reportId
}: {
  action: (formData: FormData) => Promise<void>;
  button: string;
  pending: string;
  reportId: string;
}) {
  return (
    <form action={action} className="daily-action-form">
      <input name="reportId" type="hidden" value={reportId} />
      <input
        className="control"
        minLength={8}
        name="reason"
        placeholder="输入至少 8 个字符的审核理由"
        required
      />
      <SubmitButton idle={button} pending={pending} />
    </form>
  );
}

function statusTone(status: string): "success" | "warn" | "danger" {
  if (["published", "delivered", "approved", "succeeded"].includes(status)) return "success";
  if (["dead_letter", "failed"].includes(status)) return "danger";
  return "warn";
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
