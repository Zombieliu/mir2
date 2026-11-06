// The opt-in native catalog transport preserves every original Text envelope.
// Authentication, player state and live actions are never legal batch contents.
import { crc32, inflateRawSync } from 'node:zlib';

export const CATALOG_GZIP_CAPABILITY = 'serverCatalogGzipV1';
export const MAX_CATALOG_DECODED_BYTES = 128 * 1024;
export const MAX_CATALOG_ENVELOPES = 64;
export const MAX_CATALOG_WIRE_BYTES = 132 * 1024;
const MAGIC = Buffer.from('M2CATGZ1');
const CATALOG_PACKETS = new Set(['NewItemInfo', 'NewRecipeInfo', 'GameShopInfo', 'NewQuestInfo']);
const utf8 = new TextDecoder('utf-8', { fatal: true });
const invalid = () => new Error('Invalid bounded gateway catalog batch');

function gzipBodyOffset(bytes) {
  if (bytes.length < 18 || bytes[0] !== 31 || bytes[1] !== 139 || bytes[2] !== 8 || (bytes[3] & 224)) throw invalid();
  const flags = bytes[3]; let offset = 10;
  if (flags & 4) {
    if (offset + 2 > bytes.length - 8) throw invalid();
    const length = bytes.readUInt16LE(offset); offset += 2 + length;
    if (offset > bytes.length - 8) throw invalid();
  }
  for (const flag of [8, 16]) if (flags & flag) {
    const end = bytes.indexOf(0, offset);
    if (end < offset || end >= bytes.length - 8) throw invalid();
    offset = end + 1;
  }
  if (flags & 2) {
    if (offset + 2 > bytes.length - 8 || bytes.readUInt16LE(offset) !== (crc32(bytes.subarray(0, offset)) & 65535)) throw invalid();
    offset += 2;
  }
  return offset;
}

export function decodeCatalogBatch(input) {
  try {
    const bytes = Buffer.isBuffer(input) ? input : input instanceof ArrayBuffer ? Buffer.from(input)
      : ArrayBuffer.isView(input) ? Buffer.from(input.buffer, input.byteOffset, input.byteLength) : null;
    if (!bytes || bytes.length < 34 || bytes.length > MAX_CATALOG_WIRE_BYTES || !bytes.subarray(0, 8).equals(MAGIC)) throw invalid();
    const length = bytes.readUInt32LE(8), count = bytes.readUInt32LE(12);
    if (!length || length > MAX_CATALOG_DECODED_BYTES || !count || count > MAX_CATALOG_ENVELOPES || length < count * 5) throw invalid();
    const gzip = bytes.subarray(16), offset = gzipBodyOffset(gzip);
    const result = inflateRawSync(gzip.subarray(offset), { maxOutputLength: length, info: true });
    const body = result.buffer, trailer = offset + result.engine.bytesWritten;
    // A single complete member only: padding, appended garbage and concatenated
    // gzip members cannot smuggle an unvalidated second stream into the batch.
    if (trailer + 8 !== gzip.length || body.length !== length ||
        gzip.readUInt32LE(trailer) !== crc32(body) || gzip.readUInt32LE(trailer + 4) !== body.length) throw invalid();
    const messages = []; let cursor = 0;
    for (let index = 0; index < count; index++) {
      if (cursor + 4 > body.length) throw invalid();
      const textLength = body.readUInt32LE(cursor); cursor += 4;
      if (!textLength || cursor + textLength > body.length) throw invalid();
      const message = JSON.parse(utf8.decode(body.subarray(cursor, cursor + textLength))); cursor += textLength;
      if (!message || message.type !== 'packet' || !CATALOG_PACKETS.has(message.packet) ||
          !message.payload || typeof message.payload !== 'object' || Array.isArray(message.payload)) throw invalid();
      messages.push(message);
    }
    if (cursor !== body.length) throw invalid();
    return messages;
  } catch {
    // A decompressor/JSON exception can quote input. Never expose packet data.
    throw invalid();
  }
}

export function decodePlaytestFrame(data, optedIn = false) {
  if (typeof data === 'string') {
    try { return [JSON.parse(data)]; }
    catch { throw new Error('Invalid gateway text frame'); }
  }
  if (!optedIn) throw new Error('Unnegotiated gateway binary frame');
  return decodeCatalogBatch(data);
}
