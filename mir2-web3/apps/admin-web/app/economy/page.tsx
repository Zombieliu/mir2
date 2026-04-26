import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { economyRows } from "../../lib/mock-data";

export default function EconomyPage() {
  return (
    <AdminShell active="/economy">
      <div className="page-head">
        <div>
          <p className="eyebrow">Economy Intelligence</p>
          <h2>Currency and market health</h2>
          <p className="muted">Gold, yuanbao, item circulation, auction prices, and inflation signals.</p>
        </div>
        <StatusBadge tone="warn">Blessed Oil inflation</StatusBadge>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Main Currencies</p>
          <table className="table">
            <thead>
              <tr>
                <th>Asset</th>
                <th>Produced</th>
                <th>Consumed</th>
                <th>Net</th>
                <th>State</th>
              </tr>
            </thead>
            <tbody>
              {economyRows.map(([asset, produced, consumed, net, state]) => (
                <tr key={asset}>
                  <td>{asset}</td>
                  <td>{produced}</td>
                  <td>{consumed}</td>
                  <td>{net}</td>
                  <td>
                    <StatusBadge tone={state === "Stable" ? "success" : "warn"}>
                      {state}
                    </StatusBadge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Resource Distribution</p>
          <Bars
            rows={[
              { label: "Top 1%", value: 46 },
              { label: "Guild Banks", value: 22 },
              { label: "Market", value: 19 },
              { label: "Dormant", value: 13 }
            ]}
          />
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <p className="eyebrow">Price Movement</p>
        <div className="grid three">
          {["Dragon Scale", "Blessed Oil", "Wooma Horn"].map((item, index) => (
            <div className="side-card" key={item} style={{ marginTop: 0 }}>
              <h3>{item}</h3>
              <p className="metric-value">{[12800, 8420, 22100][index].toLocaleString()}</p>
              <p className={index === 1 ? "delta negative" : "delta"}>{index === 1 ? "+18.4% abnormal" : "+3.2% normal"}</p>
            </div>
          ))}
        </div>
      </section>
    </AdminShell>
  );
}
