import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import {
  adminGet,
  type AdminAccountsReadModel,
  type AdminPlayersReadModel
} from "../../lib/admin-api";
import { formatNumber, statusTone } from "../../lib/format";
import {
  createAccountAction,
  deleteAccountAction,
  updateAccountAction,
  updateCharacterAction
} from "../ops-actions";

export default async function AccountsPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const accounts = await adminGet<AdminAccountsReadModel>("/admin/read/accounts");
  const players = await adminGet<AdminPlayersReadModel>("/admin/read/players");
  const accountRows = accounts.ok ? accounts.data.accounts : [];
  const playerRows = players.ok ? players.data.players : [];

  return (
    <AdminShell active="/accounts">
      <div className="page-head">
        <div>
          <p className="eyebrow">Account Console</p>
          <h2>Accounts and character saves</h2>
          <p className="muted">
            Create, edit, unban, clear storage passwords, delete accounts, and adjust persisted character state.
          </p>
        </div>
        <StatusBadge tone={accounts.ok ? "success" : "warn"}>
          {accounts.ok ? accounts.data.source : "unavailable"}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
      {!accounts.ok ? <p className="notice">{accounts.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Accounts</p>
          <table className="table">
            <thead>
              <tr>
                <th>Account</th>
                <th>Characters</th>
                <th>Storage</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {accountRows.map((account) => (
                <tr key={account.accountId}>
                  <td>
                    <strong>{account.accountId}</strong>
                    <div className="muted">v{account.storeVersion ?? "-"}</div>
                  </td>
                  <td>{formatNumber(account.characterCount)}</td>
                  <td>
                    {account.storageSize}
                    {account.hasStoragePassword ? " / locked" : ""}
                  </td>
                  <td>
                    <StatusBadge tone={account.isBanned ? "danger" : "success"}>
                      {account.isBanned ? "Banned" : "Active"}
                    </StatusBadge>
                  </td>
                </tr>
              ))}
              {!accountRows.length ? (
                <tr>
                  <td colSpan={4}>No accounts found.</td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Account Writes</p>
          <h3>Create account</h3>
          <form action={createAccountAction} className="form-stack">
            <input className="control" name="accountId" placeholder="account id" />
            <input className="control" name="password" placeholder="password" />
            <input className="control" name="reason" defaultValue="create account from admin console" />
            <SubmitButton idle="Create account" pending="Creating..." />
          </form>
          <div className="rune-divider" />
          <h3>Update account</h3>
          <form action={updateAccountAction} className="form-stack">
            <input className="control" name="accountId" placeholder="account id" />
            <input className="control" name="password" placeholder="new password" />
            <input className="control" name="storageSize" placeholder="storage size" type="number" />
            <label className="check-row">
              <input name="clearStoragePassword" type="checkbox" />
              <span>Clear storage password</span>
            </label>
            <label className="check-row">
              <input name="unban" type="checkbox" />
              <span>Unban account</span>
            </label>
            <input className="control" name="reason" defaultValue="update account from admin console" />
            <SubmitButton idle="Update account" pending="Updating..." />
          </form>
          <div className="rune-divider" />
          <h3>Delete account</h3>
          <form action={deleteAccountAction} className="form-stack">
            <input className="control" name="accountId" placeholder="account id" />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="delete account from admin console" />
            <SubmitButton idle="Delete account" pending="Deleting..." />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">Characters</p>
          <table className="table">
            <thead>
              <tr>
                <th>Character</th>
                <th>Level</th>
                <th>Gold</th>
                <th>PK</th>
                <th>Map</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {playerRows.map((player) => (
                <tr key={player.playerId}>
                  <td>
                    <strong>{player.characterName}</strong>
                    <div className="muted">{player.playerId}</div>
                  </td>
                  <td>{player.level}</td>
                  <td>{formatNumber(player.gold)}</td>
                  <td>{player.pkPoints}</td>
                  <td>{player.mapTitle || player.mapFileName || "-"}</td>
                  <td>
                    <StatusBadge tone={statusTone(player.status)}>{player.status}</StatusBadge>
                  </td>
                </tr>
              ))}
              {!playerRows.length ? (
                <tr>
                  <td colSpan={6}>No character saves found.</td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Character Writes</p>
          <h3>Edit save</h3>
          <form action={updateCharacterAction} className="form-grid">
            <label className="field full">
              <span>Character ID</span>
              <input className="control" name="characterId" placeholder="demo:0 or character name" />
            </label>
            <input className="control" name="name" placeholder="new name" />
            <input className="control" name="level" placeholder="level" type="number" />
            <input className="control" name="gold" placeholder="gold" type="number" />
            <input className="control" name="credit" placeholder="credit" type="number" />
            <input className="control" name="pkPoints" placeholder="PK points" type="number" />
            <input className="control" name="mapFileName" placeholder="map file" />
            <input className="control" name="mapTitle" placeholder="map title" />
            <input className="control" name="positionX" placeholder="x" type="number" />
            <input className="control" name="positionY" placeholder="y" type="number" />
            <input className="control" name="hp" placeholder="hp" type="number" />
            <input className="control" name="maxHp" placeholder="max hp" type="number" />
            <input className="control" name="mp" placeholder="mp" type="number" />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="update character save from admin console" />
            <div className="field full">
              <SubmitButton idle="Update character" pending="Updating..." />
            </div>
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
