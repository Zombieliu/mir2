#!/usr/bin/env node
// Re-runnable frontend completeness metrics for the mir2-web3 player web client.
//
// Computes, with numerator/denominator, how much of the wire protocol and UI surface
// the browser client implements, plus the committed asset inventory it ships with.
// Everything is derived from committed source so the script runs offline with no deps
// (no network, no server, no env). Path resolution is anchored to this script's own
// directory so it produces identical numbers regardless of the invocation cwd.
//
// Measured signals:
//   - ServerPacket handled vs defined: distinct `case "X"` kinds in app/page.tsx that
//     name a `pub enum ServerPacket` variant, over the total variant count.
//   - ClientPacket sent vs defined: distinct outbound `command.type === "X"` / `type: "X"`
//     tokens in app/page.tsx that name a `pub enum ClientPacket` variant (case-insensitive),
//     over the total variant count.
//   - UI window components: count of app/components/*.tsx and the dialog/window subset.
//   - Asset libraries: committed top-level dirs under public/original-ui and
//     public/original-map, with their committed PNG counts (via `git ls-files`).
//   - Map pack: committed lib/generated/crystal-map-pack/*.map.gz tiles.
//
// Usage:  node ./scripts/measure-frontend-coverage.mjs        (from apps/web)
//   or:   npm run measure:frontend-coverage
//
// Exit code is always 0; sections that cannot find their input degrade to "unavailable"
// rather than throwing, so the script is safe to run anywhere in the tree.

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const scriptDir = import.meta.dirname;
const webRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(webRoot, "..", "..");

const pageTsxPath = firstExisting([
  path.join(webRoot, "app", "page.tsx"),
  path.join(scriptDir, "..", "app", "page.tsx"),
]);

const packetsRsPath = firstExisting([
  path.join(repoRoot, "packages", "protocol", "src", "packets.rs"),
  path.join(webRoot, "..", "..", "packages", "protocol", "src", "packets.rs"),
  path.join(scriptDir, "..", "..", "..", "packages", "protocol", "src", "packets.rs"),
]);

const componentsDir = firstExisting([path.join(webRoot, "app", "components")]);

// ---------------------------------------------------------------------------
// Compute every section.
// ---------------------------------------------------------------------------
const serverVariants = parseEnumVariants(packetsRsPath, "ServerPacket");
const clientVariants = parseEnumVariants(packetsRsPath, "ClientPacket");

const pageSrc = pageTsxPath && existsSync(pageTsxPath) ? readFileSync(pageTsxPath, "utf8") : null;

const serverHandled = measureServerPackets(pageSrc, serverVariants);
const clientSent = measureClientPackets(pageSrc, clientVariants);
const components = measureComponents(componentsDir);
const originalUi = measureAssetLibrary("public/original-ui");
const originalMap = measureAssetLibrary("public/original-map");
const mapPack = measureMapPack("lib/generated/crystal-map-pack");

// ---------------------------------------------------------------------------
// Render.
// ---------------------------------------------------------------------------
const rows = [
  ratioRow("ServerPacket handlers", serverHandled),
  ratioRow("ClientPacket senders", clientSent),
  ratioRow("UI dialog/window components", components.windows),
  countRow("UI components (total .tsx)", components.total.numerator, "components", components.total.available),
  countRow("original-ui asset libraries", originalUi.libraries, "dirs", originalUi.available),
  countRow("original-ui PNG frames", originalUi.pngs, "png", originalUi.available),
  countRow("original-map asset libraries", originalMap.libraries, "dirs", originalMap.available),
  countRow("original-map PNG frames", originalMap.pngs, "png", originalMap.available),
  countRow("crystal map pack", mapPack.count, ".map.gz", mapPack.available),
];

printHeader();
printTable(rows);
printSummary();
printSourceNotes();

process.exit(0);

// ===========================================================================
// Measurement helpers
// ===========================================================================

