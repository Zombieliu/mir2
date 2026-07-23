"use client";

import { useEffect, useRef } from "react";

import { normalizeDeviceMemoryGiB, resolveRenderTier } from "../../lib/render-tier";
import type { BevyEntityRenderState } from "./original-client-shell-types";

type WebGl2EntityAtlasLayerProps = {
  enabled: boolean;
  state: BevyEntityRenderState;
  onDebugChange?: (debug: WebGl2EntityAtlasDebug) => void;
};

type TextureRecord = {
  key: string;
  width: number;
  height: number;
  texture: WebGLTexture;
  byteSize: number;
  lastUsedAt: number;
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

type LayerVertex = {
  x: number;
  y: number;
  u: number;
  v: number;
};

type TextureCacheLimits = {
  tier: "low" | "medium" | "high";
  maxBytes: number;
  maxEntries: number;
  maxTextureSize: number;
  deviceMemoryGiB: number | null;
};

export type WebGl2EntityAtlasDebug = Record<string, unknown> & {
  atlasKey?: string | null;
  enabled?: boolean;
  supported?: boolean;
  textureReady?: boolean;
  renderedLayers?: number;
  reason?: string;
  updatedAt: number;
};

const imagePromiseCache = new Map<string, Promise<HTMLImageElement>>();

export function WebGl2EntityAtlasLayer({ enabled, state, onDebugChange }: WebGl2EntityAtlasLayerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const glRef = useRef<WebGL2RenderingContext | null>(null);
  const programRef = useRef<ProgramRecord | null>(null);
  const texturesRef = useRef<Map<string, TextureRecord>>(new Map());

  useEffect(() => {
    let disposed = false;

    async function render() {
      const canvas = canvasRef.current;
      if (!canvas) {
        return;
      }

      const debugBase = {
        enabled,
        supported: false,
        atlasKey: state.atlases?.[0]?.key ?? null,
        entityCount: state.entities.length,
        layerCount: state.entities.reduce((count, entity) => count + entity.layers.length, 0),
      };

      if (!enabled || !state.enabled) {
        clearCanvas(canvas, glRef.current);
        publishDebug({ ...debugBase, renderedLayers: 0, skippedLayers: 0, reason: "disabled" });
        return;
      }

      const gl = glRef.current ?? canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: true });
      if (!gl) {
        publishDebug({ ...debugBase, supported: false, renderedLayers: 0, skippedLayers: 0, reason: "no-webgl2" });
        return;
      }
      glRef.current = gl;
      const textureLimits = resolveTextureCacheLimits(gl);

      const program = programRef.current ?? createProgramRecord(gl);
      programRef.current = program;

      resizeCanvasForDevicePixels(canvas, state.stageWidth, state.stageHeight);
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      gl.disable(gl.DEPTH_TEST);

      const atlases = new Map((state.atlases ?? []).map((atlas) => [atlas.key, atlas]));
      const activeAtlasKeys = new Set(atlases.keys());
      const missingAtlasBytes = (state.atlases ?? []).reduce((total, atlas) => {
        const cached = texturesRef.current.get(atlas.key);
        return cached && cached.width === atlas.width && cached.height === atlas.height
          ? total
          : total + textureByteSize(atlas.width, atlas.height);
      }, 0);
      const missingAtlasCount = (state.atlases ?? []).filter((atlas) => {
        const cached = texturesRef.current.get(atlas.key);
        return !cached || cached.width !== atlas.width || cached.height !== atlas.height;
      }).length;
      evictTextureCache(gl, texturesRef.current, activeAtlasKeys, {
        maxBytes: Math.max(0, textureLimits.maxBytes - missingAtlasBytes),
        maxEntries: Math.max(0, textureLimits.maxEntries - missingAtlasCount),
      });

      const atlasTextures = new Map<string, TextureRecord>();
      for (const atlas of state.atlases ?? []) {
        const texture = await textureForAtlas(gl, texturesRef.current, atlas, state);
        if (disposed) {
          return;
        }
        if (texture) {
          atlasTextures.set(atlas.key, texture);
        }
      }
      evictTextureCache(gl, texturesRef.current, activeAtlasKeys, textureLimits);

      gl.useProgram(program.program);
      gl.uniform2f(program.resolutionLocation, canvas.width, canvas.height);
      gl.uniform1i(program.textureLocation, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, program.buffer);
      gl.enableVertexAttribArray(program.positionLocation);
      gl.vertexAttribPointer(program.positionLocation, 2, gl.FLOAT, false, 16, 0);
      gl.enableVertexAttribArray(program.texCoordLocation);
      gl.vertexAttribPointer(program.texCoordLocation, 2, gl.FLOAT, false, 16, 8);

      let renderedLayers = 0;
      let skippedLayers = 0;
      const layers = state.entities
        .flatMap((entity) => entity.layers)
        .slice()
        .sort((a, b) => a.z - b.z || a.key.localeCompare(b.key));

      for (const layer of layers) {
        const atlasKey = layer.atlasKey;
        const rectKey = layer.atlasRectKey;
        if (!atlasKey || !rectKey) {
          skippedLayers += 1;
          continue;
        }
        const atlas = atlases.get(atlasKey);
        const texture = atlasTextures.get(atlasKey);
        const rect = atlas?.rects.find((candidate) => candidate.key === rectKey);
        if (!atlas || !texture || !rect) {
          skippedLayers += 1;
          continue;
        }

        drawLayer(gl, program, texture, atlas.width, atlas.height, rect, layer);
        renderedLayers += 1;
      }

      publishDebug({
        ...debugBase,
        supported: true,
        textureReady: atlasTextures.size > 0,
        renderedLayers,
        skippedLayers,
        canvasWidth: canvas.width,
        canvasHeight: canvas.height,
        textureCacheTier: textureLimits.tier,
        textureCacheBytes: textureCacheBytes(texturesRef.current),
        textureCacheEntries: texturesRef.current.size,
        textureCacheByteLimit: textureLimits.maxBytes,
        textureCacheEntryLimit: textureLimits.maxEntries,
        maxTextureSize: textureLimits.maxTextureSize,
        deviceMemoryGiB: textureLimits.deviceMemoryGiB,
        reason: renderedLayers > 0 ? "rendered" : "no-renderable-layers",
      });
    }

    void render().catch((error) => {
      publishDebug({
        enabled,
        supported: Boolean(glRef.current),
        atlasKey: state.atlases?.[0]?.key ?? null,
        entityCount: state.entities.length,
        layerCount: state.entities.reduce((count, entity) => count + entity.layers.length, 0),
        renderedLayers: 0,
        skippedLayers: 0,
        reason: "error",
        error: error instanceof Error ? error.message : String(error),
      });
    });

    function publishDebug(payload: Record<string, unknown>) {
      const debug = writeWebGl2EntityDebug(payload);
      onDebugChange?.(debug);
    }

    return () => {
      disposed = true;
    };
  }, [enabled, state, onDebugChange]);

  useEffect(
    () => () => {
      const gl = glRef.current;
      if (!gl) {
        return;
      }
      for (const record of texturesRef.current.values()) {
        gl.deleteTexture(record.texture);
      }
      texturesRef.current.clear();
      if (programRef.current) {
        gl.deleteBuffer(programRef.current.buffer);
        gl.deleteProgram(programRef.current.program);
      }
      programRef.current = null;
      glRef.current = null;
    },
    [],
  );

  return (
    <canvas
      ref={canvasRef}
      className={`webgl2-entity-atlas-canvas ${enabled ? "" : "hidden"}`}
      width={state.stageWidth}
      height={state.stageHeight}
      aria-hidden="true"
    />
  );
}

