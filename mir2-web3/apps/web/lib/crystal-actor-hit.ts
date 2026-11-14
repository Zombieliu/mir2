type Cell = { x: number; y: number };
type Actor = Cell & {
  kind: "selfPlayer" | "player" | "monster" | "npc";
  dead?: boolean;
  ai?: number;
};

const harvestableAi = new Set([1, 2, 4, 5, 7, 9, 28, 35, 153]);

/** Windows R16 scans a five-by-five area, living first, with the last actor in each cell first. */
export function pickCrystalActor<T extends Actor>(actors: readonly T[], cell: Cell, bodyHit: (actor: T) => boolean): T | undefined {
  const eligible = actors.filter(actor => actor.kind !== "selfPlayer"
    && Math.abs(actor.x - cell.x) <= 2 && Math.abs(actor.y - cell.y) <= 2
    && (!actor.dead || (actor.kind === "monster" && harvestableAi.has(actor.ai ?? -1))));
  eligible.reverse().sort((a, b) => Number(Boolean(a.dead)) - Number(Boolean(b.dead)) || b.y - a.y || b.x - a.x);
  return eligible.find(actor => (actor.x === cell.x && actor.y === cell.y) || bodyHit(actor));
}

export type CrystalBodyFrame = { path: string; width: number; height: number; x: number; y: number };
type BodyMask = { width: number; height: number; bits: Uint8Array };
const maxMaskBytes = 4 * 1024 * 1024;
const masks = new Map<string, BodyMask>();
const pending = new Set<string>();
const retryAfter = new Map<string, number>();
let maskBytes = 0;
let canvas: HTMLCanvasElement | null = null;

function matchingMask(frame: CrystalBodyFrame) {
  const mask = masks.get(frame.path);
  if (mask?.width !== frame.width || mask.height !== frame.height) return undefined;
  masks.delete(frame.path);
  masks.set(frame.path, mask);
  return mask;
}

function captureBodyMask(frame: CrystalBodyFrame, image: HTMLImageElement, crop?: Cell) {
  const { width, height } = frame;
  if (!image.complete || (crop
    ? crop.x < 0 || crop.y < 0 || crop.x + width > image.naturalWidth || crop.y + height > image.naturalHeight
    : image.naturalWidth !== width || image.naturalHeight !== height)) return undefined;
  const size = width * height;
  if (!Number.isSafeInteger(size) || size <= 0 || size > 1024 * 1024) return undefined;
  try {
    canvas ??= document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return undefined;
    context.drawImage(image, crop?.x ?? 0, crop?.y ?? 0, width, height, 0, 0, width, height);
    const rgba = context.getImageData(0, 0, width, height).data;
    const bits = new Uint8Array(Math.ceil(size / 8));
    for (let pixel = 0; pixel < size; pixel++) {
      if (rgba[4 * pixel + 3] > 0) bits[pixel >> 3] |= 1 << (pixel & 7);
    }
    const mask = { width, height, bits };
    maskBytes -= masks.get(frame.path)?.bits.byteLength ?? 0;
    masks.delete(frame.path);
    masks.set(frame.path, mask);
    maskBytes += bits.byteLength;
    while (maskBytes > maxMaskBytes) {
      const oldest = masks.keys().next().value!;
      maskBytes -= masks.get(oldest)!.bits.byteLength;
      masks.delete(oldest);
    }
    return mask;
  } catch {
    // A failed or unreadable frame cannot grant a cross-cell monster hit.
    return undefined;
  }
}

export function sampleCrystalBodyPixel(frame: CrystalBodyFrame, x: number, y: number, image: HTMLImageElement | null) {
  if (!Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0 || x >= frame.width || y >= frame.height) return false;
  const mask = matchingMask(frame) ?? (image ? captureBodyMask(frame, image) : undefined);
  if (!mask) return undefined;
  const pixel = y * frame.width + x;
  return (mask.bits[pixel >> 3] & (1 << (pixel & 7))) !== 0;
}

/** Share the already loaded render image. Read just this body's rectangle from a packed page. */
export function warmCrystalBodyPixels(frame: CrystalBodyFrame, load: () => Promise<HTMLImageElement>, crop?: Cell) {
  if (matchingMask(frame) || pending.has(frame.path) || pending.size >= 32
    || (retryAfter.get(frame.path) ?? 0) > Date.now()) return;
  pending.add(frame.path);
  void load().then(image => matchingMask(frame) ?? captureBodyMask(frame, image, crop), () => undefined)
    .then(mask => {
      if (mask) return;
      retryAfter.delete(frame.path);
      retryAfter.set(frame.path, Date.now() + 30_000);
      if (retryAfter.size > 256) retryAfter.delete(retryAfter.keys().next().value!);
    }).finally(() => { pending.delete(frame.path); });
}
