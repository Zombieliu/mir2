"use client";

import { useEffect, useRef } from "react";
import type { DubheNetworkRegion } from "../lib/dubhe-network";

type NetworkGlobeProps = {
  regions: DubheNetworkRegion[];
  selectedCode?: string;
  onSelect: (code: string) => void;
};

type ProjectedRegion = {
  code: string;
  x: number;
  y: number;
  radius: number;
};

type GeographicPoint = {
  latitude: number;
  longitude: number;
};

const DEG = Math.PI / 180;
const STARS = Array.from({ length: 90 }, (_, index) => ({
  x: seeded(index * 2 + 1),
  y: seeded(index * 2 + 2),
  radius: 0.35 + seeded(index + 800) * 1.25,
  alpha: 0.2 + seeded(index + 1_600) * 0.55
}));

export function NetworkGlobe({
  regions,
  selectedCode,
  onSelect
}: NetworkGlobeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rotationRef = useRef(-112 * DEG);
  const pointerRef = useRef({
    active: false,
    moved: false,
    x: 0,
    rotation: rotationRef.current
  });
  const projectedRef = useRef<ProjectedRegion[]>([]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;

    let frame = 0;
    let previousTime = performance.now();
    let width = 0;
    let height = 0;
    let ratio = 1;
    const reducedMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)"
    ).matches;
    const observer = new ResizeObserver(([entry]) => {
      width = Math.max(320, entry.contentRect.width);
      height = Math.max(360, entry.contentRect.height);
      ratio = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
    });
    observer.observe(canvas);

    const draw = (time: number) => {
      const delta = Math.min(48, time - previousTime);
      previousTime = time;
      if (!pointerRef.current.active) {
        const selected = regions.find((region) => region.code === selectedCode);
        if (selected) {
          const target = -selected.longitude * DEG;
          rotationRef.current +=
            shortestAngle(target - rotationRef.current) *
            Math.min(1, delta / 360);
        } else if (!reducedMotion) {
          rotationRef.current += delta * 0.000035;
        }
      }

      if (width >= 10 && height >= 10) {
        drawFrame(
          context,
          width,
          height,
          time,
          rotationRef.current,
          regions,
          selectedCode,
          projectedRef
        );
      }
      frame = window.requestAnimationFrame(draw);
    };
    frame = window.requestAnimationFrame(draw);
    return () => {
      observer.disconnect();
      window.cancelAnimationFrame(frame);
    };
  }, [regions, selectedCode]);

  return (
    <canvas
      aria-label="全球 Dubhe Node 区域分布。拖动旋转地球，点击发光区域查看节点。"
      className="network-globe-canvas"
      onPointerCancel={(event) => {
        pointerRef.current.active = false;
        event.currentTarget.releasePointerCapture(event.pointerId);
      }}
      onPointerDown={(event) => {
        pointerRef.current = {
          active: true,
          moved: false,
          x: event.clientX,
          rotation: rotationRef.current
        };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        const pointer = pointerRef.current;
        if (!pointer.active) return;
        const movement = event.clientX - pointer.x;
        pointer.moved ||= Math.abs(movement) > 4;
        rotationRef.current = pointer.rotation + movement * 0.006;
      }}
      onPointerUp={(event) => {
        const pointer = pointerRef.current;
        pointer.active = false;
        event.currentTarget.releasePointerCapture(event.pointerId);
        if (pointer.moved) return;
        const rect = event.currentTarget.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        const matched = projectedRef.current
          .map((region) => ({
            region,
            distance: Math.hypot(region.x - x, region.y - y)
          }))
          .filter(({ region, distance }) => distance <= region.radius + 12)
          .sort((left, right) => left.distance - right.distance)[0];
        if (matched) onSelect(matched.region.code);
      }}
      ref={canvasRef}
      role="img"
    />
  );
}

