// Shared Crystal `.Lib` (WeMade MLibraryV2) reader + PNG encoder.
//
// Version 3 stores a FrameSet seek after the frame count. The action records at that seek
// define original timing, direction stride, effects, reverse playback, and blend behavior.
// See Crystal/LibraryEditor/Graphics/MLibraryV2.cs and Graphics/Frames.cs.

import { deflateSync, gunzipSync } from "node:zlib";

export const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export const MIR_ACTION_NAMES = Object.freeze([
  "Standing",
  "Walking",
  "Running",
  "Pushed",
  "DashL",
  "DashR",
  "DashFail",
  "Stance",
  "Stance2",
  "Attack1",
  "Attack2",
  "Attack3",
  "Attack4",
  "Attack5",
  "AttackRange1",
  "AttackRange2",
  "AttackRange3",
  "Special",
  "Struck",
  "Harvest",
  "Spell",
  "Die",
  "Dead",
  "Skeleton",
  "Show",
  "Hide",
  "Stoned",
  "Appear",
  "Revive",
  "SitDown",
  "Mine",
  "Sneek",
  "DashAttack",
  "Lunge",
  "WalkingBow",
  "RunningBow",
  "Jump",
  "MountStanding",
  "MountWalking",
  "MountRunning",
  "MountStruck",
  "MountAttack",
  "FishingCast",
  "FishingWait",
  "FishingReel",
]);

export const CRYSTAL_FRAME_SET_RECORD_BYTES = 35;

const LIBRARY_V2_HEADER_BYTES = 8;
const LIBRARY_V3_HEADER_BYTES = 12;
const IMAGE_HEADER_BYTES = 17;
const MASK_HEADER_BYTES = 12;

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
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError("Crystal library input must be a Buffer");
  }

  assertBufferRange(buffer, 0, LIBRARY_V2_HEADER_BYTES, "library header");
  let offset = 0;

  const version = buffer.readInt32LE(offset);
  offset += 4;

  if (version < 2) {
    throw new Error(`Unsupported lib version ${version}`);
  }

  const count = buffer.readInt32LE(offset);
  offset += 4;

  if (count < 0) {
    throw new Error(`Invalid Crystal library frame count ${count}`);
  }

  const headerBytes = (version >= 3 ? LIBRARY_V3_HEADER_BYTES : LIBRARY_V2_HEADER_BYTES) + count * 4;
  if (!Number.isSafeInteger(headerBytes)) {
    throw new Error(`Crystal library header size is not safe for frame count ${count}`);
  }
  assertBufferRange(buffer, 0, headerBytes, "library frame-offset table");

  let frameSeek = null;
  if (version >= 3) {
    frameSeek = buffer.readInt32LE(offset);
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
    if (frameOffset < headerBytes) {
      throw new Error(`Crystal frame ${index} points inside the library header (${frameOffset} < ${headerBytes})`);
    }
    frames[index] = parseFrameHeader(buffer, frameOffset, index);
  }

  if (frameSeek !== null && frameSeek !== 0 && frameSeek < headerBytes) {
    throw new Error(`Crystal FrameSet points inside the library header (${frameSeek} < ${headerBytes})`);
  }
  const frameSet = version >= 3
    ? parseFrameSet(buffer, frameSeek)
    : { seek: null, count: 0, actions: [] };

  return { version, count, frames, buffer, frameSeek, frameSet };
}

