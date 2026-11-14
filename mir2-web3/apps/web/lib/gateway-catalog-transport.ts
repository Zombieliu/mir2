/** Same bounded Rust codec and original Text envelopes as the Native client. */
export const GATEWAY_CATALOG_CAPABILITY = "serverCatalogGzipV1";
export const GATEWAY_WORLD_SNAPSHOT_CAPABILITY = "serverWorldSnapshotGzipV1";
const WORLD_SNAPSHOT_MAGIC = [77, 50, 87, 83, 80, 71, 90, 49]; // M2WSPGZ1
export const MAX_CATALOG_WIRE_BYTES = 132 * 1024;
const MAX_CATALOG_DECODED_BYTES = 128 * 1024;
const MAX_CATALOG_ENVELOPES = 64;
const invalid = () => new Error("Invalid bounded gateway catalog batch");

export type GatewayCatalogDecoder = { decode(bytes: Uint8Array): readonly string[]; capabilities?: readonly string[] };
export type GatewayCatalogWasmModule = {
  gateway_catalog_abi_version(): number;
  decode_gateway_catalog(bytes: Uint8Array): string;
  gateway_world_snapshot_abi_version?(): number;
  decode_gateway_world_snapshot?(bytes: Uint8Array): string;
};

export function createGatewayCatalogDecoder(module: GatewayCatalogWasmModule): GatewayCatalogDecoder {
  if (module.gateway_catalog_abi_version?.() !== 1 || typeof module.decode_gateway_catalog !== "function") {
    throw new Error("Unsupported shared gateway catalog ABI");
  }
  const hasSnapshotAbi = typeof module.gateway_world_snapshot_abi_version === "function";
  const hasSnapshotDecoder = typeof module.decode_gateway_world_snapshot === "function";
  if (hasSnapshotAbi !== hasSnapshotDecoder || (hasSnapshotAbi && module.gateway_world_snapshot_abi_version!() !== 1)) {
    throw new Error("Unsupported shared gateway world snapshot ABI");
  }
  return Object.freeze({
    capabilities: Object.freeze([GATEWAY_CATALOG_CAPABILITY, ...(hasSnapshotAbi ? [GATEWAY_WORLD_SNAPSHOT_CAPABILITY] : [])]),
    decode(bytes: Uint8Array): readonly string[] {
      // Check before wasm-bindgen copies untrusted input into linear memory.
      if (!(bytes instanceof Uint8Array) || bytes.byteLength < 16 || bytes.byteLength > MAX_CATALOG_WIRE_BYTES) throw invalid();
      try {
        if (WORLD_SNAPSHOT_MAGIC.every((value, index) => bytes[index] === value)) {
          if (!hasSnapshotDecoder) throw invalid();
          const text = module.decode_gateway_world_snapshot!(bytes);
          const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
          const declaredBytes = view.getUint32(8, true);
          if (typeof text !== "string" || view.getUint32(12, true) !== 1 || declaredBytes > MAX_CATALOG_DECODED_BYTES
            || new TextEncoder().encode(text).byteLength + 4 !== declaredBytes) throw invalid();
          return Object.freeze([text]);
        }
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
    capabilitiesFor(next?: GatewayCatalogDecoder | null): readonly string[] {
      const selected = decoder ?? next;
      return !failed && current() && selected ? selected.capabilities ?? [GATEWAY_CATALOG_CAPABILITY] : [];
    },
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
