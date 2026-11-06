import assert from "node:assert/strict";
import test from "node:test";

import {
  isNewAccountTerminalOutcome,
  newAccountTerminalOutcome,
} from "./load-gateway-ws-outcome.mjs";

test("NewAccount serverError is immediately terminal", () => {
  const state = { serverError: "account service unavailable" };

  assert.equal(newAccountTerminalOutcome(state), "serverError");
  assert.equal(isNewAccountTerminalOutcome(state), true);
});

test("NewAccount capacity rejection is immediately terminal", () => {
  const state = { capacityRejected: true };

  assert.equal(newAccountTerminalOutcome(state), "capacity");
  assert.equal(isNewAccountTerminalOutcome(state), true);
});

test("NewAccount pending state is not terminal", () => {
  const state = {};

  assert.equal(newAccountTerminalOutcome(state), "pending");
  assert.equal(isNewAccountTerminalOutcome(state), false);
});

test("NewAccount success is terminal", () => {
  const state = { newAccountProcessed: true };

  assert.equal(newAccountTerminalOutcome(state), "success");
  assert.equal(isNewAccountTerminalOutcome(state), true);
});
