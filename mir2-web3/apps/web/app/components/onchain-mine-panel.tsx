"use client";

/**
 * On-chain smart-mine HUD (M4, WF-6) — the settlement/wallet control panel for the
 * testnet vertical slice (DESIGN §4). Mining itself is the in-world gesture (walk to the
 * vein and attack it); this panel surfaces batch/nonce/ore state and the chain-only
 * actions (connect wallet, settle, redeem). The Swing button here is just a dev shortcut
 * for the same swing. Presentation-only, like the other window components: every value
 * arrives via props and every action goes back through a callback; the mining state
 * machine itself lives in `lib/onchain-mine-state.ts` + `page.tsx`.
 *
 * Mounted only when `NEXT_PUBLIC_ONCHAIN_MINE=1` — production builds without the flag
 * never render (or even reference) it.
 */

import type { CSSProperties } from "react";

export type OnchainMinePanelReconcile = {
  deltaUnits: number;
  phantom: boolean;
  shortfall: boolean;
};

export type OnchainMinePanelProps = {
  /** Signing wallet address (null until a wallet is connected for signing). */
  walletAddress: string | null;
  walletBusy: boolean;
  /** Swings accumulated toward the next batch + the flush threshold. */
  pendingSwings: number;
  batchSize: number;
  /** Optimistic (display-only) ore units: pending + in-flight. */
  optimisticUnits: number;
  /** In-flight batch info (null/0 when idle). */
  inFlightSwings: number;
  inFlightDigest: string | null;
  /** Chain-confirmed totals this session. */
  confirmedUnits: number;
  settledBatches: number;
  lastReconcile: OnchainMinePanelReconcile | null;
  lastError: string | null;
  /** The nonce the NEXT batch will use (editable — M4 dev tool for replay recovery). */
  nextNonce: number;
  /** Vein render state from the server (`world.mineNodes`), null when not yet known. */
  veinStage: number | null;
  veinLocation: { x: number; y: number };
  submitBusy: boolean;
  redeemAmount: string;
  onRedeemAmountChange: (value: string) => void;
  onConnectWallet: () => void;
  /** Dev shortcut for one swing — the real gesture is attacking the vein in the world. */
  onSwing: () => void;
  /** Submit the pending swings now, even below the batch threshold. */
  onFlushNow: () => void;
  onRedeem: () => void;
  onNonceChange: (nextNonce: number) => void;
};

const panelStyle: CSSProperties = {
  position: "fixed",
  right: 12,
  bottom: 96,
  zIndex: 60,
  width: 252,
  padding: "10px 12px",
  borderRadius: 8,
  background: "rgba(12, 16, 24, 0.92)",
  border: "1px solid rgba(120, 150, 200, 0.35)",
  color: "#dce6f5",
  font: "12px/1.5 var(--font-geist-mono, monospace)",
};

const rowStyle: CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  gap: 8,
};

const buttonStyle: CSSProperties = {
  flex: 1,
  padding: "4px 6px",
  borderRadius: 4,
  border: "1px solid rgba(140, 170, 220, 0.5)",
  background: "rgba(40, 60, 90, 0.8)",
  color: "#e8f0ff",
  cursor: "pointer",
  font: "inherit",
};

const disabledButtonStyle: CSSProperties = {
  ...buttonStyle,
  opacity: 0.45,
  cursor: "default",
};

const inputStyle: CSSProperties = {
  width: 64,
  padding: "2px 4px",
  borderRadius: 4,
  border: "1px solid rgba(140, 170, 220, 0.5)",
  background: "rgba(20, 28, 40, 0.9)",
  color: "#e8f0ff",
  font: "inherit",
};

function shortAddress(address: string): string {
  return address.length > 14 ? `${address.slice(0, 8)}…${address.slice(-4)}` : address;
}

function veinStageLabel(stage: number | null): string {
  if (stage === null) return "—";
  if (stage >= 2) return "full vein 满";
  if (stage === 1) return "cracked 裂";
  return "depleted 空";
}