function clearCanvas(canvas: HTMLCanvasElement, gl: WebGL2RenderingContext | null) {
  if (!gl) {
    return;
  }
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
}

function resizeCanvasForDevicePixels(canvas: HTMLCanvasElement, width: number, height: number) {
  const ratio = Math.max(1, Math.min(window.devicePixelRatio || 1, 2));
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
  if (!program) {
    throw new Error("Unable to create WebGL2 program");
  }
  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(program) ?? "Unable to link WebGL2 program");
  }
  const buffer = gl.createBuffer();
  const resolutionLocation = gl.getUniformLocation(program, "u_resolution");
  const textureLocation = gl.getUniformLocation(program, "u_texture");
  const opacityLocation = gl.getUniformLocation(program, "u_opacity");
  if (!buffer || !resolutionLocation || !textureLocation || !opacityLocation) {
    throw new Error("Unable to bind WebGL2 entity renderer uniforms");
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

function compileShader(gl: WebGL2RenderingContext, type: number, source: string) {
  const shader = gl.createShader(type);
  if (!shader) {
    throw new Error("Unable to create WebGL2 shader");
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(shader) ?? "Unable to compile WebGL2 shader");
  }
  return shader;
}

async function textureForAtlas(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
  atlas: NonNullable<BevyEntityRenderState["atlases"]>[number],
  state: BevyEntityRenderState,
) {
  const maxTextureSize = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE));
  if (atlas.width > maxTextureSize || atlas.height > maxTextureSize) {
    throw new Error(
      `Atlas ${atlas.key} is ${atlas.width}x${atlas.height}, exceeding WebGL2 MAX_TEXTURE_SIZE ${maxTextureSize}`,
    );
  }
  const existing = textureCache.get(atlas.key);
  if (existing && existing.width === atlas.width && existing.height === atlas.height) {
    existing.lastUsedAt = performance.now();
    return existing;
  }
  if (existing) {
    gl.deleteTexture(existing.texture);
    textureCache.delete(atlas.key);
  }

  const texture = gl.createTexture();
  if (!texture) {
    return null;
  }
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);

  try {
    const atlasPixels = state.atlasImages?.find((image) => image.key === atlas.key);
    if (atlasPixels?.pixels) {
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RGBA,
        atlasPixels.width,
        atlasPixels.height,
        0,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        atlasPixels.pixels,
      );
    } else if (atlas.imageUrl) {
      const image = await loadImage(atlas.imageUrl);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
    } else {
      gl.deleteTexture(texture);
      return null;
    }
  } catch (error) {
    gl.deleteTexture(texture);
    throw error;
  }

  const record = {
    key: atlas.key,
    width: atlas.width,
    height: atlas.height,
    texture,
    byteSize: textureByteSize(atlas.width, atlas.height),
    lastUsedAt: performance.now(),
  };
  textureCache.set(atlas.key, record);
  return record;
}

