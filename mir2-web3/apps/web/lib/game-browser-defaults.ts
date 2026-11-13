/** Keep a game's right-button release owned even when it lands outside the frame. */
export function installGameBrowserDefaults(host: Window): () => void {
  let rightPressStartedInGame = false;
  const insideGame = (target: EventTarget | null) =>
    target instanceof Element && Boolean(target.closest(".client-stage-frame"));

  const onPointerDown = (event: PointerEvent) => {
    // Native UI may cancel pointerdown and suppress compatibility mousedown.
    // Observe ownership without canceling this pointer event ourselves.
    if (event.pointerType === "mouse" && event.button === 2) {
      rightPressStartedInGame = insideGame(event.target);
    }
  };

  const onMouseDown = (event: MouseEvent) => {
    if (event.button !== 2) return;
    rightPressStartedInGame = insideGame(event.target);
    if (rightPressStartedInGame) event.preventDefault();
  };
  const onContextMenu = (event: MouseEvent) => {
    const owned = insideGame(event.target) ||
      (event.button === 2 && rightPressStartedInGame);
    // Windows dispatches contextmenu after mouseup. Consume the press here,
    // so release outside is covered without canceling a later outside gesture.
    if (event.button === 2) rightPressStartedInGame = false;
    if (owned) event.preventDefault();
  };
  const onBlur = () => { rightPressStartedInGame = false; };
  const capture: AddEventListenerOptions = { capture: true, passive: false };

  // Keep compatibility mousedown/mouseup and the game's custom menu handlers.
  // Canceling pointerdown can suppress the classic mouse movement entry point.
  host.addEventListener("pointerdown", onPointerDown, capture);
  host.addEventListener("mousedown", onMouseDown, capture);
  host.addEventListener("contextmenu", onContextMenu, capture);
  host.addEventListener("blur", onBlur);
  return () => {
    rightPressStartedInGame = false;
    host.removeEventListener("pointerdown", onPointerDown, capture);
    host.removeEventListener("mousedown", onMouseDown, capture);
    host.removeEventListener("contextmenu", onContextMenu, capture);
    host.removeEventListener("blur", onBlur);
  };
}
