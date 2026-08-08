import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const crystalServerRoot = resolve(repoRoot, "..", "Crystal", "Server");
const crystalDropsRoot = resolve(
  repoRoot,
  "..",
  "Crystal",
  "Build",
  "Server",
  "Debug",
  "Envir",
  "Drops",
);
const crystalNpcsRoot = resolve(
  repoRoot,
  "..",
  "Crystal",
  "Build",
  "Server",
  "Debug",
  "Envir",
  "NPCs",
);
const crystalConfigsRoot = resolve(
  repoRoot,
  "..",
  "Crystal",
  "Build",
  "Server",
  "Debug",
  "Configs",
);
const envirPath = resolve(crystalServerRoot, "MirEnvir", "Envir.cs");
const buffInfoPath = resolve(crystalServerRoot, "MirDatabase", "BuffInfo.cs");
const randomItemStatsPath = resolve(crystalConfigsRoot, "RandomItemStats.ini");
const outputDir = resolve(repoRoot, "packages", "game-data", "data", "generated");
const magicOutputPath = resolve(outputDir, "crystal_magic_manifest.json");
const buffOutputPath = resolve(outputDir, "crystal_buff_manifest.json");
const dropOutputPath = resolve(outputDir, "crystal_drop_manifest.json");
const npcOutputPath = resolve(outputDir, "crystal_npc_manifest.json");
const randomItemStatsOutputPath = resolve(
  outputDir,
  "crystal_random_item_stats_manifest.json",
);

function extractMethodBody(source, signature) {
  const signatureIndex = source.indexOf(signature);
  if (signatureIndex === -1) {
    throw new Error(`could not find method signature: ${signature}`);
  }

  const openBraceIndex = source.indexOf("{", signatureIndex);
  if (openBraceIndex === -1) {
    throw new Error(`could not find method body start for: ${signature}`);
  }

  let depth = 0;
  for (let index = openBraceIndex; index < source.length; index += 1) {
    const char = source[index];
    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        return source.slice(openBraceIndex + 1, index);
      }
    }
  }

  throw new Error(`could not find method body end for: ${signature}`);
}

function parseObjectAssignments(block) {
  const assignments = {};
  const pattern = /([A-Za-z_][A-Za-z0-9_]*)\s*=\s*("(?:[^"\\]|\\.)*"|[A-Za-z0-9_.]+(?:\s*\|\s*[A-Za-z0-9_.]+)*|-?\d+(?:\.\d+)?f?|true|false)/g;
  for (const match of block.matchAll(pattern)) {
    assignments[match[1]] = match[2].trim();
  }
  return assignments;
}

function parseScalar(value) {
  const trimmed = value.trim();
  if (trimmed.startsWith("\"") && trimmed.endsWith("\"")) {
    return trimmed.slice(1, -1);
  }
  if (trimmed === "true") {
    return true;
  }
  if (trimmed === "false") {
    return false;
  }
  if (/^-?\d+(?:\.\d+)?f?$/.test(trimmed)) {
    return Number(trimmed.replace(/f$/, ""));
  }
  if (trimmed.includes(".")) {
    return trimmed.split(".").at(-1);
  }
  return trimmed;
}