function drawFrame(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  time: number,
  rotation: number,
  regions: DubheNetworkRegion[],
  selectedCode: string | undefined,
  projectedRef: React.MutableRefObject<ProjectedRegion[]>
) {
  context.clearRect(0, 0, width, height);
  drawStars(context, width, height);

  const radius = Math.min(width * 0.43, height * 0.43);
  const centerX = width * 0.5;
  const centerY = height * 0.51;
  const halo = context.createRadialGradient(
    centerX,
    centerY,
    radius * 0.25,
    centerX,
    centerY,
    radius * 1.22
  );
  halo.addColorStop(0, "rgba(13, 54, 76, .18)");
  halo.addColorStop(0.76, "rgba(8, 29, 48, .42)");
  halo.addColorStop(0.9, "rgba(56, 218, 214, .08)");
  halo.addColorStop(1, "rgba(56, 218, 214, 0)");
  context.fillStyle = halo;
  context.beginPath();
  context.arc(centerX, centerY, radius * 1.22, 0, Math.PI * 2);
  context.fill();

  const sphere = context.createRadialGradient(
    centerX - radius * 0.28,
    centerY - radius * 0.32,
    radius * 0.08,
    centerX,
    centerY,
    radius
  );
  sphere.addColorStop(0, "rgba(15, 59, 89, .82)");
  sphere.addColorStop(0.56, "rgba(6, 29, 51, .94)");
  sphere.addColorStop(1, "rgba(2, 11, 24, .99)");
  context.fillStyle = sphere;
  context.strokeStyle = "rgba(94, 199, 224, .2)";
  context.lineWidth = 1.2;
  context.beginPath();
  context.arc(centerX, centerY, radius, 0, Math.PI * 2);
  context.fill();
  context.stroke();

  context.save();
  context.beginPath();
  context.arc(centerX, centerY, radius - 1, 0, Math.PI * 2);
  context.clip();
  drawGrid(context, centerX, centerY, radius, rotation);
  drawLand(context, centerX, centerY, radius, rotation);
  context.restore();

  const projected: ProjectedRegion[] = [];
  for (const region of regions) {
    const point = project(region, rotation, centerX, centerY, radius);
    if (!point.visible) continue;
    const selected = region.code === selectedCode;
    const intensity = Math.max(region.liveNodes, region.activeSessions > 0 ? 2 : 1);
    const markerRadius = Math.min(14, 5.5 + Math.sqrt(intensity) * 2.2);
    const pulse =
      1 +
      (Math.sin(time / 700 + region.longitude * DEG) + 1) *
        (region.servingNodes > 0 ? 2.8 : 1.2);
    const color =
      region.offlineNodes === region.nodes.length
        ? "244, 111, 111"
        : region.drainingNodes > 0
          ? "245, 183, 77"
          : "86, 232, 205";

    context.strokeStyle = `rgba(${color}, ${selected ? 0.9 : 0.38})`;
    context.lineWidth = selected ? 2 : 1;
    context.beginPath();
    context.arc(point.x, point.y, markerRadius + pulse, 0, Math.PI * 2);
    context.stroke();
    context.shadowColor = `rgba(${color}, .8)`;
    context.shadowBlur = selected ? 24 : 15;
    context.fillStyle = `rgba(${color}, ${selected ? 1 : 0.9})`;
    context.beginPath();
    context.arc(point.x, point.y, markerRadius, 0, Math.PI * 2);
    context.fill();
    context.shadowBlur = 0;

    context.fillStyle = "#dffefa";
    context.font = '700 10px "IBM Plex Mono", monospace';
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText(String(region.liveNodes), point.x, point.y + 0.5);
    projected.push({
      code: region.code,
      x: point.x,
      y: point.y,
      radius: markerRadius
    });
  }
  projectedRef.current = projected;
}

function drawStars(
  context: CanvasRenderingContext2D,
  width: number,
  height: number
) {
  for (const star of STARS) {
    context.fillStyle = `rgba(139, 203, 226, ${star.alpha})`;
    context.beginPath();
    context.arc(star.x * width, star.y * height, star.radius, 0, Math.PI * 2);
    context.fill();
  }
}

function drawGrid(
  context: CanvasRenderingContext2D,
  centerX: number,
  centerY: number,
  radius: number,
  rotation: number
) {
  context.strokeStyle = "rgba(83, 172, 202, .1)";
  context.lineWidth = 0.8;
  for (const latitude of [-60, -30, 0, 30, 60]) {
    drawGeographicLine(
      context,
      Array.from({ length: 121 }, (_, index) => ({
        latitude,
        longitude: -180 + index * 3
      })),
      rotation,
      centerX,
      centerY,
      radius
    );
  }
  for (let longitude = -180; longitude < 180; longitude += 30) {
    drawGeographicLine(
      context,
      Array.from({ length: 61 }, (_, index) => ({
        latitude: -90 + index * 3,
        longitude
      })),
      rotation,
      centerX,
      centerY,
      radius
    );
  }
}

function drawGeographicLine(
  context: CanvasRenderingContext2D,
  points: GeographicPoint[],
  rotation: number,
  centerX: number,
  centerY: number,
  radius: number
) {
  let drawing = false;
  context.beginPath();
  for (const point of points) {
    const projected = project(point, rotation, centerX, centerY, radius);
    if (!projected.visible) {
      drawing = false;
      continue;
    }
    if (!drawing) {
      context.moveTo(projected.x, projected.y);
      drawing = true;
    } else {
      context.lineTo(projected.x, projected.y);
    }
  }
  context.stroke();
}

