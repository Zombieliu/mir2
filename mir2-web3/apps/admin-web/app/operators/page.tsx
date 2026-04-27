import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminOperatorsReadModel } from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { upsertOperatorAction } from "./actions";

export default async function OperatorsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const operators = await adminGet<AdminOperatorsReadModel>("/admin/read/operators");
  const data = operators.ok ? operators.data : undefined;

  return (
    <AdminShell active="/operators">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("operators.eyebrow")}</p>
          <h2>{t("operators.title")}</h2>
          <p className="muted">{t("operators.subtitle")}</p>
        </div>
        <StatusBadge tone={data?.configured ? "success" : "warn"}>
          {data?.configured ? data.source : t("common.unconfigured")}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {!operators.ok ? <p className="notice">{operators.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("operators.list")}</p>
          <table className="table">
            <thead>
              <tr>
                <th>{t("operators.operator")}</th>
                <th>{t("operators.role")}</th>
                <th>{t("table.status")}</th>
                <th>{t("operators.permissions")}</th>
              </tr>
            </thead>
            <tbody>
              {(data?.operators ?? []).map((operator) => (
                <tr key={operator.operatorId}>
                  <td>
                    <strong>{operator.operatorId}</strong>
                    <div className="muted">{operator.email}</div>
                  </td>
                  <td>{operator.role}</td>
                  <td>
                    <StatusBadge tone={operator.status === "Active" ? "success" : "warn"}>
                      {translateAdminStatus(t, operator.status)}
                    </StatusBadge>
                  </td>
                  <td>{operator.permissions.join(", ") || "-"}</td>
                </tr>
              ))}
              {!(data?.operators?.length ?? 0) ? (
                <tr>
                  <td colSpan={4}>
                    <p className="notice">{t("operators.empty")}</p>
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("operators.create")}</p>
          <form action={upsertOperatorAction} className="form-stack">
            <div className="form-grid">
              <label className="field">
                <span>{t("operators.operatorId")}</span>
                <input className="control" name="operatorId" placeholder="ops-lead" />
              </label>
              <label className="field">
                <span>{t("operators.email")}</span>
                <input className="control" name="email" required />
              </label>
              <label className="field">
                <span>{t("operators.role")}</span>
                <input className="control" name="role" defaultValue="ops_admin" required />
              </label>
              <label className="field">
                <span>{t("table.status")}</span>
                <select className="control" name="status" defaultValue="Active">
                  <option>Active</option>
                  <option>Suspended</option>
                </select>
              </label>
              <label className="field full">
                <span>{t("operators.permissions")}</span>
                <textarea
                  className="control"
                  name="permissions"
                  defaultValue="account_read,audit_read,approval_manage,content_publish"
                />
              </label>
            </div>
            <SubmitButton idle={t("common.submit")} pending={t("common.submitting")} />
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}
