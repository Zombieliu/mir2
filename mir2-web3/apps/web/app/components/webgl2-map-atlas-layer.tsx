"use client";

import { useEffect, useRef, useState } from "react";

import type { MapAtlasIndex } from "../../lib/map-atlas-manifest";
import {
  normalizeDeviceMemoryGiB,
  resolveRenderTier,
} from "../../lib/render-tier";
import {
  estimateRgba8TextureBytes,
  mapTextureResidencyBytes,
  planMapTextureEvictions,
  resolveMapTextureByteBudget,
} from "../../lib/webgl2-map-texture-cache";

// GPU renderer for MAP TILES from the packed per-library atlases (Crystal MLibrary analog:
// a few resident atlas textures + GPU blits, instead of ~450-510 per-frame DOM <img>/R2 GETs).
// Self-contained (its own GL setup, intentionally mirroring webgl2-entity-atlas-layer.tsx) so it
// adds zero risk to the working entity renderer. Renders a z-sorted tile draw list onto a canvas
// mounted behind the entity/object layers. DOM map sprites are the fallback when this is off.

export type MapTileDraw = {
  // Stable per-tile identity (the ViewportMapSprite key = spriteId:cellX:cellY:frame),
  // stable across sub-cell camera motion for static tiles. The Bevy runtime keys its
  // retained tile entities by this so it updates them in place instead of despawning
  // + respawning the whole set every motion frame. Unused by the DOM WebGl2 layer.
  key: string;
  atlasKey: string;
  rectKey: string;
  left: number;
  top: number;
  width: number;
  height: number;
  z: number;
  opacity?: number;
};

export type MapStandaloneTileDraw = {
  key: string;
  imageKey: string;
  left: number;
  top: number;
  width: number;
  height: number;
  z: number;
  opacity?: number;
  // Bevy-only: Crystal DrawBlend uses SourceAlpha + One. The legacy WebGL/DOM
  // fallback never consumes standalone draws and remains unchanged.
  additive?: boolean;
};

type WebGl2MapAtlasLayerProps = {
  enabled: boolean;
  stageWidth: number;
  stageHeight: number;
  index: MapAtlasIndex | null;
  tiles: MapTileDraw[];
  onDebugChange?: (debug: Record<string, unknown>) => void;
};

type TextureRecord = {
  key: string;
  width: number;
  height: number;
  imageUrl: string;
  texture: WebGLTexture;
  byteSize: number;
  lastUsedAt: number;
};
type PendingTextureLoad = {
  signature: string;
  token: symbol;
  promise: Promise<TextureRecord | null>;
};
type TextureCacheLimits = {
  tier: "low" | "medium" | "high";
  maxBytes: number;
  maxTextureSize: number;
  deviceMemoryGiB: number | null;
};
type ProgramRecord = {
  program: WebGLProgram;
  positionLocation: number;
  texCoordLocation: number;
  resolutionLocation: WebGLUniformLocation;
  textureLocation: WebGLUniformLocation;
  opacityLocation: WebGLUniformLocation;
  buffer: WebGLBuffer;
};

const imagePromiseCache = new Map<string, Promise<HTMLImageElement>>();

