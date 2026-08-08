import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import {
  adminGet,
  type AdminCommandRecord,
  type SystemMailReceipt
} from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import {
  banAccountAction,
  grantCurrencyAction,
  grantItemAction,
  kickPlayerAction,
  sendSystemMailAction
} from "./actions";

export default async function GmToolsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const outbox = await adminGet<SystemMailReceipt[]>("/admin/system-mail/outbox");
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const commandStatus = commandId
    ? await adminGet<AdminCommandRecord>(
        `/admin/commands/${encodeURIComponent(commandId)}/status`
      )
    : undefined;
  const commandRecord = commandStatus?.ok ? commandStatus.data : undefined;
  const activeReceipt =
    commandId && outbox.ok
      ? outbox.data.find((receipt) => receipt.outboxId === `mail-${commandId}`)
      : undefined;

  return (
    <AdminShell active="/gm-tools">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("gm.eyebrow")}</p>
          <h2>{t("gm.title")}</h2>
          <p className="muted">{t("gm.subtitle")}</p>
        </div>
        <StatusBadge tone={outbox.ok ? "success" : "warn"}>
          {outbox.ok ? t("common.apiReady") : t("common.apiOffline")}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">{t("gm.commandQueued")}: {commandId}</p> : null}
      {commandId ? (
        <section className="card">
          <div className="section-head">
            <div>
              <p className="eyebrow">{t("gm.commandStatus")}</p>
              <h3>{commandId}</h3>
            </div>
            <StatusBadge tone={commandRecord ? statusTone(commandRecord.status) : "warn"}>
              {commandRecord
                ? translateAdminStatus(t, commandRecord.status)
                : t("common.unavailable")}
            </StatusBadge>
          </div>
          {commandRecord ? (
            <table className="table">
              <tbody>
                <tr>
                  <th>{t("gm.statusTarget")}</th>
                  <td>
                    {commandRecord.envelope.target.targetType} /{" "}
                    {commandRecord.envelope.target.targetId}
                  </td>
                </tr>
                <tr>
                  <th>{t("gm.statusResult")}</th>
                  <td>{commandRecord.resultMessage ?? commandRecord.errorCode ?? "-"}</td>
                </tr>
                <tr>
                  <th>{t("gm.statusTrace")}</th>
                  <td>{commandRecord.envelope.traceId}</td>
                </tr>
                <tr>
                  <th>{t("audit.operator")}</th>
                  <td>{commandRecord.envelope.operator.email}</td>
                </tr>
              </tbody>
            </table>
          ) : (
            <p className="notice">
              {commandStatus && !commandStatus.ok
                ? commandStatus.error
                : t("gm.statusPendingRead")}
            </p>
          )}
          <div className="rune-divider" />
          {activeReceipt ? (
            <table className="table">
              <tbody>
                <tr>
                  <th>{t("gm.statusDelivery")}</th>
                  <td>
                    {activeReceipt.deliveryMode} / {activeReceipt.deliveredCount}
                  </td>
                </tr>
                <tr>
                  <th>{t("gm.mailIds")}</th>
                  <td>
                    {activeReceipt.mailIds.length ? activeReceipt.mailIds.join(", ") : "-"}
                  </td>
                </tr>
              </tbody>
            </table>
          ) : (
            <p className="notice">
              {outbox.ok ? t("gm.statusNoReceipt") : outbox.error}
            </p>
          )}
        </section>
      ) : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("gm.systemMail")}</p>
          <h3>{t("gm.queueCommand")}</h3>
          <form action={sendSystemMailAction} className="form-grid">
            <div className="field">
              <label>{t("mail.targetKind")}</label>
              <select className="control" defaultValue="character" name="targetKind">
                <option value="character">{t("mail.targetCharacter")}</option>
                <option value="account">{t("mail.targetAccount")}</option>
                <option value="global">{t("mail.targetGlobal")}</option>
              </select>
            </div>
            <div className="field">
              <label>{t("mail.targetId")}</label>
              <input className="control" defaultValue="demo:0" name="targetId" />
            </div>
            <div className="field">
              <label>{t("mail.subject")}</label>
              <input className="control" defaultValue={t("mail.defaultSubject")} name="subject" />
            </div>
            <div className="field">
              <label>{t("mail.attachment")}</label>
              <div style={{ display: "grid", gap: 8, gridTemplateColumns: "1fr 90px" }}>
                <input className="control" defaultValue="gold" name="itemId" />
                <input className="control" defaultValue="5000" min="1" name="count" type="number" />
              </div>
            </div>
            <div className="field full">
              <label>{t("mail.body")}</label>
              <textarea
                className="control"
                defaultValue={t("mail.defaultBody")}
                name="body"
              />
            </div>
            <div className="field full">
              <label>{t("mail.reason")}</label>
              <input
                className="control"
                defaultValue={t("mail.defaultReason")}
                name="reason"
              />
            </div>
            <div className="field full">
              <div className="actions">
                <SubmitButton idle={t("mail.queue")} pending={t("mail.queueing")} />
              </div>
            </div>
            <div className="notice">{t("mail.idle")}</div>
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">{t("gm.highRisk")}</p>
          <h3>{t("gm.grants")}</h3>
          <form action={grantItemAction} className="form-stack">
            <input className="control" name="commandId" placeholder={t("gm.commandId")} />
            <input className="control" name="traceId" placeholder={t("gm.traceId")} />
            <input className="control" name="approvalId" placeholder={t("gm.approvalId")} />
            <input className="control" name="characterId" placeholder={t("gm.characterId")} />
            <input className="control" name="itemId" placeholder={t("gm.itemId")} />
            <input className="control" name="count" placeholder={t("gm.count")} type="number" />
            <input className="control" name="reason" placeholder={t("mail.reason")} />
            <SubmitButton
              confirmMessage={t("common.confirmDangerous")}
              idle={t("gm.grantItem")}
              pending={t("common.submitting")}
            />
          </form>
          <div className="rune-divider" />
          <form action={grantCurrencyAction} className="form-stack">
            <input className="control" name="commandId" placeholder={t("gm.commandId")} />
            <input className="control" name="traceId" placeholder={t("gm.traceId")} />
            <input className="control" name="approvalId" placeholder={t("gm.approvalId")} />
            <input className="control" name="characterId" placeholder={t("gm.characterId")} />
            <input className="control" defaultValue="gold" name="currency" placeholder={t("gm.currency")} />
            <input className="control" name="amount" placeholder={t("gm.amount")} type="number" />
            <input className="control" name="reason" placeholder={t("mail.reason")} />
            <SubmitButton
              confirmMessage={t("common.confirmDangerous")}
              idle={t("gm.grantCurrency")}
              pending={t("common.submitting")}
            />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">{t("gm.sessionControl")}</p>
          <h3>{t("gm.kickPlayer")}</h3>
          <form action={kickPlayerAction} className="form-stack">
            <input className="control" name="commandId" placeholder={t("gm.commandId")} />
            <input className="control" name="traceId" placeholder={t("gm.traceId")} />
            <input className="control" name="characterId" placeholder={t("gm.characterId")} />
            <input className="control" name="reason" placeholder={t("mail.reason")} />
            <SubmitButton
              confirmMessage={t("common.confirmDangerous")}
              idle={t("gm.kickPlayer")}
              pending={t("common.submitting")}
            />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">{t("gm.accountControl")}</p>
          <h3>{t("gm.banAccount")}</h3>
          <form action={banAccountAction} className="form-stack">
            <input className="control" name="commandId" placeholder={t("gm.commandId")} />
            <input className="control" name="traceId" placeholder={t("gm.traceId")} />
            <input className="control" name="approvalId" placeholder={t("gm.approvalId")} />
            <input className="control" name="accountId" placeholder={t("gm.accountId")} />
            <input className="control" name="durationSeconds" placeholder={t("gm.durationSeconds")} type="number" />
            <input className="control" name="reason" placeholder={t("mail.reason")} />
            <SubmitButton
              confirmMessage={t("common.confirmDangerous")}
              idle={t("gm.banAccount")}
              pending={t("common.submitting")}
            />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">{t("gm.outboxReceipts")}</p>
          {outbox.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("gm.outbox")}</th>
                  <th>{t("gm.target")}</th>
                  <th>{t("gm.delivery")}</th>
                  <th>{t("gm.mailIds")}</th>
                </tr>
              </thead>
              <tbody>
                {outbox.data.length ? (
                  outbox.data.map((receipt) => (
                    <tr key={receipt.outboxId}>
                      <td>{receipt.outboxId}</td>
                      <td>
                        {receipt.targetKind} / {receipt.targetId}
                      </td>
                      <td>
                        {receipt.deliveryMode} / {receipt.deliveredCount}
                      </td>
                      <td>{receipt.mailIds.length ? receipt.mailIds.join(", ") : "-"}</td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={4}>{t("gm.emptyOutbox")}</td>
                  </tr>
                )}
              </tbody>
            </table>
          ) : (
            <p className="notice">{outbox.error}</p>
          )}
          <div className="rune-divider" />
          <div className="actions">
            <button className="button secondary" disabled type="button">
              {t("gm.announcement")}
            </button>
            <button className="button secondary" disabled type="button">
              {t("gm.sensitiveWords")}
            </button>
            <button className="button secondary" disabled type="button">
              {t("gm.hotfixNotice")}
            </button>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}

function statusTone(status: string): "success" | "warn" | "danger" {
  switch (status) {
    case "succeeded":
      return "success";
    case "failed":
    case "denied":
    case "rejected":
    case "dead_letter":
      return "danger";
    default:
      return "warn";
  }
}
