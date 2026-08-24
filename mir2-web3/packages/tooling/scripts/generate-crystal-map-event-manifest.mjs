#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(SCRIPT_DIR, "../../..");
const DEFAULT_ENVIR_ROOT = path.resolve(PROJECT_ROOT, "../Crystal/Build/Server/Debug/Envir");
const DEFAULT_MAP_COORDS = "SystemScripts/00Default/MapCoords.txt";
const DEFAULT_EVENTS_DIR = "Events";
const DEFAULT_OUTPUT = path.resolve(PROJECT_ROOT, "packages/game-data/data/generated/crystal_map_event_manifest.json");
const DEFAULT_LIMITS = Object.freeze({ maxDepth: 32, maxFileBytes: 1024 * 1024, maxTotalBytes: 8 * 1024 * 1024, maxResolvedLines: 200_000 });

class MapEventImportError extends Error {
  constructor(message, diagnostics) { super(message); this.name = "MapEventImportError"; this.diagnostics = diagnostics; }
}

function diagnostics() { return { danglingPaths: [], pathTraversalRejected: [], cycles: [], warnings: [] }; }

function normalizeRelativePath(rawPath, diagnosticsState, context) {
  const raw = String(rawPath ?? "").trim().replaceAll("\\", "/");
  const reject = (reason) => {
    diagnosticsState.pathTraversalRejected.push({ rawPath: raw, reason, ...context });
    throw new MapEventImportError(`unsafe Crystal include path ${JSON.stringify(raw)} (${reason})`, diagnosticsState);
  };
  if (!raw || raw.startsWith("/") || /^[A-Za-z]:/.test(raw)) reject("absolute-or-empty-path");
  const parts = raw.split("/");
  if (parts.some((part) => part === "..")) reject("parent-segment");
  if (parts.some((part) => part.includes(":"))) reject("colon-segment");
  const normalized = parts.filter((part) => part && part !== ".").join("/");
  if (!normalized) reject("empty-normalized-path");
  return normalized;
}