export function WebGl2MapAtlasLayer({
  enabled,
  stageWidth,
  stageHeight,
  index,
  tiles,
  onDebugChange,
}: WebGl2MapAtlasLayerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const glRef = useRef<WebGL2RenderingContext | null>(null);
  const programRef = useRef<ProgramRecord | null>(null);
  const texturesRef = useRef<Map<string, TextureRecord>>(new Map());
  const pendingTextureLoadsRef = useRef<Map<string, PendingTextureLoad>>(
    new Map(),
  );
  const pinnedAtlasKeysRef = useRef<Set<string>>(new Set());
  const [contextEpoch, setContextEpoch] = useState(0);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const releaseCurrentContext = () => {
      const gl = glRef.current;
      pinnedAtlasKeysRef.current.clear();
      pendingTextureLoadsRef.current.clear();
      if (gl) releaseGlResources(gl, texturesRef.current, programRef);
      glRef.current = null;
    };
    const handleContextLost = (event: Event) => {
      event.preventDefault();
      releaseCurrentContext();
      setContextEpoch((epoch) => epoch + 1);
    };
    const handleContextRestored = () => {
      setContextEpoch((epoch) => epoch + 1);
    };

    canvas.addEventListener("webglcontextlost", handleContextLost);
    canvas.addEventListener("webglcontextrestored", handleContextRestored);
    return () => {
      canvas.removeEventListener("webglcontextlost", handleContextLost);
      canvas.removeEventListener("webglcontextrestored", handleContextRestored);
      releaseCurrentContext();
    };
  }, []);

  useEffect(() => {
    let disposed = false;

    async function render() {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const neededAtlasKeys =
        enabled && index
          ? new Set(tiles.map((tile) => tile.atlasKey))
          : new Set<string>();
      pinnedAtlasKeysRef.current = neededAtlasKeys;

      if (!enabled) {
        pendingTextureLoadsRef.current.clear();
        const gl = glRef.current;
        if (gl) releaseTextureCache(gl, texturesRef.current);
        clearCanvas(canvas, gl);
        publish({ enabled, rendered: 0, reason: "disabled" });
        return;
      }

      if (!index || !tiles.length) {
        clearCanvas(canvas, glRef.current);
        publish({
          enabled,
          rendered: 0,
          reason: !index ? "no-manifest" : "no-tiles",
        });
        return;
      }

      const canvasGl = canvas.getContext("webgl2", {
        alpha: true,
        premultipliedAlpha: true,
      });
      if (glRef.current && canvasGl && glRef.current !== canvasGl) {
        pendingTextureLoadsRef.current.clear();
        releaseGlResources(glRef.current, texturesRef.current, programRef);
        glRef.current = null;
      }
      const gl = canvasGl;
      if (!gl) {
        publish({
          enabled,
          supported: false,
          rendered: 0,
          reason: "no-webgl2",
        });
        return;
      }
      if (gl.isContextLost()) {
        publish({
          enabled,
          supported: true,
          rendered: 0,
          reason: "context-lost",
        });
        return;
      }
      glRef.current = gl;
      const textureLimits = resolveTextureCacheLimits(gl);
      const program = programRef.current ?? createProgramRecord(gl);
      programRef.current = program;

      // Load only the atlas pages actually referenced by the current tiles (a handful).
      const usedAt = performance.now();
      let missingTextureBytes = 0;
      for (const atlasKey of neededAtlasKeys) {
        const page = index.pages.get(atlasKey);
        if (!page) continue;
        const existing = texturesRef.current.get(atlasKey);
        if (existing && !textureRecordMatchesPage(existing, page)) {
          gl.deleteTexture(existing.texture);
          texturesRef.current.delete(atlasKey);
        }
        const current = texturesRef.current.get(atlasKey);
        if (current) {
          current.lastUsedAt = usedAt;
        } else {
          missingTextureBytes += estimateRgba8TextureBytes(
            page.width,
            page.height,
          );
        }
      }
      evictTextureCache(
        gl,
        texturesRef.current,
        neededAtlasKeys,
        Math.max(0, textureLimits.maxBytes - missingTextureBytes),
      );

      const atlasTextures = new Map<string, TextureRecord>();
      const loadedTextures = await Promise.all(
        [...neededAtlasKeys].map(async (atlasKey) => {
          const page = index.pages.get(atlasKey);
          if (!page) return [atlasKey, null] as const;
          const texture = await textureForPage(
            gl,
            texturesRef.current,
            pendingTextureLoadsRef.current,
            page,
            usedAt,
            () =>
              glRef.current === gl &&
              !gl.isContextLost() &&
              pinnedAtlasKeysRef.current.has(page.key),
          );
          return [atlasKey, texture] as const;
        }),
      );
      if (disposed) return;
      for (const [atlasKey, texture] of loadedTextures) {
        if (texture) atlasTextures.set(atlasKey, texture);
      }
      evictTextureCache(
        gl,
        texturesRef.current,
        neededAtlasKeys,
        textureLimits.maxBytes,
      );

      // Keep the previous complete frame visible while a newly referenced page
      // is fetched/decoded. Clearing before Promise.all is what caused a full-map
      // blink whenever movement crossed an atlas-page boundary.
      resizeCanvasForDevicePixels(canvas, stageWidth, stageHeight);
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      gl.disable(gl.DEPTH_TEST);

      gl.useProgram(program.program);
      gl.uniform2f(program.resolutionLocation, canvas.width, canvas.height);
      gl.uniform1i(program.textureLocation, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, program.buffer);
      gl.enableVertexAttribArray(program.positionLocation);
      gl.vertexAttribPointer(
        program.positionLocation,
        2,
        gl.FLOAT,
        false,
        16,
        0,
      );
      gl.enableVertexAttribArray(program.texCoordLocation);
      gl.vertexAttribPointer(
        program.texCoordLocation,
        2,
        gl.FLOAT,
        false,
        16,
        8,
      );

      let rendered = 0;
      let skipped = 0;
      const sorted = tiles
        .slice()
        .sort((a, b) => a.z - b.z || a.rectKey.localeCompare(b.rectKey));
      for (const tile of sorted) {
        const page = index.pages.get(tile.atlasKey);
        const texture = atlasTextures.get(tile.atlasKey);
        const rect = index.rect.get(tile.rectKey);
        if (!page || !texture || !rect) {
          skipped += 1;
          continue;
        }
        drawTile(gl, program, texture, page.width, page.height, rect, tile);
        rendered += 1;
      }

      publish({
        enabled,
        supported: true,
        rendered,
        skipped,
        tileCount: tiles.length,
        atlasPages: atlasTextures.size,
        textureCacheTier: textureLimits.tier,
        textureCacheBytes: mapTextureResidencyBytes(
          texturesRef.current.values(),
        ),
        textureCacheEntries: texturesRef.current.size,
        textureCacheByteLimit: textureLimits.maxBytes,
        maxTextureSize: textureLimits.maxTextureSize,
        deviceMemoryGiB: textureLimits.deviceMemoryGiB,
        reason: rendered > 0 ? "rendered" : "no-renderable-tiles",
      });
    }

    void render().catch((error) => {
      if (disposed) return;
      publish({
        enabled,
        rendered: 0,
        reason: "error",
        error: error instanceof Error ? error.message : String(error),
      });
    });

    function publish(payload: Record<string, unknown>) {
      const debug = { ...payload, updatedAt: Date.now() };
      if (typeof window !== "undefined") {
        (
          window as typeof window & { __mir2WebGl2MapRendererDebug?: unknown }
        ).__mir2WebGl2MapRendererDebug = debug;
      }
      onDebugChange?.(debug);
    }

    return () => {
      disposed = true;
    };
  }, [
    enabled,
    stageWidth,
    stageHeight,
    index,
    tiles,
    onDebugChange,
    contextEpoch,
  ]);

  return (
    <canvas
      ref={canvasRef}
      className={`webgl2-map-atlas-canvas ${enabled ? "" : "hidden"}`}
      width={stageWidth}
      height={stageHeight}
      aria-hidden="true"
    />
  );
}

