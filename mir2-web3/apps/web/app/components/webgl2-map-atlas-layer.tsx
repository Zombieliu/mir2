"use client";

import { useEffect, useRef } from "react";

import type { MapAtlasIndex } from "../../lib/map-atlas-manifest";

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

// An atlas-MISS tile drawn by the Bevy runtime from its own full-image texture (no
// atlas page / sub-rect). `imageKey` is the `library#frame` rect key (stable + deduped
// across cells sharing the frame); the runtime looks it up in the same image registry
// the packed atlas pages upload into. Geometry matches MapTileDraw so the projection +
// unified-band z math is identical. Produced by buildStandaloneMapTiles.
export type MapStandaloneTileDraw = {
  key: string;
  imageKey: string;
  left: number;
  top: number;
  width: number;
  height: number;
  z: number;
  opacity?: number;
};

type WebGl2MapAtlasLayerProps = {
  enabled: boolean;
  stageWidth: number;
  stageHeight: number;
  index: MapAtlasIndex | null;
  tiles: MapTileDraw[];
  cameraOffset?: { x: number; y: number };
  onDebugChange?: (debug: Record<string, unknown>) => void;
};

type TextureRecord = { key: string; width: number; height: number; texture: WebGLTexture };
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
  cameraOffset = { x: 0, y: 0 },
  onDebugChange,
}: WebGl2MapAtlasLayerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const glRef = useRef<WebGL2RenderingContext | null>(null);
  const programRef = useRef<ProgramRecord | null>(null);
  const texturesRef = useRef<Map<string, TextureRecord>>(new Map());

  useEffect(() => {
    let disposed = false;

    async function render() {
      const canvas = canvasRef.current;
      if (!canvas) return;

      if (!enabled || !index || !tiles.length) {
        clearCanvas(canvas, glRef.current);
        publish({ enabled, rendered: 0, reason: !enabled ? "disabled" : !index ? "no-manifest" : "no-tiles" });
        return;
      }

      const gl = glRef.current ?? canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: true });
      if (!gl) {
        publish({ enabled, supported: false, rendered: 0, reason: "no-webgl2" });
        return;
      }
      glRef.current = gl;
      const program = programRef.current ?? createProgramRecord(gl);
      programRef.current = program;

      resizeCanvasForDevicePixels(canvas, stageWidth, stageHeight);
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      gl.disable(gl.DEPTH_TEST);

      // Load only the atlas pages actually referenced by the current tiles (a handful).
      const neededAtlasKeys = new Set(tiles.map((tile) => tile.atlasKey));
      const atlasTextures = new Map<string, TextureRecord>();
      for (const atlasKey of neededAtlasKeys) {
        const page = index.pages.get(atlasKey);
        if (!page) continue;
        const texture = await textureForPage(gl, texturesRef.current, page);
        if (disposed) return;
        if (texture) atlasTextures.set(atlasKey, texture);
      }

      gl.useProgram(program.program);
      gl.uniform2f(program.resolutionLocation, canvas.width, canvas.height);
      gl.uniform1i(program.textureLocation, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, program.buffer);
      gl.enableVertexAttribArray(program.positionLocation);
      gl.vertexAttribPointer(program.positionLocation, 2, gl.FLOAT, false, 16, 0);
      gl.enableVertexAttribArray(program.texCoordLocation);
      gl.vertexAttribPointer(program.texCoordLocation, 2, gl.FLOAT, false, 16, 8);

      let rendered = 0;
      let skipped = 0;
      const sorted = tiles.slice().sort((a, b) => a.z - b.z || a.rectKey.localeCompare(b.rectKey));
      for (const tile of sorted) {
        const page = index.pages.get(tile.atlasKey);
        const texture = atlasTextures.get(tile.atlasKey);
        const rect = index.rect.get(tile.rectKey);
        if (!page || !texture || !rect) {
          skipped += 1;
          continue;
        }
        drawTile(gl, program, texture, page.width, page.height, rect, tile, cameraOffset);
        rendered += 1;
      }

      publish({
        enabled,
        supported: true,
        rendered,
        skipped,
        tileCount: tiles.length,
        atlasPages: atlasTextures.size,
        reason: rendered > 0 ? "rendered" : "no-renderable-tiles",
      });
    }

    void render().catch((error) => {
      publish({ enabled, rendered: 0, reason: "error", error: error instanceof Error ? error.message : String(error) });
    });

    function publish(payload: Record<string, unknown>) {
      const debug = { ...payload, updatedAt: Date.now() };
      if (typeof window !== "undefined") {
        (window as typeof window & { __mir2WebGl2MapRendererDebug?: unknown }).__mir2WebGl2MapRendererDebug = debug;
      }
      onDebugChange?.(debug);
    }

    return () => {
      disposed = true;
    };
  }, [enabled, stageWidth, stageHeight, index, tiles, cameraOffset, onDebugChange]);

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

function clearCanvas(canvas: HTMLCanvasElement, gl: WebGL2RenderingContext | null) {
  if (!gl) return;
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
}

function resizeCanvasForDevicePixels(canvas: HTMLCanvasElement, width: number, height: number) {
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
    throw new Error(gl.getProgramInfoLog(program) ?? "Unable to link WebGL2 map program");
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

function compileShader(gl: WebGL2RenderingContext, type: number, source: string) {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("Unable to create WebGL2 shader");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(shader) ?? "Unable to compile WebGL2 shader");
  }
  return shader;
}

async function textureForPage(
  gl: WebGL2RenderingContext,
  textureCache: Map<string, TextureRecord>,
  page: { key: string; width: number; height: number; imageUrl: string },
) {
  const existing = textureCache.get(page.key);
  if (existing && existing.width === page.width && existing.height === page.height) {
    return existing;
  }
  const texture = gl.createTexture();
  if (!texture) return null;
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  const image = await loadImage(page.imageUrl);
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
  const record = { key: page.key, width: page.width, height: page.height, texture };
  textureCache.set(page.key, record);
  return record;
}

function drawTile(
  gl: WebGL2RenderingContext,
  program: ProgramRecord,
  texture: TextureRecord,
  atlasWidth: number,
  atlasHeight: number,
  rect: { x: number; y: number; width: number; height: number },
  tile: MapTileDraw,
  cameraOffset: { x: number; y: number },
) {
  const ratio = devicePixelRatioForCanvas();
  const left = (tile.left + (cameraOffset?.x ?? 0)) * ratio;
  const top = (tile.top + (cameraOffset?.y ?? 0)) * ratio;
  const right = left + tile.width * ratio;
  const bottom = top + tile.height * ratio;
  const u0 = rect.x / atlasWidth;
  const u1 = (rect.x + rect.width) / atlasWidth;
  const topV = 1 - rect.y / atlasHeight;
  const bottomV = 1 - (rect.y + rect.height) / atlasHeight;
  const vertices = [
    left, top, u0, topV,
    left, bottom, u0, bottomV,
    right, top, u1, topV,
    right, top, u1, topV,
    left, bottom, u0, bottomV,
    right, bottom, u1, bottomV,
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
      console.warn(`[mir2] WebGL2 map atlas image failed to load; falling back to DOM tiles: ${src}`);
      reject(new Error(`Unable to load WebGL2 map atlas image ${src}`));
    };
    image.src = src;
  });
  imagePromiseCache.set(src, promise);
  return promise;
}
