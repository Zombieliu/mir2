/** A focused guild widget owns its keys before the game's window-capture handlers. */
export function guildKeyboardOwnsEvent(target: EventTarget | null): boolean {
  return typeof Element !== "undefined" && target instanceof Element
    && target.closest("[data-guild-keyboard-owner]") !== null;
}
