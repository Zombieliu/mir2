import test from 'node:test';
import assert from 'node:assert/strict';
import {HP_MAINTENANCE_STATUSES, isHpMaintenanceStatusCounts, isHpMaintenanceEvidence} from './formal-native-v2-compact-wire-ladder-10/hp-maintenance-status-schema.mjs';

test('zero observed HP events is a valid empty dictionary, without a survival claim', () => {
  const counts = {};
  assert.equal(isHpMaintenanceStatusCounts(counts), true);
  assert.deepEqual(counts, {});
});
test('all ten actual producer statuses accept nonnegative safe integer counts unchanged', () => {
  assert.deepEqual(HP_MAINTENANCE_STATUSES, ['not-sent','sent','send-failed','verified','cancelled','owner-changed','owner-dead','public-error','rejected','timeout']);
  const counts = Object.fromEntries(HP_MAINTENANCE_STATUSES.map((key, i) => [key, i]));
  counts.verified = Number.MAX_SAFE_INTEGER;
  const before = JSON.stringify(counts);
  assert.equal(isHpMaintenanceStatusCounts(counts), true);
  assert.equal(JSON.stringify(counts), before);
});
test('actual verified status is accepted independently of any dose proof or game verdict', () => {
  assert.equal(isHpMaintenanceStatusCounts({sent: 3, verified: 2, timeout: 1}), true);
  assert.equal(isHpMaintenanceStatusCounts({success: 2}), false);
});
test('arrays and primitive values cannot substitute for the producer dictionary', () => {
  for (const value of [[], [1], null, undefined, false, 0, 'verified', 1n]) assert.equal(isHpMaintenanceStatusCounts(value), false);
});
test('unknown and prototype-shaped status keys fail closed', () => {
  for (const counts of [{Verified: 1}, {unknown: 0}, {'': 0}, JSON.parse('{"__proto__":0}'), {constructor: 0}]) assert.equal(isHpMaintenanceStatusCounts(counts), false);
});
test('negative, noncanonical or unsafe counts cannot pass as observed events', () => {
  for (const count of [-1, -0, 0.5, NaN, Infinity, -Infinity, Number.MAX_SAFE_INTEGER + 1, '1', true, null, undefined, {}, []]) assert.equal(isHpMaintenanceStatusCounts({verified: count}), false);
});
test('only own data properties are accepted, without invoking accessor code', () => {
  let called = false;
  const accessor = {get verified() {called = true; return 1;}};
  assert.equal(isHpMaintenanceStatusCounts(accessor), false);
  assert.equal(called, false);
  assert.equal(isHpMaintenanceStatusCounts(Object.create({verified: 1})), false);
  assert.equal(isHpMaintenanceStatusCounts({[Symbol('verified')]: 1}), false);
  assert.equal(isHpMaintenanceStatusCounts(Object.defineProperty({}, 'verified', {value: 1})), false);
});
test('frozen and null-prototype JSON-shaped dictionaries remain valid without mutation', () => {
  const frozen = Object.freeze({verified: 0, 'not-sent': 4});
  assert.equal(isHpMaintenanceStatusCounts(frozen), true);
  assert.deepEqual(frozen, {verified: 0, 'not-sent': 4});
  const nullPrototype = Object.assign(Object.create(null), {sent: 1});
  assert.equal(isHpMaintenanceStatusCounts(nullPrototype), true);
});
test('zero verified doses is valid evidence with explicit absence of a dose-proof claim', () => {
  assert.equal(isHpMaintenanceEvidence({observedStatuses: {}, verifiedEventsChecked: 0, successProofChecked: false, noVerifiedDoseObserved: true}), true);
  assert.equal(isHpMaintenanceEvidence({observedStatuses: {sent: 1, timeout: 1}, verifiedEventsChecked: 0, successProofChecked: false, noVerifiedDoseObserved: true}), true);
});
test('positive verified evidence requires the exact count and both consistent proof flags', () => {
  const evidence = Object.freeze({observedStatuses: Object.freeze({sent: 3, verified: 2, timeout: 1}), verifiedEventsChecked: 2, successProofChecked: true, noVerifiedDoseObserved: false});
  assert.equal(isHpMaintenanceEvidence(evidence), true);
  for (const change of [{verifiedEventsChecked: 1}, {verifiedEventsChecked: 3}, {successProofChecked: false}, {noVerifiedDoseObserved: true}]) assert.equal(isHpMaintenanceEvidence({...evidence, ...change}), false);
});
test('missing or invalid verified proof counters and vacuous success claims are rejected', () => {
  const empty = {observedStatuses: {}, verifiedEventsChecked: 0, successProofChecked: false, noVerifiedDoseObserved: true};
  for (const count of [undefined, -1, -0, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, '0', null]) assert.equal(isHpMaintenanceEvidence({...empty, verifiedEventsChecked: count}), false);
  for (const change of [{successProofChecked: true}, {noVerifiedDoseObserved: false}, {successProofChecked: 0}, {noVerifiedDoseObserved: 'true'}, {observedStatuses: []}]) assert.equal(isHpMaintenanceEvidence({...empty, ...change}), false);
});
