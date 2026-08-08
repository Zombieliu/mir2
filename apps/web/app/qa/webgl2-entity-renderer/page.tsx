"use client";

import { useEffect, useMemo, useState } from "react";

import { WebGl2EntityAtlasLayer } from "../../components/webgl2-entity-atlas-layer";
import type { BevyEntityRenderState } from "../../components/original-client-shell-types";

const PROBE_ATLAS_KEY = "webgl2-probe-atlas";
const PROBE_RECT_KEY = "webgl2-probe-red|1x1";
const PROBE_PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";

export default function WebGl2EntityRendererProbePage() {
  const [debug, setDebug] = useState<unknown>(null);
  const state = useMemo<BevyEntityRenderState>(
    () => ({
      enabled: true,
      stageWidth: 128,
      stageHeight: 128,
      atlases: [
        {
          key: PROBE_ATLAS_KEY,
          width: 1,
          height: 1,
          imageUrl: PROBE_PNG,
          rects: [
            {
              key: PROBE_RECT_KEY,
              x: 0,
              y: 0,
              width: 1,
              height: 1,
            },
          ],
        },
      ],
      entities: [
        {
          objectId: "webgl2-probe-entity",
          dead: false,
          layers: [
            {
              key: "webgl2-probe-entity:body:0",
              path: PROBE_RECT_KEY,
              atlasKey: PROBE_ATLAS_KEY,
              atlasRectKey: PROBE_RECT_KEY,
              left: 16,
              top: 16,
              width: 96,
              height: 96,
              z: 1,
              opacity: 1,
            },
          ],
        },
      ],
    }),
    [],
  );

  useEffect(() => {
    const timer = window.setInterval(() => {
      setDebug((window as typeof window & { __mir2WebGl2EntityRendererDebug?: unknown }).__mir2WebGl2EntityRendererDebug ?? null);
    }, 100);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <main className="webgl2-renderer-probe-page">
      <div className="webgl2-renderer-probe-stage">
        <WebGl2EntityAtlasLayer enabled state={state} />
      </div>
      <pre data-testid="webgl2-renderer-probe-debug">{JSON.stringify(debug, null, 2)}</pre>
    </main>
  );
}