function clearCanvas(
  canvas: HTMLCanvasElement,
  gl: WebGL2RenderingContext | null,
) {
  if (!gl) return;
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
}

function resizeCanvasForDevicePixels(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
) {
  const ratio = devicePixelRatioForCanvas();
  const nextWidth = Math.max(1, Math.round(width * ratio));
  const nextHeight = Math.max(1, Math.round(height * ratio));
  if (canvas.width !== nextWidth || canvas.height !== nextHeight) {
    canvas.width = nextWidth;
    canvas.height = nextHeight;
  }
}

function createProgramRecord(gl: WebGL2RenderingContext): ProgramRecord {
  const vertexShader = compileShader(
    gl,
    gl.VERTEX_SHADER,
    `#version 300 es
      in vec2 a_position;
      in vec2 a_texCoord;
      uniform vec2 u_resolution;
      out vec2 v_texCoord;
      void main() {
        vec2 zeroToOne = a_position / u_resolution;
        vec2 clipSpace = zeroToOne * 2.0 - 1.0;
        gl_Position = vec4(clipSpace * vec2(1.0, -1.0), 0.0, 1.0);
        v_texCoord = a_texCoord;
      }
    `,
  );
  const fragmentShader = compileShader(
    gl,
    gl.FRAGMENT_SHADER,
    `#version 300 es
      precision mediump float;
      uniform sampler2D u_texture;
      uniform float u_opacity;
      in vec2 v_texCoord;
      out vec4 outColor;
      void main() {
        vec4 color = texture(u_texture, v_texCoord);
        outColor = vec4(color.rgb, color.a * u_opacity);
      }
    `,
  );
  const program = gl.createProgram();
  if (!program) throw new Error("Unable to create WebGL2 program");
  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(
      gl.getProgramInfoLog(program) ?? "Unable to link WebGL2 map program",
    );
  }
  const buffer = gl.createBuffer();
  const resolutionLocation = gl.getUniformLocation(program, "u_resolution");
  const textureLocation = gl.getUniformLocation(program, "u_texture");
  const opacityLocation = gl.getUniformLocation(program, "u_opacity");
  if (!buffer || !resolutionLocation || !textureLocation || !opacityLocation) {
    throw new Error("Unable to bind WebGL2 map renderer uniforms");
  }
  return {
    program,
    positionLocation: gl.getAttribLocation(program, "a_position"),
    texCoordLocation: gl.getAttribLocation(program, "a_texCoord"),
    resolutionLocation,
    textureLocation,
    opacityLocation,
    buffer,
  };
}

