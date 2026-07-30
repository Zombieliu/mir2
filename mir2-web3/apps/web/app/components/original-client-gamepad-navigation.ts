export type Mir2SpatialDirection = "up" | "down" | "left" | "right";

export type Mir2FocusRect = {
  left: number;
  top: number;
  width: number;
  height: number;
};

const FOCUSABLE_SELECTOR = [
  "button:not(:disabled)",
  "input:not(:disabled)",
  "select:not(:disabled)",
  "textarea:not(:disabled)",
  "[role='button']:not([aria-disabled='true'])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

const BACK_SELECTOR = [
  "[data-gamepad-back='true']",
  ".inventory-close button",
  ".character-close button",
  ".npc-dialog-actions button",
  ".system-menu-close-hit",
  ".game-shop-close button",
  ".mail-close button",
  ".big-map-close button",
  ".select-action.exit button",
  ".login-button.close button",
].join(",");

const MODAL_SCOPE_SELECTOR = [
  "[role='dialog']",
  ".inventory-window",
  ".character-window",
  ".npc-dialog-panel",
  ".game-shop-window",
  ".big-map-dialog",
  ".mail-panel",
  ".report-panel",
  ".select-create-panel",
  ".select-delete-panel",
].join(",");

export function findMir2SpatialTarget(
  current: Mir2FocusRect,
  candidates: readonly Mir2FocusRect[],
  direction: Mir2SpatialDirection,
) {
  const currentCenter = rectCenter(current);
  let bestIndex = -1;
  let bestScore = Number.POSITIVE_INFINITY;

  candidates.forEach((candidate, index) => {
    const center = rectCenter(candidate);
    const dx = center.x - currentCenter.x;
    const dy = center.y - currentCenter.y;
    const primary =
      direction === "left" ? -dx : direction === "right" ? dx : direction === "up" ? -dy : dy;
    if (primary <= 1) return;
    const cross = direction === "left" || direction === "right" ? Math.abs(dy) : Math.abs(dx);
    const score = primary + cross * 2 + (cross * cross) / primary;
    if (score < bestScore) {
      bestScore = score;
      bestIndex = index;
    }
  });

  return bestIndex;
}

export function moveMir2GamepadFocus(root: HTMLElement, direction: Mir2SpatialDirection) {
  const focusRoot = activeFocusScope(root);
  const focusable = visibleFocusableElements(focusRoot);
  if (!focusable.length) return false;
  const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const currentIndex = active ? focusable.indexOf(active) : -1;

  if (currentIndex < 0) {
    const initial =
      focusable.find((element) => element.dataset.gamepadInitial === "true") ??
      focusable[0];
    initial.focus({ preventScroll: true });
    return true;
  }

  const candidates = focusable.filter((_, index) => index !== currentIndex);
  const targetIndex = findMir2SpatialTarget(
    focusable[currentIndex].getBoundingClientRect(),
    candidates.map((element) => element.getBoundingClientRect()),
    direction,
  );
  if (targetIndex < 0) return false;
  candidates[targetIndex].focus({ preventScroll: true });
  return true;
}

export function activateMir2GamepadFocus(root: HTMLElement) {
  const focusRoot = activeFocusScope(root);
  const active = document.activeElement;
  if (active instanceof HTMLElement && focusRoot.contains(active) && isVisible(active)) {
    active.click();
    return true;
  }
  const focusable = visibleFocusableElements(focusRoot);
  const first =
    focusable.find((element) => element.dataset.gamepadInitial === "true") ??
    focusable[0];
  if (!first) return false;
  first.focus({ preventScroll: true });
  return true;
}

export function closeMir2GamepadSurface(root: HTMLElement) {
  const close = Array.from(root.querySelectorAll<HTMLElement>(BACK_SELECTOR)).find(isVisible);
  if (close) {
    close.click();
    return true;
  }
  const active = document.activeElement;
  if (active instanceof HTMLElement && root.contains(active)) active.blur();
  root.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  return false;
}

function visibleFocusableElements(root: HTMLElement) {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(isVisible);
}

function activeFocusScope(root: HTMLElement) {
  const modalCandidates = Array.from(root.querySelectorAll<HTMLElement>(MODAL_SCOPE_SELECTOR))
    .filter(isVisible);
  if (!modalCandidates.length) return root;
  return modalCandidates.reduce((current, candidate) => {
    const currentZ = Number.parseInt(window.getComputedStyle(current).zIndex, 10) || 0;
    const candidateZ = Number.parseInt(window.getComputedStyle(candidate).zIndex, 10) || 0;
    return candidateZ >= currentZ ? candidate : current;
  });
}

function isVisible(element: HTMLElement) {
  if (element.hidden || element.getAttribute("aria-hidden") === "true") return false;
  const style = window.getComputedStyle(element);
  if (style.display === "none" || style.visibility === "hidden") return false;
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}

function rectCenter(rect: Mir2FocusRect) {
  return {
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
  };
}
