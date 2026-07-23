import type { CSSProperties, ReactNode } from "react";

import manifestJson from "../../public/original-ui/gdi-text/manifest.json";

type ManifestAsset = {
  key: string;
  output: string;
  text: string;
  foreground: string;
  background: string;
  outline: { enabled: boolean };
  size: {
    mode: "auto" | "fixed";
    output: { width: number; height: number };
  };
};

type GdiManifest = {
  schemaVersion: number;
  assets: ManifestAsset[];
};

export type CrystalGdiTextAsset = ManifestAsset & {
  src: string;
};

export type CrystalGdiTextRequest = {
  text: string;
  foreground: string;
  background?: string;
  outline: boolean;
  width?: number;
  height?: number;
};

const manifest = manifestJson as GdiManifest;
const assets = manifest.assets.map((asset) => ({
  ...asset,
  src: `/original-ui/gdi-text/${asset.output}`,
}));

export function findCrystalGdiTextAsset({
  text,
  foreground,
  background = "transparent",
  outline,
  width,
  height,
}: CrystalGdiTextRequest): CrystalGdiTextAsset | null {
  const normalizedForeground = cssColourToArgb(foreground);
  const normalizedBackground = cssColourToArgb(background);
  if (!normalizedForeground || !normalizedBackground) {
    return null;
  }

  const normalizedText = text.replace(/\r?\n/g, "\r\n");
  return assets.find((asset) => {
    if (
      asset.text !== normalizedText ||
      asset.foreground !== normalizedForeground ||
      asset.background !== normalizedBackground ||
      asset.outline.enabled !== outline
    ) {
      return false;
    }
    if (width === undefined && height === undefined) {
      return asset.size.mode === "auto";
    }
    return (
      asset.size.mode === "fixed" &&
      asset.size.output.width === width &&
      asset.size.output.height === height
    );
  }) ?? null;
}

export function CrystalGdiTextImage({
  asset,
  className,
  accessibleText,
  style,
}: {
  asset: CrystalGdiTextAsset;
  className?: string;
  accessibleText?: ReactNode;
  style?: CSSProperties;
}) {
  return (
    <span
      className={`crystal-gdi-text ${className ?? ""}`.trim()}
      style={{
        width: asset.size.output.width,
        height: asset.size.output.height,
        ...style,
      }}
      data-crystal-gdi-text={asset.key}
    >
      <span className="crystal-gdi-text-accessible">{accessibleText ?? asset.text}</span>
      <img
        src={asset.src}
        alt=""
        aria-hidden="true"
        draggable={false}
        width={asset.size.output.width}
        height={asset.size.output.height}
      />
    </span>
  );
}

function cssColourToArgb(value: string): string | null {
  const trimmed = value.trim();
  if (trimmed.toLowerCase() === "transparent") {
    return "#00000000";
  }
  const hex = /^#([0-9a-f]{6}|[0-9a-f]{8})$/i.exec(trimmed)?.[1];
  if (hex) {
    return `#${hex.length === 6 ? `FF${hex}` : hex}`.toUpperCase();
  }
  const rgba = /^rgba?\(\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)(?:\s*,\s*(\d*(?:\.\d+)?))?\s*\)$/i.exec(trimmed);
  if (!rgba) {
    return null;
  }
  const red = clampByte(Number(rgba[1]));
  const green = clampByte(Number(rgba[2]));
  const blue = clampByte(Number(rgba[3]));
  const alpha = rgba[4] === undefined ? 255 : clampByte(Number(rgba[4]) * 255);
  return `#${byteHex(alpha)}${byteHex(red)}${byteHex(green)}${byteHex(blue)}`;
}

function clampByte(value: number) {
  return Math.min(255, Math.max(0, Math.round(value)));
}

function byteHex(value: number) {
  return value.toString(16).padStart(2, "0").toUpperCase();
}
