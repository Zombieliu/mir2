import { fitsStorage, type StoragePreparePresentation } from "./bevy-storage-ui";

export type StoragePrepareCanvasStyle = {
  maskImage: string; WebkitMaskImage: string;
  maskSize: "100% 100%"; WebkitMaskSize: "100% 100%";
  maskRepeat: "no-repeat"; WebkitMaskRepeat: "no-repeat";
};

/** Keep layout and hit testing intact while the disabled desktop warehouse paints. */
export function storagePrepareCanvasStyle(p: StoragePreparePresentation | null, canvasId: string): StoragePrepareCanvasStyle | null {
  if (canvasId !== "mir2-quest-ui-canvas" || !p || p.touch !== false || !fitsStorage(p)) return null;
  const width = p.logicalWidth, height = p.logicalHeight, left = (width - 730) / 2;
  const rect = (x: number, y: number, w: number, h: number) => "M" + x + " " + y + "H" + (x + w) + "V" + (y + h) + "H" + x + "Z";
  // Even-odd holes match render_storage's desktop header and clipped pane viewports.
  const path = [rect(0, 0, width, height), rect(left, 24, 730, 38),
    rect(left, 72, 318, 346), rect(left + 342, 72, 388, 346)].join(" ");
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ' + width + " " + height
    + '" preserveAspectRatio="none"><path fill="white" fill-rule="evenodd" d="' + path + '"/></svg>';
  const maskImage = 'url("data:image/svg+xml,' + encodeURIComponent(svg) + '")';
  return { maskImage, WebkitMaskImage: maskImage, maskSize: "100% 100%", WebkitMaskSize: "100% 100%",
    maskRepeat: "no-repeat", WebkitMaskRepeat: "no-repeat" };
}
