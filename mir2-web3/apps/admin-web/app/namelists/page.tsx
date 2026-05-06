import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminNameListsReadModel } from "../../lib/admin-api";
import {
  addNameListAction,
  createNameListAction,
  deleteNameListAction,
  removeNameListAction
} from "../ops-actions";

export default async function NameListsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const namelists = await adminGet<AdminNameListsReadModel>("/admin/read/namelists");
  const rows = namelists.ok ? namelists.data.lists : [];

  return (
    <AdminShell active="/namelists">
      <div className="page-head">
        <div>
          <p className="eyebrow">NameLists</p>
          <h2>NPC namelist files</h2>
          <p className="muted">Add or remove character names from Crystal-style NameLists files used by NPC scripts.</p>
        </div>
        <StatusBadge tone={namelists.ok ? "success" : "warn"}>
          {namelists.ok ? namelists.data.root : "unavailable"}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
      {!namelists.ok ? <p className="notice">{namelists.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Lists</p>
          <table className="table">
            <thead>
              <tr>
                <th>List</th>
                <th>Players</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((list) => (
                <tr key={list.path}>
                  <td>
                    <strong>{list.listName}</strong>
                    <div className="muted">{list.path}</div>
                  </td>
                  <td>{list.players.join(", ") || "-"}</td>
                </tr>
              ))}
              {!rows.length ? (
                <tr>
                  <td colSpan={2}>No namelist files found.</td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Update</p>
          <h3>Create list</h3>
          <form action={createNameListAction} className="form-stack">
            <input className="control" name="listName" placeholder="Rebirth1 or Rebirth1.txt" />
            <input className="control" name="reason" defaultValue="create namelist from admin console" />
            <SubmitButton idle="Create list" pending="Creating..." />
          </form>
          <div className="rune-divider" />
          <h3>Add player</h3>
          <form action={addNameListAction} className="form-stack">
            <input className="control" name="listName" placeholder="Rebirth1 or Rebirth1.txt" />
            <input className="control" name="playerName" placeholder="character name" />
            <input className="control" name="reason" defaultValue="add namelist entry from admin console" />
            <SubmitButton idle="Add" pending="Adding..." />
          </form>
          <div className="rune-divider" />
          <h3>Remove player</h3>
          <form action={removeNameListAction} className="form-stack">
            <input className="control" name="listName" placeholder="Rebirth1 or Rebirth1.txt" />
            <input className="control" name="playerName" placeholder="character name" />
            <input className="control" name="reason" defaultValue="remove namelist entry from admin console" />
            <SubmitButton idle="Remove" pending="Removing..." />
          </form>
          <div className="rune-divider" />
          <h3>Delete list</h3>
          <form action={deleteNameListAction} className="form-stack">
            <input className="control" name="listName" placeholder="Rebirth1 or Rebirth1.txt" />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="delete namelist from admin console" />
            <SubmitButton idle="Delete list" pending="Deleting..." />
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
