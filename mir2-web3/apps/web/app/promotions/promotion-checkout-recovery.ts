/** Non-secret checkout intent recovery. Never persist a bearer, capability URL or signature. */
export type CheckoutOrder = {
  id: string; orderRequestId: string; offerId: string; state: string; transactionDigest?: string | null;
};
export type CheckoutIntent = { orderRequestId: string; offerId: string; orderId?: string; state?: string };
export type CheckoutStorage = Pick<Storage, "getItem" | "setItem" | "removeItem" | "key" | "length">;
export type CheckoutScope = { credentialId: string; characterIndex: number; authority?: "windows" };
const PREFIX = "numeron-promotion-recharge-intent-v1:";
const REQUEST = /^[A-Za-z0-9_-]{16,128}$/;
const OFFER = /^[A-Za-z0-9_-]{1,128}$/;
function prefix(scope: CheckoutScope): string {
  if (!scope.credentialId || scope.credentialId.length > 256 || !Number.isSafeInteger(scope.characterIndex) || scope.characterIndex < 0) {
    throw new Error("checkout_wallet_scope_required");
  }
  // Leave existing recovery keys readable; the original Windows ledger has a
  // separate namespace even when wallet credential and character indices match.
  if (scope.authority !== undefined && scope.authority !== "windows") throw new Error("checkout_authority_invalid");
  return `${PREFIX}${scope.authority === "windows" ? "windows-v1:" : ""}${encodeURIComponent(scope.credentialId)}:${scope.characterIndex}:`;
}
function terminal(state: string): boolean { return state === "applied" || state === "expired"; }
function valid(order: CheckoutOrder): boolean {
  return typeof order.orderRequestId === "string" && REQUEST.test(order.orderRequestId) && typeof order.offerId === "string" && OFFER.test(order.offerId);
}
function storageKey(scope: CheckoutScope, offerId: string): string {
  if (!OFFER.test(offerId)) throw new Error("checkout_offer_invalid");
  return prefix(scope) + offerId;
}
export function rememberCheckoutIntent(storage: CheckoutStorage, scope: CheckoutScope, intent: CheckoutIntent): void {
  if (!REQUEST.test(intent.orderRequestId)) throw new Error("checkout_intent_invalid");
  const key = storageKey(scope, intent.offerId);
  storage.setItem(key, intent.orderRequestId);
  // A denied/quota-limited storage write must fail before a new server request.
  if (storage.getItem(key) !== intent.orderRequestId) throw new Error("checkout_recovery_unavailable");
}

/** An empty/temporarily stale status never proves an unknown request failed. */
export function recoverCheckoutIntents(storage: CheckoutStorage, scope: CheckoutScope, orders: readonly CheckoutOrder[]): CheckoutIntent[] {
  const start = prefix(scope);
  const saved = new Map<string, string>();
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (!key?.startsWith(start)) continue;
    const offerId = key.slice(start.length), request = storage.getItem(key);
    if (OFFER.test(offerId) && request && REQUEST.test(request)) saved.set(offerId, request);
  }
  const result: CheckoutIntent[] = [];
  for (const order of orders) {
    if (!valid(order)) {
      if (!terminal(order.state)) throw new Error("checkout_order_recovery_unavailable");
      continue;
    }
    if (terminal(order.state)) {
      if (saved.get(order.offerId) === order.orderRequestId) {
        storage.removeItem(storageKey(scope, order.offerId)); saved.delete(order.offerId);
      }
      continue;
    }
    const intent = { orderRequestId: order.orderRequestId, offerId: order.offerId, orderId: order.id, state: order.state };
    result.push(intent);
    // Retain an earlier unknown request for the same offer as well as all
    // server orders; never erase one unresolved intent in favour of another.
    if (!saved.has(order.offerId)) { rememberCheckoutIntent(storage, scope, intent); saved.set(order.offerId, order.orderRequestId); }
  }
  for (const [offerId, orderRequestId] of saved) {
    if (!result.some(value => value.orderRequestId === orderRequestId && value.offerId === offerId)) result.push({ offerId, orderRequestId });
  }
  return result;
}

export function prepareCheckoutIntent(storage: CheckoutStorage, scope: CheckoutScope, orders: readonly CheckoutOrder[], offerId: string,
  originalRequestId?: string, createId: () => string = () => crypto.randomUUID()): CheckoutIntent {
  const unresolved = recoverCheckoutIntents(storage, scope, orders);
  let intent: CheckoutIntent;
  if (originalRequestId !== undefined) {
    const existing = unresolved.find(value => value.offerId === offerId && value.orderRequestId === originalRequestId);
    if (!existing) throw new Error("checkout_original_intent_required");
    intent = existing;
  } else {
    if (unresolved.length) throw new Error("checkout_unresolved_intent");
    intent = { offerId, orderRequestId: createId() };
  }
  // This key remains after success, timeout and reload until a matching
  // authoritative terminal order is observed. Payment status is never inferred.
  const existing = storage.getItem(storageKey(scope, intent.offerId));
  if (!existing || !REQUEST.test(existing) || existing === intent.orderRequestId) rememberCheckoutIntent(storage, scope, intent);
  return intent;
}
