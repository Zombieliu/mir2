"use client";

import { useTransition } from "react";
import type { AdminLocale } from "../lib/i18n";

export function LanguageSwitcher({
  locale,
  label
}: {
  locale: AdminLocale;
  label: string;
}) {
  const [isPending, startTransition] = useTransition();

  function changeLocale(nextLocale: AdminLocale) {
    document.cookie = `admin_locale=${nextLocale}; path=/; max-age=31536000; samesite=lax`;
    startTransition(() => {
      window.location.reload();
    });
  }

  return (
    <label className="language-switcher">
      <span className="muted">{label}</span>
      <select
        className="control"
        defaultValue={locale}
        disabled={isPending}
        onChange={(event) => changeLocale(event.target.value as AdminLocale)}
      >
        <option value="en">English</option>
        <option value="zh-CN">中文</option>
      </select>
    </label>
  );
}
