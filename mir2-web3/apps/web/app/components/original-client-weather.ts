export const CRYSTAL_WEATHER = Object.freeze({
  fog: 1,
  redEmber: 2,
  whiteEmber: 4,
  yellowEmber: 8,
  fireParticle: 16,
  snow: 32,
  rain: 64,
  leaves: 128,
  fireyLeaves: 256,
  purpleLeaves: 512,
});

export type CrystalWeatherLayer = {
  key: string;
  frame: number;
  className: string;
};

export function crystalWeatherTexturePath(frame: number): string {
  if (![0, 1, 43, 164, 359, 531, 587].includes(frame)) {
    throw new Error(`Unsupported Crystal weather base frame ${frame}.`);
  }
  return `/original-effects/Weather/${frame}.png`;
}

export function crystalWeatherLayers(weatherParticles: number | null | undefined): CrystalWeatherLayer[] {
  const value = Number.isFinite(weatherParticles) ? Math.max(0, Math.trunc(weatherParticles ?? 0)) : 0;
  const layers: CrystalWeatherLayer[] = [];
  const add = (flag: number, key: string, frame: number, className: string) => {
    if ((value & flag) === flag) layers.push({ key, frame, className });
  };

  add(CRYSTAL_WEATHER.fog, "fog", 0, "fog");
  add(CRYSTAL_WEATHER.redEmber, "red-ember", 1, "ember red-ember");
  add(CRYSTAL_WEATHER.whiteEmber, "white-ember", 1, "ember white-ember");
  add(CRYSTAL_WEATHER.yellowEmber, "yellow-ember", 1, "ember yellow-ember");
  // Crystal's FireParticle branch constructs an engine without textures and draws nothing.
  add(CRYSTAL_WEATHER.snow, "snow", 43, "snow");
  add(CRYSTAL_WEATHER.rain, "rain", 164, "rain");

  for (const [frame, suffix] of [[359, "a"], [531, "b"], [587, "c"]] as const) {
    add(CRYSTAL_WEATHER.leaves, `leaves-${suffix}`, frame, `leaves leaves-${suffix}`);
    add(CRYSTAL_WEATHER.fireyLeaves, `firey-leaves-${suffix}`, frame, `leaves firey-leaves leaves-${suffix}`);
    add(CRYSTAL_WEATHER.purpleLeaves, `purple-leaves-${suffix}`, frame, `leaves purple-leaves leaves-${suffix}`);
  }
  return layers;
}
