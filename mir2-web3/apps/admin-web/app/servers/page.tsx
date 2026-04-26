import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { servers } from "../../lib/mock-data";

export default function ServersPage() {
  return (
    <AdminShell active="/servers">
      <div className="page-head">
        <div>
          <p className="eyebrow">World Monitor</p>
          <h2>Server and zone runtime</h2>
          <p className="muted">Online trends, CPU, memory, latency, map load, boss timers, and instance state.</p>
        </div>
        <StatusBadge tone="success">31 zones online</StatusBadge>
      </div>
      <div className="grid three">
        <section className="card">
          <p className="eyebrow">CPU / Memory</p>
          <Bars
            rows={[
              { label: "Gateway", value: 32 },
              { label: "World S1", value: 68 },
              { label: "World S2", value: 54 },
              { label: "Admin API", value: 12 }
            ]}
          />
        </section>
        <section className="card">
          <p className="eyebrow">Latency</p>
          <p className="metric-value">41ms</p>
          <p className="muted">Global p50 / p95 112ms / error rate 0.018%</p>
        </section>
        <section className="card">
          <p className="eyebrow">Alerts</p>
          <p className="metric-value">3</p>
          <p className="muted">One hot-map load warning, two auction queue delays.</p>
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <p className="eyebrow">Realm List</p>
        <table className="table">
          <tbody>
            {servers.map(([realm, online, latency, status]) => (
              <tr key={realm}>
                <td>{realm}</td>
                <td>{online} online</td>
                <td>{latency}</td>
                <td>
                  <StatusBadge tone={status === "Warn" ? "warn" : "success"}>
                    {status}
                  </StatusBadge>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}
