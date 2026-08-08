import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import {
  adminGet,
  type AdminAuthMeResponse,
  type ApprovalRecord
} from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import {
  approveApprovalAction,
  createApprovalAction,
  rejectApprovalAction
} from "./actions";

export default async function ApprovalsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const approvals = await adminGet<ApprovalRecord[]>("/admin/approvals");
  const auth = await adminGet<AdminAuthMeResponse>("/admin/auth/me");
  const operatorId = auth.ok ? auth.data.operator.operatorId : undefined;

  return (
    <AdminShell active="/approvals">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("approvals.eyebrow")}</p>
          <h2>{t("approvals.title")}</h2>
          <p className="muted">{t("approvals.subtitle")}</p>
        </div>
        <StatusBadge tone={approvals.ok ? "success" : "warn"}>
          {approvals.ok ? t("common.apiReady") : t("common.apiOffline")}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("approvals.create")}</p>
          <form action={createApprovalAction} className="form-grid">
            <input
              className="control"
              name="approvalId"
              placeholder={t("approvals.approvalId")}
            />
            <input
              className="control"
              name="commandId"
              placeholder={t("approvals.commandId")}
              required
            />
            <input
              className="control"
              name="commandType"
              placeholder={t("approvals.commandType")}
              required
            />
            <textarea
              className="control"
              name="reason"
              placeholder={t("approvals.reason")}
              required
            />
            <SubmitButton
              idle={t("approvals.request")}
              pending={t("common.submitting")}
            />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">{t("approvals.queue")}</p>
          {approvals.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("approvals.approval")}</th>
                  <th>{t("approvals.command")}</th>
                  <th>{t("table.status")}</th>
                  <th>{t("approvals.decision")}</th>
                </tr>
              </thead>
              <tbody>
                {approvals.data.length ? (
                  approvals.data.map((approval) => (
                    <tr key={approval.approvalId}>
                      <td>{approval.approvalId}</td>
                      <td>
                        {approval.commandType} / {approval.commandId}
                        <div className="muted">
                          {t("approvals.requestedBy")}: {approval.requestedBy}
                          {approval.decidedBy ? ` / ${t("approvals.decidedBy")}: ${approval.decidedBy}` : ""}
                        </div>
                      </td>
                      <td>
                        <StatusBadge
                          tone={approval.status === "approved" ? "success" : "warn"}
                        >
                          {translateAdminStatus(t, approval.status)}
                        </StatusBadge>
                      </td>
                      <td>
                        {approval.status === "pending" && approval.requestedBy === operatorId ? (
                          <p className="muted">{t("approvals.peerRequired")}</p>
                        ) : approval.status === "pending" ? (
                          <div className="actions">
                            <form action={approveApprovalAction}>
                              <input
                                name="approvalId"
                                type="hidden"
                                value={approval.approvalId}
                              />
                              <input
                                name="reason"
                                type="hidden"
                                value={t("approvals.defaultApproveReason")}
                              />
                              <SubmitButton
                                confirmMessage={t("common.confirmDangerous")}
                                idle={t("approvals.approve")}
                                pending={t("common.submitting")}
                              />
                            </form>
                            <form action={rejectApprovalAction}>
                              <input
                                name="approvalId"
                                type="hidden"
                                value={approval.approvalId}
                              />
                              <input
                                name="reason"
                                type="hidden"
                                value={t("approvals.defaultRejectReason")}
                              />
                              <SubmitButton
                                className="button secondary"
                                confirmMessage={t("common.confirmDangerous")}
                                idle={t("approvals.reject")}
                                pending={t("common.submitting")}
                              />
                            </form>
                          </div>
                        ) : (
                          approval.decisionReason ?? "-"
                        )}
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={4}>{t("approvals.empty")}</td>
                  </tr>
                )}
              </tbody>
            </table>
          ) : (
            <p className="notice">{approvals.error}</p>
          )}
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
