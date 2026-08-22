#!/usr/bin/env node

/**
 * Deterministic native Windows UI evidence runner.
 *
 * The live path attaches to an already-running native client and uses the
 * Win32 helper for ClientToScreen + SendInput. It never injects gateway or
 * websocket commands and never starts/kills processes. `--list`, `--dry-run`
 * and `--self-test` are side-effect free with respect to the desktop.
 */

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const HELPER = path.join(SCRIPT_DIR, 'native-ui-mouse-matrix', 'win32-input.ps1');
const STAGE = Object.freeze({ width: 1024, height: 768 });
const PHASES = Object.freeze(['closed', 'hover', 'pressed', 'opened', 'result']);
const DEFAULT_REGISTRY = path.resolve(SCRIPT_DIR, '../../../../docs/generated/player-qa/native-ui-controls/original-control-registry.json');

const CASES = Object.freeze([
  { id: 'login', page: 'Login', entry: 'Login.OK', point: { x: 596, y: 376 }, safe: false, precondition: 'Login form is populated; opens CharacterSelect.' },
  { id: 'register', page: 'Register', entry: 'Login.NEW_ACCOUNT', point: { x: 458, y: 449 }, safe: true, precondition: 'Login screen; registration form can be opened without submitting.' },
  { id: 'character-select', page: 'CharacterSelect', entry: 'Select.SLOT', point: { x: 781, y: 222 }, safe: true, precondition: 'CharacterSelect with an occupied slot.' },
  { id: 'create', page: 'Create', entry: 'Select.NEW_CHARACTER', point: { x: 346, y: 748 }, safe: true, precondition: 'CharacterSelect with a free slot.' },
  { id: 'delete', page: 'Delete', entry: 'Select.DELETE_CHARACTER', point: { x: 510, y: 748 }, safe: true, precondition: 'CharacterSelect with a selected character; only opens confirmation, never confirms deletion.' },
  { id: 'hud-character', page: 'HUD.Character', entry: 'HUD.CHARACTER', point: { x: 915, y: 702 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-inventory', page: 'HUD.Inventory', entry: 'HUD.INVENTORY', point: { x: 938, y: 702 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-skill', page: 'HUD.Skill', entry: 'HUD.SKILL', point: { x: 961, y: 702 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-quest', page: 'Quest', entry: 'HUD.QUEST', point: { x: 984, y: 702 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-options', page: 'Options', entry: 'HUD.OPTION', point: { x: 1007, y: 702 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-bigmap', page: 'BigMap', entry: 'HUD.BIG_MAP', point: { x: 933, y: 141 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-mail', page: 'Mail', entry: 'HUD.MAIL', point: { x: 912, y: 141 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-shop', page: 'Shop', entry: 'HUD.GAME_SHOP', point: { x: 939, y: 670 }, safe: true, precondition: 'InGame.' },
  { id: 'hud-warehouse', page: 'Warehouse', entry: 'HUD.WAREHOUSE', point: { x: 985, y: 670 }, safe: true, precondition: 'InGame and storage/NPC service is available.' },
  { id: 'npc', page: 'NPC', entry: 'NPC.TALK', point: null, safe: true, precondition: 'InGame with an NPC at a known logical coordinate; provide --point npc=x,y.' },
  { id: 'inventory-item-use', page: 'Inventory.ItemUse', entry: 'HUD.BELT', point: { x: 258, y: 637 }, safe: false, precondition: 'InGame with a disposable/useable item in belt slot 1; requires --allow-mutations.' },
  { id: 'disconnect', page: 'Disconnect', entry: 'DISCONNECT.OBSERVE', point: null, safe: true, observeOnly: true, precondition: 'Client is already on the intentional disconnected/connection-lost screen; provide no click point.' }
]);

function usage() {
  console.log(`Usage:
  node native-ui-mouse-matrix.mjs --list
  node native-ui-mouse-matrix.mjs --dry-run [--case id,...] [--evidence-dir dir]
  node native-ui-mouse-matrix.mjs --self-test
  node native-ui-mouse-matrix.mjs --run --pid PID --case id,... [options]

Live options:
  --pid PID                 Existing mir2-platform-windows PID; attach-only.
  --case id,...             Selected case ids (default: all).
  --evidence-dir DIR        Output root (default: docs/generated/player-qa/native-ui-mouse-matrix/<run>).
  --client-log FILE         Optional client.log for line-range references.
  --gateway-events FILE     Optional gateway-events.jsonl for line-range references.
  --point id=x,y            Override a dynamic case point, repeatable.
  --allow-mutation-case ID  Permit one state-changing case; repeatable and explicit.
  --expected-client-regex id=REGEX
                             Require a matching new client-log line for this case.
  --expected-gateway-regex id=REGEX
                             Require a matching new gateway-event line for this case.
  --expected-process-name NAME   Expected executable name (default: mir2-platform-windows).
  --expected-process-path FILE   Expected executable path.
  --expected-process-sha256 HEX  Expected executable SHA-256.
  --expected-window-title TEXT   Expected exact native window title.
  --registry FILE             Native control registry for coverage reporting.
  --timeout-ms N             Per Win32 helper timeout (default: 10000).
  --settle-ms N              Wait after input before capture (default: 350).
  --keep-going              Record blocked cases and continue (default: fail-closed on first blocked case).

No mode starts, kills, focuses unrelated processes, or sends protocol commands.`);
}

function parseArgs(argv) {
  const args = { mode: null, cases: null, points: {}, expectedClientRegex: {}, expectedGatewayRegex: {}, allowMutationCases: new Set(), keepGoing: false, timeoutMs: 10000, settleMs: 350, expectedProcessName: 'mir2-platform-windows', registry: DEFAULT_REGISTRY };
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--list') args.mode = 'list';
    else if (token === '--dry-run') args.mode = 'dry-run';
    else if (token === '--self-test') args.mode = 'self-test';
    else if (token === '--run') args.mode = 'run';
    else if (token === '--help' || token === '-h') args.mode = 'help';
    else if (token === '--keep-going') args.keepGoing = true;
    else if (token === '--allow-mutations') throw new Error('--allow-mutations was removed; use --allow-mutation-case id');
    else if (token === '--case') args.cases = argv[++i];
    else if (token === '--pid') args.pid = Number(argv[++i]);
    else if (token === '--evidence-dir') args.evidenceDir = argv[++i];
    else if (token === '--client-log') args.clientLog = argv[++i];
    else if (token === '--gateway-events') args.gatewayEvents = argv[++i];
    else if (token === '--expected-client-regex' || token === '--expected-gateway-regex') {
      const value = argv[++i] ?? '';
      const match = /^([^=]+)=(.*)$/.exec(value);
      if (!match || !match[2]) throw new Error('invalid ' + token + ': expected id=REGEX');
      try { new RegExp(match[2]); } catch (error) { throw new Error('invalid ' + token + ' regex: ' + error.message); }
      (token === '--expected-client-regex' ? args.expectedClientRegex : args.expectedGatewayRegex)[match[1]] = match[2];
    } else if (token === '--expected-process-name') args.expectedProcessName = argv[++i];
    else if (token === '--expected-process-path') args.expectedProcessPath = path.resolve(argv[++i]);
    else if (token === '--expected-process-sha256') args.expectedProcessSha256 = String(argv[++i]).toLowerCase();
    else if (token === '--expected-window-title') args.expectedWindowTitle = argv[++i];
    else if (token === '--registry') args.registry = path.resolve(argv[++i]);
    else if (token === '--allow-mutation-case') args.allowMutationCases.add(argv[++i]);
    else if (token === '--point') {
      const value = argv[++i] ?? '';
      const match = /^([^=]+)=(-?\d+),(-?\d+)$/.exec(value);
      if (!match) throw new Error(`invalid --point: ${value}; expected id=x,y`);
      args.points[match[1]] = { x: Number(match[2]), y: Number(match[3]) };
    } else if (token === '--timeout-ms') args.timeoutMs = Number(argv[++i]);
    else if (token === '--settle-ms') args.settleMs = Number(argv[++i]);
    else throw new Error(`unknown argument: ${token}`);
  }
  if (!args.mode) args.mode = 'help';
  if (args.cases) args.cases = args.cases.split(',').map((id) => id.trim()).filter(Boolean);
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1000) throw new Error('--timeout-ms must be >= 1000');
  if (!Number.isInteger(args.settleMs) || args.settleMs < 0) throw new Error('--settle-ms must be >= 0');
  if (args.expectedProcessSha256 && !/^[0-9a-f]{64}$/.test(args.expectedProcessSha256)) throw new Error('--expected-process-sha256 must be 64 hex characters');
  return args;
}

function selectedCases(args) {
  if (!args.cases) return [...CASES];
  const known = new Set(CASES.map((item) => item.id));
  const unknown = args.cases.filter((id) => !known.has(id));
  if (unknown.length) throw new Error(`unknown case id(s): ${unknown.join(', ')}`);
  return args.cases.map((id) => CASES.find((item) => item.id === id));
}

function runId() {
  return `${new Date().toISOString().replace(/[-:.]/g, '').replace('Z', 'Z')}-${crypto.randomBytes(3).toString('hex')}`;
}

function ensureDir(dir) { fs.mkdirSync(dir, { recursive: true }); }

function writeJson(file, value) {
  ensureDir(path.dirname(file));
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function jsonLine(value) { return JSON.stringify(value); }

function fileSnapshot(file) {
  if (!file) return { configured: false, path: null, exists: false, lines: null };
  try {
    const stat = fs.statSync(file);
    const content = fs.readFileSync(file, 'utf8');
    return { configured: true, path: path.resolve(file), exists: true, bytes: stat.size, lines: content ? content.split(/\r?\n/).length - (content.endsWith('\n') ? 1 : 0) : 0 };
  } catch {
    return { configured: true, path: path.resolve(file), exists: false, lines: null };
  }
}

function logRef(before, after) {
  if (!before.configured && !after.configured) return { status: 'not-configured', path: null };
  if (!after.exists) return { status: 'missing', path: after.path, beforeLine: before.lines, afterLine: null };
  return { status: 'available', path: after.path, beforeLine: before.lines ?? 0, afterLine: after.lines ?? 0, lineCount: Math.max(0, (after.lines ?? 0) - (before.lines ?? 0)) };
}

function isWithin(child, parent) {
  const relative = path.relative(path.resolve(parent), path.resolve(child));
  return relative === '' || (relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative));
}

function pngMetadata(file) {
  const bytes = fs.readFileSync(file);
  if (bytes.length < 24 || bytes.readUInt32BE(0) !== 0x89504e47 || bytes.readUInt32BE(4) !== 0x0d0a1a0a || bytes.toString('ascii', 12, 16) !== 'IHDR') throw new Error('invalid PNG capture: ' + file);
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20), bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
}

function captureMetadata(file, runDir, expectedWidth, expectedHeight) {
  if (!isWithin(file, runDir)) throw new Error('capture path escapes evidence directory: ' + file);
  if (!fs.existsSync(file)) throw new Error('capture was not created: ' + file);
  const metadata = pngMetadata(file);
  if (metadata.width !== expectedWidth || metadata.height !== expectedHeight) throw new Error('capture dimensions ' + metadata.width + 'x' + metadata.height + ' do not match client ' + expectedWidth + 'x' + expectedHeight);
  return { path: path.resolve(file), ...metadata };
}

function readLog(file) {
  if (!file) return { configured: false, exists: false, lines: [] };
  try { return { configured: true, exists: true, lines: fs.readFileSync(file, 'utf8').split(/\r?\n/).filter((line, index, all) => !(index === all.length - 1 && line === '')) }; } catch { return { configured: true, exists: false, lines: [] }; }
}

function assertNewLogMatch(file, before, pattern, label) {
  if (!pattern) return { status: 'not-requested', pattern: null, matched: false };
  const after = readLog(file);
  if (!after.exists) return { status: 'missing-log', pattern, matched: false };
  const regex = new RegExp(pattern);
  const start = before.lines.length;
  const delta = after.lines.slice(start);
  const index = delta.findIndex((line) => regex.test(line));
  return { status: index >= 0 ? 'matched' : 'not-matched', pattern, matched: index >= 0, label, beforeLine: start, afterLine: after.lines.length, matchedLine: index >= 0 ? start + index + 1 : null, line: index >= 0 ? delta[index] : null };
}

function registryCoverage(file, items) {
  if (!file) return { status: 'not-configured' };
  try {
    const registry = JSON.parse(fs.readFileSync(file, 'utf8'));
    const controls = Array.isArray(registry) ? registry : (registry.controls ?? registry.entries ?? []);
    const pages = new Set(items.map((item) => item.page));
    const visible = controls.filter((control) => control.visible !== false && control.hidden !== true);
    const represented = visible.filter((control) => pages.has(control.page ?? control.screen ?? control.screenId));
    return {
      status: 'available',
      path: path.resolve(file),
      total: controls.length,
      visible: visible.length,
      representedByCases: represented.length,
      unrepresentedVisible: Math.max(0, visible.length - represented.length),
      candidateOnly: Boolean(registry.candidateOnly),
      summary: registry.summary ?? null
    };
  } catch (error) { return { status: 'invalid', path: path.resolve(file), error: error.message }; }
}

function phasePath(runDir, caseId, phase) { return path.join(runDir, `${caseId}-${phase}.png`); }

function resolvePoint(item, args) { return args.points[item.id] ?? item.point; }

function validateCasePlan(item, args) {
  const point = resolvePoint(item, args);
  if (item.observeOnly) return { valid: true, point: null, reason: 'observation-only case' };
  if (!point) return { valid: false, point: null, reason: `${item.id} requires an explicit --point ${item.id}=x,y override` };
  if (point.x < 0 || point.y < 0 || point.x >= STAGE.width || point.y >= STAGE.height) return { valid: false, point, reason: `logical point outside ${STAGE.width}x${STAGE.height}` };
  return { valid: true, point };
}

function planFor(items, args, runDir = null) {
  return items.map((item) => {
    const validation = validateCasePlan(item, args);
    return {
      id: item.id,
      page: item.page,
      entry: item.entry,
      point: validation.point,
      phases: PHASES.map((phase) => runDir ? phasePath(runDir, item.id, phase) : null),
      safe: item.safe,
      observeOnly: Boolean(item.observeOnly),
      precondition: item.precondition,
      valid: validation.valid,
      invalidReason: validation.valid ? null : validation.reason
    };
  });
}

function runHelper(args, operation, point, capturePath = '', expectedHandle = null) {
  const helperArgs = ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', HELPER, '-Operation', operation, '-Pid', String(args.pid), '-LogicalWidth', String(STAGE.width), '-LogicalHeight', String(STAGE.height), '-SettleMs', String(args.settleMs), '-ExpectedProcessName', args.expectedProcessName];
  if (args.expectedProcessPath) helperArgs.push('-ExpectedProcessPath', args.expectedProcessPath);
  if (args.expectedProcessSha256) helperArgs.push('-ExpectedProcessSha256', args.expectedProcessSha256);
  if (args.expectedWindowTitle !== undefined) helperArgs.push('-ExpectedWindowTitle', args.expectedWindowTitle);
  if (expectedHandle) helperArgs.push('-ExpectedHandle', String(expectedHandle));
  if (point) helperArgs.push('-X', String(point.x), '-Y', String(point.y));
  if (capturePath) helperArgs.push('-CapturePath', capturePath);
  const result = spawnSync('powershell.exe', helperArgs, { encoding: 'utf8', timeout: args.timeoutMs, windowsHide: true, maxBuffer: 2 * 1024 * 1024 });
  if (result.error) throw new Error(`Win32 helper failed to start: ${result.error.message}`);
  if (result.status === null) throw new Error(`Win32 helper timed out after ${args.timeoutMs}ms`);
  const output = (result.stdout ?? '').trim().split(/\r?\n/).filter(Boolean).at(-1) ?? '';
  let payload;
  try { payload = JSON.parse(output); } catch { throw new Error(`Win32 helper returned invalid JSON: ${output || result.stderr || 'empty output'}`); }
  if (!payload.ok) {
    const diagnostic = payload.diagnostics ? ` ${jsonLine(payload.diagnostics)}` : '';
    throw new Error(`${payload.error ?? 'Win32 helper failed'}${diagnostic}`);
  }
  return payload;
}

function resultBase(item, plan) {
  return { id: item.id, page: item.page, entry: item.entry, status: 'PENDING', point: plan.point, phases: {}, clientLogRefs: [], gatewayEventRefs: [], diagnostics: [], precondition: item.precondition };
}

function capturePhase(args, item, plan, phase, refs) {
  const file = phasePath(args.runDir, item.id, phase);
  const beforeClient = fileSnapshot(args.clientLog);
  const beforeGateway = fileSnapshot(args.gatewayEvents);
  const payload = runHelper(args, 'capture', null, file, refs.expectedHandle);
  const afterClient = fileSnapshot(args.clientLog);
  const afterGateway = fileSnapshot(args.gatewayEvents);
  refs.clientLogRefs.push({ phase, ...logRef(beforeClient, afterClient) });
  refs.gatewayEventRefs.push({ phase, ...logRef(beforeGateway, afterGateway) });
  const state = payload.state ?? {};
  return { capture: captureMetadata(file, args.runDir, state.clientWidth, state.clientHeight), helper: payload };
}

function executeCase(args, item, plan) {
  const result = resultBase(item, plan);
  const validation = validateCasePlan(item, args);
  if (!validation.valid) { result.status = 'BLOCKED'; result.diagnostics.push(validation.reason); return result; }
  if (!item.safe && !args.allowMutationCases.has(item.id)) { result.status = 'BLOCKED'; result.diagnostics.push('state-changing case requires --allow-mutation-case ' + item.id); return result; }
  try {
    const probe = runHelper(args, 'probe', null);
    result.window = probe.state;
    result.logBaseline = { client: readLog(args.clientLog), gateway: readLog(args.gatewayEvents) };
    result.expectedAssertions = { client: args.expectedClientRegex[item.id] ?? null, gateway: args.expectedGatewayRegex[item.id] ?? null };
    const refs = { ...result, expectedHandle: probe.state.handle };
    result.phases.closed = capturePhase(args, item, plan, 'closed', refs);
    if (item.observeOnly) {
      result.phases.hover = capturePhase(args, item, plan, 'hover', refs);
      result.phases.pressed = capturePhase(args, item, plan, 'pressed', refs);
      result.phases.opened = capturePhase(args, item, plan, 'opened', refs);
      result.phases.result = capturePhase(args, item, plan, 'result', refs);
      result.status = 'UNVERIFIED';
      result.diagnostics.push('observation-only case has no state detector; screenshots alone cannot verify Disconnect');
      return result;
    }
    const point = validation.point;
    runHelper(args, 'hover', point, '', refs.expectedHandle);
    result.phases.hover = capturePhase(args, item, plan, 'hover', refs);
    result.phases.pressed = capturePhase(args, item, plan, 'pressed', refs);
    result.phases.opened = capturePhase(args, item, plan, 'opened', refs);
    result.phases.result = capturePhase(args, item, plan, 'result', refs);
    const postProbe = runHelper(args, 'probe', null, '', refs.expectedHandle);
    result.postWindow = postProbe.state;
    result.assertions = {
      client: assertNewLogMatch(args.clientLog, result.logBaseline.client, result.expectedAssertions.client, 'client log'),
      gateway: assertNewLogMatch(args.gatewayEvents, result.logBaseline.gateway, result.expectedAssertions.gateway, 'gateway events')
    };
    const requested = [result.expectedAssertions.client, result.expectedAssertions.gateway].filter(Boolean).length;
    const matched = [result.assertions.client, result.assertions.gateway].filter((item) => item.matched).length;
    if (requested === 0) {
      result.status = 'INPUT_SEQUENCE_COMPLETED';
      result.diagnostics.push('no expected log/event regex supplied; functional result is unverified');
    } else if (matched !== requested) {
      result.status = 'UNVERIFIED';
      result.diagnostics.push('one or more expected log/event assertions did not match');
    } else result.status = 'FUNCTIONAL_PASS';
    return result;
  } catch (error) {
    result.status = 'BLOCKED';
    result.diagnostics.push(String(error.message ?? error));
    result.diagnostics.push('fail-closed: no further input was sent for this case');
    return result;
  }
}

function makeVerdict(args, items, plans, results, mode, runDirectory) {
  const blocked = results.filter((item) => item.status === 'BLOCKED');
  const inputOnly = results.filter((item) => item.status === 'INPUT_SEQUENCE_COMPLETED');
  const unverified = results.filter((item) => item.status === 'UNVERIFIED');
  const functionalPass = mode === 'run' && blocked.length === 0 && results.length === items.length && results.every((item) => item.status === 'FUNCTIONAL_PASS');
  return {
    schema: 'mir2-native-ui-mouse-matrix/v1',
    runId: path.basename(runDirectory),
    mode,
    generatedAt: new Date().toISOString(),
    logicalStage: STAGE,
    dpiPolicy: 'Win32 GetDpiForWindow + client-pixel scaling + ClientToScreen; no screen-coordinate constants',
    inputPolicy: 'Win32 SendInput only; no websocket/browser/protocol injection',
    processPolicy: 'attach-only; no process start/kill',
    logSources: { clientLog: fileSnapshot(args.clientLog), gatewayEvents: fileSnapshot(args.gatewayEvents) },
    coverage: { requested: items.map((item) => item.id), total: items.length, pages: items.map((item) => item.page), allRequiredPagesPresent: ['Login', 'Register', 'CharacterSelect', 'Create', 'Delete', 'HUD.Character', 'HUD.Inventory', 'HUD.Skill', 'Quest', 'NPC', 'Inventory.ItemUse', 'Options', 'BigMap', 'Mail', 'Shop', 'Warehouse', 'Disconnect'].every((page) => items.some((item) => item.page === page)), registry: registryCoverage(args.registry, items) },
    phases: PHASES,
    results,
    blockedCaseIds: blocked.map((item) => item.id),
    statusCounts: { functionalPass: results.filter((item) => item.status === 'FUNCTIONAL_PASS').length, inputSequenceCompleted: inputOnly.length, unverified: unverified.length, blocked: blocked.length },
    verdict: mode === 'dry-run' ? 'DRY_RUN' : blocked.length ? 'BLOCKED' : functionalPass ? 'FUNCTIONAL_PASS' : unverified.length ? 'UNVERIFIED' : inputOnly.length ? 'INPUT_SEQUENCE_COMPLETED' : 'UNVERIFIED',
    evidenceDirectory: runDirectory,
    note: 'A PASS means the real input/capture sequence completed. It does not replace human visual/feel acceptance or server-side semantic review.'
  };
}

function selfTest() {
  const failures = [];
  const ids = new Set(CASES.map((item) => item.id));
  if (ids.size !== CASES.length) failures.push('duplicate case id');
  for (const phase of PHASES) if (!phase) failures.push('empty phase');
  for (const item of CASES) {
    if (!item.id || !item.page || !item.entry || !item.precondition) failures.push(`${item.id || '<unknown>'}: incomplete case metadata`);
    if (!item.observeOnly && item.point && (item.point.x < 0 || item.point.x >= STAGE.width || item.point.y < 0 || item.point.y >= STAGE.height)) failures.push(`${item.id}: point outside stage`);
  }
  const transforms = [{ w: 1024, h: 768, x: 258, y: 637, ex: 258, ey: 637 }, { w: 1280, h: 960, x: 512, y: 384, ex: 640, ey: 480 }, { w: 1536, h: 1152, x: 1023, y: 767, ex: 1535, ey: 1151 }];
  for (const t of transforms) {
    const x = Math.round(t.x * t.w / STAGE.width); const y = Math.round(t.y * t.h / STAGE.height);
    if (x !== t.ex || y !== t.ey) failures.push(`DPI transform ${JSON.stringify(t)} -> ${x},${y}`);
  }
  if (!isWithin(path.join('run', 'capture.png'), 'run')) failures.push('path containment rejected valid child');
  if (isWithin(path.resolve('run', '..', 'escape.png'), 'run')) failures.push('path containment accepted escape');
  const containScale = Math.min(1280 / STAGE.width, 800 / STAGE.height);
  if (Math.round((1280 - STAGE.width * containScale) / 2) !== 107) failures.push('letterbox contain transform');
  try { new RegExp('['); failures.push('invalid regex accepted'); } catch { /* expected */ }
  if (!fs.existsSync(HELPER)) failures.push(`missing helper: ${HELPER}`);
  if (failures.length) { console.error(JSON.stringify({ ok: false, failures }, null, 2)); process.exitCode = 1; return; }
  console.log(JSON.stringify({ ok: true, cases: CASES.length, phases: PHASES, helper: HELPER, desktopTouched: false }, null, 2));
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.mode === 'help') { usage(); return; }
  if (args.mode === 'self-test') { selfTest(); return; }
  const items = selectedCases(args);
  if (args.mode === 'list') {
    console.log(JSON.stringify({ schema: 'mir2-native-ui-mouse-matrix/v1', logicalStage: STAGE, phases: PHASES, cases: CASES }, null, 2));
    return;
  }
  const root = args.evidenceDir ? path.resolve(args.evidenceDir) : path.resolve(SCRIPT_DIR, '../../../../docs/generated/player-qa/native-ui-mouse-matrix');
  const directory = path.join(root, runId());
  args.runDir = directory;
  ensureDir(directory);
  const plans = planFor(items, args, directory);
  writeJson(path.join(directory, 'plan.json'), { schema: 'mir2-native-ui-mouse-matrix/v1', mode: args.mode, logicalStage: STAGE, phases: PHASES, cases: plans, clientLog: args.clientLog ?? null, gatewayEvents: args.gatewayEvents ?? null });
  if (args.mode === 'dry-run') {
    const results = plans.map((plan) => ({ id: plan.id, page: plan.page, status: plan.valid ? 'READY' : 'BLOCKED', diagnostics: plan.invalidReason ? [plan.invalidReason] : ['dry-run: no Win32 input or screenshot was performed'] }));
    const verdict = makeVerdict(args, items, plans, results, 'dry-run', directory);
    writeJson(path.join(directory, 'verdict.json'), verdict);
    console.log(JSON.stringify(verdict, null, 2));
    return;
  }
  if (args.mode !== 'run') throw new Error(`unsupported mode: ${args.mode}`);
  if (process.platform !== 'win32') throw new Error('live mode is Windows-only and was not run');
  if (!Number.isInteger(args.pid) || args.pid <= 0) throw new Error('--pid is required in --run mode; attach-only prevents accidental process selection');
  if (!fs.existsSync(HELPER)) throw new Error(`missing Win32 helper: ${HELPER}`);
  const results = [];
  for (const [index, item] of items.entries()) {
    const result = executeCase(args, item, plans[index]);
    results.push(result);
    if (result.status === 'BLOCKED' && !args.keepGoing) break;
  }
  const verdict = makeVerdict(args, items, plans, results, 'run', directory);
  writeJson(path.join(directory, 'verdict.json'), verdict);
  console.log(JSON.stringify(verdict, null, 2));
  if (verdict.verdict !== 'PASS') process.exitCode = 2;
}

try { main(); } catch (error) { console.error(`native-ui-mouse-matrix: ${error.message ?? error}`); usage(); process.exitCode = 1; }
