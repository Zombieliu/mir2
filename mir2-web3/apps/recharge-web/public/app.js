import {CONFIG, IntentJournal, PortalError, PortalSession, accountScope} from "./core.js";
import {LOCALES, normalizeLocale} from "./locales.js";

const byId = id => document.getElementById(id);
let savedLocale;
try { savedLocale = localStorage.getItem("numeron-language"); } catch { /* Preferences are optional. */ }
let locale = normalizeLocale(savedLocale || navigator.language);
const state = {epoch: 0, loggedIn: false, busy: false, scope: null, characterIndex: null,
  status: null, offerId: null, checkout: null, notice: null, poll: null, checks: 0, lastRefresh: 0};
const session = new PortalSession({onDisconnect: () => signOut(false)});
// A storage getter may itself throw in privacy modes. Fail closed on checkout.
let journal;
try { journal = new IntentJournal(sessionStorage); } catch { journal = null; }
const t = key => LOCALES[locale][key] || LOCALES[locale].serviceUnavailable;
const number = value => new Intl.NumberFormat(locale).format(value);
function node(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}
function notice(key, error = false) {
  state.notice = {key, error};
  byId("notice").textContent = t(key); byId("notice").classList.toggle("error", error); byId("notice").hidden = false;
}
function errorNotice(error, financial = false) {
  const code = error instanceof PortalError ? error.code : "serviceUnavailable";
  if (code === "sessionExpired" || code === "billingAuthenticationRequired") { signOut(false); return; }
  const key = code === "loginFailed" || code === "accountBanned" || code === "billingAccountBanned" ? "loginFailed"
    : code === "storageUnavailable" ? code
    : financial ? "unknown" : "serviceUnavailable";
  notice(key, true);
}
function setBusy(busy) {
  state.busy = busy;
  byId("login").disabled = busy; byId("account").disabled = busy; byId("password").disabled = busy;
  byId("character").disabled = busy; byId("refresh").disabled = busy || state.characterIndex === null;
  byId("offers").setAttribute("aria-busy", String(busy));
  byId("new-order").disabled = busy;
  for (const button of byId("offers").querySelectorAll("button")) button.disabled = busy || !state.status?.checkoutEnabled;
}
function renderOffers() {
  const container = byId("offers"); container.replaceChildren();
  if (state.loggedIn && !state.status?.offers.length) {
    container.append(node("p", "login-placeholder", t(state.loggedIn ? "serviceUnavailable" : "loginFirst"))); return;
  }
  for (const offer of state.status?.offers ?? CONFIG.previewOffers) {
    const button = node("button", "offer"); button.type = "button";
    button.dataset.offerId = offer.id;
    button.classList.toggle("selected", offer.id === state.offerId);
    button.disabled = state.busy || !state.loggedIn || !state.status?.checkoutEnabled;
    button.setAttribute("aria-pressed", String(offer.id === state.offerId));
    button.append(node("span", "offer-label", t("credits")), node("strong", "offer-credits", number(offer.credits)));
    const bottom = node("span", "offer-bottom");
    const price = offer.amountLabel || new Intl.NumberFormat(locale, {style:"currency", currency:offer.currency.toUpperCase()}).format(offer.amountMinor / 100);
    bottom.append(node("span", "offer-price", price), node("span", "offer-arrow", "↗"));
    button.append(bottom); button.setAttribute("aria-label", `${t("pay")} ${number(offer.credits)} ${t("credits")} · ${price}`);
    if (offer.id === state.offerId) button.append(node("span", "offer-selected-mark", "✓"));
    button.addEventListener("click", () => createCheckout(offer.id)); container.append(button);
  }
  if (!state.loggedIn) container.append(node("p", "offer-lock-note", t("loginFirst")));
}
function renderStatus() {
  const value = state.status;
  byId("credit").textContent = value ? number(value.credit) : "—";
  byId("pending-row").hidden = !value?.pendingCredit;
  byId("pending").textContent = value ? number(value.pendingCredit) : "";
  byId("monthly-state").textContent = t(value?.monthlyCard.active ? "monthlyActive" : "monthlyInactive");
  byId("monthly-expiry").textContent = value?.monthlyCard.active && value.monthlyCard.expiresAtMs
    ? new Intl.DateTimeFormat(locale, {dateStyle:"medium", timeStyle:"short"}).format(value.monthlyCard.expiresAtMs) : "";
  byId("monthly-price").textContent = `${value?.monthlyCardCreditPrice !== null && value?.monthlyCardCreditPrice !== undefined ? `${number(value.monthlyCardCreditPrice)} ${t("credits")} · ` : ""}${t("monthlyNote")}`;
  byId("order-section").hidden = !value;
  const orders = byId("orders"); orders.replaceChildren();
  if (value && !value.orders.length) orders.append(node("p", "muted", t("noOrders")));
  else if (value) {
    const list = node("ul", "order-list");
    for (const order of value.orders) {
      const row = node("li", "order-row");
      row.append(node("span", "order-id", order.id), node("span", `order-state ${order.state}`, t(order.state))); list.append(row);
    }
    orders.append(list);
  }
  renderOffers(); renderCheckout(); setBusy(state.busy);
}
function renderCheckout() {
  byId("checkout-panel").hidden = !state.checkout;
  byId("checkout-link").hidden = !state.checkout?.checkoutUrl;
  byId("checkout-link").removeAttribute("href");
  if (state.checkout?.checkoutUrl) byId("checkout-link").href = state.checkout.checkoutUrl;
  byId("checkout-message").textContent = t(state.checkout?.checkoutUrl ? "checkoutReady" : "completed");
  byId("new-order").hidden = !state.checkout;
}
function renderLocale() {
  document.documentElement.lang = locale; document.documentElement.dir = locale === "ar" ? "rtl" : "ltr";
  document.title = t("siteTitle"); byId("language").value = locale;
  for (const element of document.querySelectorAll("[data-i18n]")) element.textContent = t(element.dataset.i18n);
  if (state.notice) notice(state.notice.key, state.notice.error);
  renderStatus();
}
function stopPoll() { clearInterval(state.poll); state.poll = null; }
async function refresh(showNotice = true) {
  if (state.busy || !state.loggedIn || state.characterIndex === null) return;
  const epoch = state.epoch, index = state.characterIndex;
  state.lastRefresh = Date.now(); setBusy(true);
  try {
    let value = await session.status(index), online = false;
    // The HTTP snapshot is saved state. Check the existing WSS route fence
    // even with no pending Credits: a playing character may have a newer
    // in-game balance that hasn't reached its next normal save yet.
    try { value = await session.applyPending(index); }
    catch (error) {
      if (error.code !== "billingCharacterOnline") throw error;
      online = true;
    }
    if (epoch !== state.epoch) return;
    state.status = value; renderStatus();
    if (online) notice("onlinePending"); else if (showNotice) notice("refreshDone");
    const current = value.orders.find(order => order.id === state.checkout?.orderId);
    if (current && ["applied", "review"].includes(current.state)) {
      stopPoll(); state.checkout.checkoutUrl = null; renderCheckout();
    }
  } catch (error) { if (epoch === state.epoch) errorNotice(error); }
  finally { if (epoch === state.epoch) setBusy(false); }
}
function pollAfterCheckout() {
  stopPoll(); state.checks = 0;
  state.poll = setInterval(() => {
    if (++state.checks > 20) { stopPoll(); return; }
    if (!document.hidden) refresh(false);
  }, 15000);
}
async function createCheckout(offerId) {
  if (state.busy || state.characterIndex === null || !state.status?.checkoutEnabled) return;
  const epoch = state.epoch, index = state.characterIndex;
  setBusy(true); notice("creating");
  try {
    if (!journal) throw new PortalError("storageUnavailable");
    // Persist the nonce before sending. Clicking again after a timeout uses it.
    const intent = journal.getOrCreate(state.scope, index, offerId);
    state.offerId = offerId; renderOffers();
    const result = await session.checkout(index, offerId, intent);
    if (epoch !== state.epoch) return;
    state.checkout = result; renderCheckout(); notice(result.checkoutUrl ? "checkoutReady" : "completed");
    pollAfterCheckout();
  } catch (error) { if (epoch === state.epoch) errorNotice(error, true); }
  finally { if (epoch === state.epoch) setBusy(false); }
}
async function signOut(explicit = true) {
  const epoch = ++state.epoch; stopPoll(); state.loggedIn = false; state.scope = null;
  state.status = null; state.characterIndex = null; state.offerId = null; state.checkout = null;
  byId("password").value = ""; byId("account").value = ""; byId("account-name").textContent = "";
  byId("character").replaceChildren(); byId("signed-in").hidden = true; byId("login-form").hidden = false;
  await session.close();
  if (epoch !== state.epoch) return;
  setBusy(false); renderStatus(); notice(explicit ? "signedOut" : "sessionExpired", !explicit);
}
byId("login-form").addEventListener("submit", async event => {
  event.preventDefault(); if (state.busy) return;
  const account = byId("account").value.trim(), password = byId("password").value;
  if (!account || !password) return;
  const epoch = ++state.epoch; setBusy(true); notice("loading");
  try {
    const characters = await session.login(account, password);
    if (epoch !== state.epoch) return;
    const scope = await accountScope(session.accountId);
    if (epoch !== state.epoch || !session.token) return;
    state.scope = scope; state.loggedIn = true;
    byId("account-name").textContent = session.accountId;
    byId("password").value = ""; byId("account").value = "";
    byId("signed-in").hidden = false; byId("login-form").hidden = true;
    byId("character").replaceChildren();
    for (const character of characters) {
      const option = node("option", null, `${character.name} · Lv.${number(character.level)}`);
      option.value = String(character.index); byId("character").append(option);
    }
    byId("empty-characters").hidden = characters.length !== 0; byId("character").hidden = characters.length === 0;
    state.characterIndex = characters[0]?.index ?? null;
    setBusy(false);
    if (state.characterIndex !== null) await refresh(); else renderStatus();
  } catch (error) { if (epoch === state.epoch) errorNotice(error); }
  finally { byId("password").value = ""; if (epoch === state.epoch) setBusy(false); }
});
byId("character").addEventListener("change", async () => {
  ++state.epoch; stopPoll(); state.characterIndex = Number(byId("character").value);
  state.status = null; state.offerId = null; state.checkout = null; renderStatus(); await refresh();
});
byId("refresh").addEventListener("click", () => refresh());
byId("logout").addEventListener("click", () => signOut());
byId("new-order").addEventListener("click", () => {
  if (state.busy || !state.checkout || !window.confirm(t("newOrderConfirm"))) return;
  try {
    journal.clear(state.scope, state.characterIndex, state.offerId);
    state.checkout = null; state.offerId = null; stopPoll(); renderStatus();
  } catch (error) { errorNotice(error, true); }
});
byId("language").addEventListener("change", () => {
  locale = normalizeLocale(byId("language").value);
  try { localStorage.setItem("numeron-language", locale); } catch { /* Optional. */ }
  renderLocale();
});
document.addEventListener("visibilitychange", () => {
  if (!document.hidden && Date.now() - state.lastRefresh > 5000) refresh(false);
});
window.addEventListener("pagehide", () => { stopPoll(); session.close(); });
// Do not interpret ?paid, ?result, session IDs or any URL value as payment proof.
renderLocale();
