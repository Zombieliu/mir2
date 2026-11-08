import { createHash, createHmac, randomUUID, timingSafeEqual } from "node:crypto";

export const MAX_ARCHIVE_BYTES = 2 * 1024 * 1024;
const ENVELOPE_KEYS = ["schema", "version", "exportId", "exportedAt", "characterArchive", "playerGrowth", "integrity"];

export function plainRecord(value, keys, label) {
  if (!value || typeof value !== "object" || Array.isArray(value) || ![Object.prototype, null].includes(Object.getPrototypeOf(value)) ||
      Reflect.ownKeys(value).some(key => typeof key !== "string" || !keys.includes(key))) throw new TypeError(`${label} contains unsupported fields`);
  return value;
}

export function archiveId(value, label = "ID") {
  if (typeof value !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._:/@,+-]{0,99}$/.test(value)) throw new TypeError(`${label} must be a bounded ASCII reference`);
  return value;
}

export function nonnegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) throw new TypeError(`${label} must be a nonnegative safe integer`);
  return value;
}

/** One deterministic representation for signatures and idempotency hashes. */
export function canonicalJson(value) {
  const visit = (item, depth = 0) => {
    if (depth > 24) throw new TypeError("Archive nesting is too deep");
    if (item === null || typeof item === "string" || typeof item === "boolean") return item;
    if (typeof item === "number" && Number.isSafeInteger(item)) return item;
    if (Array.isArray(item)) return item.map(entry => visit(entry, depth + 1));
    if (item && typeof item === "object" && [Object.prototype, null].includes(Object.getPrototypeOf(item))) {
      const output = Object.create(null);
      for (const key of Reflect.ownKeys(item).sort()) {
        if (typeof key !== "string" || ["__proto__", "prototype", "constructor"].includes(key)) throw new TypeError("Unsupported archive key");
        output[key] = visit(item[key], depth + 1);
      }
      return output;
    }
    throw new TypeError("Unsupported archive value");
  };
  return JSON.stringify(visit(value));
}

export const payloadHash = value => createHash("sha256").update(canonicalJson(value)).digest("hex");

function signingKey(key) {
  if (!(typeof key === "string" || key instanceof Uint8Array)) throw new TypeError("An external archive signing key is required");
  const bytes = Buffer.from(key);
  if (bytes.length < 32 || bytes.length > 4096) throw new TypeError("Archive signing keys must contain 32 to 4096 bytes");
  return bytes;
}

export function signArchive({ characterArchive, playerGrowth, key }) {
  const payload = { schema: "mir2-companion-archive/1", version: 1, exportId: randomUUID(), exportedAt: Date.now(), characterArchive, playerGrowth };
  const signature = createHmac("sha256", signingKey(key)).update(canonicalJson(payload)).digest("hex");
  const encoded = canonicalJson({ ...payload, integrity: { algorithm: "HMAC-SHA256", signature } });
  if (Buffer.byteLength(encoded) > MAX_ARCHIVE_BYTES) throw new Error("Archive exceeds the 2 MiB export limit; history was not truncated");
  return encoded;
}

export function verifyArchive(envelope, key) {
  const bytes = signingKey(key);
  if (typeof envelope === "string" && Buffer.byteLength(envelope) > MAX_ARCHIVE_BYTES) throw new Error("Archive exceeds the 2 MiB import limit");
  const value = typeof envelope === "string" ? JSON.parse(envelope) : envelope;
  plainRecord(value, ENVELOPE_KEYS, "Archive envelope");
  if (Buffer.byteLength(canonicalJson(value)) > MAX_ARCHIVE_BYTES) throw new Error("Archive exceeds the 2 MiB import limit");
  if (value.schema !== "mir2-companion-archive/1" || value.version !== 1) throw new Error("Unsupported archive version");
  archiveId(value.exportId, "exportId"); nonnegativeInteger(value.exportedAt, "exportedAt");
  plainRecord(value.integrity, ["algorithm", "signature"], "Archive integrity");
  if (value.integrity.algorithm !== "HMAC-SHA256" || !/^[a-f0-9]{64}$/.test(value.integrity.signature)) throw new Error("Invalid archive integrity");
  const { integrity, ...payload } = value;
  const expected = createHmac("sha256", bytes).update(canonicalJson(payload)).digest();
  if (!timingSafeEqual(expected, Buffer.from(integrity.signature, "hex"))) throw new Error("Archive signature does not match the trusted owner key");
  return structuredClone(value);
}
