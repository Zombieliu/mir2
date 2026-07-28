import Link from "next/link";
import { LanguageSwitcher } from "../../components/language-switcher";
import { getAdminI18n } from "../../lib/i18n";
import { loginAction } from "./actions";

export default async function LoginPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { locale, t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const returnTo = safeReturnTo(firstParam(params.next));

  return (
    <main className="login-screen">
      <section className="login-panel">
        <div className="login-head">
          <div className="sigil">M</div>
          <div>
            <p className="eyebrow">{t("login.eyebrow")}</p>
            <h1>{t("login.title")}</h1>
          </div>
        </div>
        {error ? <p className="notice">{t("login.tokenRequired")}</p> : null}
        <form action={loginAction} className="form-stack">
          <input name="returnTo" type="hidden" value={returnTo} />
          <label className="field">
            <span>{t("login.token")}</span>
            <input
              autoComplete="current-password"
              className="control"
              name="token"
              required
              type="password"
            />
          </label>
          <button className="button" type="submit">
            {t("login.submit")}
          </button>
        </form>
        <div className="login-foot">
          <LanguageSwitcher locale={locale} label={t("shell.language")} />
          <Link className="muted" href="/">
            {t("login.return")}
          </Link>
        </div>
      </section>
    </main>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}

function safeReturnTo(value: string) {
  return value.startsWith("/") && !value.startsWith("//") ? value : "/dubhe-nodes";
}
