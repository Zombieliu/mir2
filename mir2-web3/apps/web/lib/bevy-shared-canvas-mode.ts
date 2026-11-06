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

/** Resolve requested shared UI surfaces; host readiness gates remain separate. */
export function resolveSharedUiRequests(search: string, strictRaw = false): Readonly<{ quest: boolean; bag: boolean }> {
  if (strictRaw) {
    const first = firstRawValues(search);
    return {
      quest: !first.has("bevyQuestUi") || first.get("bevyQuestUi") === "1",
      bag: !first.has("bevyBagUi") || first.get("bevyBagUi") === "1",
    };
  }
  const params = new URLSearchParams(search);
  const quest = params.get("bevyQuestUi");
  const bag = params.get("bevyBagUi");
  return { quest: quest === null || quest === "1", bag: bag === null || bag === "1" };
}

/** An explicit shared-canvas key retains the old exact-only override. */
export function wantsPrimarySharedCanvas(search: string): boolean {
  if (isWebGl2SharedCanvasPrototype(search)) return true;
  if (firstRawValues(search).has("bevySharedCanvas")) return false;
  const requests = resolveSharedUiRequests(search, true);
  return requests.quest || requests.bag;
}

/** Freeze all startup-relevant raw query parts, including duplicate and encoded spellings. */
export function sharedCanvasQuerySignature(search: string): string {
  const parts: string[] = [];
  for (const part of search.replace(/^\?/, "").split("&")) {
    const rawKey = part.split("=", 1)[0];
    try {
      if (SHARED_CANVAS_KEYS.has(decodeURIComponent(rawKey.replace(/\+/g, " ")))) parts.push(part);
    } catch { /* An undecodable key cannot select a shared canvas. */ }
  }
  return JSON.stringify(parts);
}

export function isWebGl2SharedCanvasPrototype(search: string): boolean {
  const first = firstRawValues(search);
  return first.get("bevyBackend") === "webgl2"
    && first.get("bevySharedCanvas") === "1"
    && (first.get("bevyQuestUi") === "1" || first.get("bevyBagUi") === "1");
}

/** Shared UI always lives on the fixed stage canvas, including GL2 primary UI. */
export function sharedUiCanvasId(_sharedWebGl2: boolean): "mir2-quest-ui-canvas" {
  return "mir2-quest-ui-canvas";
}

export function sharedCanvasUsesWebGl2(sharedWebGl2: boolean, backend: "webgpu" | "webgl2" | null): boolean {
  return sharedWebGl2 && backend === "webgl2";
}
