import Link from "next/link";

const navItems = [
  { href: "/", label: "Dashboard", count: "Live" },
  { href: "/players", label: "Players", count: "12k" },
  { href: "/players/AZ-1048", label: "Player Detail", count: "GM" },
  { href: "/economy", label: "Economy", count: "Risk" },
  { href: "/activities", label: "Activities", count: "7" },
  { href: "/servers", label: "World Monitor", count: "31" },
  { href: "/risk", label: "Anti-Cheat", count: "42" },
  { href: "/gm-tools", label: "Mail & GM Tools", count: "API" },
  { href: "/audit", label: "Audit Log", count: "RBAC" }
];

export function AdminShell({
  children,
  active
}: {
  children: React.ReactNode;
  active: string;
}) {
  return (
    <div className="admin-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="sigil">M</div>
          <div>
            <h1>Aether Ops</h1>
            <p>Mir2 live operations control plane</p>
          </div>
        </div>
        <nav className="nav">
          {navItems.map((item) => (
            <Link
              className={item.href === active ? "active" : ""}
              href={item.href}
              key={item.href}
            >
              <span>{item.label}</span>
              <span className="muted">{item.count}</span>
            </Link>
          ))}
        </nav>
        <div className="side-card">
          <p className="eyebrow">Safety Mode</p>
          <h3>Command first, audit always</h3>
          <p className="muted">
            Dangerous actions require a reason and go through the Rust Admin API.
          </p>
        </div>
      </aside>
      <main className="main">
        <div className="topbar">
          <input
            className="control"
            placeholder="Search player ID, nickname, account, item, command..."
          />
          <select className="control" defaultValue="global">
            <option value="global">Global realm</option>
            <option value="s1">S1 Dragon Gate</option>
            <option value="s2">S2 Jade Ruins</option>
          </select>
          <div className="user-chip">GM Operator</div>
        </div>
        {children}
      </main>
    </div>
  );
}
