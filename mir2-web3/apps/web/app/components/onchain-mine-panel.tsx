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

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";

export type OnchainMinePanelReconcile = {
  deltaUnits: number;
  phantom: boolean;
  shortfall: boolean;
};

export type OnchainMinePanelProps = {
  /** Touch layouts use a fixed, compact presentation instead of a draggable desktop tool window. */
  compactMode?: boolean;
  /** Temporarily yield the viewport to higher-priority onboarding or modal UI. */
  suppressed?: boolean;
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
  /** Active session-key address (popup-free signing), null when none/expired. */
  sessionAddress: string | null;
  /** Session expiry (epoch ms), null when no active session. */
  sessionExpiresAt: number | null;
  /** Authorize an ephemeral session key (one wallet popup) — kills per-batch popups. */
  onActivateSession: () => void;
  /** Revoke the active session key. */
  onDeactivateSession: () => void;
  /** Dev shortcut for one swing — the real gesture is attacking the vein in the world. */
  onSwing: () => void;
  /** Submit the pending swings now, even below the batch threshold. */
  onFlushNow: () => void;
  onRedeem: () => void;
  onNonceChange: (nextNonce: number) => void;
};

const PANEL_WIDTH = 252;
const POSITION_STORAGE_KEY = "mir2.onchainMinePanel.pos.v1";
const COLLAPSED_STORAGE_KEY = "mir2.onchainMinePanel.collapsed.v1";

const panelStyle: CSSProperties = {
  position: "absolute",
  zIndex: 60,
  width: PANEL_WIDTH,
  padding: "10px 12px",
  borderRadius: 8,
  background: "rgba(12, 16, 24, 0.92)",
  border: "1px solid rgba(120, 150, 200, 0.35)",
  color: "#dce6f5",
  font: "12px/1.5 var(--font-geist-mono, monospace)",
};

type PanelPosition = { left: number; top: number };

const DEFAULT_STAGE_WIDTH = 1024;
const DEFAULT_STAGE_HEIGHT = 768;
const DEFAULT_PANEL_TOP = 158;

function stageRootFor(element?: Element | null): HTMLElement | null {
  return element?.closest<HTMLElement>(".client-stage-frame") ?? null;
}

function stageBounds(element?: Element | null): { width: number; height: number } {
  const stageRoot = stageRootFor(element);
  return {
    width: stageRoot?.clientWidth || DEFAULT_STAGE_WIDTH,
    height: stageRoot?.clientHeight || DEFAULT_STAGE_HEIGHT,
  };
}

/**
 * Default the panel to the right gutter just under the minimap — clear of Mir2's
 * bottom control bar / system-menu cluster, which the old `bottom: 96` anchor used to
 * sit on top of and swallow clicks for. The player can still drag it anywhere or
 * collapse it to the title bar; both choices persist.
 */
function defaultPosition(element?: Element | null): PanelPosition {
  const bounds = stageBounds(element);
  return {
    left: Math.max(12, bounds.width - PANEL_WIDTH - 12),
    top: Math.min(DEFAULT_PANEL_TOP, Math.max(8, bounds.height - 40)),
  };
}

/** Keep the panel inside the Crystal stage after drags / resizes. */
function clampPosition(pos: PanelPosition, element?: Element | null): PanelPosition {
  const bounds = stageBounds(element);
  const maxLeft = Math.max(0, bounds.width - PANEL_WIDTH - 4);
  const maxTop = Math.max(0, bounds.height - 40 - 4);
  return {
    left: Math.min(Math.max(0, pos.left), maxLeft),
    top: Math.min(Math.max(0, pos.top), maxTop),
  };
}

const dragHandleStyle: CSSProperties = {
  cursor: "move",
  userSelect: "none",
  touchAction: "none",
};

