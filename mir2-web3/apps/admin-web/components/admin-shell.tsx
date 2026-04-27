import Link from "next/link";
import { getAdminI18n } from "../lib/i18n";
import { LanguageSwitcher } from "./language-switcher";

const navItems = [
  { href: "/", labelKey: "shell.nav.dashboard", count: "Live" },
  { href: "/players", labelKey: "shell.nav.players", count: "12k" },
  { href: "/players/AZ-1048", labelKey: "shell.nav.playerDetail", count: "GM" },
  { href: "/economy", labelKey: "shell.nav.economy", count: "Risk" },
  { href: "/activities", labelKey: "shell.nav.activities", count: "7" },
  { href: "/servers", labelKey: "shell.nav.servers", count: "31" },
  { href: "/risk", labelKey: "shell.nav.risk", count: "42" },
  { href: "/gm-tools", labelKey: "shell.nav.gmTools", count: "API" },
  { href: "/approvals", labelKey: "shell.nav.approvals", count: "Gate" },
  { href: "/timeline", labelKey: "shell.nav.timeline", count: "Read" },
  { href: "/audit", labelKey: "shell.nav.audit", count: "RBAC" }
];

export async function AdminShell({
  children,
  active
}: {
  children: React.ReactNode;
  active: string;
}) {
  const { locale, t } = await getAdminI18n();

  return (
    <div className="admin-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="sigil">M</div>
          <div>
            <h1>{t("shell.brandTitle")}</h1>
            <p>{t("shell.brandSubtitle")}</p>
          </div>
        </div>
        <nav className="nav">
          {navItems.map((item) => (
            <Link
              className={item.href === active ? "active" : ""}
              href={item.href}
              key={item.href}
            >
              <span>{t(item.labelKey)}</span>
              <span className="muted">{item.count}</span>
            </Link>
          ))}
        </nav>
        <div className="side-card">
          <p className="eyebrow">{t("shell.safetyEyebrow")}</p>
          <h3>{t("shell.safetyTitle")}</h3>
          <p className="muted">{t("shell.safetyBody")}</p>
        </div>
      </aside>
      <main className="main">
        <div className="topbar">
          <input
            className="control"
            placeholder={t("shell.searchPlaceholder")}
          />
          <select className="control" defaultValue="global">
            <option value="global">{t("shell.realmGlobal")}</option>
            <option value="s1">{t("shell.realmS1")}</option>
            <option value="s2">{t("shell.realmS2")}</option>
          </select>
          <LanguageSwitcher locale={locale} label={t("shell.language")} />
          <div className="user-chip">{t("shell.operator")}</div>
        </div>
        {children}
      </main>
    </div>
  );
}
