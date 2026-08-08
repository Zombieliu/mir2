import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminControlReadModel } from "../../lib/admin-api";
import {
  broadcastAction,
  killPetsAction,
  killPlayerAction,
  messagePlayerAction,
  returnSafeZoneAction,
  serverControlAction
} from "../ops-actions";

export default async function ConsolePage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const control = await adminGet<AdminControlReadModel>("/admin/read/control");
  const data = control.ok ? control.data : undefined;

  return (
    <AdminShell active="/console">
      <div className="page-head">
        <div>
          <p className="eyebrow">Crystal Engine Console</p>
          <h2>Server control and GM session actions</h2>
          <p className="muted">
            Reload services, broadcast notices, message players, return characters to safe zone, and review log tails.
          </p>
        </div>
        <StatusBadge tone={control.ok ? "success" : "warn"}>
          {control.ok ? "console API" : "unavailable"}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
      {!control.ok ? <p className="notice">{control.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Server</p>
          <h3>Control actions</h3>
          <form action={serverControlAction} className="form-grid">
            <label className="field">
              <span>Action</span>
              <select className="control" name="action" defaultValue="status">
                {(data?.controlActions ?? ["status"]).map((action) => (
                  <option key={action} value={action}>
                    {action}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Target</span>
              <input className="control" name="target" placeholder="gateway / world / all" />
            </label>
            <label className="field full">
              <span>Approval ID</span>
              <input className="control" name="approvalId" placeholder="required for stop/reboot/close" />
            </label>
            <label className="field full">
              <span>Reason</span>
              <input className="control" name="reason" defaultValue="server console operation" />
            </label>
            <div className="field full">
              <SubmitButton idle="Submit control command" pending="Submitting..." />
            </div>
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">Announcement</p>
          <h3>Broadcast</h3>
          <form action={broadcastAction} className="form-stack">
            <textarea className="control" name="message" defaultValue="Server maintenance notice." />
            <input className="control" name="reason" defaultValue="broadcast from Crystal console parity page" />
            <SubmitButton idle="Broadcast" pending="Broadcasting..." />
          </form>
          <div className="rune-divider" />
          <h3>Direct message</h3>
          <form action={messagePlayerAction} className="form-stack">
            <input className="control" name="characterId" placeholder="demo:0 or character name" />
            <textarea className="control" name="message" defaultValue="GM message." />
            <input className="control" name="reason" defaultValue="direct GM message" />
            <SubmitButton idle="Send message" pending="Sending..." />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">Player Session</p>
          <h3>Position and death tools</h3>
          <form action={returnSafeZoneAction} className="form-stack">
            <input className="control" name="characterId" placeholder="demo:0 or character name" />
            <input className="control" name="reason" defaultValue="return player to safe zone" />
            <SubmitButton idle="Return safe zone" pending="Submitting..." />
          </form>
          <div className="rune-divider" />
          <form action={killPlayerAction} className="form-stack">
            <input className="control" name="characterId" placeholder="demo:0 or character name" />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="GM kill player command" />
            <SubmitButton idle="Kill player" pending="Submitting..." />
          </form>
          <div className="rune-divider" />
          <form action={killPetsAction} className="form-stack">
            <input className="control" name="characterId" placeholder="demo:0 or character name" />
            <input className="control" name="reason" defaultValue="clear companion state" />
            <SubmitButton idle="Kill pets" pending="Submitting..." />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">Logs</p>
          <h3>Server log tails</h3>
          {(data?.logs ?? []).map((log) => (
            <div className="notice" key={`${log.name}-${log.path}`}>
              <strong>{log.name}</strong>
              <div className="muted">{log.path}</div>
              {log.error ? <div>{log.error}</div> : null}
              {log.lines.length ? (
                <pre style={{ whiteSpace: "pre-wrap", margin: 0 }}>{log.lines.join("\n")}</pre>
              ) : null}
            </div>
          ))}
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
