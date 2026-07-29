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
};

export type Mir2StagePresentation = {
  scale: number;
  left: number;
  top: number;
};

export type Mir2TouchControlDeck = {
  left: number;
  width: number;
};

export const MIR2_TOUCH_GAME_RAIL_CSS_PX = 128;

/**
 * Fits the native 1024x768 composition inside the visual viewport while
 * keeping its transformed bounds on whole device pixels.
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
}: Mir2StagePresentationInput): Mir2StagePresentation {
  const safeCssWidth = Math.max(1, cssWidth);
  const safeCssHeight = Math.max(1, cssHeight);
  const deviceScale = Math.max(0.25, devicePixelRatio || 1);
  const deviceWidth = Math.max(1, Math.floor(safeCssWidth * deviceScale));
  const deviceHeight = Math.max(1, Math.floor(safeCssHeight * deviceScale));
  const nativeDeviceWidth = Math.floor((layout === "tv" ? 2048 : 1024) * deviceScale);
  const reserveTouchGameRails = layout === "touch" && input === "touch" && screen === "game";
  const railDeviceWidth = reserveTouchGameRails
    ? Math.floor(MIR2_TOUCH_GAME_RAIL_CSS_PX * deviceScale)
    : 0;
  const railConstrainedDeviceWidth = Math.max(4, deviceWidth - railDeviceWidth * 2);
  const fittedDeviceWidth = Math.min(
    nativeDeviceWidth,
    railConstrainedDeviceWidth,
    Math.floor((deviceHeight * 4) / 3),
  );
  const displayDeviceWidth = Math.max(4, Math.floor(fittedDeviceWidth / 4) * 4);
  const displayDeviceHeight = (displayDeviceWidth * 3) / 4;

  return {
    scale: displayDeviceWidth / deviceScale / 1024,
    left: Math.round((deviceWidth - displayDeviceWidth) / 2) / deviceScale,
    top: Math.round((deviceHeight - displayDeviceHeight) / 2) / deviceScale,
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
  const stageWidth = 1024 * presentation.scale;
  return {
    left: Math.max(0, presentation.left - MIR2_TOUCH_GAME_RAIL_CSS_PX),
    width: stageWidth + MIR2_TOUCH_GAME_RAIL_CSS_PX * 2,
  };
}
