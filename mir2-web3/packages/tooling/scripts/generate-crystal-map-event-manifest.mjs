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
const DEFAULT_RESPAWN_MANIFEST = path.resolve(PROJECT_ROOT, "packages/game-data/data/generated/crystal_respawn_manifest.json");
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

function normalizeMapId(value) { return String(value).trim().replace(/\.map$/i, "").toUpperCase(); }

function e1Error(message, source) {
  const sourceFile = source.sourceFile ?? source.bindingSourceFile;
  const sourceLine = source.sourceLine ?? source.bindingSourceLine;
  throw new MapEventImportError(`${message} at ${sourceFile}:${sourceLine}`, { ...diagnostics(), warnings: [{ sourceFile, sourceLine, message }] });
}

function parseE1Condition(line) {
  const parts = line.text.trim().split(/\s+/);
  if (parts.length !== 3) e1Error(`unsupported E1 command ${JSON.stringify(line.text.trim())}`, line);
  const kind = { LEVEL: "level", CHECKPKPOINT: "pkPoints" }[parts[0].toUpperCase()];
  if (!kind || !["<", ">", "<=", ">=", "==", "!="].includes(parts[1]) || !/^-?\d+$/.test(parts[2])) {
    e1Error(`unsupported E1 command ${JSON.stringify(line.text.trim())}`, line);
  }
  return { kind, operator: parts[1], value: Number.parseInt(parts[2], 10), sourceFile: line.sourceFile, sourceLine: line.sourceLine };
}

function parseE1LocalMessage(line) {
  const match = line.text.trim().match(/^LocalMessage\s+"([^"]*)"\s+(\S+)\s*$/i);
  if (!match) e1Error(`unsupported E1 command ${JSON.stringify(line.text.trim())}`, line);
  return { kind: "localMessage", message: match[1], chatType: match[2], sourceFile: line.sourceFile, sourceLine: line.sourceLine };
}

function parseE1Binding(binding) {
  let phase = null;
  const conditions = [];
  let onPass = null;
  let onFail = null;
  for (const line of binding.resolvedSection.lines) {
    const text = line.text.trim();
    if (!text || text === "{" || text === "}") continue;
    const directive = text.toUpperCase();
    if (directive === "#IF") {
      if (phase !== null) e1Error("invalid E1 #IF phase", line);
      phase = "if";
      continue;
    }
    if (directive === "#ACT") {
      if (phase !== "if") e1Error("invalid E1 #ACT phase", line);
      phase = "act";
      continue;
    }
    if (directive === "#ELSEACT") {
      if (phase !== "act" || !onPass) e1Error("invalid E1 #ELSEACT phase", line);
      phase = "else";
      continue;
    }
    if (phase === "if") {
      conditions.push(parseE1Condition(line));
      continue;
    }
    if (phase === "act") {
      if (text.toUpperCase() !== "ENTERMAP") {
        e1Error(`unsupported E1 command ${JSON.stringify(text)}`, line);
      }
      if (onPass) e1Error("multiple E1 pass actions", line);
      onPass = { kind: "enterMap", sourceFile: line.sourceFile, sourceLine: line.sourceLine };
      continue;
    }
    if (phase === "else") {
      if (onFail) e1Error("multiple E1 fail actions", line);
      onFail = parseE1LocalMessage(line);
      continue;
    }
    e1Error(`unsupported E1 command ${JSON.stringify(text)}`, line);
  }
  if (!conditions.length) e1Error("E1 binding is missing a condition", binding);
  if (!onPass) e1Error("E1 binding is missing ENTERMAP", binding);
  if (!onFail) e1Error("E1 binding is missing #ELSEACT action", binding);
  return { conditions, onPass, onFail };
}

function loadRespawnManifest(respawnManifestPath) {
  let manifest;
  try { manifest = JSON.parse(fs.readFileSync(respawnManifestPath, "utf8")); } catch (error) {
    throw new MapEventImportError(`cannot read Crystal respawn manifest ${respawnManifestPath}: ${error instanceof Error ? error.message : error}`, diagnostics());
  }
  if (!Array.isArray(manifest.maps)) throw new MapEventImportError(`Crystal respawn manifest has no maps array: ${respawnManifestPath}`, diagnostics());
  return manifest;
}

