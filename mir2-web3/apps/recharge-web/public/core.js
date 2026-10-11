// Shared by the browser and protocol tests. No payment authority lives here.
export const CONFIG = Object.freeze({
  websocket: "wss://165.154.65.136.sslip.io/playtest/ws",
  api: "/playtest/v1/billing",
  revoke: "/playtest/v1/identity/sessions/revoke",
  testMode: true,
  // Informational preview only; payment is disabled until the authenticated
  // server catalogue supplies the actual quote and offer ID.
  previewOffers: Object.freeze([100, 500, 1000, 2000].map(credits => Object.freeze({
    id: `preview-${credits}`, currency: "usd", amountMinor: credits, credits,
  }))),
});
export class PortalError extends Error {
  constructor(code) { super(code); this.code = code; }
}
function requireValue(condition, code = "invalidResponse") {
  if (!condition) throw new PortalError(code);
}
export function checkedCheckoutUrl(raw) {
  requireValue(typeof raw === "string" && raw.length <= 4096
    && !/[\s\\\u0000-\u001f\u007f]/u.test(raw), "invalidCheckoutUrl");
  let url;
  try { url = new URL(raw); } catch { throw new PortalError("invalidCheckoutUrl"); }
  requireValue(url.protocol === "https:" && url.hostname === "checkout.stripe.com"
    && !url.username && !url.password && (!url.port || url.port === "443")
    && url.pathname.startsWith("/c/pay/") && url.pathname.length > 7, "invalidCheckoutUrl");
  return url.href;
}
export function checkedCharacters(raw) {
  requireValue(Array.isArray(raw) && raw.length <= 100);
  const seen = new Set();
  return raw.map(character => {
    requireValue(character && Number.isSafeInteger(character.index) && character.index >= 0
      && character.index <= 2147483647 && typeof character.name === "string"
      && character.name.length > 0 && character.name.length <= 100 && !seen.has(character.index));
    seen.add(character.index);
    return {index: character.index, name: character.name,
      level: Number.isSafeInteger(character.level) ? character.level : 0,
      class: String(character.class ?? "")};
  });
}
export function checkedStatus(raw) {
  requireValue(raw && typeof raw.checkoutEnabled === "boolean"
    && typeof raw.enabled === "boolean" && Number.isInteger(raw.credit) && raw.credit >= 0
    && raw.credit <= 4294967295 && Number.isInteger(raw.pendingCredit)
    && raw.pendingCredit >= 0 && raw.pendingCredit <= 4294967295);
  requireValue(Array.isArray(raw.offers) && raw.offers.length <= 12
    && Array.isArray(raw.orders) && raw.orders.length <= 20 && raw.monthlyCard);
  const seen = new Set();
  const offers = raw.offers.map(offer => {
    requireValue(offer && typeof offer.id === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(offer.id)
      && !seen.has(offer.id) && /^[a-z]{3}$/.test(offer.currency)
      && Number.isSafeInteger(offer.amountMinor) && offer.amountMinor > 0
      && Number.isInteger(offer.credits) && offer.credits > 0 && offer.credits <= 4294967295);
    seen.add(offer.id);
    return {id: offer.id, currency: offer.currency, amountMinor: offer.amountMinor,
      amountLabel: typeof offer.amountLabel === "string" ? offer.amountLabel : "",
      credits: offer.credits};
  });
  const states = new Set(["prepared", "checkoutBound", "paidPending", "applied", "review"]);
  const orders = raw.orders.map(order => {
    requireValue(order && typeof order.id === "string" && order.id.length <= 160
      && states.has(order.state));
    return {id: order.id, state: order.state};
  });
  const monthly = raw.monthlyCard;
  requireValue(typeof monthly.active === "boolean" && Number.isSafeInteger(monthly.serverNowMs)
    && monthly.serverNowMs > 0 && (monthly.expiresAtMs === null
      || Number.isSafeInteger(monthly.expiresAtMs) && monthly.expiresAtMs > 0));
  return {enabled: raw.enabled, checkoutEnabled: raw.checkoutEnabled,
    credit: raw.credit, pendingCredit: raw.pendingCredit, offers, orders,
    monthlyCardCreditPrice: Number.isInteger(raw.monthlyCardCreditPrice) ? raw.monthlyCardCreditPrice : null,
    monthlyCard: {active: monthly.active, serverNowMs: monthly.serverNowMs, expiresAtMs: monthly.expiresAtMs}};
}
export function checkoutBody(characterIndex, offerId, orderRequestId, requestId) {
  requireValue(Number.isInteger(characterIndex) && characterIndex >= 0
    && characterIndex <= 2147483647 && /^[A-Za-z0-9_-]{1,64}$/.test(offerId)
    && /^web-[a-f0-9-]{36}$/.test(orderRequestId)
    && Number.isSafeInteger(requestId) && requestId > 0, "invalidIntent");
  return {characterIndex, requestId, orderRequestId, offerId};
}
export async function accountScope(accountId, origin = CONFIG.websocket) {
  requireValue(typeof accountId === "string" && accountId.length > 0);
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(`${origin}\n${accountId}`));
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
}
// Only an unprivileged correlation ID is persisted, never a password, bearer,
// Checkout URL, claimed amount or payment state. Failed requests retain the ID.
export class IntentJournal {
  constructor(storage, uuid = () => crypto.randomUUID()) { this.storage = storage; this.uuid = uuid; }
  key(scope, characterIndex, offerId) { return `numeron-checkout-v1:${scope}:${characterIndex}:${offerId}`; }
  getOrCreate(scope, characterIndex, offerId) {
    const key = this.key(scope, characterIndex, offerId);
    let value;
    try { value = this.storage.getItem(key); } catch { throw new PortalError("storageUnavailable"); }
    if (value !== null && !/^web-[a-f0-9-]{36}$/.test(value)) throw new PortalError("invalidIntent");
    if (!value) {
      value = `web-${this.uuid()}`;
      requireValue(/^web-[a-f0-9-]{36}$/.test(value), "invalidIntent");
      try { this.storage.setItem(key, value); } catch { throw new PortalError("storageUnavailable"); }
    }
    return value;
  }
  clear(scope, characterIndex, offerId) {
    try { this.storage.removeItem(this.key(scope, characterIndex, offerId)); }
    catch { throw new PortalError("storageUnavailable"); }
  }
}
export class PortalSession {
  constructor({WebSocketClass = globalThis.WebSocket, fetcher = (...args) => globalThis.fetch(...args), onDisconnect = () => {}} = {}) {
    this.WebSocketClass = WebSocketClass; this.fetcher = fetcher; this.onDisconnect = onDisconnect;
    this.socket = null; this.token = null; this.sessionId = null; this.accountId = null; this.characters = [];
    this.waiters = new Map(); this.requestId = 0; this.generation = 0; this.timer = null;
  }
  async login(accountId, password) {
    await this.close();
    const generation = ++this.generation;
    const socket = new this.WebSocketClass(CONFIG.websocket);
    this.socket = socket;
    let characters = null, grant = null;
    try {
      const result = await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new PortalError("connectionTimeout")), 25000);
        const fail = code => { clearTimeout(timeout); reject(new PortalError(code)); };
        socket.onopen = () => {
          if (generation !== this.generation) { socket.close(); return; }
          socket.send(JSON.stringify({type: "login", accountId: accountId.trim(), password}));
          password = "";
          this.timer = setInterval(() => {
            if (socket.readyState === 1) socket.send(JSON.stringify({type: "keepAlive", time: Date.now()}));
          }, 2000);
        };
        socket.onmessage = event => {
          if (generation !== this.generation) return;
          let value;
          try {
            if (typeof event.data !== "string" || event.data.length > 2000000) return;
            value = JSON.parse(event.data);
            if (value.type === "packet" && ["Login", "LoginBanned"].includes(value.packet)) {
              fail(value.packet === "LoginBanned" ? "accountBanned" : "loginFailed"); return;
            }
            if (value.type === "error") { fail("serviceUnavailable"); return; }
            if (value.type === "packet" && value.packet === "LoginSuccess") characters = checkedCharacters(value.payload?.characters);
            if (value.type === "identitySession") {
              const session = value.session;
              requireValue(session && session.accountId === accountId.trim() && session.authMethod === "password"
                && session.revokedAtMs === null && session.expiresAtMs > Date.now()
                && typeof session.sessionId === "string" && session.sessionId.length > 0 && session.sessionId.length <= 128
                && typeof value.token === "string" && value.token.length > 20, "loginFailed");
              grant = {accountId: session.accountId, sessionId: session.sessionId, token: value.token};
            }
            if (characters && grant) { clearTimeout(timeout); resolve({characters, ...grant}); }
            if (value.type === "billing") {
              const reply = value.payload, waiter = this.waiters.get(reply?.requestId);
              if (waiter && reply.characterIndex === waiter.characterIndex && reply.operation === "status") {
                this.waiters.delete(reply.requestId); clearTimeout(waiter.timeout);
                if (reply.error) waiter.reject(new PortalError(reply.error));
                else {
                  try { waiter.resolve(checkedStatus(reply.status)); }
                  catch { waiter.reject(new PortalError("invalidResponse")); }
                }
              }
            }
          } catch { fail("invalidResponse"); }
        };
        socket.onerror = () => fail("serviceUnavailable");
        socket.onclose = () => {
          if (generation !== this.generation) return;
          this.token = null; this.sessionId = null; this.accountId = null; clearInterval(this.timer);
          for (const waiter of this.waiters.values()) { clearTimeout(waiter.timeout); waiter.reject(new PortalError("sessionExpired")); }
          this.waiters.clear(); fail("sessionExpired"); this.onDisconnect();
        };
      });
      requireValue(generation === this.generation, "sessionExpired");
      this.token = result.token; this.sessionId = result.sessionId; this.accountId = result.accountId; this.characters = result.characters;
      return this.characters;
    } catch (error) { await this.close(); throw error; }
  }
  owned(characterIndex) {
    requireValue(this.token && this.characters.some(character => character.index === characterIndex), "sessionExpired");
  }
  async api(path, options = {}) {
    requireValue(this.token, "sessionExpired");
    const generation = this.generation;
    const controller = new AbortController(), timer = setTimeout(() => controller.abort(), 25000);
    try {
      const response = await this.fetcher(`${CONFIG.api}${path}`, {...options, signal: controller.signal,
        credentials: "omit", cache: "no-store", redirect: "error",
        headers: {Authorization: `Bearer ${this.token}`, Accept: "application/json", ...(options.body ? {"Content-Type": "application/json"} : {})}});
      requireValue(generation === this.generation && this.token, "sessionExpired");
      const text = await response.text(); requireValue(text.length <= 100000, "invalidResponse");
      let body;
      try { body = JSON.parse(text); } catch { throw new PortalError("invalidResponse"); }
      if (!response.ok || body.error) throw new PortalError(typeof body.error === "string" ? body.error : "serviceUnavailable");
      return body;
    } catch (error) {
      if (error instanceof PortalError) throw error;
      throw new PortalError("connectionTimeout");
    } finally { clearTimeout(timer); }
  }
  async status(characterIndex) {
    this.owned(characterIndex);
    return checkedStatus(await this.api(`/status?characterIndex=${characterIndex}&requestId=${++this.requestId}`));
  }
  async applyPending(characterIndex) {
    this.owned(characterIndex);
    requireValue(this.socket?.readyState === 1, "sessionExpired");
    const requestId = ++this.requestId;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => { this.waiters.delete(requestId); reject(new PortalError("connectionTimeout")); }, 25000);
      this.waiters.set(requestId, {resolve, reject, timeout, characterIndex});
      this.socket.send(JSON.stringify({type: "billingStatus", characterIndex, requestId}));
    });
  }
  async checkout(characterIndex, offerId, orderRequestId) {
    this.owned(characterIndex);
    const body = checkoutBody(characterIndex, offerId, orderRequestId, ++this.requestId);
    const result = await this.api("/checkout", {method: "POST", body: JSON.stringify(body)});
    requireValue(typeof result.orderId === "string" && result.orderId.length <= 160);
    return {orderId: result.orderId, checkoutUrl: result.checkoutUrl === null ? null : checkedCheckoutUrl(result.checkoutUrl)};
  }
  async close() {
    const token = this.token, sessionId = this.sessionId, socket = this.socket;
    ++this.generation; this.token = null; this.sessionId = null; this.accountId = null; this.characters = [];
    clearInterval(this.timer); this.timer = null;
    for (const waiter of this.waiters.values()) { clearTimeout(waiter.timeout); waiter.reject(new PortalError("sessionExpired")); }
    this.waiters.clear();
    this.socket = null;
    // Crystal selection/Disconnect is not identity logout. Revoke exactly the
    // website grant, never the player's other game/browser sessions. Keepalive
    // lets the bounded request finish during ordinary page navigation.
    if (token && sessionId) {
      const controller = new AbortController(), timer = setTimeout(() => controller.abort(), 5000);
      try {
        await this.fetcher(CONFIG.revoke, {method: "POST", credentials: "omit", cache: "no-store",
          redirect: "error", keepalive: true, signal: controller.signal,
          headers: {Authorization: `Bearer ${token}`, "Content-Type": "application/json"},
          body: JSON.stringify({sessionId, reason: "recharge_website_sign_out"})});
      } catch { /* Memory credentials are still cleared when the network fails. */ }
      finally { clearTimeout(timer); }
    }
    if (socket) {
      try { if (socket.readyState === 1) socket.send(JSON.stringify({type: "disconnect"})); }
      catch { /* A closing transport already has no usable portal credentials. */ }
      socket.close();
    }
  }
}
