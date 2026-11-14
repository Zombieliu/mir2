import pins from "./generated/r16-entry-visuals.json";

type Gender = "male" | "female";
export type R16StandingImages = Readonly<Record<string, { png: string; width: number; height: number }>>;
export type R16StandingGroup = { key: string; assetReleaseId: string; gender: Gender; direction: number };
const imports = {
  male: [
    () => import("./generated/r16-standing/male-0.json"), () => import("./generated/r16-standing/male-1.json"),
    () => import("./generated/r16-standing/male-2.json"), () => import("./generated/r16-standing/male-3.json"),
    () => import("./generated/r16-standing/male-4.json"), () => import("./generated/r16-standing/male-5.json"),
    () => import("./generated/r16-standing/male-6.json"), () => import("./generated/r16-standing/male-7.json"),
  ],
  female: [
    () => import("./generated/r16-standing/female-0.json"), () => import("./generated/r16-standing/female-1.json"),
    () => import("./generated/r16-standing/female-2.json"), () => import("./generated/r16-standing/female-3.json"),
    () => import("./generated/r16-standing/female-4.json"), () => import("./generated/r16-standing/female-5.json"),
    () => import("./generated/r16-standing/female-6.json"), () => import("./generated/r16-standing/female-7.json"),
  ],
};

export function r16StandingGroup(assetReleaseId: string | null | undefined, bodyPath: string | null | undefined): R16StandingGroup | null {
  if (assetReleaseId !== pins.assetReleaseId) return null;
  const match = bodyPath?.match(/^\/original-ui\/CArmour\/00\/(0|[1-9][0-9]*)\.png$/);
  if (!match) return null;
  const index = Number(match[1]), gender = index >= 808 ? "female" : "male";
  const relative = index - (gender === "male" ? 0 : 808);
  if (relative < 0 || relative >= 32) return null;
  const direction = Math.floor(relative / 4);
  return { assetReleaseId, gender, direction, key: assetReleaseId + ":" + gender + ":" + direction };
}

function checkChunk(value: unknown, group: R16StandingGroup): R16StandingImages | null {
  if (!value || typeof value !== "object") return null;
  const data = value as { schemaVersion?: number; assetReleaseId?: string; sourceSha256?: string;
    gender?: Gender; direction?: number; images?: R16StandingImages };
  if (data.schemaVersion !== 1 || data.assetReleaseId !== group.assetReleaseId || data.gender !== group.gender ||
      data.direction !== group.direction || data.sourceSha256 !== pins.players[group.gender].sha256 ||
      !data.images || Object.keys(data.images).length !== 8) return null;
  const first = (group.gender === "male" ? 0 : 808) + group.direction * 4;
  for (const library of ["CArmour/00", "CHair/00"]) for (let phase = 0; phase < 4; phase++) {
    const frame = data.images["/original-ui/" + library + "/" + (first + phase) + ".png"];
    if (!frame || !Number.isSafeInteger(frame.width) || !Number.isSafeInteger(frame.height) ||
        frame.width < 1 || frame.height < 1 || typeof frame.png !== "string" || frame.png.length > 16_000 ||
        !frame.png.startsWith("data:image/png;base64,")) return null;
  }
  return data.images;
}

/** Fixed, finite chunk set; each direction's four animation phases share one read. */
export function createR16StandingSpriteReader(
  importer: (group: R16StandingGroup) => Promise<unknown> = group => imports[group.gender][group.direction]().then(module => module.default),
  options: { timeoutMs?: number; now?: () => number } = {},
) {
  const pending = new Map<string, Promise<R16StandingImages | null>>();
  const failures = new Map<string, { attempts: number; retryAt: number }>();
  const now = options.now ?? Date.now;
  return (group: R16StandingGroup, owner: AbortSignal): Promise<R16StandingImages | null> => {
    if (owner.aborted || group.assetReleaseId !== pins.assetReleaseId || !["male", "female"].includes(group.gender) ||
        !Number.isInteger(group.direction) || group.direction < 0 || group.direction > 7 ||
        group.key !== group.assetReleaseId + ":" + group.gender + ":" + group.direction) return Promise.resolve(null);
    let read = pending.get(group.key);
    const failed = failures.get(group.key);
    if (!read && failed && (failed.attempts >= 2 || now() < failed.retryAt)) return Promise.resolve(null);
    if (!read) {
      const started = Promise.resolve().then(() => importer(group)).then(value => checkChunk(value, group)).catch(() => null);
      read = started.then(images => {
        if (images) failures.delete(group.key);
        else {
          pending.delete(group.key);
          failures.set(group.key, { attempts: (failures.get(group.key)?.attempts ?? 0) + 1, retryAt: now() + 5_000 });
        }
        return images;
      });
      pending.set(group.key, read);
    }
    const shared = read;
    return new Promise(resolve => {
      let settled = false;
      const finish = (images: R16StandingImages | null) => {
        if (settled) return;
        settled = true; clearTimeout(timer); owner.removeEventListener("abort", aborted); resolve(images);
      };
      const aborted = () => finish(null);
      const timer = setTimeout(aborted, options.timeoutMs ?? 20_000);
      owner.addEventListener("abort", aborted, { once: true });
      if (owner.aborted) aborted(); else void shared.then(finish);
    });
  };
}

export const readR16StandingSprites = createR16StandingSpriteReader();