function parseDirective(text, sourceFile, sourceLine, diagnosticsState) {
  const trimmed = text.trim();
  if (!/^#(?:INSERT|INCLUDE)\b/i.test(trimmed)) return null;
  const match = trimmed.match(/^#(INSERT|INCLUDE)\s+\[([^\]]+)\](?:\s+([^\s]+))?\s*$/i);
  if (!match) {
    diagnosticsState.warnings.push({ sourceFile, sourceLine, message: "malformed include/insert directive", text });
    throw new MapEventImportError(`malformed include/insert directive at ${sourceFile}:${sourceLine}`, diagnosticsState);
  }
  const kind = match[1].toUpperCase();
  if (kind === "INCLUDE" && !match[3]) {
    diagnosticsState.warnings.push({ sourceFile, sourceLine, message: "include directive is missing its section name", text });
    throw new MapEventImportError(`include directive without section at ${sourceFile}:${sourceLine}`, diagnosticsState);
  }
  return { kind, rawTarget: match[2], section: match[3] ?? null, sourceFile, sourceLine };
}

function mapCoordinateFromLine(text) {
  const trimmed = text.trim();
  if (trimmed.startsWith(";") || !trimmed.startsWith("[@_")) return null;
  const match = trimmed.match(/^\[@_MAPCOORD\(\s*([^,]+?)\s*,\s*(-?\d+)\s*,\s*(-?\d+)\s*\)\]\s*$/i);
  if (!match) return null;
  const rawMapId = match[1].trim();
  return {
    mapId: /^-?\d+$/.test(rawMapId) ? String(Number.parseInt(rawMapId, 10)) : rawMapId.replace(/\s+/g, " "),
    x: Number.parseInt(match[2], 10), y: Number.parseInt(match[3], 10), rawMapId,
  };
}

function sectionName(value) { return value.startsWith("[") && value.endsWith("]") ? value.slice(1, -1) : value; }

function makeImporter(envirRoot, requestedLimits = {}) {
  const root = path.resolve(envirRoot);
  const limits = { ...DEFAULT_LIMITS, ...requestedLimits };
  const state = { diagnostics: diagnostics(), rawFiles: new Map(), totalBytes: 0, resolvedLines: 0, references: [] };
  const checkedRelative = (rawPath, context) => normalizeRelativePath(rawPath, state.diagnostics, context);

  function loadRaw(relativePath, context = {}) {
    const normalized = checkedRelative(relativePath, context);
    const cached = state.rawFiles.get(normalized);
    if (cached) return cached;
    const absolute = path.resolve(root, ...normalized.split("/"));
    const relativeToRoot = path.relative(root, absolute);
    if (relativeToRoot.startsWith("..") || path.isAbsolute(relativeToRoot)) {
      state.diagnostics.pathTraversalRejected.push({ rawPath: relativePath, reason: "resolved-outside-envir-root", ...context });
      throw new MapEventImportError(`resolved Crystal path escapes Envir root: ${relativePath}`, state.diagnostics);
    }
    let bytes;
    try { bytes = fs.readFileSync(absolute); } catch {
      state.diagnostics.danglingPaths.push({ targetFile: normalized, absolutePath: absolute, ...context });
      throw new MapEventImportError(`Crystal include target does not exist: ${normalized}`, state.diagnostics);
    }
    if (bytes.byteLength > limits.maxFileBytes) throw new MapEventImportError(`Crystal source file exceeds maxFileBytes: ${normalized}`, state.diagnostics);
    if (state.totalBytes + bytes.byteLength > limits.maxTotalBytes) throw new MapEventImportError(`Crystal source set exceeds maxTotalBytes while loading ${normalized}`, state.diagnostics);
    state.totalBytes += bytes.byteLength;
    const lines = bytes.toString("utf8").split(/\r?\n/).map((line, index) => ({ text: line, sourceFile: normalized, sourceLine: index + 1, includeChain: [] }));
    const file = { path: normalized, bytes: bytes.byteLength, lines };
    state.rawFiles.set(normalized, file);
    return file;
  }

  function failCycle(target, stack, directive) {
    const cycle = [...stack, target];
    state.diagnostics.cycles.push({ targetFile: target, chain: cycle, sourceFile: directive.sourceFile, sourceLine: directive.sourceLine });
    throw new MapEventImportError(`Crystal include cycle at ${directive.sourceFile}:${directive.sourceLine}: ${cycle.join(" -> ")}`, state.diagnostics);
  }

  function ensureDepth(depth, directive) {
    if (depth <= limits.maxDepth) return;
    throw new MapEventImportError(`Crystal include depth exceeds ${limits.maxDepth} at ${directive.sourceFile}:${directive.sourceLine}`, state.diagnostics);
  }

  function resolveLines(lines, stack, depth, chain) {
    const output = [];
    for (const line of lines) {
      const directive = parseDirective(line.text, line.sourceFile, line.sourceLine, state.diagnostics);
      if (!directive) {
        output.push({ text: line.text, sourceFile: line.sourceFile, sourceLine: line.sourceLine, includeChain: chain });
        state.resolvedLines += 1;
        if (state.resolvedLines > limits.maxResolvedLines) throw new MapEventImportError(`resolved Crystal lines exceed ${limits.maxResolvedLines}`, state.diagnostics);
        continue;
      }
      const target = checkedRelative(directive.rawTarget, { sourceFile: directive.sourceFile, sourceLine: directive.sourceLine });
      const reference = { kind: directive.kind.toLowerCase(), sourceFile: directive.sourceFile, sourceLine: directive.sourceLine, targetFile: target, section: directive.section, resolved: false };
      state.references.push(reference);
      const childChain = [...chain, `${directive.sourceFile}:${directive.sourceLine}`];
      if (stack.includes(target)) failCycle(target, stack, directive);
      ensureDepth(depth + 1, directive);
      const child = loadRaw(target, directive);
      if (directive.kind === "INSERT") {
        output.push(...resolveLines(child.lines, [...stack, target], depth + 1, childChain));
      } else {
        const section = findSection(child, directive.section);
        if (!section) {
          state.diagnostics.danglingPaths.push({ targetFile: target, section: directive.section, sourceFile: directive.sourceFile, sourceLine: directive.sourceLine, reason: "section-not-found" });
          throw new MapEventImportError(`Crystal include section ${directive.section} not found in ${target}`, state.diagnostics);
        }
        output.push(...resolveLines(section.body, [...stack, target], depth + 1, childChain));
      }
      reference.resolved = true;
    }
    return output;
  }

  function findSection(file, requestedName) {
    const wanted = sectionName(requestedName).toUpperCase();
    for (let index = 0; index < file.lines.length; index += 1) {
      const match = file.lines[index].text.trim().match(/^\[([^\]]+)\]\s*$/);
      if (!match || match[1].toUpperCase() !== wanted) continue;
      let bodyStart = index + 1;
      while (bodyStart < file.lines.length && file.lines[bodyStart].text.trim() === "") bodyStart += 1;
      const braced = bodyStart < file.lines.length && file.lines[bodyStart].text.trim() === "{";
      if (braced) bodyStart += 1;
      let bodyEnd = file.lines.length;
      if (braced) {
        for (let cursor = bodyStart; cursor < file.lines.length; cursor += 1) {
          if (file.lines[cursor].text.trim() === "}") { bodyEnd = cursor; break; }
        }
        if (bodyEnd === file.lines.length) throw new MapEventImportError(`unterminated Crystal section ${requestedName} in ${file.path}:${index + 1}`, state.diagnostics);
      } else {
        for (let cursor = bodyStart; cursor < file.lines.length; cursor += 1) {
          if (/^\s*\[[^\]]+\]\s*$/.test(file.lines[cursor].text)) { bodyEnd = cursor; break; }
        }
      }
      return { name: match[1], header: `[${match[1]}]`, sourceFile: file.path, sourceLine: index + 1, braced, body: file.lines.slice(bodyStart, bodyEnd) };
    }
    return null;
  }

  function resolvedSection(relativePath, requestedName, stack, depth, chain) {
    const normalized = checkedRelative(relativePath, { sourceFile: stack.at(-1) ?? relativePath, sourceLine: 0 });
    if (stack.includes(normalized)) failCycle(normalized, stack, { sourceFile: stack.at(-1) ?? normalized, sourceLine: 0 });
    ensureDepth(depth + 1, { sourceFile: stack.at(-1) ?? normalized, sourceLine: 0 });
    const file = loadRaw(normalized);
    const section = findSection(file, requestedName);
    if (!section) {
      state.diagnostics.danglingPaths.push({ targetFile: normalized, section: requestedName, reason: "section-not-found" });
      throw new MapEventImportError(`Crystal section ${requestedName} not found in ${normalized}`, state.diagnostics);
    }
    return { name: section.name, header: section.header, sourceFile: section.sourceFile, sourceLine: section.sourceLine, braced: section.braced, lines: resolveLines(section.body, [...stack, normalized], depth + 1, chain) };
  }

  function resolvedFile(relativePath) {
    const normalized = checkedRelative(relativePath, { sourceFile: relativePath });
    const file = loadRaw(normalized);
    return { file, lines: resolveLines(file.lines, [normalized], 0, []) };
  }

  function sourceFileSections(file) {
    const sections = [];
    for (let index = 0; index < file.lines.length; index += 1) {
      const match = file.lines[index].text.trim().match(/^\[([^\]]+)\]\s*$/);
      if (!match) continue;
      const section = findSection(file, match[1]);
      sections.push({ name: section.name, header: section.header, sourceFile: section.sourceFile, sourceLine: section.sourceLine, braced: section.braced, lines: resolveLines(section.body, [file.path], 0, []) });
    }
    return sections;
  }

  function collectEventFiles(eventsRelativeDir) {
    const relativeDir = checkedRelative(eventsRelativeDir, { sourceFile: "<events-root>", sourceLine: 0 });
    const absoluteDir = path.resolve(root, ...relativeDir.split("/"));
    if (!fs.existsSync(absoluteDir)) throw new MapEventImportError(`Crystal Events directory does not exist: ${relativeDir}`, state.diagnostics);
    const files = [];
    function walk(directory) {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
        const absolute = path.join(directory, entry.name);
        if (entry.isDirectory()) walk(absolute);
        else if (entry.isFile() && entry.name.toLowerCase().endsWith(".txt")) files.push(path.relative(root, absolute).split(path.sep).join("/"));
      }
    }
    walk(absoluteDir);
    return files.sort((left, right) => left.localeCompare(right));
  }

  function build({ mapCoordsRelative = DEFAULT_MAP_COORDS, eventsRelativeDir = DEFAULT_EVENTS_DIR } = {}) {
    const mapCoordsPath = checkedRelative(mapCoordsRelative, { sourceFile: "<map-coordinates-root>", sourceLine: 0 });
    const mapFile = loadRaw(mapCoordsPath);
    const mapCoordinates = [];
    for (let index = 0; index < mapFile.lines.length; index += 1) {
      const line = mapFile.lines[index];
      const coordinate = mapCoordinateFromLine(line.text);
      if (!coordinate) continue;
      let include = null;
      for (let cursor = index + 1; cursor < mapFile.lines.length; cursor += 1) {
        const next = mapFile.lines[cursor];
        if (mapCoordinateFromLine(next.text)) break;
        const directive = parseDirective(next.text, next.sourceFile, next.sourceLine, state.diagnostics);
        if (directive?.kind === "INCLUDE") { include = directive; break; }
      }
      if (!include) throw new MapEventImportError(`Map coordinate has no include at ${line.sourceFile}:${line.sourceLine}`, state.diagnostics);
      const target = checkedRelative(include.rawTarget, include);
      state.references.push({ kind: 'include', sourceFile: include.sourceFile, sourceLine: include.sourceLine, targetFile: target, section: include.section, resolved: true });
      mapCoordinates.push({
        mapId: coordinate.mapId, x: coordinate.x, y: coordinate.y,
        eventId: include.section, eventName: sectionName(include.section),
        bindingSourceFile: line.sourceFile, bindingSourceLine: line.sourceLine,
        include: { sourceFile: include.sourceFile, sourceLine: include.sourceLine, targetFile: target, section: include.section },
        resolvedSection: resolvedSection(target, include.section, [mapCoordsPath], 0, [`${include.sourceFile}:${include.sourceLine}`]),
      });
    }
    const eventFiles = collectEventFiles(eventsRelativeDir).map((relative) => {
      const resolved = resolvedFile(relative);
      return { sourceFile: resolved.file.path, bytes: resolved.file.bytes, resolvedLines: resolved.lines, sections: sourceFileSections(resolved.file) };
    });
    const references = [...state.references].sort((left, right) => `${left.sourceFile}:${left.sourceLine}:${left.kind}:${left.targetFile}:${left.section ?? ""}`.localeCompare(`${right.sourceFile}:${right.sourceLine}:${right.kind}:${right.targetFile}:${right.section ?? ""}`));
    return {
      schemaVersion: 1,
      source: { envirRoot: "Envir", mapCoordinates: mapCoordsPath, events: checkedRelative(eventsRelativeDir, { sourceFile: "<events-root>", sourceLine: 0 }) },
      limits,
      mapCoordinates: mapCoordinates.sort((left, right) => `${left.mapId}:${left.x}:${left.y}:${left.bindingSourceLine}`.localeCompare(`${right.mapId}:${right.x}:${right.y}:${right.bindingSourceLine}`)),
      events: eventFiles, references, diagnostics: state.diagnostics,
    };
  }

  return { build, state };
}