function textureByteSize(width: number, height: number) {
  return Math.max(0, width) * Math.max(0, height) * 4;
}

function textureCacheBytes(textureCache: Map<string, TextureRecord>) {
  let total = 0;
  for (const record of textureCache.values()) {
    total += record.byteSize;
  }
  return total;
}

function resolveTextureCacheLimits(gl: WebGL2RenderingContext): TextureCacheLimits {
  const maxTextureSize = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE));
  const forcedTier = new URLSearchParams(window.location.search).get("renderTier");
  const deviceMemoryGiB = normalizeDeviceMemoryGiB(
    (navigator as Navigator & { deviceMemory?: number }).deviceMemory,
  );
  const coarsePointer = window.matchMedia?.("(pointer: coarse)").matches ?? false;
  const tier = resolveRenderTier({ forcedTier, deviceMemoryGiB, coarsePointer, maxTextureSize });

  if (tier === "low") {
    const maxBytes = deviceMemoryGiB !== null && deviceMemoryGiB <= 2 ? 64 : 96;
    const maxEntries = deviceMemoryGiB !== null && deviceMemoryGiB <= 2 ? 12 : 20;
    return { tier, maxBytes: maxBytes * 1024 * 1024, maxEntries, maxTextureSize, deviceMemoryGiB };
  }
  if (tier === "medium") {
    return { tier, maxBytes: 160 * 1024 * 1024, maxEntries: 32, maxTextureSize, deviceMemoryGiB };
  }
  return { tier, maxBytes: 256 * 1024 * 1024, maxEntries: 48, maxTextureSize, deviceMemoryGiB };
}

