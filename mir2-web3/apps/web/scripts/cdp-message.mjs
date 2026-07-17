const decoder = new TextDecoder();

export async function decodeCdpMessage(raw) {
  if (typeof raw === "string") return JSON.parse(raw);
  if (raw instanceof ArrayBuffer) {
    return JSON.parse(decoder.decode(new Uint8Array(raw)));
  }
  if (ArrayBuffer.isView(raw)) {
    return JSON.parse(decoder.decode(new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength)));
  }
  if (raw && typeof raw.text === "function") {
    return JSON.parse(await raw.text());
  }
  throw new TypeError(`Unsupported CDP message payload: ${Object.prototype.toString.call(raw)}`);
}
