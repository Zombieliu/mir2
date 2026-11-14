import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha256 = value => createHash('sha256').update(value).digest('hex');
const workspace = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const MAX_LINES = 20_000;
const MAX_FILES = 128;
const MAX_BYTES = 4 * 1024 * 1024;

function lines(text) {
  const result = text.replace(/^\uFEFF/, '').split(/\r?\n/);
  if (result.at(-1) === '') result.pop();
  return result;
}
function target(line, command) {
  // NPCScript.ParseInsert / ParseInclude split the untrimmed source on space.
  const parts = line.split(' ');
  if (parts[0]?.toUpperCase() !== command || parts.length < 2
    || !parts[1].startsWith('[') || !parts[1].endsWith(']')) {
    throw new Error(`Malformed source directive: ${line}`);
  }
  const relative = parts[1].slice(1, -1).replaceAll('\\', '/');
  if (!relative.startsWith('SystemScripts/00Default/') || relative.split('/').some(p => p === '..')
    || !relative.endsWith('.txt')) throw new Error(`Unbounded source include: ${relative}`);
  return { relative, label: parts[2] ?? '@Main' };
}

/** Reproduce the supplied C# append-then-include expansion without a live FS. */
export function expandDefaultNpc(rootText, sourceFiles) {
  const used = new Set();
  const source = relative => {
    if (!sourceFiles.has(relative)) throw new Error(`Missing original source ${relative}`);
    used.add(relative);
    if (used.size > MAX_FILES) throw new Error('Default NPC source file limit exceeded');
    return lines(sourceFiles.get(relative));
  };
  let expanded = lines(rootText);
  for (let i = 0; i < expanded.length; i++) {
    if (!expanded[i].toUpperCase().startsWith('#INSERT')) continue;
    expanded.push(...source(target(expanded[i], '#INSERT').relative));
    if (expanded.length > MAX_LINES) throw new Error('Default NPC insert cycle or line limit exceeded');
  }
  expanded = expanded.filter(line => !line.toUpperCase().startsWith('#INSERT'));
  for (let i = 0; i < expanded.length; i++) {
    if (!expanded[i].toUpperCase().startsWith('#INCLUDE')) continue;
    const { relative, label } = target(expanded[i], '#INCLUDE');
    const included = source(relative);
    const header = `[${label}]`.toUpperCase();
    let start = false;
    let finish = false;
    const body = [];
    for (let j = 0; j < included.length; j++) {
      if (!included[j].toUpperCase().startsWith(header)) continue;
      for (let x = j + 1; x < included.length; x++) {
        if (included[x].trim() === '{') { start = true; continue; }
        if (included[x].trim() === '}') { finish = true; break; }
        body.push(included[x]);
      }
    }
    // C# only inserts an include page when both braces have been found.
    if (start && finish) expanded.splice(i + 1, 0, ...body);
    if (expanded.length > MAX_LINES) throw new Error('Default NPC include cycle or line limit exceeded');
  }
  expanded = expanded.filter(line => !line.toUpperCase().startsWith('#INCLUDE'));
  return { expanded, used: [...used].sort() };
}

export function parseExpandedScript(rootText, expanded) {
  const labels = [];
  const sections = [];
  let section;
  for (let i = 0; i < expanded.length; i++) {
    const match = /^\[(@[^\]]+)\]$/.exec(expanded[i]);
    if (match) {
      const label = match[1];
      labels.push({ label, line_number: i + 1 });
      section = { label, line_number: i + 1, lines: [] };
      sections.push(section);
    } else if (section) section.lines.push(expanded[i]);
  }
  return {
    script_key: '00Default', relative_path: '00Default.txt', raw_text: rootText,
    lines: expanded, line_count: expanded.length,
    non_empty_line_count: expanded.filter(line => line.trim()).length,
    label_count: labels.length,
    insert_count: lines(rootText).filter(line => line.toUpperCase().startsWith('#INSERT')).length,
    command_directives: [...new Set(expanded.filter(line => line.startsWith('#')).map(line => line.split(' ')[0].toUpperCase()))].sort(),
    labels, sections, inserts: [],
  };
}

function parserMethod(source, name) {
  const start = source.indexOf(`        public void ${name}(`);
  if (start < 0) throw new Error(`Missing original parser method ${name}`);
  const tail = source.slice(start + 1);
  const end = /\n        (?:public|private|protected) /.exec(tail);
  if (!end) throw new Error(`Unbounded original parser method ${name}`);
  return source.slice(start, start + 1 + end.index);
}

/** The original parser silently omits unknown opcodes and missing name files.
 * This is source compilation, not a reinterpretation of those checks as false.
 */
