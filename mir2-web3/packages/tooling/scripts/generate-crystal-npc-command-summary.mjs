import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const manifestPath = path.join(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_npc_manifest.json",
);
const outJsonPath = path.join(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_npc_command_summary.json",
);
const outMarkdownPath = path.join(
  repoRoot,
  "docs",
  "generated",
  "crystal-npc-command-summary.md",
);

const KNOWN_ACTION_COMMANDS = new Set([
  "BREAK",
  "BREAKTIMERECALL",
  "ADDTOGUILD",
  "ADDNAMELIST",
  "BUYGT",
  "CALC",
  "CHANGEHAIR",
  "CHANGELEVEL",
  "CHECKITEM",
  "CLEARPETS",
  "CLOSE",
  "CLOSEGATE",
  "CONQUESTGATE",
  "CONQUESTGUARD",
  "CONQUESTREPAIRALL",
  "CONQUESTWALL",
  "DELNAMELIST",
  "ELSESAY",
  "EXTENDGT",
  "GIVEEXP",
  "GIVEBUFF",
  "GIVEGOLD",
  "GIVEITEM",
  "GIVEPET",
  "GIVESKILL",
  "GOTO",
  "GLOBALMESSAGE",
  "GROUPGOTO",
  "GROUPTELEPORT",
  "GTALLRECALL",
  "GTRECALL",
  "LINEMESSAGE",
  "LOADVALUE",
  "LOCALMESSAGE",
  "MONCLEAR",
  "MONGEN",
  "MOV",
  "MOVE",
  "OPENGATE",
  "PARAM1",
  "PARAM2",
  "PARAM3",
  "REMOVEPET",
  "REVIVEHERO",
  "SAVEVALUE",
  "SEALHERO",
  "SET",
  "SETCONQUESTRATE",
  "STARTCONQUEST",
  "TAKEGOLD",
  "TAKEITEM",
  "TAKECONQUESTGOLD",
  "TELEPORTGT",
  "TIMERECALL",
]);

const KNOWN_CONDITION_COMMANDS = new Set([
  "AFFORDGATE",
  "AFFORDGUARD",
  "AFFORDWALL",
  "CHECK",
  "CHECKBUFF",
  "CHECKCALC",
  "CHECKCLASS",
  "CHECKCONQUEST",
  "CHECKEXACTMON",
  "CHECKGENDER",
  "CHECKGOLD",
  "CHECKHUM",
  "CHECKITEM",
  "CHECKMAP",
  "CHECKMAPLIGHT",
  "CHECKMON",
  "CHECKPET",
  "CHECKPERMISSION",
  "CHECKPKPOINT",
  "CHECKQUEST",
  "CHECKRANGE",
  "CONQUESTOWNER",
  "DAYOFWEEK",
  "GENDER",
  "GROUPCHECKNEARBY",
  "GROUPCOUNT",
  "GROUPLEADER",
  "HASBAGSPACE",
  "HASGT",
  "HOUR",
  "INGUILD",
  "ISADMIN",
  "LEVEL",
  "MIN",
  "PETCOUNT",
  "PETLEVEL",
  "RANDOM",
]);

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function firstToken(line) {
  return line.trim().split(/\s+/)[0]?.toUpperCase() ?? "";
}

function commandKey(kind, command) {
  return `${kind}:${command}`;
}

function getOrCreateCommand(commands, kind, command) {
  const key = commandKey(kind, command);
  if (!commands.has(key)) {
    commands.set(key, {
      kind,
      command,
      count: 0,
      scriptCount: 0,
      runtimeStatus: "unimplemented",
      examples: [],
      scripts: new Set(),
    });
  }
  return commands.get(key);
}

function recordCommand(commands, kind, command, scriptKey, lineNumber, line) {
  if (!command) return;
  const entry = getOrCreateCommand(commands, kind, command);
  entry.count += 1;
  entry.scripts.add(scriptKey);
  if (entry.examples.length < 5) {
    entry.examples.push({ scriptKey, lineNumber, line });
  }
}

