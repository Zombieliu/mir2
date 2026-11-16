"use client";

import { useEffect, useState } from "react";

type StatusResponse = {
  ok: boolean;
  checkedAt: string;
  latencyMs?: number;
  source?: string;
};

export function GameStatusPanel({ labels }: { labels: { checking: string; online: string; degraded: string; checked: string } }) {
  const [status, setStatus] = useState<StatusResponse | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let active = true;
    void fetch("/api/game-status", { cache: "no-store", signal: controller.signal })
      .then(async (response) => response.json() as Promise<StatusResponse>)
      .then((result) => { if (active) setStatus(result); })
      .catch(() => { if (active) setStatus({ ok: false, checkedAt: new Date().toISOString() }); });
    return () => {
      active = false;
      controller.abort();
    };
  }, []);

  return (
    <div className={`live-health${status?.ok ? " is-online" : status ? " is-degraded" : ""}`} aria-live="polite">
      <span><i />{status ? status.ok ? labels.online : labels.degraded : labels.checking}</span>
      <small>{status ? `${labels.checked} ${new Date(status.checkedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}${status.latencyMs ? ` · ${status.latencyMs}ms` : ""}` : "WORLD 01"}</small>
    </div>
  );
}
