"use client";

import { useEffect } from "react";

const STORAGE_KEY = "numeron.attribution.v1";
const CAMPAIGN_KEYS = ["utm_source", "utm_medium", "utm_campaign", "utm_content", "utm_term", "ref"] as const;

type Attribution = {
  version: 1;
  firstTouchAt: string;
  lastTouchAt: string;
  landingPath: string;
  referrer: string;
  campaign: Partial<Record<(typeof CAMPAIGN_KEYS)[number], string>>;
};

function readStoredAttribution(): Attribution | null {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    return value ? JSON.parse(value) as Attribution : null;
  } catch {
    return null;
  }
}

export function AttributionCapture() {
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const campaign = Object.fromEntries(
      CAMPAIGN_KEYS.flatMap((key) => {
        const value = params.get(key)?.trim();
        return value ? [[key, value.slice(0, 160)]] : [];
      }),
    ) as Attribution["campaign"];
    const now = new Date().toISOString();
    const previous = readStoredAttribution();
    const next: Attribution = {
      version: 1,
      firstTouchAt: previous?.firstTouchAt ?? now,
      lastTouchAt: now,
      landingPath: previous?.landingPath ?? `${window.location.pathname}${window.location.search}`,
      referrer: previous?.referrer ?? document.referrer.slice(0, 400),
      campaign: Object.keys(campaign).length > 0 ? campaign : previous?.campaign ?? {},
    };

    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Attribution must never block navigation when storage is unavailable.
    }

    const trackClick = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-track]") : null;
      const eventName = target?.dataset.track;
      if (!eventName) return;
      const payload = JSON.stringify({
        event: eventName,
        path: window.location.pathname,
        href: target instanceof HTMLAnchorElement ? target.href : undefined,
        attribution: readStoredAttribution(),
        occurredAt: new Date().toISOString(),
      });
      if (navigator.sendBeacon) {
        navigator.sendBeacon("/api/attribution", new Blob([payload], { type: "application/json" }));
      } else {
        void fetch("/api/attribution", { method: "POST", headers: { "content-type": "application/json" }, body: payload, keepalive: true });
      }
    };

    document.addEventListener("click", trackClick, { passive: true });
    return () => document.removeEventListener("click", trackClick);
  }, []);

  return null;
}
