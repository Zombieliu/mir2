// Public nativeHpRecovery status counts, never a gameplay success decision.
export const HP_MAINTENANCE_STATUSES = Object.freeze([
  'not-sent', 'sent', 'send-failed', 'verified', 'cancelled',
  'owner-changed', 'owner-dead', 'public-error', 'rejected', 'timeout',
]);
const allowed = new Set(HP_MAINTENANCE_STATUSES);
export function isHpMaintenanceStatusCounts(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return false;
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string' || !allowed.has(key)) return false;
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor?.enumerable || !Object.hasOwn(descriptor, 'value')) return false;
    const count = descriptor.value;
    if (!Number.isSafeInteger(count) || count < 0 || Object.is(count, -0)) return false;
  }
  return true;
}
export function isHpMaintenanceEvidence(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
  if (!isHpMaintenanceStatusCounts(value.observedStatuses)) return false;
  const checked = value.verifiedEventsChecked;
  return Number.isSafeInteger(checked) && checked >= 0 && !Object.is(checked, -0) &&
    checked === (value.observedStatuses.verified ?? 0) &&
    value.successProofChecked === (checked > 0) &&
    value.noVerifiedDoseObserved === (checked === 0);
}