function compileShader(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
) {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("Unable to create WebGL2 shader");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(
      gl.getShaderInfoLog(shader) ?? "Unable to compile WebGL2 shader",
    );
  }
  return shader;
}

async function textureForPage(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
  pendingLoads: Map<string, PendingTextureLoad>,
  page: { key: string; width: number; height: number; imageUrl: string },
  usedAt: number,
  isContextCurrent: () => boolean,
) {
  const maxTextureSize = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE));
  if (page.width > maxTextureSize || page.height > maxTextureSize) {
    throw new Error(
      `Map atlas ${page.key} is ${page.width}x${page.height}, exceeding WebGL2 MAX_TEXTURE_SIZE ${maxTextureSize}`,
    );
  }
  const existing = textureCache.get(page.key);
  if (existing && textureRecordMatchesPage(existing, page)) {
    existing.lastUsedAt = usedAt;
    return existing;
  }
  if (existing) {
    gl.deleteTexture(existing.texture);
    textureCache.delete(page.key);
  }

  const signature = texturePageSignature(page);
  const pending = pendingLoads.get(page.key);
  if (pending?.signature === signature) {
    const record = await pending.promise;
    if (record) record.lastUsedAt = Math.max(record.lastUsedAt, usedAt);
    return record;
  }

  const token = Symbol(page.key);
  const promise = (async () => {
    try {
      const image = await loadImage(page.imageUrl);
      if (!isContextCurrent() || pendingLoads.get(page.key)?.token !== token)
        return null;

      const texture = gl.createTexture();
      if (!texture) return null;
      try {
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
        gl.texImage2D(
          gl.TEXTURE_2D,
          0,
          gl.RGBA,
          gl.RGBA,
          gl.UNSIGNED_BYTE,
          image,
        );
      } catch (error) {
        gl.deleteTexture(texture);
        throw error;
      }

      if (!isContextCurrent() || pendingLoads.get(page.key)?.token !== token) {
        gl.deleteTexture(texture);
        return null;
      }
      const record: TextureRecord = {
        key: page.key,
        width: page.width,
        height: page.height,
        imageUrl: page.imageUrl,
        texture,
        byteSize: estimateRgba8TextureBytes(page.width, page.height),
        lastUsedAt: usedAt,
      };
      const replaced = textureCache.get(page.key);
      if (replaced) gl.deleteTexture(replaced.texture);
      textureCache.set(page.key, record);
      return record;
    } finally {
      if (pendingLoads.get(page.key)?.token === token)
        pendingLoads.delete(page.key);
    }
  })();
  pendingLoads.set(page.key, { signature, token, promise });
  return promise;
}

function textureRecordMatchesPage(
  record: TextureRecord,
  page: { width: number; height: number; imageUrl: string },
) {
  return (
    record.width === page.width &&
    record.height === page.height &&
    record.imageUrl === page.imageUrl
  );
}

