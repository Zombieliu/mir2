"use client";

import { useEffect, useId, useRef, useState } from "react";
import type { PromotionCopy } from "./promotion-copy";
import { validatePromotionApprovalUrl } from "./promotion-model";
import styles from "./promotion.module.css";

type LoginState = "creating" | "pending" | "approved" | "rejected" | "expired" | "error";

async function authRequest(path: string, signal: AbortSignal): Promise<Record<string, unknown>> {
  const response = await fetch(`/api/promotions/auth/${path}`, {
    method: "POST", credentials: "same-origin", cache: "no-store",
    signal: AbortSignal.any([signal, AbortSignal.timeout(12_000)]),
  });
  const body: unknown = await response.json();
  if (!response.ok || !body || typeof body !== "object" || Array.isArray(body)) throw new Error("wallet_login_unavailable");
  return body as Record<string, unknown>;
}

function delay(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted();
    const abort = () => { clearTimeout(timer); reject(new DOMException("Cancelled", "AbortError")); };
    const timer = setTimeout(() => { signal.removeEventListener("abort", abort); resolve(); }, ms);
    signal.addEventListener("abort", abort, { once: true });
  });
}

/** The relay intent and game session remain in encrypted HttpOnly cookies managed by the BFF. */
export function PromotionWalletLogin({ copy, onClose, onAuthenticated }: {
  copy: PromotionCopy; onClose: () => void; onAuthenticated: () => void;
}) {
  const [state, setState] = useState<LoginState>("creating");
  const [url, setUrl] = useState<string | null>(null);
  const [image, setImage] = useState<string | null>(null);
  const [expiresAt, setExpiresAt] = useState<number | null>(null);
  const [now, setNow] = useState(Date.now);
  const [attempt, setAttempt] = useState(0);
  const panel = useRef<HTMLDivElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const authenticatedRef = useRef(onAuthenticated);
  authenticatedRef.current = onAuthenticated;
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    const controller = new AbortController();
    let active = true;
    setState("creating"); setUrl(null); setImage(null); setExpiresAt(null);
    void (async () => {
      try {
        const created = await authRequest("challenge", controller.signal);
        if (!active) return;
        const approvedUrl = validatePromotionApprovalUrl(created.approvalUrl);
        const deadline = created.expiresAt;
        if (!approvedUrl || typeof deadline !== "number" || !Number.isFinite(deadline) ||
          deadline <= Date.now() || deadline > Date.now() + 300_000) throw new Error("wallet_challenge_invalid");
        setUrl(approvedUrl); setExpiresAt(deadline); setState("pending");
        const { default: QRCode } = await import("qrcode");
        const qr = await QRCode.toDataURL(approvedUrl, { width: 280, margin: 3, errorCorrectionLevel: "M" });
        if (!active) return;
        setImage(qr);
        while (active && Date.now() < deadline) {
          await delay(Math.min(1500, Math.max(1, deadline - Date.now())), controller.signal);
          if (document.visibilityState !== "visible") continue;
          const result = await authRequest("poll", controller.signal);
          if (!active) return;
          if (result.status === "approved") {
            setState("approved"); authenticatedRef.current(); return;
          }
          if (result.status === "rejected" || result.status === "expired") {
            setState(result.status); return;
          }
          if (result.status !== "pending") throw new Error("wallet_poll_invalid");
        }
        if (active) setState("expired");
      } catch {
        if (active && !controller.signal.aborted) setState("error");
      }
    })();
    return () => { active = false; controller.abort(); };
  }, [attempt]);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const trapFocus = (event: FocusEvent) => {
      if (event.target instanceof Node && !panel.current?.contains(event.target)) cancelButton.current?.focus();
    };
    document.addEventListener("focusin", trapFocus);
    cancelButton.current?.focus();
    return () => {
      document.removeEventListener("focusin", trapFocus);
      if (previous?.isConnected) previous.focus({ preventScroll: true });
    };
  }, []);

  const expired = expiresAt !== null && now >= expiresAt;
  const effectiveState = state === "pending" && expired ? "expired" : state;
  const canApprove = effectiveState === "pending" && url !== null;
  const status = {
    creating: copy.loading, pending: copy.walletPending, approved: copy.walletApproved,
    rejected: copy.walletRejected, expired: copy.walletExpired, error: copy.walletError,
  }[effectiveState];

  return <div className={styles.backdrop}>
    <div ref={panel} className={styles.modal} role="dialog" aria-modal="true" aria-labelledby={titleId}
      aria-describedby={descriptionId} onKeyDown={event => {
        if (event.key === "Escape") { event.preventDefault(); onClose(); }
        if (event.key !== "Tab") return;
        const controls = panel.current?.querySelectorAll<HTMLElement>("a[href],button:not(:disabled)");
        const first = controls?.[0], last = controls?.[controls.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
      }}>
      <h2 id={titleId}>{copy.walletTitle}</h2>
      <p className={styles.muted} id={descriptionId}>{copy.walletBody}</p>
      <div className={styles.qr} aria-busy={effectiveState === "creating" || (canApprove && !image)}>
        {canApprove && image ? <img src={image} width={280} height={280} alt={copy.qrLabel} /> :
          <span>{effectiveState === "creating" ? <span className={styles.loadingMark} aria-label={copy.loading} /> : status}</span>}
      </div>
      <p className={styles.modalStatus} role="status" aria-live="polite">{status}</p>
      {canApprove && expiresAt !== null ? <p className={styles.modalExpiry}>{copy.expires} {Math.max(0, Math.ceil((expiresAt - now) / 1000))} s</p> : null}
      <div className={styles.modalActions}>
        {canApprove ? <a className={styles.primary} href={url!} target="_blank" rel="noopener noreferrer">{copy.walletSameDevice} ↗</a> : null}
        {["error", "expired", "rejected"].includes(effectiveState) ? <button className={styles.primary} onClick={() => setAttempt(value => value + 1)}>{copy.retry}</button> : null}
        <button className={styles.secondary} ref={cancelButton} onClick={onClose}>{copy.cancel}</button>
      </div>
    </div>
  </div>;
}