// Parse top-level variant names of `pub enum <Name> { ... }` from a Rust source file
// using brace-depth tracking, so the enum body is located without hard-coded line
// numbers. Variants are the CamelCase identifiers indented exactly one level (4 spaces)
// inside the enum body, whether unit (`Foo,`), struct (`Foo {`), or tuple (`Foo(`).
function parseEnumVariants(filePath, enumName) {
  if (!filePath || !existsSync(filePath)) {
    return { available: false, names: [] };
  }
  const src = readFileSync(filePath, "utf8");
  const lines = src.split(/\r?\n/);
  const startIdx = lines.findIndex((l) => new RegExp(`\\bpub enum ${enumName}\\b`).test(l));
  if (startIdx === -1) {
    return { available: false, names: [] };
  }

  const names = new Set();
  let depth = 0;
  let started = false;
  // Variant lines look like: `    Foo {` / `    Foo(` / `    Foo,` / `    Foo = 1,`.
  const variantRe = /^ {4}([A-Z][A-Za-z0-9]*)\s*(?:\{|\(|,|=|$)/;

  for (let i = startIdx; i < lines.length; i++) {
    const line = lines[i];
    // Only inspect a line for a variant name while we are exactly at the enum's
    // top level (depth === 1) and not inside a nested struct/tuple body.
    if (started && depth === 1) {
      const m = variantRe.exec(line);
      if (m) names.add(m[1]);
    }
    for (const ch of line) {
      if (ch === "{") {
        depth++;
        started = true;
      } else if (ch === "}") {
        depth--;
      }
    }
    if (started && depth === 0) break; // closed the enum body
  }

  return { available: true, names: [...names].sort() };
}

// Distinct PascalCase `case "X"` kinds in page.tsx that name a ServerPacket variant.
function measureServerPackets(src, variants) {
  if (!src || !variants.available) {
    return { available: false, numerator: 0, denominator: variants.names.length };
  }
  const cases = distinctTokens(src, /case\s+"([A-Za-z0-9]+)"/g);
  const defined = new Set(variants.names);
  const handled = [...cases].filter((c) => defined.has(c));
  return {
    available: true,
    numerator: handled.length,
    denominator: variants.names.length,
    matched: handled.sort(),
  };
}

// Distinct outbound `command.type === "X"` / `type: "X"` tokens in page.tsx that name a
// ClientPacket variant. Matching is case-insensitive because the client emits camelCase
// tokens (`clientVersion`, `dropGold`) for PascalCase Rust variants (`ClientVersion`,
// `DropGold`). Tokens with no matching variant are higher-level UI/transport messages
// (e.g. moveTo, castSkill, error, packet, tick) and are excluded from the numerator.
function measureClientPackets(src, variants) {
  if (!src || !variants.available) {
    return { available: false, numerator: 0, denominator: variants.names.length };
  }
  const outbound = new Set([
    ...distinctTokens(src, /command\.type\s*===\s*"([A-Za-z0-9]+)"/g),
    ...distinctTokens(src, /type:\s*"([A-Za-z0-9]+)"/g),
  ]);
  const definedLc = new Map(variants.names.map((n) => [n.toLowerCase(), n]));
  const matched = [...outbound]
    .map((t) => definedLc.get(t.toLowerCase()))
    .filter(Boolean);
  return {
    available: true,
    numerator: new Set(matched).size,
    denominator: variants.names.length,
    outboundTokens: outbound.size,
    matched: [...new Set(matched)].sort(),
  };
}

function measureComponents(dir) {
  if (!dir || !existsSync(dir)) {
    return { available: false, total: { numerator: 0 }, windows: { numerator: 0, denominator: 0 } };
  }
  const files = readdirSync(dir).filter((f) => f.endsWith(".tsx"));
  // Windowing surface: dialogs, windows, panels, overlays, tooltips, menus.
  const windowRe = /(dialog|window|panel|overlay|tooltip|menu)/i;
  const windows = files.filter((f) => windowRe.test(f));
  return {
    available: true,
    total: { available: true, numerator: files.length },
    windows: {
      available: true,
      numerator: windows.length,
      denominator: files.length,
      files: windows.sort(),
    },
  };
}

// Count committed top-level subdirectories under `relRoot` and their committed PNGs.
function measureAssetLibrary(relRoot) {
  const files = gitLsFiles(relRoot);
  if (files === null) {
    // Git unavailable: fall back to a filesystem walk over committed-style content.
    return measureAssetLibraryFs(relRoot);
  }
  const prefix = `${relRoot.replace(/\\/g, "/")}/`;
  const libraries = new Set();
  let pngs = 0;
  for (const f of files) {
    const norm = f.replace(/\\/g, "/");
    const rest = norm.startsWith(prefix) ? norm.slice(prefix.length) : norm;
    const slash = rest.indexOf("/");
    if (slash > 0) libraries.add(rest.slice(0, slash)); // first path segment = a library dir
    if (norm.toLowerCase().endsWith(".png")) pngs++;
  }
  return { available: files.length > 0, libraries: libraries.size, pngs, source: "git" };
}

function measureAssetLibraryFs(relRoot) {
  const abs = path.join(webRoot, relRoot);
  if (!existsSync(abs)) return { available: false, libraries: 0, pngs: 0, source: "fs" };
  let libraries = 0;
  let pngs = 0;
  for (const entry of readdirSync(abs)) {
    const full = path.join(abs, entry);
    if (statSync(full).isDirectory()) {
      libraries++;
      pngs += countPngsRecursive(full);
    }
  }
  return { available: true, libraries, pngs, source: "fs" };
}

function countPngsRecursive(dir) {
  let n = 0;
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) n += countPngsRecursive(full);
    else if (entry.toLowerCase().endsWith(".png")) n++;
  }
  return n;
}

