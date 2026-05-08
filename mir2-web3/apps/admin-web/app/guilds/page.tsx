import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminGuildsReadModel } from "../../lib/admin-api";
import { removeGuildMemberAction, sendGuildMessageAction } from "../ops-actions";

export default async function GuildsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const guilds = await adminGet<AdminGuildsReadModel>("/admin/read/guilds");
  const rows = guilds.ok ? guilds.data.guilds : [];

  return (
    <AdminShell active="/guilds">
      <div className="page-head">
        <div>
          <p className="eyebrow">Guild Console</p>
          <h2>Guild members, ranks, and notices</h2>
          <p className="muted">Mirror Crystal GuildItem operations over persisted Stage 5 guild state.</p>
        </div>
        <StatusBadge tone={guilds.ok ? "success" : "warn"}>
          {guilds.ok ? guilds.data.source : "unavailable"}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
      {!guilds.ok ? <p className="notice">{guilds.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Guilds</p>
          <table className="table">
            <thead>
              <tr>
                <th>Guild</th>
                <th>Members</th>
                <th>Ranks</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((guild) => (
                <tr key={guild.guildName}>
                  <td>
                    <strong>{guild.guildName}</strong>
                    <div className="muted">{guild.chatLog.slice(-1)[0] ?? "no notice"}</div>
                  </td>
                  <td>{guild.members.join(", ") || "-"}</td>
                  <td>{guild.ranks.join(", ") || "-"}</td>
                </tr>
              ))}
              {!rows.length ? (
                <tr>
                  <td colSpan={3}>No guild state found.</td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Moderation</p>
          <h3>Remove member</h3>
          <form action={removeGuildMemberAction} className="form-stack">
            <input className="control" name="guildName" placeholder="guild name" />
            <input className="control" name="memberName" placeholder="member name" />
            <input className="control" name="reason" defaultValue="guild moderation from admin console" />
            <SubmitButton idle="Remove member" pending="Removing..." />
          </form>
          <div className="rune-divider" />
          <h3>Guild message</h3>
          <form action={sendGuildMessageAction} className="form-stack">
            <input className="control" name="guildName" placeholder="guild name" />
            <textarea className="control" name="message" defaultValue="Guild notice from GM." />
            <input className="control" name="reason" defaultValue="guild message from admin console" />
            <SubmitButton idle="Send guild message" pending="Sending..." />
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
