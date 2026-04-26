import { AdminShell } from "../../../components/admin-shell";
import { Bars } from "../../../components/bars";
import { StatusBadge } from "../../../components/status-badge";

export default async function PlayerDetailPage({
  params
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <AdminShell active="/players/AZ-1048">
      <div className="page-head">
        <div>
          <p className="eyebrow">Player Detail</p>
          <h2>MoonBlade</h2>
          <p className="muted">{id} / Warrior / S1 Dragon Gate / last seen 4 minutes ago</p>
        </div>
        <div className="actions">
          <StatusBadge tone="success">Normal</StatusBadge>
          <button className="button secondary">Mute</button>
          <button className="button">Send Mail</button>
          <button className="button secondary">Ban Review</button>
        </div>
      </div>

      <div className="grid three">
        <section className="card">
          <p className="eyebrow">Character</p>
          <h3>Combat profile</h3>
          <Bars
            rows={[
              { label: "HP", value: 86 },
              { label: "MP", value: 52 },
              { label: "Attack", value: 74 },
              { label: "Defense", value: 68 }
            ]}
          />
        </section>
        <section className="card">
          <p className="eyebrow">Inventory</p>
          <h3>Equipment and bags</h3>
          <p className="muted">Weapon: Dragon Slayer Replica</p>
          <p className="muted">Warehouse: 118 / 160 slots used</p>
          <p className="muted">Quest items: 7 active bindings</p>
        </section>
        <section className="card">
          <p className="eyebrow">Risk Signals</p>
          <h3>Device and IP</h3>
          <p className="muted">2 devices, 1 ASN, 0 emulator flags</p>
          <p className="muted">Trade graph score: 18 / 100</p>
          <StatusBadge tone="success">Low risk</StatusBadge>
        </section>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">Records</p>
          <table className="table">
            <tbody>
              {[
                ["Recharge", "$2,184 lifetime / last $49.99"],
                ["Trade", "83 player trades / 0 chargeback risk"],
                ["Chat", "2 warning keywords in last 7 days"],
                ["Quest", "Mainline chapter 4 / Wooma temple"],
                ["GM Ops", "1 compensation mail, 0 disciplinary actions"]
              ].map(([label, value]) => (
                <tr key={label}>
                  <th>{label}</th>
                  <td>{value}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Action Drawer</p>
          <h3>High-risk operation requirements</h3>
          <p className="muted">Every write requires permission, reason, impact preview, command ID, and audit trace.</p>
          <div className="actions">
            <StatusBadge tone="warn">Second confirm for ban</StatusBadge>
            <StatusBadge>Rollback note required</StatusBadge>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