function measureMapPack(relRoot) {
  const files = gitLsFiles(relRoot);
  if (files !== null) {
    const count = files.filter((f) => f.toLowerCase().endsWith(".map.gz")).length;
    return { available: count > 0, count, source: "git" };
  }
  const abs = path.join(webRoot, relRoot);
  if (!existsSync(abs)) return { available: false, count: 0, source: "fs" };
  const count = readdirSync(abs).filter((f) => f.toLowerCase().endsWith(".map.gz")).length;
  return { available: true, count, source: "fs" };
}

// ===========================================================================
// Low-level utilities
// ===========================================================================

function distinctTokens(src, regex) {
  const out = new Set();
  let m;
  while ((m = regex.exec(src)) !== null) out.add(m[1]);
  return out;
}

// Run `git ls-files <relPath>` anchored at webRoot so output is independent of cwd.
// Returns an array of repo paths (relative to webRoot) or null if git is unavailable.
function gitLsFiles(relPath) {
  try {
    const out = execFileSync("git", ["ls-files", "--", relPath], {
      cwd: webRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 64 * 1024 * 1024,
    });
    return out.split(/\r?\n/).filter(Boolean);
  } catch {
    return null;
  }
}

function firstExisting(candidates) {
  for (const c of candidates) {
    if (c && existsSync(c)) return path.resolve(c);
  }
  return null;
}

// ===========================================================================
// Rendering
// ===========================================================================

function ratioRow(label, m) {
  if (!m || !m.available) {
    return { label, value: "unavailable", pct: "" };
  }
  const pct = m.denominator > 0 ? `${((m.numerator / m.denominator) * 100).toFixed(1)}%` : "n/a";
  return { label, value: `${m.numerator} / ${m.denominator}`, pct };
}

function countRow(label, value, unit, available) {
  if (!available || value === undefined || value === null) {
    return { label, value: "unavailable", pct: "" };
  }
  return { label, value: `${value.toLocaleString("en-US")} ${unit}`, pct: "" };
}