function typedNeedMove(binding, respawnManifest) {
  const sourceMaps = respawnManifest.maps.filter((map) => normalizeMapId(map.map_file_name) === normalizeMapId(binding.mapId));
  if (sourceMaps.length !== 1) e1Error(`E1 NeedMove source map is ${sourceMaps.length === 0 ? "missing" : "ambiguous"}`, binding);
  const sourceMap = sourceMaps[0];
  const moves = (sourceMap.movements ?? []).filter((movement) => movement.need_move && movement.source?.x === binding.x && movement.source?.y === binding.y);
  if (moves.length !== 1) e1Error(`E1 NeedMove is ${moves.length === 0 ? "missing" : "ambiguous"} for _MAPCOORD(${binding.mapId},${binding.x},${binding.y})`, binding);
  const movement = moves[0];
  const targets = respawnManifest.maps.filter((map) => map.map_index === movement.map_index);
  if (targets.length !== 1) e1Error(`E1 NeedMove target map index ${movement.map_index} is ${targets.length === 0 ? "missing" : "ambiguous"}`, binding);
  const target = targets[0];
  return {
    sourceMapIndex: sourceMap.map_index,
    sourceMapFileName: sourceMap.map_file_name,
    targetMapIndex: target.map_index,
    targetMapFileName: target.map_file_name,
    targetMapTitle: target.map_title,
    source: movement.source,
    destination: movement.destination,
    conquestIndex: movement.conquest_index,
    // Server.MirDB is binary. The MapCoords source location is the exact
    // textual origin that links this E1 binding to its NeedMove record.
    sourceFile: binding.bindingSourceFile,
    sourceLine: binding.bindingSourceLine,
  };
}

function buildTypedMapCoordinateBindings(mapCoordinates, respawnManifest) {
  const seen = new Map();
  return mapCoordinates.map((binding) => {
    const key = `${normalizeMapId(binding.mapId)}:${binding.x}:${binding.y}`;
    const previous = seen.get(key);
    if (previous) e1Error(`duplicate E1 _MAPCOORD; first declared at ${previous.bindingSourceFile}:${previous.bindingSourceLine}`, binding);
    seen.set(key, binding);
    const parsed = parseE1Binding(binding);
    return {
      mapId: binding.mapId,
      x: binding.x,
      y: binding.y,
      bindingSourceFile: binding.bindingSourceFile,
      bindingSourceLine: binding.bindingSourceLine,
      conditions: parsed.conditions,
      onPass: parsed.onPass,
      onFail: parsed.onFail,
      needMove: typedNeedMove(binding, respawnManifest),
    };
  });
}

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

  function build({ mapCoordsRelative = DEFAULT_MAP_COORDS, eventsRelativeDir = DEFAULT_EVENTS_DIR, respawnManifestPath = DEFAULT_RESPAWN_MANIFEST } = {}) {
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
    const typedMapCoordinateBindings = buildTypedMapCoordinateBindings(mapCoordinates, loadRespawnManifest(respawnManifestPath));
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
      typedMapCoordinateBindings: typedMapCoordinateBindings.sort((left, right) => `${left.mapId}:${left.x}:${left.y}:${left.bindingSourceLine}`.localeCompare(`${right.mapId}:${right.x}:${right.y}:${right.bindingSourceLine}`)),
      generalEventScripts: { status: "open", detail: "Only validated _MAPCOORD E1 bindings are typed and executable; general Crystal event scripts remain imported source data only." },
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
    const respawnManifestPath = path.join(fixtureRoot, "respawns.json");
    const writeFixture = ({ mapCoords = "[@_MAPCOORD(0,1,2)]\n#INCLUDE [Events/a.txt] @Main\n", script = "[@Main]\n{\n#IF\nLevel > 9\n#ACT\nENTERMAP\n#ELSEACT\nLocalMessage \"locked\" Hint\n}\n", movements = [{ map_index: 2, source: { x: 1, y: 2 }, destination: { x: 3, y: 4 }, need_move: true, conquest_index: 0 }] } = {}) => {
      fs.writeFileSync(path.join(fixtureRoot, "MapCoords.txt"), mapCoords);
      fs.writeFileSync(path.join(fixtureRoot, "Events/a.txt"), script);
      fs.writeFileSync(respawnManifestPath, JSON.stringify({ maps: [{ map_index: 1, map_file_name: "0", map_title: "Source", movements }, { map_index: 2, map_file_name: "1", map_title: "Target", movements: [] }] }));
    };
    writeFixture();
    const good = makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath });
    assert(good.mapCoordinates.length === 1, "fixture binding resolves");
    assert(good.typedMapCoordinateBindings.length === 1, "fixture E1 binding resolves");
    assert(good.references.every((reference) => reference.resolved), "fixture references resolve");
    fs.writeFileSync(path.join(fixtureRoot, "Events/traversal.txt"), "#INSERT [../outside.txt]\n");
    let traversalFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath }); } catch (error) { traversalFailed = error instanceof MapEventImportError && error.diagnostics.pathTraversalRejected.length > 0; }
    assert(traversalFailed, "path traversal fails closed");
    fs.rmSync(path.join(fixtureRoot, "Events/traversal.txt"));
    fs.writeFileSync(path.join(fixtureRoot, "Events/cycle-a.txt"), "#INSERT [Events/cycle-b.txt]\n");
    fs.writeFileSync(path.join(fixtureRoot, "Events/cycle-b.txt"), "#INSERT [Events/cycle-a.txt]\n");
    let cycleFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath }); } catch (error) { cycleFailed = error instanceof MapEventImportError && error.diagnostics.cycles.length > 0; }
    assert(cycleFailed, "include cycle fails closed");
    fs.rmSync(path.join(fixtureRoot, "Events/cycle-a.txt"));
    fs.rmSync(path.join(fixtureRoot, "Events/cycle-b.txt"));
    writeFixture({ mapCoords: "[@_MAPCOORD(0,1,2)]\n#INCLUDE [Events/a.txt] @Main\n[@_MAPCOORD(0,1,2)]\n#INCLUDE [Events/a.txt] @Main\n" });
    let duplicateFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath }); } catch (error) { duplicateFailed = error instanceof MapEventImportError && /MapCoords\.txt:3/.test(error.message); }
    assert(duplicateFailed, "duplicate E1 coordinate fails at source line");
    writeFixture({ movements: [] });
    let missingNeedMoveFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath }); } catch (error) { missingNeedMoveFailed = error instanceof MapEventImportError && /NeedMove is missing.*MapCoords\.txt:1/.test(error.message); }
    assert(missingNeedMoveFailed, "missing NeedMove fails at source line");
    writeFixture({ movements: [{ map_index: 2, source: { x: 1, y: 2 }, destination: { x: 3, y: 4 }, need_move: true, conquest_index: 0 }, { map_index: 2, source: { x: 1, y: 2 }, destination: { x: 3, y: 4 }, need_move: true, conquest_index: 0 }] });
    let multipleNeedMoveFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath }); } catch (error) { multipleNeedMoveFailed = error instanceof MapEventImportError && /NeedMove is ambiguous.*MapCoords\.txt:1/.test(error.message); }
    assert(multipleNeedMoveFailed, "multiple NeedMove records fail at source line");
    writeFixture({ script: "[@Main]\n{\n#IF\nLevel > 9\n#ACT\nTELEPORT\n#ELSEACT\nLocalMessage \"locked\" Hint\n}\n" });
    let unsupportedFailed = false;
    try { makeImporter(fixtureRoot).build({ mapCoordsRelative: "MapCoords.txt", eventsRelativeDir: "Events", respawnManifestPath }); } catch (error) { unsupportedFailed = error instanceof MapEventImportError && /unsupported E1 command.*Events\/a\.txt:6/.test(error.message); }
    assert(unsupportedFailed, "unsupported E1 command fails at exact source line");
    console.log("map-event self-test: 7/7 passed");
  } finally { fs.rmSync(fixtureRoot, { recursive: true, force: true }); }
}