function parseMagicEntries(fillMethodBody) {
  const defaults = {
    baseCost: 0,
    levelCost: 0,
    icon: 0,
    level1: 0,
    level2: 0,
    level3: 0,
    need1: 0,
    need2: 0,
    need3: 0,
    delayBase: 1800,
    delayReduction: 0,
    powerBase: 0,
    powerBonus: 0,
    mpowerBase: 0,
    mpowerBonus: 0,
    multiplierBase: 1,
    multiplierBonus: 0,
    range: 9,
  };

  const entries = [];
  const pattern = /MagicInfoList\.Add\(new MagicInfo\s*\{([\s\S]*?)\}\s*\);/g;
  for (const match of fillMethodBody.matchAll(pattern)) {
    const assignments = parseObjectAssignments(match[1]);
    entries.push({
      ...defaults,
      name: parseScalar(assignments.Name ?? "\"\""),
      spell: parseScalar(assignments.Spell ?? ""),
      baseCost: parseScalar(assignments.BaseCost ?? "0"),
      levelCost: parseScalar(assignments.LevelCost ?? "0"),
      icon: parseScalar(assignments.Icon ?? "0"),
      level1: parseScalar(assignments.Level1 ?? "0"),
      level2: parseScalar(assignments.Level2 ?? "0"),
      level3: parseScalar(assignments.Level3 ?? "0"),
      need1: parseScalar(assignments.Need1 ?? "0"),
      need2: parseScalar(assignments.Need2 ?? "0"),
      need3: parseScalar(assignments.Need3 ?? "0"),
      delayBase: parseScalar(assignments.DelayBase ?? "1800"),
      delayReduction: parseScalar(assignments.DelayReduction ?? "0"),
      powerBase: parseScalar(assignments.PowerBase ?? "0"),
      powerBonus: parseScalar(assignments.PowerBonus ?? "0"),
      mpowerBase: parseScalar(assignments.MPowerBase ?? "0"),
      mpowerBonus: parseScalar(assignments.MPowerBonus ?? "0"),
      multiplierBase: parseScalar(assignments.MultiplierBase ?? "1"),
      multiplierBonus: parseScalar(assignments.MultiplierBonus ?? "0"),
      range: parseScalar(assignments.Range ?? "9"),
    });
  }

  return entries;
}

function applyMagicOverrides(entries, updateMethodBody) {
  const overrides = new Map();
  const casePattern = /case Spell\.([A-Za-z0-9_]+):([\s\S]*?)break;/g;
  for (const match of updateMethodBody.matchAll(casePattern)) {
    const spell = match[1];
    const body = match[2];
    const spellOverrides = {};
    const assignmentPattern = /MagicInfoList\[i\]\.([A-Za-z0-9_]+)\s*=\s*([^;]+);/g;
    for (const assignment of body.matchAll(assignmentPattern)) {
      const key = assignment[1];
      const value = parseScalar(assignment[2]);
      spellOverrides[key] = value;
    }
    overrides.set(spell, spellOverrides);
  }

  return entries.map((entry) => {
    const override = overrides.get(entry.spell);
    if (!override) {
      return entry;
    }

    return {
      ...entry,
      ...(override.BaseCost !== undefined ? { baseCost: override.BaseCost } : {}),
      ...(override.LevelCost !== undefined ? { levelCost: override.LevelCost } : {}),
      ...(override.Icon !== undefined ? { icon: override.Icon } : {}),
      ...(override.Level1 !== undefined ? { level1: override.Level1 } : {}),
      ...(override.Level2 !== undefined ? { level2: override.Level2 } : {}),
      ...(override.Level3 !== undefined ? { level3: override.Level3 } : {}),
      ...(override.Need1 !== undefined ? { need1: override.Need1 } : {}),
      ...(override.Need2 !== undefined ? { need2: override.Need2 } : {}),
      ...(override.Need3 !== undefined ? { need3: override.Need3 } : {}),
      ...(override.DelayBase !== undefined ? { delayBase: override.DelayBase } : {}),
      ...(override.DelayReduction !== undefined ? { delayReduction: override.DelayReduction } : {}),
      ...(override.PowerBase !== undefined ? { powerBase: override.PowerBase } : {}),
      ...(override.PowerBonus !== undefined ? { powerBonus: override.PowerBonus } : {}),
      ...(override.MPowerBase !== undefined ? { mpowerBase: override.MPowerBase } : {}),
      ...(override.MPowerBonus !== undefined ? { mpowerBonus: override.MPowerBonus } : {}),
      ...(override.MultiplierBase !== undefined
        ? { multiplierBase: override.MultiplierBase }
        : {}),
      ...(override.MultiplierBonus !== undefined
        ? { multiplierBonus: override.MultiplierBonus }
        : {}),
      ...(override.Range !== undefined ? { range: override.Range } : {}),
    };
  });
}

function parseBuffEntries(buffSource) {
  const entries = [];
  const pattern = /new BuffInfo\s*\{([^}]*)\}/g;
  for (const match of buffSource.matchAll(pattern)) {
    const assignments = parseObjectAssignments(match[1]);
    const properties = String(assignments.Properties ?? "BuffProperty.None")
      .split("|")
      .map((value) => parseScalar(value))
      .filter(Boolean);
    const visibleRaw = assignments.Visible?.trim();

    entries.push({
      buffType: parseScalar(assignments.Type ?? ""),
      stackType: parseScalar(assignments.StackType ?? "None"),
      properties,
      visible:
        visibleRaw === "true" || visibleRaw === "false" ? parseScalar(visibleRaw) : null,
      visibleExpression:
        visibleRaw && visibleRaw !== "true" && visibleRaw !== "false" ? visibleRaw : null,
    });
  }
  return entries;
}

