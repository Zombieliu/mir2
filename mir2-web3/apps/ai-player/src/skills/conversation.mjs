/** Produce one normal nearby-chat line, never a Crystal command or macro. */
export function prepareNearbyChat(text, { maxLength = 160 } = {}) {
  if (typeof text !== "string") throw new TypeError("Nearby chat must be text");
  if (!Number.isSafeInteger(maxLength) || maxLength < 1 || maxLength > 1000) {
    throw new RangeError("Nearby chat maxLength must be an integer from 1 to 1000");
  }
  // Reject before trim as well: a trailing newline must never disappear silently.
  const forbiddenControls = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}\p{Cs}]/u;
  if (forbiddenControls.test(text)) throw new Error("Nearby chat cannot contain control characters or line breaks");
  const prepared = text.normalize("NFKC").trim();
  if (!prepared) throw new Error("Nearby chat cannot be empty");
  if (forbiddenControls.test(prepared)) throw new Error("Nearby chat contains forbidden characters");
  if (/^(?:[@/!]|:\))/u.test(prepared)) {
    throw new Error("Nearby chat cannot use command or channel prefixes");
  }
  if (/\$\s*pos\b/iu.test(prepared) || /[<>]|\$\{|\{\{|\[\s*item\s*:|item:\/\//iu.test(prepared)) {
    throw new Error("Nearby chat cannot contain position or item-link interpolation");
  }
  if ([...prepared].length > maxLength) throw new RangeError("Nearby chat exceeds maxLength");
  return prepared;
}
