"use client";
import { useEffect, useState } from "react";
import HomePage from "../../page";
import { promotionRequest, type PromotionBootstrap } from "../promotion-model";
import { PROMOTION_COPY, normalizePromotionLocale, type PromotionLocale } from "../promotion-copy";
import { PROMOTION_BILLING_LABELS } from "../promotion-billing";

export function PromotionGame() {
  const [status, setStatus] = useState<"loading" | "ready" | "login" | "error">("loading");
  const [locale, setLocale] = useState<PromotionLocale>("en");
  useEffect(() => {
    try { setLocale(normalizePromotionLocale(localStorage.getItem("numeron-language") || navigator.language)); } catch { setLocale(normalizePromotionLocale(navigator.language)); }
    const controller = new AbortController();
    void promotionRequest<PromotionBootstrap>("bootstrap", { signal: controller.signal }).then(value => {
      if (!controller.signal.aborted) setStatus(value.authenticated && value.binding?.status === "linked" ? "ready" : "login");
    }).catch(() => { if (!controller.signal.aborted) setStatus("error"); });
    return () => controller.abort();
  }, []);
  const copy = PROMOTION_COPY[locale];
  return <>
    <aside style={{ position: "fixed", top: 8, left: 8, zIndex: 10000, maxWidth: "90vw", padding: "8px 12px", background: "#10150fed", color: "#e5d4a7", border: "1px solid #776841", borderRadius: 4, font: "13px system-ui" }}>
      <a href="/promotions#billing" style={{ color: "inherit" }}>← {copy.program}</a> · {PROMOTION_BILLING_LABELS[locale][8]} · Sui Testnet
    </aside>
    {status === "ready" ? <HomePage /> : <main style={{ padding: 64, color: "#e5d4a7", background: "#11140f", minHeight: "100vh" }}>
      <h1>{PROMOTION_BILLING_LABELS[locale][8]}</h1><p>{copy.testnetBody}</p>
      <p role="status">{status === "loading" ? copy.loading : status === "login" ? copy.notAuthenticated : copy.unavailable}</p>
      <a href="/promotions" style={{ color: "inherit" }}>{copy.login} ↗</a>
    </main>}
  </>;
}
