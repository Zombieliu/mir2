// Shared Crystal `.Lib` (WeMade MLibraryV2) reader + PNG encoder.
//
// Extracted verbatim from export-crystal-ui.mjs so multiple exporters (UI sprites,
// magic/spell effects, …) share one parser instead of duplicating the binary format.
// Pure functions, no side effects — safe to import. The `.Lib` layout (little-endian):
//   int32 version (>=2) · int32 count · [version>=3: int32 reserved] · int32 frameOffset[count]
//   per frame @offset: i16 width,height,x,y,shadowX,shadowY · u8 shadow · i32 len · gz(BGRA) · [mask]
// See Crystal/LibraryEditor/Graphics/MLibraryV2.cs for the authoritative writer.

import { deflateSync, gunzipSync } from "node:zlib";

export const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export function normalizeLibraryName(libraryName) {
  return String(libraryName)
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");
}

export function allPresentFrameIndices(library) {
  return library.frames
    .map((frame, index) => (frame ? index : null))
    .filter((index) => index !== null);
}

export function parseLibrary(buffer) {
  let offset = 0;

  const version = buffer.readInt32LE(offset);
  offset += 4;

  if (version < 2) {
    throw new Error(`Unsupported lib version ${version}`);
  }

  const count = buffer.readInt32LE(offset);
  offset += 4;

  if (version >= 3) {
    offset += 4;
  }

  const frameOffsets = [];
  for (let index = 0; index < count; index += 1) {
    frameOffsets.push(buffer.readInt32LE(offset));
    offset += 4;
  }

  const frames = new Array(count).fill(null);
  for (let index = 0; index < frameOffsets.length; index += 1) {
    const frameOffset = frameOffsets[index];
    if (frameOffset <= 0 || frameOffset >= buffer.length) {
      continue;
    }
    frames[index] = parseFrameHeader(buffer, frameOffset, index);
  }

  return { version, count, frames, buffer };
}

export function parseFrameHeader(buffer, offset, index) {
  const width = buffer.readInt16LE(offset);
  offset += 2;
  const height = buffer.readInt16LE(offset);
  offset += 2;
  const x = buffer.readInt16LE(offset);
  offset += 2;
  const y = buffer.readInt16LE(offset);
  offset += 2;
  const shadowX = buffer.readInt16LE(offset);
  offset += 2;
  const shadowY = buffer.readInt16LE(offset);
  offset += 2;
  const shadow = buffer.readUInt8(offset);
  offset += 1;
  const length = buffer.readInt32LE(offset);
  offset += 4;

  const dataOffset = offset;
  offset += length;

  const hasMask = (shadow >> 7) === 1;
  let maskWidth;
  let maskHeight;
  let maskX;
  let maskY;
  let maskLength;
  let maskDataOffset;

  if (hasMask) {
    maskWidth = buffer.readInt16LE(offset);
    offset += 2;
    maskHeight = buffer.readInt16LE(offset);
    offset += 2;
    maskX = buffer.readInt16LE(offset);
    offset += 2;
    maskY = buffer.readInt16LE(offset);
    offset += 2;
    maskLength = buffer.readInt32LE(offset);
    offset += 4;
    maskDataOffset = offset;
    offset += maskLength;
  }

  return {
    index,
    width,
    height,
    x,
    y,
    shadowX,
    shadowY,
    shadow,
    dataOffset,
    dataLength: length,
    maskWidth,
    maskHeight,
    maskX,
    maskY,
    maskLength,
    maskDataOffset,
    maskRgba: hasMask,
  };
}

export function decodeFrameRgba(library, frame) {
  return decodeFrame(
    frame.width,
    frame.height,
    library.buffer.subarray(frame.dataOffset, frame.dataOffset + frame.dataLength),
  );
}

export function decodeMaskFrameRgba(library, frame) {
  return decodeFrame(
    frame.maskWidth,
    frame.maskHeight,
    library.buffer.subarray(frame.maskDataOffset, frame.maskDataOffset + frame.maskLength),
  );
}

export function decodeFrame(width, height, compressed) {
  if (!width || !height) {
    return Buffer.alloc(0);
  }

  const bgra = gunzipSync(compressed);
  const rgba = Buffer.allocUnsafe(width * height * 4);

  for (let source = 0; source < bgra.length; source += 4) {
    const dest = source;
    rgba[dest] = bgra[source + 2];
    rgba[dest + 1] = bgra[source + 1];
    rgba[dest + 2] = bgra[source];
    rgba[dest + 3] = bgra[source + 3];
  }

  return rgba;
}

export function encodePng(width, height, rgba, deflateLevel = 1) {
  const raw = Buffer.allocUnsafe((width * 4 + 1) * height);

  for (let row = 0; row < height; row += 1) {
    const rawOffset = row * (width * 4 + 1);
    raw[rawOffset] = 0;
    rgba.copy(raw, rawOffset + 1, row * width * 4, (row + 1) * width * 4);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const idat = deflateSync(raw, { level: deflateLevel });
  return Buffer.concat([PNG_SIGNATURE, chunk("IHDR", ihdr), chunk("IDAT", idat), chunk("IEND", Buffer.alloc(0))]);
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type, "ascii");
  const lengthBuffer = Buffer.alloc(4);
  lengthBuffer.writeUInt32BE(data.length, 0);

  const crcBuffer = Buffer.concat([typeBuffer, data]);
  const checksum = crc32(crcBuffer);
  const crcOutput = Buffer.alloc(4);
  crcOutput.writeUInt32BE(checksum >>> 0, 0);

  return Buffer.concat([lengthBuffer, typeBuffer, data, crcOutput]);
}

function crc32(buffer) {
  let crc = 0xffffffff;

  for (let index = 0; index < buffer.length; index += 1) {
    crc ^= buffer[index];

    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1) === 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }

  return (crc ^ 0xffffffff) >>> 0;
}
