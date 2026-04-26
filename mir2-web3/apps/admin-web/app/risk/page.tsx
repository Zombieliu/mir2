import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { riskRows } from "../../lib/mock-data";

export default function RiskPage() {
  return (
    <AdminShell active="/risk">
      <div className="page-head">
        <div>
          <p className="eyebrow">Anti-Cheat</p>
          <h2>Risk and fraud control</h2>
          <p className="muted">Suspicious login, multi-boxing, script idle, abnormal resource growth, and trade graph review.</p>
        </div>
        <div className="actions">
          <button className="button secondary">Batch Review</button>
          <button className="button">Open Case Queue</button>
        </div>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Case Queue</p>
          <table className="table">
            <thead>
              <tr>
                <th>Player</th>
                <th>Signal</th>
                <th>Risk</th>
                <th>Evidence</th>
              </tr>
            </thead>
            <tbody>
              {riskRows.map(([player, signal, risk, evidence]) => (
                <tr key={player}>
                  <td>{player}</td>
                  <td>{signal}</td>
                  <td>
                    <StatusBadge tone={risk === "Critical" ? "danger" : risk === "High" ? "warn" : "default"}>
                      {risk}
                    </StatusBadge>
                  </td>
                  <td>{evidence}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card relationship-map">
          <p className="eyebrow">Suspicious Trade Graph</p>
          <div className="node hot" style={{ left: "38%", top: "34%" }}>
            SilentCart
          </div>
          <div className="node" style={{ left: "8%", top: "12%" }}>
            Mule 07
          </div>
          <div className="node" style={{ right: "8%", top: "14%" }}>
            Broker
          </div>
          <div className="node" style={{ left: "14%", bottom: "8%" }}>
            Miner
          </div>
          <div className="node" style={{ right: "16%", bottom: "10%" }}>
            Auction
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