function assert(condition, message) { if (!condition) throw new Error(`self-test assertion failed: ${message}`); }

function runSelfTests() {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-map-events-"));
  try {
    fs.mkdirSync(path.join(fixtureRoot, "Events"), { recursive: true });
    fs.writeFileSync(path.join(fixtureRoot, "MapCoords.txt"), "[@_MAPCOORD(0,1,2)]\n#INCLUDE [Events/a.txt] @Main\n");
    fs.writeFileSync(path.join(fixtureRoot, "Events/a.txt"), "[@Main]\n{\n#INSERT [Events/b.txt]\n}\n");
    fs.writeFileSync(path.join(fixtureRoot, "Events/b.txt"), "ok\n");
    const good = makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events" });
    assert(good.mapCoordinates.length === 1, "fixture binding resolves");
    assert(good.references.every((reference) => reference.resolved), "fixture references resolve");
    fs.writeFileSync(path.join(fixtureRoot, "Events/traversal.txt"), "#INSERT [../outside.txt]\n");
    let traversalFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events" }); } catch (error) { traversalFailed = error instanceof MapEventImportError && error.diagnostics.pathTraversalRejected.length > 0; }
    assert(traversalFailed, "path traversal fails closed");
    fs.rmSync(path.join(fixtureRoot, "Events/traversal.txt"));
    fs.writeFileSync(path.join(fixtureRoot, "Events/a.txt"), "[@Main]\n{\n#INSERT [Events/b.txt]\n}\n");
    fs.writeFileSync(path.join(fixtureRoot, "Events/b.txt"), "#INSERT [Events/a.txt]\n");
    let cycleFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events" }); } catch (error) { cycleFailed = error instanceof MapEventImportError && error.diagnostics.cycles.length > 0; }
    assert(cycleFailed, "include cycle fails closed");
    console.log("map-event self-test: 3/3 passed");
  } finally { fs.rmSync(fixtureRoot, { recursive: true, force: true }); }
}

function parseArgs(argv) {
  const args = { envirRoot: null, output: DEFAULT_OUTPUT, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--self-test") args.selfTest = true;
    else if (value === "--envir-root") args.envirRoot = argv[++index];
    else if (value === "--output") args.output = path.resolve(argv[++index]);
    else throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) runSelfTests();
  const envirRoot = args.envirRoot ?? process.env.MIR2_CRYSTAL_ENVIR_ROOT ?? process.env.MIR2_ENVIR_ROOT ?? DEFAULT_ENVIR_ROOT;
  const manifest = makeImporter(envirRoot).build();
  if (manifest.diagnostics.danglingPaths.length || manifest.diagnostics.pathTraversalRejected.length || manifest.diagnostics.cycles.length) throw new Error("Crystal map-event manifest contains unsafe or dangling diagnostics");
  fs.mkdirSync(path.dirname(args.output), { recursive: true });
  fs.writeFileSync(args.output, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`generated ${path.relative(PROJECT_ROOT, args.output)}: ${manifest.mapCoordinates.length} map bindings, ${manifest.events.length} event files, ${manifest.references.length} resolved directives`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; }
}

export { DEFAULT_LIMITS, MapEventImportError, makeImporter, runSelfTests };
