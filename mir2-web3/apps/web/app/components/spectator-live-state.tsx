"use client";

import { useEffect, useState } from 'react';
import { spectatorLiveState } from '../../lib/spectator-live-state';
import type { SpectatorStatus } from './spectator-overlay';

export function SpectatorLiveStateBadge({ status, connectionState }: {
  status: SpectatorStatus | null;
  connectionState: string;
}) {
  const [now, setNow] = useState(0);
  useEffect(() => {
    setNow(Date.now());
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, []);
  const state = connectionState === 'open' ? spectatorLiveState(status, now) : 'waiting';
  const labels = {
    waiting: '觀戰尚未連接 / 等待玩家畫面',
    buffering: '正在建立公開觀戰延遲緩衝',
    live: `即時觀戰 · 延遲 ${Math.round((status?.delayMs ?? 0) / 1_000)} 秒`,
    stale: '畫面已停止更新 · 等待玩家重新進入',
    replay: '錄像回放',
  };
  return <div data-testid="spectator-live-state" data-state={state}
    role="status" style={{ position: 'fixed', top: 12, left: '50%', transform: 'translateX(-50%)',
      zIndex: 3501, padding: '7px 14px', borderRadius: 6, fontFamily: 'sans-serif', fontSize: 14,
      background: 'rgba(5, 14, 27, 0.94)', color: state === 'live' ? '#9ff3ba' : '#ffe09b',
      border: '1px solid rgba(120, 210, 255, 0.35)', pointerEvents: 'none' }}>
    {labels[state]}
  </div>;
}
