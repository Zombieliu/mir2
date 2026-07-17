const LIGHT_STATES = new Map([
  [0, { label: "day", overlayClass: null, miniMapIcon: "/original-ui/Prguse/2093.png" }],
  [1, { label: "dawn", overlayClass: "dawn", miniMapIcon: "/original-ui/Prguse/2095.png" }],
  [2, { label: "day", overlayClass: null, miniMapIcon: "/original-ui/Prguse/2093.png" }],
  [3, { label: "evening", overlayClass: "evening", miniMapIcon: "/original-ui/Prguse/2094.png" }],
  [4, { label: "night", overlayClass: "night", miniMapIcon: "/original-ui/Prguse/2092.png" }],
]);

const LIGHT_ALIASES = new Map([
  ["dawn", 1],
  ["day", 2],
  ["evening", 3],
  ["night", 4],
]);

export function parseCaptureLightSetting(raw) {
  if (raw === null || raw === undefined || raw === "") return null;

  const normalized = String(raw).trim().toLowerCase();
  if (normalized === "") return null;
  const aliased = LIGHT_ALIASES.get(normalized);
  const value = aliased ?? Number(normalized);
  if (!Number.isInteger(value) || !LIGHT_STATES.has(value)) {
    throw new Error(`captureLightSetting must be 0..4 or dawn/day/evening/night, received ${raw}`);
  }
  return value;
}

export function parseCaptureEffectFrame(raw, frameCount = 10) {
  if (raw === null || raw === undefined || raw === "") return null;
  const normalized = String(raw).trim();
  if (normalized === "") return null;
  const value = Number(normalized);
  if (!Number.isInteger(value) || value < 0 || value >= frameCount) {
    throw new Error(`capture effect frame must be an integer in 0..${frameCount - 1}, received ${raw}`);
  }
  return value;
}

export function crystalCaptureLightState(setting) {
  const state = LIGHT_STATES.get(setting);
  if (!state) throw new Error(`Unsupported Crystal light setting: ${setting}`);
  return { setting, ...state };
}

export function isDayCaptureLight(setting) {
  return setting === 0 || setting === 2;
}

export function assertPairedCaptureLightSetting(requestedSetting, serverSetting) {
  const actualServerSetting = parseLightSettingNumber(serverSetting);
  if (requestedSetting === null || requestedSetting === undefined) {
    return { requestedSetting: null, serverSetting: actualServerSetting };
  }

  if (actualServerSetting !== requestedSetting) {
    throw new Error(
      `Paired Crystal/Web capture requires server lightSetting ${requestedSetting}, got ${displayLightSetting(serverSetting)}.`,
    );
  }

  return { requestedSetting, serverSetting: actualServerSetting };
}

export function iconMatchesExpected(actual, expectedPath) {
  if (!actual) return false;
  try {
    return new URL(actual, "http://capture.invalid").pathname === expectedPath;
  } catch {
    return false;
  }
}

export function additiveEffectHasDirectWorldBackdrop(effect) {
  if (effect?.blend !== "additive") return true;
  const effectZIndex = numericZIndex(effect.effectNodeZIndex);
  const rendererZIndex = numericZIndex(effect.worldRendererZIndex);
  return (
    effect.effectOverlayZIndex === "auto" &&
    noVisualTransform(effect.effectOverlayTranslate) &&
    noVisualTransform(effect.effectOverlayTransform) &&
    Number.isFinite(effectZIndex) &&
    Number.isFinite(rendererZIndex) &&
    effectZIndex > rendererZIndex &&
    effect.worldCompositeIsolation === "isolate" &&
    effect.worldCompositeVisible === true
  );
}

function numericZIndex(value) {
  if (value === null || value === undefined || value === "" || value === "auto") {
    return Number.NaN;
  }
  return Number(value);
}

function noVisualTransform(value) {
  return value === null || value === undefined || value === "" || value === "none";
}

function displayLightSetting(value) {
  const parsed = parseLightSettingNumber(value);
  return Number.isFinite(parsed) ? String(parsed) : "missing";
}

function parseLightSettingNumber(value) {
  if (value === null || value === undefined || value === "") return Number.NaN;
  return Number(value);
}
