/** Preserve Crystal u64 identities before JSON.parse can round them. */
export function parseGatewayJson(text) {
  if (typeof text !== "string") throw new TypeError("Gateway JSON must be text");
  return JSON.parse(text, (_key, value, context) => {
    if (typeof value !== "number") return value;
    if (!Number.isFinite(value)) throw new SyntaxError("Non-finite gateway number");
    if (!Number.isInteger(value) || Number.isSafeInteger(value)) return value;
    const source = context?.source;
    if (typeof source !== "string" || !/^-?(?:0|[1-9]\d*)$/.test(source)) {
      throw new SyntaxError("Unsafe gateway number must be an exact integer literal");
    }
    return BigInt(source);
  });
}

/** BigInt becomes a JSON number literal; already-rounded Numbers fail closed. */
export function stringifyGatewayJson(value) {
  if (typeof JSON.rawJSON !== "function") {
    throw new Error("Lossless gateway JSON requires Node.js 22.18.0 or newer");
  }
  return JSON.stringify(value, (_key, entry) => {
    if (typeof entry === "bigint") return JSON.rawJSON(entry.toString());
    if (typeof entry === "number" && (
      !Number.isFinite(entry) || (Number.isInteger(entry) && !Number.isSafeInteger(entry))
    )) {
      throw new RangeError("Gateway numbers must be finite and integers must be safe Numbers or BigInt");
    }
    return entry;
  });
}
