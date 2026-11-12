/** Same bounded Rust codec and original Text envelopes as the Native client. */
export const GATEWAY_CATALOG_CAPABILITY = "serverCatalogGzipV1";
export const MAX_CATALOG_WIRE_BYTES = 132 * 1024;
const MAX_CATALOG_DECODED_BYTES = 128 * 1024;
const MAX_CATALOG_ENVELOPES = 64;
const invalid = () => new Error("Invalid bounded gateway catalog batch");

export type GatewayCatalogDecoder = { decode(bytes: Uint8Array): readonly string[] };
export type GatewayCatalogWasmModule = {
  gateway_catalog_abi_version(): number;
  decode_gateway_catalog(bytes: Uint8Array): string;
};

export function createGatewayCatalogDecoder(module: GatewayCatalogWasmModule): GatewayCatalogDecoder {
  if (module.gateway_catalog_abi_version?.() !== 1 || typeof module.decode_gateway_catalog !== "function") {
    throw new Error("Unsupported shared gateway catalog ABI");
  }
  return Object.freeze({
    decode(bytes: Uint8Array): readonly string[] {
      // Check before wasm-bindgen copies untrusted input into linear memory.
      if (!(bytes instanceof Uint8Array) || bytes.byteLength < 16 || bytes.byteLength > MAX_CATALOG_WIRE_BYTES) throw invalid();
      try {
        const texts: unknown = JSON.parse(module.decode_gateway_catalog(bytes));
        if (!Array.isArray(texts) || texts.length < 1 || texts.length > MAX_CATALOG_ENVELOPES
          || !texts.every((text): text is string => typeof text === "string" && text.length > 0)) throw invalid();
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const declaredBytes = view.getUint32(8, true), declaredCount = view.getUint32(12, true);
        if (declaredCount !== texts.length || !declaredBytes || declaredBytes > MAX_CATALOG_DECODED_BYTES) throw invalid();
        // The shared Rust side has already checked gzip, the full batch and
        // real wire structs. Keep its exact strings, including integer text.
        const utf8 = new TextEncoder();
        if (texts.reduce((sum, text) => sum + 4 + utf8.encode(text).byteLength, 0) !== declaredBytes) throw invalid();
        return Object.freeze(texts);
      } catch { throw invalid(); }
    },
  });
}

export type GatewayCatalogIngress = ReturnType<typeof createGatewayCatalogIngress>;
export function createGatewayCatalogIngress({ current, deliver, reject }: {
  current(): boolean;
  deliver(text: string): void;
  reject(): void;
}) {
  let decoder: GatewayCatalogDecoder | null = null, failed = false;
  return {
    declared: () => decoder !== null,
    /** Called only after the complete connection capability frame was sent. */
    declare(codec: GatewayCatalogDecoder | null | undefined) {
      if (!failed && current() && decoder === null && codec) decoder = codec;
    },
    receive(data: unknown) {
      if (failed || !current()) return;
      if (typeof data === "string") { deliver(data); return; }
      let texts: readonly string[];
      try {
        // binaryType=arraybuffer keeps delivery synchronous with Text frames.
        // Blob conversion must not introduce an asynchronous reordering queue.
        if (!decoder || !(data instanceof ArrayBuffer) || data.byteLength > MAX_CATALOG_WIRE_BYTES) throw invalid();
        texts = decoder.decode(new Uint8Array(data));
      } catch {
        failed = true;
        reject();
        return;
      }
      // No callback ran until every envelope passed the shared Rust validator.
      // A callback can replace this physical connection: fence the remainder.
      for (const text of texts) {
        if (failed || !current()) return;
        deliver(text);
      }
    },
  };
}
