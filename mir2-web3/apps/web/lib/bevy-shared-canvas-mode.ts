const SHARED_CANVAS_KEYS = new Set(["bevyBackend", "bevySharedCanvas", "bevyQuestUi", "bevyBagUi"]);

/** Read the first raw value for each decoded key; encoded keys/values never opt in. */
function firstRawValues(search: string): Map<string, string | null> {
  const first = new Map<string, string | null>();
  for (const part of search.replace(/^\?/, "").split("&")) {
    const equals = part.indexOf("=");
    const rawKey = equals < 0 ? part : part.slice(0, equals);
    const rawValue = equals < 0 ? "" : part.slice(equals + 1);
    let key: string;
    try { key = decodeURIComponent(rawKey.replace(/\+/g, " ")); }
    catch { continue; }
    if (SHARED_CANVAS_KEYS.has(key) && !first.has(key)) {
      first.set(key, rawKey === key ? rawValue : null);
    }
  }
  return first;
}

export function exactFirstRawQueryValue(search: string, key: "bevyBackend" | "bevySharedCanvas" | "bevyQuestUi" | "bevyBagUi", value: string): boolean {
  return firstRawValues(search).get(key) === value;
}

export function isWebGl2SharedCanvasPrototype(search: string): boolean {
  const first = firstRawValues(search);
  return first.get("bevyBackend") === "webgl2"
    && first.get("bevySharedCanvas") === "1"
    && (first.get("bevyQuestUi") === "1" || first.get("bevyBagUi") === "1");
}

export function sharedUiCanvasId(prototype: boolean): "mir2-web3-canvas" | "mir2-quest-ui-canvas" {
  return prototype ? "mir2-web3-canvas" : "mir2-quest-ui-canvas";
}

export function sharedCanvasUsesWebGl2(prototype: boolean, backend: "webgpu" | "webgl2" | null): boolean {
  return prototype && backend === "webgl2";
}