function drawLand(
  context: CanvasRenderingContext2D,
  centerX: number,
  centerY: number,
  radius: number,
  rotation: number
) {
  for (const point of LAND_DOTS) {
    const projected = project(point, rotation, centerX, centerY, radius);
    if (!projected.visible) continue;
    const depthAlpha = 0.24 + projected.depth * 0.62;
    context.fillStyle = `rgba(93, 197, 226, ${depthAlpha})`;
    context.fillRect(projected.x - 1, projected.y - 1, 2, 2);
  }
}

function project(
  point: GeographicPoint,
  rotation: number,
  centerX: number,
  centerY: number,
  radius: number
) {
  const latitude = point.latitude * DEG;
  const longitude = point.longitude * DEG + rotation;
  const depth = Math.cos(latitude) * Math.cos(longitude);
  return {
    x: centerX + radius * Math.cos(latitude) * Math.sin(longitude),
    y: centerY - radius * Math.sin(latitude),
    visible: depth > 0,
    depth
  };
}

function buildLandDots() {
  const points: GeographicPoint[] = [];
  for (let latitude = -58; latitude <= 78; latitude += 4) {
    const longitudeStep = Math.max(4, Math.round(4 / Math.cos(latitude * DEG)));
    for (let longitude = -180; longitude < 180; longitude += longitudeStep) {
      if (
        CONTINENTS.some((polygon) =>
          pointInPolygon({ latitude, longitude }, polygon)
        )
      ) {
        points.push({ latitude, longitude });
      }
    }
  }
  return points;
}

function pointInPolygon(
  point: GeographicPoint,
  polygon: GeographicPoint[]
) {
  let inside = false;
  for (
    let current = 0, previous = polygon.length - 1;
    current < polygon.length;
    previous = current++
  ) {
    const a = polygon[current];
    const b = polygon[previous];
    const intersects =
      a.latitude > point.latitude !== b.latitude > point.latitude &&
      point.longitude <
        ((b.longitude - a.longitude) * (point.latitude - a.latitude)) /
          (b.latitude - a.latitude) +
          a.longitude;
    if (intersects) inside = !inside;
  }
  return inside;
}

function shortestAngle(value: number) {
  return Math.atan2(Math.sin(value), Math.cos(value));
}

function seeded(seed: number) {
  const value = Math.sin(seed * 12.9898) * 43758.5453;
  return value - Math.floor(value);
}

const CONTINENTS: GeographicPoint[][] = [
  [
    { latitude: 72, longitude: -168 },
    { latitude: 70, longitude: -125 },
    { latitude: 55, longitude: -100 },
    { latitude: 50, longitude: -65 },
    { latitude: 30, longitude: -80 },
    { latitude: 18, longitude: -98 },
    { latitude: 25, longitude: -115 },
    { latitude: 50, longitude: -130 },
    { latitude: 62, longitude: -150 }
  ],
  [
    { latitude: 13, longitude: -81 },
    { latitude: 5, longitude: -62 },
    { latitude: -12, longitude: -48 },
    { latitude: -34, longitude: -52 },
    { latitude: -55, longitude: -68 },
    { latitude: -18, longitude: -78 }
  ],
  [
    { latitude: 35, longitude: -18 },
    { latitude: 37, longitude: 10 },
    { latitude: 30, longitude: 33 },
    { latitude: 10, longitude: 50 },
    { latitude: -35, longitude: 20 },
    { latitude: -28, longitude: 8 },
    { latitude: 5, longitude: -18 }
  ],
  [
    { latitude: 72, longitude: -10 },
    { latitude: 70, longitude: 70 },
    { latitude: 58, longitude: 145 },
    { latitude: 40, longitude: 150 },
    { latitude: 20, longitude: 120 },
    { latitude: 5, longitude: 105 },
    { latitude: 8, longitude: 72 },
    { latitude: 28, longitude: 42 },
    { latitude: 38, longitude: 20 },
    { latitude: 55, longitude: -8 }
  ],
  [
    { latitude: -10, longitude: 112 },
    { latitude: -12, longitude: 154 },
    { latitude: -40, longitude: 151 },
    { latitude: -44, longitude: 115 }
  ],
  [
    { latitude: 83, longitude: -72 },
    { latitude: 78, longitude: -18 },
    { latitude: 60, longitude: -42 },
    { latitude: 62, longitude: -62 }
  ]
];

const LAND_DOTS = buildLandDots();