function scanScript(commands, script) {
  for (const section of script.sections ?? []) {
    let mode = "none";
    for (const [index, rawLine] of (section.lines ?? []).entries()) {
      const line = String(rawLine).trim();
      if (!line) continue;
      if (line.startsWith(";") || line.startsWith("//")) continue;
      if (line.startsWith("[") && line.endsWith("]")) break;
      if (line.startsWith("#")) {
        mode = line.toUpperCase();
        continue;
      }

      const lineNumber = Number(section.line_number ?? 0) + index;
      if (mode === "#IF") {
        recordCommand(commands, "condition", firstToken(line), script.script_key, lineNumber, line);
      } else if (mode === "#ACT" || mode === "#ELSEACT") {
        recordCommand(commands, "action", firstToken(line), script.script_key, lineNumber, line);
      }
    }
  }
}

function finalizeCommands(commands) {
  return [...commands.values()]
    .map((entry) => {
      const known =
        entry.kind === "action"
          ? KNOWN_ACTION_COMMANDS.has(entry.command)
          : KNOWN_CONDITION_COMMANDS.has(entry.command);
      return {
        kind: entry.kind,
        command: entry.command,
        count: entry.count,
        scriptCount: entry.scripts.size,
        runtimeStatus: known ? "implemented" : "unimplemented",
        examples: entry.examples,
      };
    })
    .sort((a, b) => {
      if (a.runtimeStatus !== b.runtimeStatus) {
        return a.runtimeStatus === "unimplemented" ? -1 : 1;
      }
      if (b.count !== a.count) return b.count - a.count;
      return `${a.kind}:${a.command}`.localeCompare(`${b.kind}:${b.command}`);
    });
}

function markdownTable(rows) {
  const lines = [
    "| Kind | Command | Count | Scripts | Status | Example |",
    "| --- | --- | ---: | ---: | --- | --- |",
  ];
  for (const row of rows) {
    const example = row.examples[0]
      ? `${row.examples[0].scriptKey}:${row.examples[0].lineNumber}`
      : "";
    lines.push(
      `| ${row.kind} | \`${row.command}\` | ${row.count} | ${row.scriptCount} | ${row.runtimeStatus} | ${example} |`,
    );
  }
  return lines.join("\n");
}

const manifest = readJson(manifestPath);
const commands = new Map();
for (const script of manifest.scripts ?? []) {
  scanScript(commands, script);
}

const commandRows = finalizeCommands(commands);
const unimplemented = commandRows.filter((row) => row.runtimeStatus === "unimplemented");
const implemented = commandRows.filter((row) => row.runtimeStatus === "implemented");
const summary = {
  generated_at: new Date().toISOString(),
  source_file: "packages/game-data/data/generated/crystal_npc_manifest.json",
  total_scripts: manifest.total_scripts,
  total_commands: commandRows.length,
  implemented_commands: implemented.length,
  unimplemented_commands: unimplemented.length,
  implemented_occurrences: implemented.reduce((sum, row) => sum + row.count, 0),
  unimplemented_occurrences: unimplemented.reduce((sum, row) => sum + row.count, 0),
  commands: commandRows,
};

fs.writeFileSync(outJsonPath, `${JSON.stringify(summary, null, 2)}\n`);
fs.mkdirSync(path.dirname(outMarkdownPath), { recursive: true });
fs.writeFileSync(
  outMarkdownPath,
  [
    "# Crystal NPC Command Summary",
    "",
    `Generated at: ${summary.generated_at}`,
    "",
    `Source scripts: ${summary.total_scripts}`,
    "",
    `Runtime command coverage: ${summary.implemented_commands}/${summary.total_commands} command names implemented, ${summary.implemented_occurrences}/${summary.implemented_occurrences + summary.unimplemented_occurrences} command occurrences covered by the current Rust baseline.`,
    "",
    "## Unimplemented Commands",
    "",
    unimplemented.length ? markdownTable(unimplemented) : "No unimplemented command names found.",
    "",
    "## Implemented Commands",
    "",
    markdownTable(implemented),
    "",
  ].join("\n"),
);

console.log(
  `Wrote ${path.relative(repoRoot, outJsonPath)} and ${path.relative(
    repoRoot,
    outMarkdownPath,
  )}`,
);