export async function compileParserOmissions(script, parserText, nameListExists) {
  const recognized = Object.fromEntries([['check', 'ParseCheck'], ['act', 'ParseAct']].map(([kind, name]) => [
    kind, new Set([...parserMethod(parserText, name).matchAll(/case "([A-Z0-9]+)":/g)].map(match => match[1])),
  ]));
  // This omission is bound to the supplied original ParseCheck source, not a
  // newly invented shorthand. Keep complete checks even when their runtime
  // values are invalid. ParseArguments replaces tokens without re-splitting.
  if (recognized.check.has('CHECKHUM')
    && !/case "CHECKHUM":\s*if \(parts\.Length < 4\) return;/.test(parserMethod(parserText, 'ParseCheck'))) {
    throw new Error('CHECKHUM original parser minimum-argument proof differs');
  }
  const ignored = [];
  const nameLists = new Map();
  for (const section of script.sections) {
    let kind;
    for (let index = 0; index < section.lines.length; index++) {
      const raw = section.lines[index];
      const line = raw.trim();
      if (line.startsWith('#')) {
        kind = line.toUpperCase() === '#IF' ? 'check'
          : ['#ACT', '#ELSEACT'].includes(line.toUpperCase()) ? 'act' : undefined;
        continue;
      }
      if (!kind || !line || line.startsWith(';')) continue;
      const parts = line.split(/\s+/);
      const opcode = parts[0].toUpperCase();
      let reason;
      if (!recognized[kind].has(opcode)) reason = 'unknown-original-opcode';
      else if (kind === 'check' && opcode === 'CHECKHUM'
        && line.split(' ').filter(part => part.length > 0).length < 4) {
        reason = 'missing-original-checkhum-arguments';
      }
      else if (['CHECKNAMELIST', 'CLEARNAMELIST', 'DELNAMELIST'].includes(opcode)) {
        const filename = /"([^"]*)"/.exec(line)?.[1] ?? parts[1];
        if (!filename || filename.includes('\\') || filename.includes('/') || filename === '..') {
          throw new Error('Unbounded original default name-list file');
        }
        const relative = `NameLists/${filename}`;
        if (!nameLists.has(relative)) nameLists.set(relative, Boolean(await nameListExists(relative)));
        if (!nameLists.get(relative)) reason = 'missing-original-name-list';
      }
      if (reason) ignored.push({ section_label: section.label, line_number: section.line_number + index + 1, line: raw, reason });
    }
  }
  return { ignored_lines: ignored, name_lists: [...nameLists].map(([relative_path, exists]) => ({ relative_path, exists })) };
}

export async function generate({ crystalRoot, output }) {
  if (!crystalRoot) throw new Error('Supply --crystal-root or MIR2_CRYSTAL_ROOT; no drive is assumed');
  const envir = path.resolve(crystalRoot, 'Build', 'Server', 'Debug', 'Envir');
  const rootPath = 'NPCs/00Default.txt';
  const sources = new Map();
  const records = new Map();
  let bytesRead = 0;
  async function read(relative) {
    const absolute = path.resolve(envir, relative);
    const bounded = path.relative(envir, absolute);
    if (bounded.startsWith('..') || path.isAbsolute(bounded)) throw new Error('Source escaped Envir root');
    const bytes = await readFile(absolute);
    bytesRead += bytes.length;
    if (bytesRead > MAX_BYTES) throw new Error('Default NPC source byte limit exceeded');
    // Reject undecodable data instead of silently changing original commands.
    new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    const raw = bytes.toString('utf8');
    sources.set(relative, raw);
    records.set(relative, { relative_path: relative, sha256: sha256(bytes), raw_text: raw });
    for (const line of lines(raw)) {
      const directive = line.toUpperCase().startsWith('#INSERT') ? '#INSERT'
        : line.toUpperCase().startsWith('#INCLUDE') ? '#INCLUDE' : undefined;
      if (!directive) continue;
      const child = target(line, directive).relative;
      if (!sources.has(child)) {
        if (sources.size >= MAX_FILES) throw new Error('Default NPC source file limit exceeded');
        await read(child);
      }
    }
    return raw;
  }
  const rootText = await read(rootPath);
  const { expanded, used } = expandDefaultNpc(rootText, sources);
  const script = parseExpandedScript(rootText, expanded);
  const parserPath = 'Server/MirObjects/NPC/NPCSegment.cs';
  const parserBytes = await readFile(path.resolve(crystalRoot, parserPath));
  new TextDecoder('utf-8', { fatal: true }).decode(parserBytes);
  const parserText = parserBytes.toString('utf8');
  const omissions = await compileParserOmissions(script, parserText, async relative => {
    try { await readFile(path.resolve(envir, relative)); return true; }
    catch (error) { if (error.code === 'ENOENT') return false; throw error; }
  });
  const parserContract = {
    parser_source: { relative_path: parserPath, sha256: sha256(parserBytes), raw_text: parserText },
    ...omissions,
  };
  const artifact = {
    schema_version: 1,
    source_dir: 'Crystal/Build/Server/Debug/Envir',
    source_parser: 'Server/MirObjects/NPC/NPCScript.cs:ParseInsert,ParseInclude,ParseDefault',
    root_sha256: records.get(rootPath).sha256,
    sources: [rootPath, ...used].map(relative => records.get(relative)),
    execution_sha256: sha256(JSON.stringify(script)),
    parser_contract_sha256: sha256(JSON.stringify(parserContract)),
    parser_contract: parserContract,
    script,
  };
  const destination = path.resolve(output ?? path.join(workspace, 'packages/game-data/data/generated/crystal_default_npc_scripts.json'));
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, `${JSON.stringify(artifact, null, 2)}\n`, 'utf8');
  return { output: destination, sources: artifact.sources.length, sections: sectionsCount(script), executionSha256: artifact.execution_sha256,
    ignoredLines: omissions.ignored_lines.length, parserContractSha256: artifact.parser_contract_sha256 };
}
const sectionsCount = script => script.sections.length;

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const value = name => { const i = args.indexOf(name); return i < 0 ? undefined : args[i + 1]; };
  console.log(JSON.stringify(await generate({ crystalRoot: value('--crystal-root') ?? process.env.MIR2_CRYSTAL_ROOT, output: value('--output') })));
}
