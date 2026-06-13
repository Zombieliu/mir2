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
 * never render (or even reference) it. Rendered *inside* the 1024×768 game stage (via the
 * shell's `gameOverlaySlot`) so it tracks/scales with the HUD instead of floating in the
 * viewport letterbox. Styling lives in `globals.css` (`.onchain-mine-*`) to match the
 * game's gold-framed window chrome; labels follow the game's language toggle.
 */

import type { Mir2Language } from "../../lib/localization";

export type OnchainMinePanelReconcile = {
  deltaUnits: number;
  phantom: boolean;
  shortfall: boolean;
};

export type OnchainMinePanelProps = {
  /** Active UI language — labels follow the game's language toggle. */
  language: Mir2Language;
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

function shortAddress(address: string): string {
  return address.length > 14 ? `${address.slice(0, 8)}…${address.slice(-4)}` : address;
}

/**
 * Per-language label table. The on-chain HUD is testnet-only (it carries no keys in the
 * generated localization bundle), so it ships a small self-contained dictionary keyed off
 * the game's `language` state. One language is shown at a time — no more mixed CN/EN
 * labels. Spanish reuses the English copy for this dev panel.
 */
function makeLabels(language: Mir2Language) {
  const zh = language === "zh-CN";
  return {
    title: zh ? "链上矿脉" : "On-chain Mine",
    testnet: zh ? "测试网" : "testnet",
    connect: zh ? "连接 Sui 钱包" : "Connect Sui wallet",
    connecting: zh ? "连接中…" : "Connecting…",
    wallet: zh ? "签名钱包" : "Signing wallet",
    pending: zh ? "待结算" : "Pending",
    optimistic: zh ? "乐观矿石" : "Optimistic ore",
    confirmed: zh ? "链上已确认" : "Confirmed",
    ore: zh ? "矿石" : "ore",
    batches: zh ? "批" : "batches",
    reconcile: zh ? "上次对账" : "Last reconcile",
    reconcileOk: zh ? "一致 ✓" : "matched ✓",
    inFlight: zh ? "在途批次" : "In-flight",
    swingUnit: zh ? "挥" : "swings",
    signing: zh ? "签名中" : "signing",
    nonce: "nonce",
    swing: zh ? "挖矿(调试)" : "Swing (dev)",
    swingHint: zh
      ? "调试用：正式挖矿请走到矿脉前攻击它"
      : "Dev shortcut — to mine, attack the vein in the world",
    settle: zh ? "立即结算" : "Settle now",
    settling: zh ? "上链中…" : "Submitting…",
    redeem: zh ? "兑换金币" : "Redeem",
    veinFull: zh ? "矿脉饱满" : "full vein",
    veinCracked: zh ? "矿脉开裂" : "cracked",
    veinDepleted: zh ? "矿脉枯竭" : "depleted",
    veinUnknown: "—",
  };
}

type Labels = ReturnType<typeof makeLabels>;

function veinStageInfo(stage: number | null, labels: Labels) {
  if (stage === null) return { text: labels.veinUnknown, modifier: "" };
  if (stage >= 2) return { text: labels.veinFull, modifier: "onchain-mine-vein--full" };
  if (stage === 1) return { text: labels.veinCracked, modifier: "onchain-mine-vein--cracked" };
  return { text: labels.veinDepleted, modifier: "onchain-mine-vein--depleted" };
}

function reconcileText(reconcile: OnchainMinePanelReconcile, labels: Labels, zh: boolean) {
  if (reconcile.deltaUnits === 0) return labels.reconcileOk;
  if (reconcile.phantom) {
    return zh ? `幻影 ${-reconcile.deltaUnits}(已退)` : `phantom ${-reconcile.deltaUnits} (rolled back)`;
  }
  return zh ? `补差 +${reconcile.deltaUnits}` : `+${reconcile.deltaUnits} reconciled`;
}

/**
 * Turn a raw chain/wallet error into a calm, actionable one-liner. The verbatim message
 * is preserved in the element's `title` for debugging.
 */
function humanizeError(raw: string, zh: boolean): string {
  const lower = raw.toLowerCase();
  if (lower.includes("reject") || lower.includes("denied") || lower.includes("cancel")) {
    return zh ? "签名已取消 — 请重试" : "Signature cancelled — try again";
  }
  if (lower.includes("insufficient") || lower.includes("gas") || lower.includes("balance")) {
    return zh ? "余额 / Gas 不足" : "Insufficient balance for gas";
  }
  if (lower.includes("nonce")) {
    return zh ? "nonce 冲突 — 调整后重试" : "Nonce conflict — adjust and retry";
  }
  if (lower.includes("timeout") || lower.includes("timed out")) {
    return zh ? "结算超时 — 请重试" : "Settlement timed out — retry";
  }
  return raw;
}

export function OnchainMinePanel({
  language,
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
  const zh = language === "zh-CN";
  const labels = makeLabels(language);
  const canAct = walletAddress !== null && !submitBusy;
  const inFlight = inFlightSwings > 0;
  const vein = veinStageInfo(veinStage, labels);
  return (
    <div className="onchain-mine-panel" data-testid="onchain-mine-panel">
      <div className="onchain-mine-title">
        <b>
          {labels.title} <small>({labels.testnet})</small>
        </b>
        <span>
          ({veinLocation.x},{veinLocation.y}){" "}
          <em className={vein.modifier}>{vein.text}</em>
        </span>
      </div>

      {walletAddress === null ? (
        <button
          type="button"
          className="onchain-mine-btn onchain-mine-btn--wide"
          disabled={walletBusy}
          onClick={onConnectWallet}
        >
          {walletBusy ? labels.connecting : labels.connect}
        </button>
      ) : (
        <div className="onchain-mine-row">
          <span>{labels.wallet}</span>
          <span title={walletAddress}>{shortAddress(walletAddress)}</span>
        </div>
      )}

      <div className="onchain-mine-row">
        <span>{labels.pending}</span>
        <span>
          {pendingSwings}/{batchSize}
        </span>
      </div>
      <div className="onchain-mine-row">
        <span>{labels.optimistic}</span>
        <span>~{optimisticUnits}</span>
      </div>
      <div className="onchain-mine-row">
        <span>{labels.confirmed}</span>
        <span>
          {confirmedUnits} {labels.ore} / {settledBatches} {labels.batches}
        </span>
      </div>
      {lastReconcile ? (
        <div className="onchain-mine-row">
          <span>{labels.reconcile}</span>
          <span>{reconcileText(lastReconcile, labels, zh)}</span>
        </div>
      ) : null}
      {inFlight ? (
        <div className="onchain-mine-row">
          <span>{labels.inFlight}</span>
          <span title={inFlightDigest ?? undefined}>
            {inFlightSwings} {labels.swingUnit}{" "}
            {inFlightDigest ? `· ${inFlightDigest.slice(0, 8)}…` : `(${labels.signing})`}
          </span>
        </div>
      ) : null}

      <div className="onchain-mine-row onchain-mine-row--input">
        <span>{labels.nonce}</span>
        <input
          className="onchain-mine-input"
          type="number"
          min={1}
          value={nextNonce}
          onChange={(event) => {
            const value = Number(event.target.value);
            if (Number.isInteger(value) && value >= 1) onNonceChange(value);
          }}
        />
      </div>

      <div className="onchain-mine-actions">
        <button
          type="button"
          className="onchain-mine-btn"
          disabled={!canAct}
          onClick={onSwing}
          title={labels.swingHint}
        >
          {labels.swing}
        </button>
        <button
          type="button"
          className="onchain-mine-btn"
          disabled={!canAct || pendingSwings === 0 || inFlight}
          onClick={onFlushNow}
        >
          {submitBusy ? labels.settling : labels.settle}
        </button>
      </div>

      <div className="onchain-mine-actions onchain-mine-row--input">
        <input
          className="onchain-mine-input"
          type="number"
          min={1}
          value={redeemAmount}
          onChange={(event) => onRedeemAmountChange(event.target.value)}
        />
        <button
          type="button"
          className="onchain-mine-btn"
          disabled={!canAct}
          onClick={onRedeem}
        >
          {labels.redeem}
        </button>
      </div>

      {lastError ? (
        <div className="onchain-mine-error" role="alert" title={lastError}>
          {humanizeError(lastError, zh)}
        </div>
      ) : null}
    </div>
  );
}
