"use client";
import { useEffect, useState } from "react";
import HomePage from "../../page";
import { promotionRequest, type PromotionBootstrap } from "../promotion-model";
import { PROMOTION_COPY, normalizePromotionLocale, type PromotionLocale } from "../promotion-copy";
import { PROMOTION_BILLING_LABELS } from "../promotion-billing";
import { PROMOTION_WINDOWS_COPY, windowsDownloadPath } from "../promotion-windows-copy";

export function PromotionGame() {
  const [status, setStatus] = useState<"loading" | "ready" | "login" | "error" | "windows">("loading");
  const [locale, setLocale] = useState<PromotionLocale>("en");
  const [promotionLink, setPromotionLink] = useState("/promotions");
  useEffect(() => {
    const requestedLanguage = new URLSearchParams(location.search).get("lang");
    try { setLocale(normalizePromotionLocale(requestedLanguage || localStorage.getItem("numeron-language") || navigator.language)); } catch { setLocale(normalizePromotionLocale(requestedLanguage || navigator.language)); }
    const controller = new AbortController();
    void promotionRequest<PromotionBootstrap>("bootstrap", { signal: controller.signal }).then(value => {
      if (!controller.signal.aborted) {
        setStatus(value.gameClient === "windows" ? "windows" : value.authenticated && value.binding?.status === "linked" ? "ready" : "login");
        if (value.campaign?.campaignId) setPromotionLink(`/promotions?campaignId=${encodeURIComponent(value.campaign.campaignId)}`);
      }
    }).catch(() => { if (!controller.signal.aborted) setStatus("error"); });
    return () => controller.abort();
  }, []);
  const copy = PROMOTION_COPY[locale];
  const windowsCopy = PROMOTION_WINDOWS_COPY[locale];
  if (status === "windows") return <main lang={locale} dir={locale === "ar" ? "rtl" : "ltr"} style={{ padding: "48px 24px", color: "#e5d4a7", background: "#11140f", minHeight: "100vh" }}>
    <h1>{windowsCopy.title}</h1><p style={{ maxWidth: 720 }}>{windowsCopy.body}</p>
    <p><a href={windowsDownloadPath(locale)} style={{ color: "inherit" }}>{windowsCopy.download} ↗</a></p>
    <a href={`${promotionLink}#billing`} style={{ color: "inherit" }}>← {copy.program}</a>
  </main>;
  return <>
    <aside style={{ position: "fixed", top: 8, left: 8, zIndex: 10000, maxWidth: "90vw", padding: "8px 12px", background: "#10150fed", color: "#e5d4a7", border: "1px solid #776841", borderRadius: 4, font: "13px system-ui" }}>
      <a href={`${promotionLink}#billing`} style={{ color: "inherit" }}>← {copy.program}</a> · {PROMOTION_BILLING_LABELS[locale][8]} · Sui Testnet
    </aside>
    {status === "ready" ? <HomePage /> : <main style={{ padding: 64, color: "#e5d4a7", background: "#11140f", minHeight: "100vh" }}>
      <h1>{PROMOTION_BILLING_LABELS[locale][8]}</h1><p>{copy.testnetBody}</p>
      <p role="status">{status === "loading" ? copy.loading : status === "login" ? copy.notAuthenticated : copy.unavailable}</p>
      <a href={promotionLink} style={{ color: "inherit" }}>{copy.login} ↗</a>
    </main>}
  </>;
}