function normalizePath(value) {
  return value.replaceAll("\\", "/");
}

function collectFiles(directory) {
  const entries = readdirSync(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFiles(fullPath));
      continue;
    }

    if (entry.isFile() && entry.name.toLowerCase().endsWith(".txt")) {
      files.push(fullPath);
    }
  }

  return files.sort((left, right) => left.localeCompare(right));
}

function ensureDropSection(sections) {
  if (sections.length === 0) {
    sections.push({
      name: "default",
      entries: [],
    });
  }

  return sections.at(-1);
}

function parseDropEntry(rawLine) {
  const match = rawLine.match(/^(?<chance>\d+\/\d+|0)\s*(?<rest>.+)$/);
  if (!match?.groups) {
    throw new Error(`could not parse drop line: ${rawLine}`);
  }

  const chanceRaw = match.groups.chance;
  let chanceNumerator = null;
  let chanceDenominator = null;

  if (chanceRaw.includes("/")) {
    const [numerator, denominator] = chanceRaw.split("/");
    chanceNumerator = Number(numerator);
    chanceDenominator = Number(denominator);
  }

  const tokens = match.groups.rest.trim().split(/\s+/);
  const modifiers = [];
  while (tokens.length > 1 && /^(?:Q|LV\d*)$/.test(tokens.at(-1))) {
    modifiers.unshift(tokens.pop());
  }

  let amount = null;
  if (tokens.length > 1 && /^\d+$/.test(tokens.at(-1))) {
    amount = Number(tokens.pop());
  }

  const itemName = tokens.join(" ");
  const groupMatch = itemName.match(/^GROUP([*^])?$/i);
  const entry = {
    raw_line: rawLine,
    chance_raw: chanceRaw,
    chance_numerator: chanceNumerator,
    chance_denominator: chanceDenominator,
    item_name: groupMatch ? "GROUP" : itemName,
    amount,
    modifiers,
  };

  if (groupMatch) {
    entry.group = {
      random: groupMatch[1] === "*",
      first: groupMatch[1] === "^",
      entries: [],
    };
  }

  return entry;
}

function countDropEntries(entries) {
  return entries.reduce((sum, entry) => {
    const childCount = entry.group ? countDropEntries(entry.group.entries) : 0;
    return sum + 1 + childCount;
  }, 0);
}

function parseDropGroup(lines, startIndex) {
  let sawOpenBrace = false;
  const entries = [];

  for (let index = startIndex; index < lines.length; index += 1) {
    const line = lines[index].trim();

    if (!sawOpenBrace) {
      if (line === "{") {
        sawOpenBrace = true;
      }
      continue;
    }

    if (line === "}") {
      return { entries, nextIndex: index + 1 };
    }

    if (!line || line.startsWith(";")) {
      continue;
    }

    const entry = parseDropEntry(line);
    if (entry.group) {
      const group = parseDropGroup(lines, index + 1);
      entry.group.entries = group.entries;
      index = group.nextIndex - 1;
    }
    entries.push(entry);
  }

  return { entries: [], nextIndex: lines.length };
}

function parseDropInsertLines(lines) {
  const keptLines = [];
  const insertedLines = [];

  for (const line of lines) {
    const insertMatch = line.trim().match(/^#INSERT\s+\[(.+?)\]/i);
    if (!insertMatch) {
      keptLines.push(line);
      continue;
    }

    const insertPath = resolve(crystalDropsRoot, insertMatch[1]);
    insertedLines.push(...readFileSync(insertPath, "utf8").split(/\r?\n/));
  }

  return [...keptLines, ...insertedLines];
}

function parseDropTable(filePath) {
  const source = readFileSync(filePath, "utf8");
  const relativePath = normalizePath(relative(crystalDropsRoot, filePath));
  const tableKey = relativePath.replace(/\.txt$/i, "");
  const sections = [];
  const lines = parseDropInsertLines(source.split(/\r?\n/));

  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const line = rawLine.trim();
    if (!line) {
      continue;
    }

    if (line.startsWith(";")) {
      sections.push({
        name: line.slice(1).trim() || "default",
        entries: [],
      });
      continue;
    }

    if (line === "{" || line === "}") {
      continue;
    }

    const entry = parseDropEntry(line);
    if (entry.group) {
      const group = parseDropGroup(lines, index + 1);
      entry.group.entries = group.entries;
      index = group.nextIndex - 1;
    }

    ensureDropSection(sections).entries.push(entry);
  }

  return {
    table_key: tableKey,
    relative_path: relativePath,
    total_entries: sections.reduce((sum, section) => sum + countDropEntries(section.entries), 0),
    sections,
  };
}