const collapseButtonStyle: CSSProperties = {
  flex: "0 0 auto",
  width: 20,
  height: 18,
  lineHeight: "16px",
  textAlign: "center",
  padding: 0,
  borderRadius: 4,
  border: "1px solid rgba(140, 170, 220, 0.5)",
  background: "rgba(40, 60, 90, 0.8)",
  color: "#e8f0ff",
  cursor: "pointer",
  font: "inherit",
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
  compactMode = false,
  suppressed = false,
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
  sessionAddress,
  sessionExpiresAt,
  onActivateSession,
  onDeactivateSession,
  onSwing,
  onFlushNow,
  onRedeem,
  onNonceChange,
}: OnchainMinePanelProps) {
  const canAct = walletAddress !== null && !submitBusy;
  const inFlight = inFlightSwings > 0;

  // Draggable + collapsible so the HUD never permanently camps on the game's
  // bottom-right controls. `position === null` means "use the default anchor" and is
  // the SSR/first-paint state; the real position/collapsed flag are hydrated from
  // localStorage after mount to avoid a hydration mismatch.
  const [position, setPosition] = useState<PanelPosition | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{
    pointerStartX: number;
    pointerStartY: number;
    originLeft: number;
    originTop: number;
    scaleX: number;
    scaleY: number;
  } | null>(
    null,
  );

  useEffect(() => {
    try {
      const storedPos = window.localStorage.getItem(POSITION_STORAGE_KEY);
      if (storedPos) {
        const parsed = JSON.parse(storedPos) as Partial<PanelPosition>;
        if (typeof parsed?.left === "number" && typeof parsed?.top === "number") {
          setPosition(clampPosition({ left: parsed.left, top: parsed.top }, panelRef.current));
        } else {
          setPosition(defaultPosition(panelRef.current));
        }
      } else {
        setPosition(defaultPosition(panelRef.current));
      }
      const storedCollapsed = window.localStorage.getItem(COLLAPSED_STORAGE_KEY);
      setCollapsed(storedCollapsed === null ? compactMode : storedCollapsed === "1");
    } catch {
      setPosition(defaultPosition(panelRef.current));
      setCollapsed(compactMode);
    }
  }, [compactMode]);

  useEffect(() => {
    const onResize = () => {
      setPosition((current) =>
        current ? clampPosition(current, panelRef.current) : defaultPosition(panelRef.current),
      );
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const persistPosition = useCallback((next: PanelPosition) => {
    try {
      window.localStorage.setItem(POSITION_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // ignore storage failures — placement is a convenience, not state we depend on.
    }
  }, []);

  const onDragPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (compactMode || event.button !== 0) return;
      const origin = position ?? defaultPosition(event.currentTarget);
      const stageRoot = stageRootFor(event.currentTarget);
      const stageRect = stageRoot?.getBoundingClientRect();
      const scaleX = stageRoot && stageRect ? stageRect.width / Math.max(1, stageRoot.clientWidth) : 1;
      const scaleY = stageRoot && stageRect ? stageRect.height / Math.max(1, stageRoot.clientHeight) : 1;
      dragRef.current = {
        pointerStartX: event.clientX,
        pointerStartY: event.clientY,
        originLeft: origin.left,
        originTop: origin.top,
        scaleX: scaleX || 1,
        scaleY: scaleY || 1,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [compactMode, position],
  );

  const onDragPointerMove = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag) return;
    setPosition(
      clampPosition(
        {
          left: drag.originLeft + (event.clientX - drag.pointerStartX) / drag.scaleX,
          top: drag.originTop + (event.clientY - drag.pointerStartY) / drag.scaleY,
        },
        event.currentTarget,
      ),
    );
  }, []);

  const onDragPointerUp = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (!dragRef.current) return;
      dragRef.current = null;
      try {
        event.currentTarget.releasePointerCapture(event.pointerId);
      } catch {
        // pointer may already be released
      }
      setPosition((current) => {
        if (current) persistPosition(current);
        return current;
      });
    },
    [persistPosition],
  );

  const toggleCollapsed = useCallback(() => {
    setCollapsed((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(COLLAPSED_STORAGE_KEY, next ? "1" : "0");
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  const positionedStyle: CSSProperties = compactMode
    ? {
        ...panelStyle,
        left: "50%",
        top: 8,
        width: collapsed ? 206 : "min(360px, calc(100% - 24px))",
        maxHeight: "calc(100% - 16px)",
        overflowY: collapsed ? "hidden" : "auto",
        transform: "translateX(-50%)",
        padding: collapsed ? "7px 10px" : panelStyle.padding,
      }
    : position
      ? { ...panelStyle, left: position.left, top: position.top }
      : { ...panelStyle, right: 12, top: DEFAULT_PANEL_TOP };

  if (suppressed) return null;

  return (
    <div
      ref={panelRef}
      style={positionedStyle}
      data-testid="onchain-mine-panel"
      data-compact={compactMode ? "true" : "false"}
      data-collapsed={collapsed ? "true" : "false"}
    >
      <div
        style={{
          ...rowStyle,
          marginBottom: collapsed ? 0 : 6,
          alignItems: "center",
          ...dragHandleStyle,
          cursor: compactMode ? "default" : dragHandleStyle.cursor,
        }}
        onPointerDown={onDragPointerDown}
        onPointerMove={onDragPointerMove}
        onPointerUp={onDragPointerUp}
        onPointerCancel={onDragPointerUp}
        title="拖动标题栏移动面板 / drag the title bar to move"
      >
        <strong>On-chain Mine (testnet)</strong>
        <button
          type="button"
          style={collapseButtonStyle}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={toggleCollapsed}
          aria-label={collapsed ? "展开 / expand" : "收起 / collapse"}
          title={collapsed ? "展开 / expand" : "收起 / collapse"}
        >
          {collapsed ? "▸" : "▾"}
        </button>
      </div>

      {collapsed && !compactMode ? (
        <div style={{ ...rowStyle, marginTop: 4, opacity: 0.85 }}>
          <span>
            ({veinLocation.x},{veinLocation.y})
          </span>
          <span>{veinStageLabel(veinStage)}</span>
        </div>
      ) : collapsed ? null : (
        <>
      <div style={{ ...rowStyle, marginBottom: 6 }}>
        <span>矿脉 vein</span>
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

      {walletAddress !== null ? (
        sessionAddress !== null ? (
          <div style={{ ...rowStyle, marginTop: 4 }}>
            <span title={sessionAddress}>
              会话 {shortAddress(sessionAddress)} ✓免弹窗
              {sessionExpiresAt ? ` 至${new Date(sessionExpiresAt).toLocaleTimeString()}` : ""}
            </span>
            <button
              type="button"
              style={walletBusy ? disabledButtonStyle : { ...buttonStyle, flex: "0 0 auto", padding: "2px 6px" }}
              disabled={walletBusy}
              onClick={onDeactivateSession}
              title="撤销会话密钥 / revoke session key"
            >
              撤销
            </button>
          </div>
        ) : (
          <button
            type="button"
            style={walletBusy ? disabledButtonStyle : { ...buttonStyle, marginTop: 4 }}
            disabled={walletBusy}
            onClick={onActivateSession}
            title="授权一个临时会话密钥，之后连续挖矿不再弹钱包 / authorize a session key so continuous mining is popup-free"
          >
            {walletBusy ? "…" : "开启免弹窗会话 / Activate session"}
          </button>
        )
      ) : null}

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
        </>
      )}
    </div>
  );
}
