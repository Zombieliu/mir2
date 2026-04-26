import Link from "next/link";
import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { players } from "../../lib/mock-data";

export default function PlayersPage() {
  return (
    <AdminShell active="/players">
      <div className="page-head">
        <div>
          <p className="eyebrow">Player Operations</p>
          <h2>Player management</h2>
          <p className="muted">Search, inspect, mute, ban, mail, compensate, and review access history.</p>
        </div>
        <div className="actions">
          <button className="button secondary">Export CSV</button>
          <button className="button">Open GM Drawer</button>
        </div>
      </div>
      <section className="card">
        <div className="form-grid" style={{ marginBottom: 16 }}>
          <input className="control" placeholder="Player ID, nickname, account..." />
          <select className="control" defaultValue="all">
            <option value="all">All realms</option>
            <option value="risk">Risk only</option>
            <option value="paid">Paid players</option>
          </select>
        </div>
        <table className="table">
          <thead>
            <tr>
              <th>Player</th>
              <th>Class</th>
              <th>Level</th>
              <th>Power</th>
              <th>VIP</th>
              <th>Status</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            {players.map(([id, name, role, level, power, vip, status]) => (
              <tr key={id}>
                <td>
                  <strong>{name}</strong>
                  <div className="muted">{id}</div>
                </td>
                <td>{role}</td>
                <td>{level}</td>
                <td>{power}</td>
                <td>{vip}</td>
                <td>
                  <StatusBadge
                    tone={
                      status === "Normal" ? "success" : status === "Risk" ? "warn" : "danger"
                    }
                  >
                    {status}
                  </StatusBadge>
                </td>
                <td>
                  <Link className="badge" href={`/players/${id}`}>
                    View detail
                  </Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}
