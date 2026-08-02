export type Mir2StageLayout = "desktop" | "touch" | "tv";
export type Mir2StageInput = "keyboardMouse" | "touch" | "gamepad";
export type Mir2StageScreen = "login" | "select" | "game";

export type Mir2StagePresentationInput = {
  cssWidth: number;
  cssHeight: number;
  devicePixelRatio: number;
  layout: Mir2StageLayout;
  input: Mir2StageInput;
  screen: Mir2StageScreen;
  wideMobile?: boolean;
};

export type Mir2StagePresentation = {
  scale: number;
  left: number;
  top: number;
  virtualWidth: number;
  virtualHeight: number;
  wideMobile: boolean;
};

export type Mir2TouchControlDeck = {
  left: number;
  width: number;
};

export type Mir2TouchControlMetrics = {
  actionSize: number;
  primarySize: number;
  panelSize: number;
  padWidth: number;
  columnStep: number;
  quickRowTops: [number, number, number];
  runBottom: number;
  sideRight: number;
  pickBottom: number;
};

export const MIR2_TOUCH_GAME_RAIL_CSS_PX = 128;

export function wideMobileVirtualWidth(cssWidth: number, cssHeight: number, virtualHeight = 768) {
  const safeCssWidth = Math.max(1, cssWidth);
  const safeCssHeight = Math.max(1, cssHeight);
  const requestedWidth = Math.max(1024, Math.ceil((safeCssWidth / safeCssHeight) * virtualHeight));
  return Math.max(1024, Math.ceil(requestedWidth / 4) * 4);
}
export const MIR2_TOUCH_COMPACT_HEIGHT_CSS_PX = 360;

const REGULAR_TOUCH_CONTROL_METRICS: Mir2TouchControlMetrics = {
  actionSize: 44,
  primarySize: 64,
  panelSize: 44,
  padWidth: 114,
  columnStep: 48,
  quickRowTops: [52, 100, 148],
  runBottom: 68,
  sideRight: 70,
  pickBottom: 48,
};

const COMPACT_TOUCH_CONTROL_METRICS: Mir2TouchControlMetrics = {
  actionSize: 36,
  primarySize: 52,
  panelSize: 36,
  padWidth: 98,
  columnStep: 38,
  quickRowTops: [40, 78, 116],
  runBottom: 56,
  sideRight: 56,
  pickBottom: 38,
};

/**
 * Compacts the action rail when browser chrome or docked DevTools leaves a
 * very short visual viewport. The regular rail needs about 324 CSS px before
 * its top quick slots and bottom combat cluster can no longer collide.
 */
export function calculateMir2TouchControlMetrics(cssHeight: number): Mir2TouchControlMetrics {
  return cssHeight <= MIR2_TOUCH_COMPACT_HEIGHT_CSS_PX
    ? COMPACT_TOUCH_CONTROL_METRICS
    : REGULAR_TOUCH_CONTROL_METRICS;
}

/**
 * Fits the native 1024x768 composition inside the visual viewport while
 * keeping its transformed bounds on whole device pixels. The opt-in wide
 * mobile mode expands the virtual stage horizontally instead of cropping the
 * original composition; the camera/HUD consumers can then use that width.
 *
 * Touch gameplay additionally reserves two control rails. This prevents the
 * joystick and action cluster from covering Crystal's original HUD on narrow
 * landscape phones.
 */
export function calculateMir2StagePresentation({
  cssWidth,
  cssHeight,
  devicePixelRatio,
  layout,
  input,
  screen,
  wideMobile = false,
}: Mir2StagePresentationInput): Mir2StagePresentation {
  const safeCssWidth = Math.max(1, cssWidth);
  const safeCssHeight = Math.max(1, cssHeight);
  const deviceScale = Math.max(0.25, devicePixelRatio || 1);
  const deviceWidth = Math.max(1, Math.floor(safeCssWidth * deviceScale));
  const deviceHeight = Math.max(1, Math.floor(safeCssHeight * deviceScale));
  const wideMobileLandscape =
    wideMobile &&
    layout === "touch" &&
    input === "touch" &&
    safeCssWidth > safeCssHeight;
  const virtualHeight = 768;
  const virtualWidth = wideMobileLandscape ? wideMobileVirtualWidth(safeCssWidth, safeCssHeight, virtualHeight) : 1024;
  const nativeDeviceWidth = Math.floor((layout === "tv" ? 2048 : virtualWidth) * deviceScale);
  const reserveTouchGameRails = layout === "touch" && input === "touch" && screen === "game";
  const railDeviceWidth = reserveTouchGameRails
    ? Math.floor(MIR2_TOUCH_GAME_RAIL_CSS_PX * deviceScale)
    : 0;
  const railConstrainedDeviceWidth = Math.max(4, deviceWidth - railDeviceWidth * 2);
  const fittedDeviceWidth = wideMobileLandscape
    ? Math.min(nativeDeviceWidth, deviceWidth)
    : Math.min(
        nativeDeviceWidth,
        railConstrainedDeviceWidth,
        Math.floor((deviceHeight * 4) / 3),
      );
  const displayDeviceWidth = Math.max(4, Math.floor(fittedDeviceWidth / 4) * 4);
  const displayDeviceHeight = wideMobileLandscape
    ? Math.floor((virtualHeight * displayDeviceWidth) / virtualWidth)
    : (displayDeviceWidth * 3) / 4;

  return {
    scale: displayDeviceWidth / deviceScale / virtualWidth,
    left: Math.round((deviceWidth - displayDeviceWidth) / 2) / deviceScale,
    top: Math.round((deviceHeight - displayDeviceHeight) / 2) / deviceScale,
    virtualWidth,
    virtualHeight,
    wideMobile: wideMobileLandscape,
  };
}

/**
 * Centers the touch controls around the rendered game stage instead of pinning
 * them to the physical viewport edges. The reserved 128px rails stay directly
 * beside the 4:3 composition, so ultra-wide landscape screens remain compact
 * and normal phones keep the same non-overlap guarantee.
 */
export function calculateMir2TouchControlDeck(
  presentation: Mir2StagePresentation,
): Mir2TouchControlDeck {
  const stageWidth = presentation.virtualWidth * presentation.scale;
  if (presentation.wideMobile) {
    return { left: 0, width: stageWidth };
  }
  return {
    left: Math.max(0, presentation.left - MIR2_TOUCH_GAME_RAIL_CSS_PX),
    width: stageWidth + MIR2_TOUCH_GAME_RAIL_CSS_PX * 2,
  };
}
