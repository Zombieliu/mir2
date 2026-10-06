/** Read-only skill bar rows from Native normalization and its Crystal catalog. */
export type CrystalSkillBarRuntime = { getMir2SkillBarDocument?: (json: string) => string };
export type CrystalSkillBarRow = Readonly<{
  key: string; name: string; hotkey?: number; icon?: number; mpCost?: number;
  delayMs?: number; cooldownRemainingTicks?: number;
}>;
export const CRYSTAL_SKILL_BAR_POSITIONS_STORAGE_KEY = "mir2.crystalSkillBars.v1";
export type CrystalSkillBarPositions = readonly [readonly [number, number], readonly [number, number]];
export const DEFAULT_CRYSTAL_SKILL_BAR_POSITIONS: CrystalSkillBarPositions = [[0, 0], [216, 0]];
const OPTIONAL_FIELDS = ["hotkey", "icon", "mpCost", "delayMs", "cooldownRemainingTicks"] as const;
const own = (value: object, key: string) => Object.hasOwn(value, key);
const record = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
export function readSharedSkillBarDocument(runtime: CrystalSkillBarRuntime | null, learned: readonly unknown[]): readonly CrystalSkillBarRow[] | null {
  if (!Array.isArray(learned) || learned.length > 512) return null;
  try {
    const getter = runtime?.getMir2SkillBarDocument;
    if (typeof getter !== "function") return null;
    const input = JSON.stringify({ version: 1, learned });
    if (input.length > 262144) return null;
    const output = getter(input);
    if (JSON.stringify({ version: 1, learned }) !== input) return null;
    if (typeof output !== "string" || output.length > 262144) return null;
    const result: unknown = JSON.parse(output);
    if (!record(result) || Object.keys(result).sort().join() !== "ok,rows,version" || result.version !== 1
      || result.ok !== true || !Array.isArray(result.rows) || result.rows.length !== learned.length) return null;
    const keys = new Set<string>(), slots = new Set<number>(), rows: CrystalSkillBarRow[] = [];
    for (const [index, value] of result.rows.entries()) {
      const raw = learned[index];
      if (!record(raw) || !record(value) || typeof raw.key !== "string" || typeof raw.name !== "string"
        || value.key !== raw.key || value.name !== raw.name || !raw.key || raw.key.length > 128
        || !raw.name || raw.name.length > 256 || raw.key.includes("\0") || raw.name.includes("\0")
        || keys.has(raw.key) || Object.keys(value).some(key => key !== "key" && key !== "name" && !OPTIONAL_FIELDS.includes(key as typeof OPTIONAL_FIELDS[number]))) return null;
      const row: { key: string; name: string; hotkey?: number; icon?: number; mpCost?: number; delayMs?: number; cooldownRemainingTicks?: number } = { key: raw.key, name: raw.name };
      keys.add(raw.key);
      for (const field of OPTIONAL_FIELDS) {
        if (own(value, field)) {
          const number = value[field], max = field === "hotkey" || field === "icon" ? 255 : 0xffffffff;
          if (typeof number !== "number" || !Number.isSafeInteger(number) || number < 0 || number > max
            || own(raw, field) && raw[field] !== number || !own(raw, field) && field !== "icon"
            || !own(raw, field) && field === "icon" && (typeof raw.spell !== "string" || !raw.spell)) return null;
          row[field] = number;
        } else if (own(raw, field) && raw[field] !== null && raw[field] !== undefined) return null;
      }
      if (row.hotkey && row.hotkey <= 16 && slots.has(row.hotkey)) return null;
      if (row.hotkey && row.hotkey <= 16) slots.add(row.hotkey);
      rows.push(Object.freeze(row));
    }
    return Object.freeze(rows);
  } catch { return null; }
}
export function parseCrystalSkillBarPositions(text: string): CrystalSkillBarPositions | null {
  if (typeof text !== "string" || text.length > 256) return null;
  try {
    const value: unknown = JSON.parse(text);
    if (!record(value) || Object.keys(value).sort().join() !== "positions,version" || value.version !== 1
      || !Array.isArray(value.positions) || value.positions.length !== 2) return null;
    const points: Array<readonly [number, number]> = [];
    for (const point of value.positions) {
      if (!Array.isArray(point) || point.length !== 2 || !point.every(Number.isSafeInteger)
        || point[0] < 0 || point[0] > 808 || point[1] < 0 || point[1] > 740) return null;
      points.push(Object.freeze([point[0], point[1]] as const));
    }
    return Object.freeze([points[0], points[1]] as const);
  } catch { return null; }
}
export function serializeCrystalSkillBarPositions(positions: CrystalSkillBarPositions): string | null {
  try {
    const text = JSON.stringify({ version: 1, positions });
    return parseCrystalSkillBarPositions(text) ? text : null;
  } catch { return null; }
}
