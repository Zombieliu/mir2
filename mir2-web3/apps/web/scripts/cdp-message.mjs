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

export function isCriticalConsoleError(error) {
  const text = String(error?.text ?? "");
  if (!text.trim()) return false;
  if (text.includes("net::ERR_FAILED")) return false;
  if (text.includes("favicon")) return false;
  if (
    error?.source === "other" &&
    text.startsWith("Unchecked runtime.lastError: The message port closed before a response was received.")
  ) {
    return false;
  }
  return true;
}
