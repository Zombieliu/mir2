/** A cache refresh must not discard a login attempt, character or live world. */
export function canReloadAssetNamespace(document: Document): boolean {
  if (document.visibilityState !== "visible") return false;
  const login = document.querySelector("main[data-client-screen='login']");
  const form = login?.querySelector("form[data-login-auth-surface='login']");
  if (!login || !form?.isConnected) return false;

  // Wallet/passkey requests disable the ordinary form; other auth surfaces
  // and visible dialogs have their own in-progress state to preserve.
  if (Array.from(form.querySelectorAll<HTMLInputElement | HTMLButtonElement>("input,button"))
    .some(control => control.disabled)) return false;
  if (Array.from(document.querySelectorAll<HTMLElement>("dialog[open],[role='dialog']"))
    .some(dialog => dialog.getClientRects().length > 0)) return false;
  return !Array.from(login.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("input,textarea"))
    .some(input => !["checkbox", "radio", "range", "button", "submit"].includes(input.type)
      && input.value.length > 0);
}
