export type CrystalPlayerActionFrame = {
  start: number;
  count: number;
  skip: number;
  intervalMs: number;
  reverse?: boolean;
  mountStart?: number;
};

// Crystal Client/MirObjects/Frames.cs `FrameSet.Player`.
export const CRYSTAL_PLAYER_ACTION_FRAMES = {
  standing: { start: 0, count: 4, skip: 0, intervalMs: 500 },
  walking: { start: 32, count: 6, skip: 0, intervalMs: 100 },
  running: { start: 80, count: 6, skip: 0, intervalMs: 100 },
  attack1: { start: 136, count: 6, skip: 0, intervalMs: 100 },
  attack2: { start: 184, count: 6, skip: 0, intervalMs: 100 },
  attack3: { start: 232, count: 8, skip: 0, intervalMs: 100 },
  attack4: { start: 416, count: 6, skip: 0, intervalMs: 100 },
  attackRange: { start: 96, count: 8, skip: 0, intervalMs: 100 },
  struck: { start: 360, count: 3, skip: 0, intervalMs: 100 },
  dying: { start: 384, count: 4, skip: 0, intervalMs: 100 },
  dead: { start: 387, count: 1, skip: 3, intervalMs: 1_000 },
  reviving: { start: 384, count: 4, skip: 0, intervalMs: 100, reverse: true },
  archerWalking: { start: 0, count: 6, skip: 0, intervalMs: 100 },
  archerRunning: { start: 48, count: 6, skip: 0, intervalMs: 100 },
  mountStanding: { start: 416, count: 4, skip: 0, intervalMs: 500, mountStart: 0 },
  mountWalking: { start: 448, count: 8, skip: 0, intervalMs: 100, mountStart: 32 },
  mountRunning: { start: 512, count: 6, skip: 0, intervalMs: 100, mountStart: 96 },
  mountStruck: { start: 560, count: 3, skip: 0, intervalMs: 100, mountStart: 144 },
  mountAttack: { start: 584, count: 6, skip: 0, intervalMs: 100, mountStart: 168 },
} as const satisfies Record<string, CrystalPlayerActionFrame>;

export type CrystalPlayerActionKey = keyof typeof CRYSTAL_PLAYER_ACTION_FRAMES;

export function crystalPlayerDirectionStride(frame: CrystalPlayerActionFrame) {
  return frame.count + frame.skip;
}

export function crystalPlayerAnimationMeta(
  action: CrystalPlayerActionKey,
  frameBaseOffset: number,
  weaponFrameOffset: number | null | undefined,
  includeWeapon = true,
) {
  const frame: CrystalPlayerActionFrame = CRYSTAL_PLAYER_ACTION_FRAMES[action];
  return {
    frameBaseOffset: frameBaseOffset + frame.start,
    mountFrameBaseOffset: frame.mountStart,
    weaponFrameOffset:
      includeWeapon && weaponFrameOffset !== undefined && weaponFrameOffset !== null
        ? weaponFrameOffset + frame.start
        : null,
    frameCount: frame.count,
    directionStride: crystalPlayerDirectionStride(frame),
    frameIntervalMs: frame.intervalMs,
    reverse: frame.reverse,
  };
}

export function crystalPlayerFrameIndex(
  action: CrystalPlayerActionKey,
  directionIndex: number,
  phase: number,
  libraryOffset = 0,
) {
  const frame = CRYSTAL_PLAYER_ACTION_FRAMES[action];
  return libraryOffset + frame.start + crystalPlayerDirectionStride(frame) * directionIndex + phase;
}

export function crystalMountFrameIndex(
  action: "mountStanding" | "mountWalking" | "mountRunning" | "mountStruck" | "mountAttack",
  directionIndex: number,
  phase: number,
  mountOffset = 0,
) {
  const frame = CRYSTAL_PLAYER_ACTION_FRAMES[action];
  return (
    mountOffset +
    frame.mountStart +
    crystalPlayerDirectionStride(frame) * directionIndex +
    phase
  );
}