function parseDropTables(rootDir) {
  return collectFiles(rootDir).map(parseDropTable);
}

function parseNpcScript(filePath) {
  const source = readFileSync(filePath, "utf8");
  const lines = source.length === 0 ? [] : source.split(/\r?\n/);
  const relativePath = normalizePath(relative(crystalNpcsRoot, filePath));
  const scriptKey = relativePath.replace(/\.txt$/i, "");
  const labels = [];
  const inserts = [];
  const sections = [];
  let currentSection = null;
  const commandDirectives = new Set();

  for (const [index, rawLine] of lines.entries()) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }

    const labelMatch = line.match(/^\[(.+)\]$/);
    if (labelMatch) {
      currentSection = {
        label: labelMatch[1],
        line_number: index + 1,
        lines: [],
      };
      sections.push(currentSection);
      labels.push({
        label: labelMatch[1],
        line_number: index + 1,
      });
      continue;
    }

    if (currentSection) {
      currentSection.lines.push(rawLine);
    }

    const insertMatch = line.match(/^#INSERT\s+\[(.+?)\](?:\s+([^\s]+))?$/i);
    if (insertMatch) {
      inserts.push({
        line_number: index + 1,
        target_path: normalizePath(insertMatch[1]),
        target_label: insertMatch[2] ?? null,
      });
    }

    const commandMatch = line.match(/^(#[A-Za-z]+)/);
    if (commandMatch) {
      commandDirectives.add(commandMatch[1].toUpperCase());
    }
  }

  return {
    script_key: scriptKey,
    relative_path: relativePath,
    raw_text: source,
    lines,
    line_count: lines.length,
    non_empty_line_count: lines.filter((line) => line.trim().length > 0).length,
    label_count: labels.length,
    insert_count: inserts.length,
    command_directives: [...commandDirectives].sort((left, right) =>
      left.localeCompare(right),
    ),
    labels,
    sections,
    inserts,
  };
}

function parseNpcScripts(rootDir) {
  return collectFiles(rootDir).map(parseNpcScript);
}

function parseIniSections(source) {
  const sections = new Map();
  let currentSection = null;

  for (const rawLine of source.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith(";")) {
      continue;
    }

    const sectionMatch = line.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = {};
      sections.set(sectionMatch[1], currentSection);
      continue;
    }

    if (!currentSection) {
      continue;
    }

    const assignment = line.match(/^([^=]+)=(.*)$/);
    if (!assignment) {
      continue;
    }
    currentSection[assignment[1].trim()] = assignment[2].trim();
  }

  return sections;
}

function parseIniNumber(section, key) {
  const raw = section[key];
  if (raw === undefined) {
    throw new Error(`missing RandomItemStats key ${key}`);
  }
  const value = Number(raw);
  if (!Number.isInteger(value)) {
    throw new Error(`invalid RandomItemStats value ${key}=${raw}`);
  }
  return value;
}

function randomStatRoll(section, prefix, maxStatKey = `${prefix}MaxStat`) {
  return {
    chance: parseIniNumber(section, `${prefix}Chance`),
    stat_chance: parseIniNumber(section, `${prefix}StatChance`),
    max_stat: parseIniNumber(section, maxStatKey),
  };
}