function texturePageSignature(page: {
  width: number;
  height: number;
  imageUrl: string;
}) {
  return `${page.width}x${page.height}:${page.imageUrl}`;
}

function resolveTextureCacheLimits(
  gl: WebGL2RenderingContext,
): TextureCacheLimits {
  const maxTextureSize = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE));
  const forcedTier = new URLSearchParams(window.location.search).get(
    "renderTier",
  );
  const deviceMemoryGiB = normalizeDeviceMemoryGiB(
    (navigator as Navigator & { deviceMemory?: number }).deviceMemory,
  );
  const coarsePointer =
    window.matchMedia?.("(pointer: coarse)").matches ?? false;
  const tier = resolveRenderTier({
    forcedTier,
    deviceMemoryGiB,
    coarsePointer,
    maxTextureSize,
  });
  return {
    tier,
    maxBytes: resolveMapTextureByteBudget({
      tier,
      deviceMemoryGiB,
      maxTextureSize,
    }),
    maxTextureSize,
    deviceMemoryGiB,
  };
}

function evictTextureCache(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
  pinnedKeys: ReadonlySet<string>,
  maxBytes: number,
) {
  const plan = planMapTextureEvictions(
    textureCache.values(),
    pinnedKeys,
    maxBytes,
  );
  for (const key of plan.evictKeys) {
    const record = textureCache.get(key);
    if (!record) continue;
    gl.deleteTexture(record.texture);
    textureCache.delete(key);
  }
  return plan;
}

function releaseGlResources(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
  programRef: { current: ProgramRecord | null },
) {
  releaseTextureCache(gl, textureCache);
  if (programRef.current) {
    gl.deleteBuffer(programRef.current.buffer);
    gl.deleteProgram(programRef.current.program);
    programRef.current = null;
  }
}

function releaseTextureCache(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
) {
  for (const record of textureCache.values()) gl.deleteTexture(record.texture);
  textureCache.clear();
}

function drawTile(
  gl: WebGL2RenderingContext,
  program: ProgramRecord,
  texture: TextureRecord,
  atlasWidth: number,
  atlasHeight: number,
  rect: { x: number; y: number; width: number; height: number },
  tile: MapTileDraw,
) {
  const ratio = devicePixelRatioForCanvas();
  const left = tile.left * ratio;
  const top = tile.top * ratio;
  const right = left + tile.width * ratio;
  const bottom = top + tile.height * ratio;
  const u0 = rect.x / atlasWidth;
  const u1 = (rect.x + rect.width) / atlasWidth;
  const topV = 1 - rect.y / atlasHeight;
  const bottomV = 1 - (rect.y + rect.height) / atlasHeight;
  const vertices = [
    left,
    top,
    u0,
    topV,
    left,
    bottom,
    u0,
    bottomV,
    right,
    top,
    u1,
    topV,
    right,
    top,
    u1,
    topV,
    left,
    bottom,
    u0,
    bottomV,
    right,
    bottom,
    u1,
    bottomV,
  ];
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, texture.texture);
  gl.uniform1f(program.opacityLocation, tile.opacity ?? 1);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.STREAM_DRAW);
  gl.drawArrays(gl.TRIANGLES, 0, 6);
}

function devicePixelRatioForCanvas() {
  return Math.max(1, Math.min(window.devicePixelRatio || 1, 2));
}

function loadImage(src: string) {
  const existing = imagePromiseCache.get(src);
  if (existing) return existing;
  const promise = new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.decoding = "async";
    image.crossOrigin = "anonymous";
    image.onload = () => resolve(image);
    image.onerror = () => {
      console.warn(
        `[mir2] WebGL2 map atlas image failed to load; falling back to DOM tiles: ${src}`,
      );
      reject(new Error(`Unable to load WebGL2 map atlas image ${src}`));
    };
    image.src = src;
  });
  imagePromiseCache.set(src, promise);
  const release = () => {
    if (imagePromiseCache.get(src) === promise) imagePromiseCache.delete(src);
  };
  void promise.then(release, release);
  return promise;
}