function parseArgs(argv) {
  const args = { envirRoot: null, output: DEFAULT_OUTPUT, respawnManifestPath: DEFAULT_RESPAWN_MANIFEST, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--self-test") args.selfTest = true;
    else if (value === "--envir-root") args.envirRoot = argv[++index];
    else if (value === "--output") args.output = path.resolve(argv[++index]);
    else if (value === "--respawn-manifest") args.respawnManifestPath = path.resolve(argv[++index]);
    else throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.selfTest) runSelfTests();
  const envirRoot = args.envirRoot ?? process.env.MIR2_CRYSTAL_ENVIR_ROOT ?? process.env.MIR2_ENVIR_ROOT ?? DEFAULT_ENVIR_ROOT;
  const manifest = makeImporter(envirRoot).build({ respawnManifestPath: args.respawnManifestPath });
  if (manifest.diagnostics.danglingPaths.length || manifest.diagnostics.pathTraversalRejected.length || manifest.diagnostics.cycles.length) throw new Error("Crystal map-event manifest contains unsafe or dangling diagnostics");
  fs.mkdirSync(path.dirname(args.output), { recursive: true });
  fs.writeFileSync(args.output, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`generated ${path.relative(PROJECT_ROOT, args.output)}: ${manifest.mapCoordinates.length} map bindings, ${manifest.events.length} event files, ${manifest.references.length} resolved directives`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; }
}

export { DEFAULT_LIMITS, MapEventImportError, makeImporter, runSelfTests };
