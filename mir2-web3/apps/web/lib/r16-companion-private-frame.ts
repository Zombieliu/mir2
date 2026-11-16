/** Classify the owner-private envelope BEFORE the general packet decoder,
 * Hero ingress, World replica, diagnostics, or error logger sees it. Gateway
 * serializes payload/type in either order. Malformed private input is consumed
 * with no raw error text; ordinary frames retain their established decoder. */
export function readR16CompanionPrivateFrame(data: unknown):
  { private: false } | { private: true; event: unknown | null } {
  if (typeof data !== "string") return { private: false };
  const marker = data.includes('"companionResult"');
  // JSON permits escaped characters in both property names and type values.
  // Decode this rare case before ordinary ingress too; use the existing raw
  // frame ceiling, then the much smaller private-envelope ceiling below.
  if (!marker && !data.includes("\\u")) return { private: false };
  if (data.length > 16 * 1024 * 1024 || marker && data.length > 128 * 1024) return { private: true, event: null };
  try {
    const event: unknown = JSON.parse(data);
    if (!marker && (!event || typeof event !== "object" || Array.isArray(event)
      || (event as { type?: unknown }).type !== "companionResult")) return { private: false };
    // Fail closed for a private marker in an invalid envelope too. Its fields
    // must never be copied into generic history while rejecting the request.
    return { private: true, event: data.length > 128 * 1024 ? null : event };
  } catch {
    return { private: true, event: null };
  }
}