function printHeader() {
  console.log("");
  console.log("Frontend completeness coverage  (mir2-web3 / apps/web)");
  console.log(`page.tsx : ${rel(pageTsxPath)}`);
  console.log(`protocol : ${rel(packetsRsPath)}`);
  console.log(`measured : ${new Date().toISOString()}`);
}

function printTable(rows) {
  const labelW = Math.max(...rows.map((r) => r.label.length), "Signal".length);
  const valueW = Math.max(...rows.map((r) => r.value.length), "Numerator / Denominator".length);
  const bar = `+-${"-".repeat(labelW)}-+-${"-".repeat(valueW)}-+-------+`;
  console.log("");
  console.log(bar);
  console.log(`| ${pad("Signal", labelW)} | ${pad("Numerator / Denominator", valueW)} | %     |`);
  console.log(bar);
  for (const r of rows) {
    console.log(`| ${pad(r.label, labelW)} | ${pad(r.value, valueW)} | ${pad(r.pct, 5)} |`);
  }
  console.log(bar);
}

function printSummary() {
  console.log("");
  console.log("Summary");
  if (serverHandled.available) {
    console.log(
      `  ServerPacket coverage : ${serverHandled.numerator}/${serverHandled.denominator} ` +
        `(${pctOf(serverHandled)}) variants handled by a case branch in page.tsx`,
    );
  }
  if (clientSent.available) {
    console.log(
      `  ClientPacket coverage : ${clientSent.numerator}/${clientSent.denominator} ` +
        `(${pctOf(clientSent)}) variants emitted from page.tsx ` +
        `(${clientSent.outboundTokens} distinct outbound tokens total)`,
    );
  }
  if (components.total.available) {
    console.log(
      `  UI components         : ${components.total.numerator} .tsx files, ` +
        `${components.windows.numerator} dialog/window/panel surfaces`,
    );
  }
  if (originalUi.available) {
    console.log(
      `  original-ui assets    : ${originalUi.libraries} libraries, ` +
        `${originalUi.pngs.toLocaleString("en-US")} committed PNG frames`,
    );
  }
  if (originalMap.available) {
    console.log(
      `  original-map assets   : ${originalMap.libraries} libraries, ` +
        `${originalMap.pngs.toLocaleString("en-US")} committed PNG frames`,
    );
  }
  if (mapPack.available) {
    console.log(
      `  crystal map pack      : ${mapPack.count.toLocaleString("en-US")} committed .map.gz tiles`,
    );
  }
}

function printSourceNotes() {
  console.log("");
  console.log("Notes");
  console.log("  - Protocol counts are parsed from `pub enum ServerPacket` / `pub enum ClientPacket`.");
  console.log("  - ServerPacket numerator = distinct PascalCase `case \"X\"` kinds in page.tsx");
  console.log("    that name a defined variant (direction/chat-channel cases are excluded).");
  console.log("  - ClientPacket numerator = distinct outbound `command.type === \"X\"` / `type: \"X\"`");
  console.log("    tokens whose name matches a variant (case-insensitive). Non-protocol UI/transport");
  console.log("    tokens (moveTo, castSkill, error, packet, tick, ...) are excluded from the numerator.");
  console.log("  - Asset counts use committed files only (`git ls-files`).");
  console.log("");
  console.log("Re-run: npm run measure:frontend-coverage   (from apps/web)");
  console.log("");
}

function pad(s, w) {
  s = String(s ?? "");
  return s.length >= w ? s : s + " ".repeat(w - s.length);
}

function pctOf(m) {
  return m.denominator > 0 ? `${((m.numerator / m.denominator) * 100).toFixed(1)}%` : "n/a";
}

function rel(p) {
  if (!p) return "(not found)";
  const r = path.relative(repoRoot, p);
  return r.startsWith("..") ? p : r;
}