export function parseFrameHeader(buffer, offset, index) {
  assertBufferRange(buffer, offset, IMAGE_HEADER_BYTES, `frame ${index} header`);
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

  if (length < 0) {
    throw new Error(`Crystal frame ${index} has a negative data length ${length}`);
  }

  const dataOffset = offset;
  assertBufferRange(buffer, dataOffset, length, `frame ${index} image data`);
  offset += length;

  const hasMask = (shadow >> 7) === 1;
  let maskWidth;
  let maskHeight;
  let maskX;
  let maskY;
  let maskLength;
  let maskDataOffset;

  if (hasMask) {
    assertBufferRange(buffer, offset, MASK_HEADER_BYTES, `frame ${index} mask header`);
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
    if (maskLength < 0) {
      throw new Error(`Crystal frame ${index} has a negative mask data length ${maskLength}`);
    }
    maskDataOffset = offset;
    assertBufferRange(buffer, maskDataOffset, maskLength, `frame ${index} mask data`);
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

export function parseFrameSet(buffer, frameSeek) {
  if (frameSeek === 0) {
    return { seek: 0, count: 0, actions: [] };
  }
  if (!Number.isInteger(frameSeek) || frameSeek < 0) {
    throw new Error(`Invalid Crystal FrameSet seek ${frameSeek}`);
  }

  assertBufferRange(buffer, frameSeek, 4, "FrameSet count");
  const count = buffer.readInt32LE(frameSeek);
  if (count < 0) {
    throw new Error(`Invalid Crystal FrameSet action count ${count}`);
  }

  const actions = parseFrameSetRecords(buffer, frameSeek + 4, count);
  return { seek: frameSeek, count, actions };
}

export function parseFrameSetRecords(buffer, offset, count) {
  if (!Number.isInteger(count) || count < 0) {
    throw new Error(`Invalid Crystal FrameSet action count ${count}`);
  }

  const byteLength = count * CRYSTAL_FRAME_SET_RECORD_BYTES;
  if (!Number.isSafeInteger(byteLength)) {
    throw new Error(`Crystal FrameSet size is not safe for action count ${count}`);
  }
  assertBufferRange(buffer, offset, byteLength, "FrameSet action records");

  const actions = [];
  for (let index = 0; index < count; index += 1) {
    const actionId = buffer.readUInt8(offset);
    offset += 1;
    const start = buffer.readInt32LE(offset);
    offset += 4;
    const frameCount = buffer.readInt32LE(offset);
    offset += 4;
    const skip = buffer.readInt32LE(offset);
    offset += 4;
    const interval = buffer.readInt32LE(offset);
    offset += 4;
    const effectStart = buffer.readInt32LE(offset);
    offset += 4;
    const effectCount = buffer.readInt32LE(offset);
    offset += 4;
    const effectSkip = buffer.readInt32LE(offset);
    offset += 4;
    const effectInterval = buffer.readInt32LE(offset);
    offset += 4;
    const reverse = buffer.readUInt8(offset) !== 0;
    offset += 1;
    const blend = buffer.readUInt8(offset) !== 0;
    offset += 1;

    actions.push({
      actionId,
      actionName: MIR_ACTION_NAMES[actionId] ?? null,
      start,
      count: frameCount,
      skip,
      interval,
      effectStart,
      effectCount,
      effectSkip,
      effectInterval,
      reverse,
      blend,
    });
  }

  return actions;
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
  if (width <= 0 || height <= 0) {
    return Buffer.alloc(0);
  }

  const bgra = gunzipSync(compressed);
  const rowBytes = width * 4;
  const expectedBytes = rowBytes * height;
  const sourceStride = bgra.byteLength / height;
  if (
    !Number.isSafeInteger(expectedBytes) ||
    expectedBytes <= 0 ||
    !Number.isSafeInteger(sourceStride) ||
    sourceStride < rowBytes ||
    sourceStride % 4 !== 0
  ) {
    throw new Error(
      `Crystal frame decoded layout mismatch: expected at least ${expectedBytes} bytes for ${width}x${height}, got ${bgra.byteLength}`,
    );
  }
  const rgba = Buffer.allocUnsafe(expectedBytes);

  for (let row = 0; row < height; row += 1) {
    const sourceRow = row * sourceStride;
    const destRow = row * rowBytes;
    for (let columnByte = 0; columnByte < rowBytes; columnByte += 4) {
      const source = sourceRow + columnByte;
      const dest = destRow + columnByte;
      rgba[dest] = bgra[source + 2];
      rgba[dest + 1] = bgra[source + 1];
      rgba[dest + 2] = bgra[source];
      rgba[dest + 3] = bgra[source + 3];
    }
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

function assertBufferRange(buffer, offset, length, label) {
  if (!Number.isInteger(offset) || !Number.isInteger(length) || offset < 0 || length < 0) {
    throw new Error(`Invalid ${label} range: offset=${offset}, length=${length}`);
  }
  if (offset > buffer.length || length > buffer.length - offset) {
    throw new Error(
      `Truncated ${label}: need bytes ${offset}..${offset + length}, buffer length is ${buffer.length}`,
    );
  }
}
