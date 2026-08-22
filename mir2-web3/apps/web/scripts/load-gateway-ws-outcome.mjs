/**
 * Classify the outcome of the NewAccount handshake without performing I/O.
 *
 * The precedence is intentional: capacity and server errors are terminal
 * failures even when a late success reply has already been observed.
 */
export function newAccountTerminalOutcome({
  newAccountProcessed = false,
  capacityRejected = false,
  serverError = null,
} = {}) {
  if (capacityRejected) return "capacity";
  if (serverError !== null) return "serverError";
  if (newAccountProcessed) return "success";
  return "pending";
}

export function isNewAccountTerminalOutcome(state) {
  return newAccountTerminalOutcome(state) !== "pending";
}