function parseRandomItemStats(filePath) {
  const source = readFileSync(filePath, "utf8");
  const sections = parseIniSections(source);
  const profiles = [];

  for (const [sectionName, section] of sections.entries()) {
    const match = sectionName.match(/^Item(\d+)$/i);
    if (!match) {
      continue;
    }
    if (section.MaxDuraStatChance === undefined) {
      continue;
    }

    profiles.push({
      id: Number(match[1]),
      max_dura: randomStatRoll(section, "MaxDura"),
      max_ac: randomStatRoll(section, "MaxAc"),
      max_mac: randomStatRoll(section, "MaxMac", "MaxMACMaxStat"),
      max_dc: randomStatRoll(section, "MaxDc"),
      max_mc: randomStatRoll(section, "MaxMc"),
      max_sc: randomStatRoll(section, "MaxSc"),
      accuracy: randomStatRoll(section, "Accuracy"),
      agility: randomStatRoll(section, "Agility"),
      hp: randomStatRoll(section, "Hp"),
      mp: randomStatRoll(section, "Mp"),
      strong: randomStatRoll(section, "Strong"),
      magic_resist: randomStatRoll(section, "MagicResist"),
      poison_resist: randomStatRoll(section, "PoisonResist"),
      hp_recovery: randomStatRoll(section, "HpRecov"),
      mp_recovery: randomStatRoll(section, "MpRecov"),
      poison_recovery: randomStatRoll(section, "PoisonRecov"),
      critical_rate: randomStatRoll(section, "CriticalRate"),
      critical_damage: randomStatRoll(section, "CriticalDamage"),
      freezing: randomStatRoll(section, "Freeze"),
      poison_attack: randomStatRoll(section, "PoisonAttack"),
      attack_speed: randomStatRoll(section, "AttackSpeed"),
      luck: randomStatRoll(section, "Luck"),
      curse_chance: parseIniNumber(section, "CurseChance"),
      slot: randomStatRoll(section, "Slot"),
    });
  }

  return profiles.sort((left, right) => left.id - right.id);
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

const envirSource = readFileSync(envirPath, "utf8");
const buffSource = readFileSync(buffInfoPath, "utf8");
const fillMagicInfoBody = extractMethodBody(envirSource, "private void FillMagicInfoList()");
const updateMagicInfoBody = extractMethodBody(envirSource, "private void UpdateMagicInfo()");

const magics = applyMagicOverrides(parseMagicEntries(fillMagicInfoBody), updateMagicInfoBody).sort(
  (left, right) => left.spell.localeCompare(right.spell),
);
const buffs = parseBuffEntries(buffSource).sort((left, right) =>
  left.buffType.localeCompare(right.buffType),
);
const dropTables = parseDropTables(crystalDropsRoot);
const npcScripts = parseNpcScripts(crystalNpcsRoot);
const randomItemStats = parseRandomItemStats(randomItemStatsPath);

writeJson(magicOutputPath, {
  generated_at: new Date().toISOString(),
  source_file: "Crystal/Server/MirEnvir/Envir.cs",
  total_magics: magics.length,
  magics,
});

writeJson(buffOutputPath, {
  generated_at: new Date().toISOString(),
  source_file: "Crystal/Server/MirDatabase/BuffInfo.cs",
  total_buffs: buffs.length,
  buffs,
});

writeJson(dropOutputPath, {
  generated_at: new Date().toISOString(),
  source_dir: "Crystal/Build/Server/Debug/Envir/Drops",
  total_tables: dropTables.length,
  total_entries: dropTables.reduce((sum, table) => sum + table.total_entries, 0),
  tables: dropTables,
});

writeJson(npcOutputPath, {
  generated_at: new Date().toISOString(),
  source_dir: "Crystal/Build/Server/Debug/Envir/NPCs",
  total_scripts: npcScripts.length,
  total_labels: npcScripts.reduce((sum, script) => sum + script.label_count, 0),
  total_inserts: npcScripts.reduce((sum, script) => sum + script.insert_count, 0),
  scripts: npcScripts,
});

writeJson(randomItemStatsOutputPath, {
  generated_at: new Date().toISOString(),
  source_file: "Crystal/Build/Server/Debug/Configs/RandomItemStats.ini",
  total_profiles: randomItemStats.length,
  profiles: randomItemStats,
});

console.log(`wrote ${magicOutputPath}`);
console.log(`wrote ${buffOutputPath}`);
console.log(`wrote ${dropOutputPath}`);
console.log(`wrote ${npcOutputPath}`);
console.log(`wrote ${randomItemStatsOutputPath}`);
