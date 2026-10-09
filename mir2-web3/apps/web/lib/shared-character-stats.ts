/** Read-only rows from Crystal's existing Rust character_stats rule. */
export type CrystalStatsRuntime = { getMir2CharacterStatsDocument?: (json: string) => string };
export type CrystalStatsPlayer = Readonly<{
  hp: number; mp: number; level: number; experience: number; maxExperience: number;
  crystalStats: readonly Readonly<{ stat: number; value: number }>[];
  weights: Readonly<{ bag: number; wear: number; hand: number }> | null;
}>;
export type CrystalStatRow = Readonly<{ text: string; top: number }>;
export function readSharedCharacterStats(runtime: CrystalStatsRuntime | null, player: CrystalStatsPlayer,
  statePage: boolean): readonly CrystalStatRow[] | null {
  const getter = runtime?.getMir2CharacterStatsDocument;
  if (typeof getter !== "function" || typeof statePage !== "boolean") return null;
  try {
    const input = JSON.stringify({ version: 1, player, statePage });
    if (input.length > 262144) return null;
    const output = getter(input);
    if (typeof output !== "string" || output.length > 16384) return null;
    const result: unknown = JSON.parse(output);
    if (!result || typeof result !== "object" || Array.isArray(result)) return null;
    const value = result as Record<string, unknown>;
    if (Object.keys(value).sort().join() !== "ok,rows,statePage,version" || value.version !== 1
      || value.ok !== true || value.statePage !== statePage || !Array.isArray(value.rows)
      || value.rows.length !== (statePage ? 12 : 13)) return null;
    const rows: CrystalStatRow[] = [];
    for (const [index, row] of value.rows.entries()) {
      if (!row || typeof row !== "object" || Array.isArray(row)
        || Object.keys(row).sort().join() !== "text,top" || typeof row.text !== "string"
        || row.text.length > 256 || row.top !== 110 + index * 18) return null;
      rows.push(Object.freeze({ text: row.text, top: row.top }));
    }
    return Object.freeze(rows);
  } catch { return null; }
}
