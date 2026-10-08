import test from 'node:test';
import assert from 'node:assert/strict';
import { expandDefaultNpc, parseExpandedScript, compileParserOmissions } from './generate-crystal-default-npc-scripts.mjs';

test('source inserts append whole files before includes, with original brace rule', () => {
  const root = '#INSERT [SystemScripts\\00Default\\First.txt] @Main\n#INSERT [SystemScripts\\00Default\\Second.txt] @Main\n';
  const sources = new Map([
    ['SystemScripts/00Default/First.txt', '[@_UseItem(1)]\n#INCLUDE [SystemScripts\\00Default\\Leaf.txt] @Main\n'],
    ['SystemScripts/00Default/Second.txt', '[@_UseItem(500)]\n#ACT\nGIVEGOLD 500\n'],
    ['SystemScripts/00Default/Leaf.txt', '[@Main]\n{\n#ACT\nGIVEGOLD 1\n}\n[@Other]\n{\nGIVEGOLD 999\n}\n'],
  ]);
  const { expanded } = expandDefaultNpc(root, sources);
  assert.deepEqual(expanded, ['[@_UseItem(1)]', '#ACT', 'GIVEGOLD 1', '[@_UseItem(500)]', '#ACT', 'GIVEGOLD 500']);
  const script = parseExpandedScript(root, expanded);
  assert.equal(script.sections[0].label, '@_UseItem(1)');
  assert.equal(script.sections[1].label, '@_UseItem(500)');
  assert.equal(script.insert_count, 2);
});

test('unbraced include is removed, and comments never become hook labels', () => {
  const root = '#INSERT [SystemScripts\\00Default\\First.txt] @Main';
  const sources = new Map([
    ['SystemScripts/00Default/First.txt', ';[@_Die]\n[@_LOGIN]\n#INCLUDE [SystemScripts\\00Default\\Leaf.txt] @Main'],
    ['SystemScripts/00Default/Leaf.txt', '[@Main]\nGIVEGOLD 999'],
  ]);
  const { expanded } = expandDefaultNpc(root, sources);
  assert.deepEqual(parseExpandedScript(root, expanded).sections, [{ label: '@_LOGIN', line_number: 2, lines: [] }]);
});

test('missing sources, path escape and cycles fail closed', () => {
  assert.throws(() => expandDefaultNpc('#INSERT [SystemScripts\\00Default\\Missing.txt] @Main', new Map()), /Missing/);
  assert.throws(() => expandDefaultNpc('#INSERT [SystemScripts\\00Default\\..\\Other.txt] @Main', new Map()), /Unbounded/);
  const cycle = '#INSERT [SystemScripts\\00Default\\Cycle.txt] @Main';
  assert.throws(() => expandDefaultNpc(cycle, new Map([['SystemScripts/00Default/Cycle.txt', cycle]])), /cycle/);
});

test('source compiler omissions retain exact mode, line and absent extensionless file', async () => {
  const parser = '        public void ParseCheck(string line)\n{\nswitch(line) { case "LEVEL": break; case "CHECKNAMELIST": break; }\n}\n'
    + '        public void ParseAct(List<NPCActions> acts, string line)\n{\nswitch(line) { case "CLEARNAMELIST": break; case "REMOVEFROMGUILD": break; }\n}\n'
    + '        public void Other() {}';
  const script = parseExpandedScript('', ['[@_LevelUp]', '#IF', 'CHECKNAMELIST NewbieGuild', 'CHECKLEVEL > 30', '#ACT',
    'REMOVEFROMGUILD NewbieGuild', 'REMOVENAMELIST NewbieGuild', 'CLEARNAMELIST NAMELISTFILENAME.txt', '#SAY', 'CHECKLEVEL > 30']);
  const compiled = await compileParserOmissions(script, parser, relative => relative === 'NameLists/NewbieGuild.txt');
  assert.deepEqual(compiled.ignored_lines.map(line => [line.line_number, line.line, line.reason]), [
    [3, 'CHECKNAMELIST NewbieGuild', 'missing-original-name-list'],
    [4, 'CHECKLEVEL > 30', 'unknown-original-opcode'],
    [7, 'REMOVENAMELIST NewbieGuild', 'unknown-original-opcode'],
    [8, 'CLEARNAMELIST NAMELISTFILENAME.txt', 'missing-original-name-list'],
  ]);
  assert.deepEqual(compiled.name_lists, [
    { relative_path: 'NameLists/NewbieGuild', exists: false },
    { relative_path: 'NameLists/NAMELISTFILENAME.txt', exists: false },
  ]);
});
