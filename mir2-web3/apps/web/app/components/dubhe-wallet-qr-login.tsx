"use client";

import { useEffect, useId, useRef, useState } from "react";

import type { DubheQrLoginState } from "../../lib/dubhe-wallet-login";
export type { DubheQrLoginState } from "../../lib/dubhe-wallet-login";

type DubheWalletQrLoginProps = {
  language: string;
  state: DubheQrLoginState;
  onRetry: () => void;
  onCancel: () => void;
};

export function DubheWalletQrLogin({ language, state, onRetry, onCancel }: DubheWalletQrLoginProps) {
  const zh = /^zh/iu.test(language);
  const titleId = useId();
  const descriptionId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const [now, setNow] = useState(Date.now);
  const [qrImage, setQrImage] = useState<{ url: string; dataUrl: string } | null>(null);
  const [qrErrorUrl, setQrErrorUrl] = useState<string | null>(null);
  const approvalUrl = validApprovalUrl(state.approvalUrl);
  const secondsLeft = typeof state.expiresAt === "number" && Number.isFinite(state.expiresAt)
    ? Math.max(0, Math.ceil((state.expiresAt - now) / 1_000))
    : null;
  const status = state.status === "pending" && secondsLeft === 0 ? "expired"
    : state.status === "pending" && !approvalUrl ? "error" : state.status;
  const canApprove = status === "pending" && approvalUrl !== null;
  const qrFailed = canApprove && qrErrorUrl === approvalUrl;
  const canRetry = status === "expired" || status === "rejected" || status === "error" || qrFailed;
  const imageUrl = canApprove && qrImage?.url === approvalUrl ? qrImage.dataUrl : null;

  useEffect(() => {
    if (state.status !== "pending" || !state.expiresAt) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [state.status, state.expiresAt]);

  useEffect(() => {
    if (!canApprove || !approvalUrl) return;
    let current = true;
    void import("qrcode")
      .then(({ default: QRCode }) => QRCode.toDataURL(approvalUrl, {
        margin: 4,
        errorCorrectionLevel: "M",
        width: 240,
      }))
      .then((dataUrl) => {
        if (current) {
          setQrImage({ url: approvalUrl, dataUrl });
          setQrErrorUrl(null);
        }
      })
      .catch(() => {
        if (current) setQrErrorUrl(approvalUrl);
      });
    return () => { current = false; };
  }, [approvalUrl, canApprove]);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const panel = panelRef.current;
    const containFocus = (event: FocusEvent) => {
      if (event.target instanceof Node && !panel?.contains(event.target)) {
        cancelRef.current?.focus({ preventScroll: true });
      }
    };
    document.addEventListener("focusin", containFocus);
    cancelRef.current?.focus({ preventScroll: true });
    return () => {
      document.removeEventListener("focusin", containFocus);
      const restoreFocus = document.activeElement === document.body || Boolean(panel?.contains(document.activeElement));
      if (!restoreFocus) return;
      queueMicrotask(() => {
        if (document.querySelector(".dubhe-wallet-qr-panel")) return;
        const target = previousFocus?.isConnected && !previousFocus.matches(":disabled")
          ? previousFocus
          : document.querySelector<HTMLElement>("[data-dubhe-qr-return]:not(:disabled)");
        target?.focus({ preventScroll: true });
      });
    };
  }, []);

  const statusText = {
    creating: zh ? "正在创建登录二维码…" : "Creating your login code…",
    pending: zh ? "等待钱包确认登录" : "Waiting for wallet approval",
    approved: zh ? "钱包已确认，正在登录游戏…" : "Wallet approved. Signing in…",
    rejected: zh ? "已拒绝本次登录，可以重新扫码。" : "Login declined. You can try again.",
    expired: zh ? "二维码已过期，请刷新后重新扫码。" : "This code has expired. Refresh to try again.",
    error: zh ? "暂时无法完成扫码登录，请重试。" : "QR login could not be completed. Please try again.",
  }[status];

  return (
    <div className="dubhe-wallet-qr-backdrop">
      <div
        className="dubhe-wallet-qr-panel"
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onCancel();
          } else if (event.key === "Tab") {
            const targets = panelRef.current?.querySelectorAll<HTMLElement>("button:not(:disabled), a[href]");
            const first = targets?.[0];
            const last = targets?.[targets.length - 1];
            if (event.shiftKey && document.activeElement === first) {
              event.preventDefault();
              last?.focus();
            } else if (!event.shiftKey && document.activeElement === last) {
              event.preventDefault();
              first?.focus();
            }
          }
        }}
      >
        <div className="dubhe-wallet-qr-heading">
          <span className="login-wallet-fallback-icon" aria-hidden="true">D</span>
          <h2 id={titleId}>{zh ? "Dubhe Wallet 扫码登录" : "Dubhe Wallet QR login"}</h2>
        </div>
        <p className="dubhe-wallet-qr-description" id={descriptionId}>
          {zh ? "用手机扫描二维码，在打开的页面中连接 Dubhe Wallet 并签名确认登录。" : "Scan with your phone, then connect Dubhe Wallet and sign to approve your login."}
        </p>
        <div className={`dubhe-wallet-qr-code${imageUrl ? " ready" : ""}`} aria-busy={canApprove && !imageUrl && !qrFailed}>
          {imageUrl ? (
            <img src={imageUrl} width={240} height={240} alt={zh ? "用 Dubhe Wallet 确认登录的二维码" : "QR code for Dubhe Wallet login approval"} />
          ) : (
            <span>{qrFailed ? (zh ? "二维码生成失败" : "Code generation failed") : statusText}</span>
          )}
        </div>
        <p className="dubhe-wallet-qr-status" role="status" aria-live="polite" aria-atomic="true">{statusText}</p>
        {canApprove && secondsLeft !== null ? (
          <p className="dubhe-wallet-qr-expiry" aria-live="off">{zh ? `${secondsLeft} 秒后过期` : `Expires in ${secondsLeft}s`}</p>
        ) : null}
        {qrFailed ? <p className="dubhe-wallet-qr-error" role="alert">{zh ? "请重试，或点击下方链接在当前设备继续。" : "Please retry, or use the link below on this device."}</p> : null}
        {canApprove ? (
          <a className="dubhe-wallet-qr-device-link" href={approvalUrl!} target="_blank" rel="noopener noreferrer">
            {zh ? "在当前设备打开钱包确认" : "Approve on this device"}
          </a>
        ) : null}
        <div className="dubhe-wallet-qr-actions">
          {canRetry ? <button type="button" onClick={onRetry}>{zh ? "刷新二维码" : "Refresh code"}</button> : null}
          <button ref={cancelRef} type="button" onClick={onCancel}>{zh ? "取消" : "Cancel"}</button>
        </div>
      </div>
    </div>
  );
}

function validApprovalUrl(value?: string) {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : null;
  } catch {
    return null;
  }
}
