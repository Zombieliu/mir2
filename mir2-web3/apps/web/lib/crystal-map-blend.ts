export type CrystalMapBlendMode = "normal" | "additive";

export function decodeCrystalMiddleAnimationCount(animationFrame: number) {
  return animationFrame <= 0 || animationFrame >= 255 ? 0 : animationFrame & 0x0f;
}

export function decodeCrystalFrontAnimationCount(animationFrame: number) {
  return animationFrame > 0 ? animationFrame & 0x7f : 0;
}

export function crystalMiddleMapBlendMode(animationFrame: number): CrystalMapBlendMode {
  const count = decodeCrystalMiddleAnimationCount(animationFrame);
  return count === 8 || count === 10 ? "additive" : "normal";
}

export function crystalFrontMapBlendMode(animationFrame: number): CrystalMapBlendMode {
  return (animationFrame & 0x80) !== 0 ? "additive" : "normal";
}
