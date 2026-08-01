import Link from "next/link";
import { logoutAction } from "../app/login/actions";
import { adminGet, type AdminAuthMeResponse } from "../lib/admin-api";
import { getAdminI18n } from "../lib/i18n";
import { LanguageSwitcher } from "./language-switcher";
import { SubmitButton } from "./submit-button";

const navItems = [
  { href: "/", labelKey: "shell.nav.dashboard", count: "Live" },
  { href: "/console", labelKey: "shell.nav.console", count: "Ops" },
  { href: "/network", labelKey: "shell.nav.network", count: "Live" },
  { href: "/accounts", labelKey: "shell.nav.accounts", count: "GM" },
  { href: "/identity-security", labelKey: "shell.nav.identitySecurity", count: "IAM" },
  { href: "/players", labelKey: "shell.nav.players", count: "Read" },
  { href: "/service-trace", labelKey: "shell.nav.serviceTrace", count: "Trace" },
  { href: "/economy", labelKey: "shell.nav.economy", count: "Read" },
  { href: "/market", labelKey: "shell.nav.market", count: "Mod" },
  { href: "/guilds", labelKey: "shell.nav.guilds", count: "Mod" },
  { href: "/namelists", labelKey: "shell.nav.namelists", count: "NPC" },
  { href: "/content", labelKey: "shell.nav.content", count: "DB" },
  { href: "/activities", labelKey: "shell.nav.activities", count: "Empty" },
  { href: "/world-director", labelKey: "shell.nav.worldDirector", count: "AI" },
  { href: "/daily-reports", labelKey: "shell.nav.dailyReports", count: "AI" },
  { href: "/servers", labelKey: "shell.nav.servers", count: "Health" },
  { href: "/dubhe-nodes", labelKey: "shell.nav.dubheNodes", count: "Nodes" },
  { href: "/risk", labelKey: "shell.nav.risk", count: "Bans" },
  { href: "/gm-tools", labelKey: "shell.nav.gmTools", count: "API" },
  { href: "/approvals", labelKey: "shell.nav.approvals", count: "Gate" },
  { href: "/operators", labelKey: "shell.nav.operators", count: "RBAC" },
  { href: "/timeline", labelKey: "shell.nav.timeline", count: "Read" },
  { href: "/audit", labelKey: "shell.nav.audit", count: "Audit" }
];

export async function AdminShell({
  children,
  active
}: {
  children: React.ReactNode;
  active: string;
}) {
  const { locale, t } = await getAdminI18n();
  const auth = await adminGet<AdminAuthMeResponse>("/admin/auth/me");
  const operator = auth.ok ? auth.data.operator : undefined;

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
          <LanguageSwitcher locale={locale} label={t("shell.language")} />
          <div className="topbar-actions">
            <div className="user-chip">
              {operator ? `${operator.operatorId} · ${operator.role}` : t("shell.operator")}
            </div>
            <Link className="user-chip" href="/login">
              {auth.ok ? t("shell.switchOperator") : t("shell.login")}
            </Link>
            {auth.ok ? (
              <form action={logoutAction}>
                <SubmitButton
                  className="user-chip user-chip-button"
                  idle={t("shell.logout")}
                  pending={t("common.working")}
                />
              </form>
            ) : null}
          </div>
        </div>
        {children}
      </main>
    </div>
  );
}
