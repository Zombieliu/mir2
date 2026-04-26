import { AdminShell } from "../../components/admin-shell";
import { MailCommandForm } from "../../components/mail-command-form";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type SystemMailReceipt } from "../../lib/admin-api";
import { getAdminI18n } from "../../lib/i18n";

export default async function GmToolsPage() {
  const { t } = await getAdminI18n();
  const outbox = await adminGet<SystemMailReceipt[]>("/admin/system-mail/outbox");

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
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("gm.systemMail")}</p>
          <h3>{t("gm.queueCommand")}</h3>
          <MailCommandForm
            text={{
              idle: t("mail.idle"),
              submitting: t("mail.submitting"),
              rejected: t("mail.rejected"),
              queued: t("mail.queued"),
              targetKind: t("mail.targetKind"),
              targetCharacter: t("mail.targetCharacter"),
              targetAccount: t("mail.targetAccount"),
              targetGlobal: t("mail.targetGlobal"),
              targetId: t("mail.targetId"),
              subject: t("mail.subject"),
              defaultSubject: t("mail.defaultSubject"),
              attachment: t("mail.attachment"),
              body: t("mail.body"),
              defaultBody: t("mail.defaultBody"),
              reason: t("mail.reason"),
              defaultReason: t("mail.defaultReason"),
              queueing: t("mail.queueing"),
              queue: t("mail.queue"),
              preview: t("mail.preview")
            }}
          />
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
            <button className="button secondary">{t("gm.announcement")}</button>
            <button className="button secondary">{t("gm.sensitiveWords")}</button>
            <button className="button secondary">{t("gm.hotfixNotice")}</button>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
