import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { activities } from "../../lib/mock-data";

export default function ActivitiesPage() {
  return (
    <AdminShell active="/activities">
      <div className="page-head">
        <div>
          <p className="eyebrow">LiveOps</p>
          <h2>Activity configuration</h2>
          <p className="muted">Draft, preview, target, and publish multi-realm events with audit trail.</p>
        </div>
        <button className="button">Create Activity</button>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Activity List</p>
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Start</th>
                <th>Type</th>
                <th>Status</th>
                <th>Signal</th>
              </tr>
            </thead>
            <tbody>
              {activities.map(([name, start, type, status, signal]) => (
                <tr key={name}>
                  <td>{name}</td>
                  <td>{start}</td>
                  <td>{type}</td>
                  <td>
                    <StatusBadge tone={status === "Running" ? "success" : "warn"}>
                      {status}
                    </StatusBadge>
                  </td>
                  <td>{signal}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Config Preview</p>
          <div className="form-grid">
            <div className="field full">
              <label>Activity Name</label>
              <input className="control" defaultValue="Red Moon Boss Rush" />
            </div>
            <div className="field">
              <label>Reward</label>
              <input className="control" defaultValue="red-moon-token x12" />
            </div>
            <div className="field">
              <label>Condition</label>
              <input className="control" defaultValue="level >= 35" />
            </div>
            <div className="field full">
              <label>Realms</label>
              <input className="control" defaultValue="S1, S2, S3, EU Mirror" />
            </div>
          </div>
          <div className="rune-divider" />
          <div className="actions">
            <button className="button secondary">Save Draft</button>
            <button className="button">Submit Review</button>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