export function OnchainMinePanel({
  walletAddress,
  walletBusy,
  pendingSwings,
  batchSize,
  optimisticUnits,
  inFlightSwings,
  inFlightDigest,
  confirmedUnits,
  settledBatches,
  lastReconcile,
  lastError,
  nextNonce,
  veinStage,
  veinLocation,
  submitBusy,
  redeemAmount,
  onRedeemAmountChange,
  onConnectWallet,
  onSwing,
  onFlushNow,
  onRedeem,
  onNonceChange,
}: OnchainMinePanelProps) {
  const canAct = walletAddress !== null && !submitBusy;
  const inFlight = inFlightSwings > 0;
  return (
    <div style={panelStyle} data-testid="onchain-mine-panel">
      <div style={{ ...rowStyle, marginBottom: 6 }}>
        <strong>On-chain Mine (testnet)</strong>
        <span>
          ({veinLocation.x},{veinLocation.y}) {veinStageLabel(veinStage)}
        </span>
      </div>

      {walletAddress === null ? (
        <button
          type="button"
          style={walletBusy ? disabledButtonStyle : buttonStyle}
          disabled={walletBusy}
          onClick={onConnectWallet}
        >
          {walletBusy ? "连接中…" : "连接 Sui 钱包 / Connect wallet"}
        </button>
      ) : (
        <div style={rowStyle}>
          <span>签名钱包</span>
          <span title={walletAddress}>{shortAddress(walletAddress)}</span>
        </div>
      )}

      <div style={rowStyle}>
        <span>攒挥 pending</span>
        <span>
          {pendingSwings}/{batchSize}
        </span>
      </div>
      <div style={rowStyle}>
        <span>乐观矿石(显示)</span>
        <span>~{optimisticUnits}</span>
      </div>
      <div style={rowStyle}>
        <span>链上已确认</span>
        <span>
          {confirmedUnits} 矿石 / {settledBatches} 批
        </span>
      </div>
      {lastReconcile ? (
        <div style={rowStyle}>
          <span>上次对账</span>
          <span>
            {lastReconcile.deltaUnits === 0
              ? "一致 ✓"
              : lastReconcile.phantom
                ? `幻影 ${-lastReconcile.deltaUnits}(已退)`
                : `补差 +${lastReconcile.deltaUnits}`}
          </span>
        </div>
      ) : null}
      {inFlight ? (
        <div style={rowStyle}>
          <span>在途批次</span>
          <span title={inFlightDigest ?? undefined}>
            {inFlightSwings} 挥 {inFlightDigest ? `· ${inFlightDigest.slice(0, 8)}…` : "(签名中)"}
          </span>
        </div>
      ) : null}

      <div style={{ ...rowStyle, alignItems: "center", margin: "4px 0" }}>
        <span>nonce</span>
        <input
          style={inputStyle}
          type="number"
          min={1}
          value={nextNonce}
          onChange={(event) => {
            const value = Number(event.target.value);
            if (Number.isInteger(value) && value >= 1) onNonceChange(value);
          }}
        />
      </div>

      <div style={{ ...rowStyle, marginTop: 6 }}>
        <button
          type="button"
          style={canAct ? buttonStyle : disabledButtonStyle}
          disabled={!canAct}
          onClick={onSwing}
          title="调试用：正式挖矿请走到矿脉前攻击它 / Dev shortcut — to mine, attack the vein in the world"
        >
          挥镐(调试) Swing
        </button>
        <button
          type="button"
          style={canAct && pendingSwings > 0 && !inFlight ? buttonStyle : disabledButtonStyle}
          disabled={!canAct || pendingSwings === 0 || inFlight}
          onClick={onFlushNow}
        >
          {submitBusy ? "上链中…" : "立即结算"}
        </button>
      </div>

      <div style={{ ...rowStyle, alignItems: "center", marginTop: 6 }}>
        <input
          style={{ ...inputStyle, width: 56 }}
          type="number"
          min={1}
          value={redeemAmount}
          onChange={(event) => onRedeemAmountChange(event.target.value)}
        />
        <button
          type="button"
          style={canAct ? buttonStyle : disabledButtonStyle}
          disabled={!canAct}
          onClick={onRedeem}
        >
          兑换金币 Redeem
        </button>
      </div>

      {lastError ? (
        <div style={{ marginTop: 6, color: "#ff9f9f", wordBreak: "break-all" }}>{lastError}</div>
      ) : null}
    </div>
  );
}