function evictTextureCache(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
  activeKeys: Set<string>,
  limits: Pick<TextureCacheLimits, "maxBytes" | "maxEntries">,
) {
  let totalBytes = textureCacheBytes(textureCache);
  if (totalBytes <= limits.maxBytes && textureCache.size <= limits.maxEntries) {
    return;
  }

  const candidates = [...textureCache.values()]
    .filter((record) => !activeKeys.has(record.key))
    .sort((left, right) => left.lastUsedAt - right.lastUsedAt || left.key.localeCompare(right.key));
  for (const record of candidates) {
    if (totalBytes <= limits.maxBytes && textureCache.size <= limits.maxEntries) {
      break;
    }
    gl.deleteTexture(record.texture);
    textureCache.delete(record.key);
    totalBytes -= record.byteSize;
  }
}

function drawLayer(
  gl: WebGL2RenderingContext,
  program: ProgramRecord,
  texture: TextureRecord,
  atlasWidth: number,
  atlasHeight: number,
  rect: { x: number; y: number; width: number; height: number },
  layer: { left: number; top: number; width: number; height: number; opacity?: number },
) {
  const left = layer.left * devicePixelRatioForCanvas();
  const top = layer.top * devicePixelRatioForCanvas();
  const right = left + layer.width * devicePixelRatioForCanvas();
  const bottom = top + layer.height * devicePixelRatioForCanvas();
  const u0 = rect.x / atlasWidth;
  const u1 = (rect.x + rect.width) / atlasWidth;
  const topV = 1 - rect.y / atlasHeight;
  const bottomV = 1 - (rect.y + rect.height) / atlasHeight;
  const vertices: LayerVertex[] = [
    { x: left, y: top, u: u0, v: topV },
    { x: left, y: bottom, u: u0, v: bottomV },
    { x: right, y: top, u: u1, v: topV },
    { x: right, y: top, u: u1, v: topV },
    { x: left, y: bottom, u: u0, v: bottomV },
    { x: right, y: bottom, u: u1, v: bottomV },
  ];
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, texture.texture);
  gl.uniform1f(program.opacityLocation, layer.opacity ?? 1);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices.flatMap((vertex) => [vertex.x, vertex.y, vertex.u, vertex.v])), gl.STREAM_DRAW);
  gl.drawArrays(gl.TRIANGLES, 0, 6);
}

function devicePixelRatioForCanvas() {
  return Math.max(1, Math.min(window.devicePixelRatio || 1, 2));
}

function loadImage(src: string) {
  const existing = imagePromiseCache.get(src);
  if (existing) {
    return existing;
  }
  const promise = new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => {
      // Logged once per unique src (loadImage is memoised in imagePromiseCache). The shell
      // watches the "error" debug reason and falls back to DOM entity sprites so entities do
      // not silently vanish when an atlas texture cannot be fetched.
      console.warn(`[mir2] WebGL2 entity atlas image failed to load; falling back to DOM sprites: ${src}`);
      reject(new Error(`Unable to load WebGL2 atlas image ${src}`));
    };
    image.src = src;
  });
  imagePromiseCache.set(src, promise);
  const release = () => {
    if (imagePromiseCache.get(src) === promise) {
      imagePromiseCache.delete(src);
    }
  };
  void promise.then(release, release);
  return promise;
}

function writeWebGl2EntityDebug(payload: Record<string, unknown>): WebGl2EntityAtlasDebug {
  const debug = {
    ...payload,
    updatedAt: Date.now(),
  };
  if (typeof window === "undefined") {
    return debug;
  }
  (
    window as typeof window & {
      __mir2WebGl2EntityRendererDebug?: unknown;
    }
  ).__mir2WebGl2EntityRendererDebug = debug;
  return debug;
}
