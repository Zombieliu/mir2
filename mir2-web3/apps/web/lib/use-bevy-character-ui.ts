"use client";
import { useEffect, useRef, useState } from "react";
import { CharacterHost, type CharacterRuntime, type CharacterInput, type CharacterIntent, type CharacterCommandProof, type CharacterPointerEdge } from "./bevy-character-ui";
import { parseStateItemMetadata, type StateItemMetadata } from "./bevy-character-model";
type Options = { requested: boolean; runtimeGeneration: number; runtimeRef: { current: CharacterRuntime | null };
  read: (metadata: StateItemMetadata | null) => CharacterInput; onIntent: (intent: CharacterIntent, proof: CharacterCommandProof) => boolean };
export function useBevyCharacterUi(options: Options) {
  const latest = useRef(options); latest.current = options;
  const host = useRef<CharacterHost | null>(null), metadata = useRef<StateItemMetadata | null>(null);
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const runtime = latest.current.runtimeRef.current;
    if (!options.requested || !runtime) { setReady(false); return; }
    let live = true;
    const controller = new AbortController();
    const current = new CharacterHost({ runtime, isCurrent: () => latest.current.runtimeRef.current === runtime && latest.current.requested, read: () => latest.current.read(metadata.current), now: () => performance.now(),
      onState: value => { if (live) setReady(value); }, onIntent: (intent, proof) => latest.current.onIntent(intent, proof) });
    host.current = current;
    // Resource/version cache never associates geometry with an instance or cell.
    metadata.current = null;
    void fetch("/api/original-ui-meta?library=StateItem", { signal: controller.signal, cache: "no-store" })
      .then(response => response.ok ? response.json() : null).then(value => { if (live) { metadata.current = parseStateItemMetadata(value); current.tick(); } })
      .catch(() => { if (live) { metadata.current = null; current.tick(); } });
    current.tick(); const timer = window.setInterval(() => current.tick(), 50);
    const cancel = () => { current.withdraw(); setReady(false); };
    window.addEventListener("blur", cancel); window.addEventListener("resize", cancel);
    document.addEventListener("visibilitychange", cancel);
    return () => { live = false; controller.abort(); window.clearInterval(timer); window.removeEventListener("blur", cancel);
      window.removeEventListener("resize", cancel); document.removeEventListener("visibilitychange", cancel);
      current.stop(); if (host.current === current) host.current = null; };
  }, [options.requested, options.runtimeGeneration]);
  return { ready, pointerContext: () => host.current?.pointerContext() ?? null,
    pointer: (edge: CharacterPointerEdge) => host.current?.pointer(edge) ?? false,
    allows: (proof: CharacterCommandProof, ownReservation?: number) => host.current?.allows(proof, ownReservation) ?? false };
}
