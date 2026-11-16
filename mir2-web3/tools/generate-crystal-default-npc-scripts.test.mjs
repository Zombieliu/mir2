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

const checkHumParser = minimum => '        public void ParseCheck(string line)\n{\nswitch(line) { case "CHECKHUM":\n'
  + `if (parts.Length < ${minimum}) return;\nCheckList.Add(new NPCChecks());\nbreak; }\n}\n`
  + '        public void ParseAct(List<NPCActions> acts, string line)\n{\nswitch(line) { case "BREAK": break; }\n}\n'
  + '        public void Other() {}';

test('short CHECKHUM is compiled out while complete invalid checks and SAY text remain', async () => {
  const script = parseExpandedScript('', ['[@_OnAcceptQuest(149)]', '#IF',
    'CHECKHUM', 'CHECKHUM 1', 'CHECKHUM 1 D10071', 'checkhum   1   EM002', 'CHECKHUM %ARG(0)',
    'CHECKHUM >= 1 D10071', 'CHECKHUM >= 1 D10071 2', 'CHECKHUM >= invalid D10071',
    'CHECKHUM = 1 D10071', 'CHECKHUM >= 1 D10071 invalid', 'CHECKHUM >= 1 D10071 1 unused',
    '#SAY', 'CHECKHUM 1 D10071']);
  const before = JSON.stringify(script);
  const compiled = await compileParserOmissions(script, checkHumParser(4), () => false);
  assert.deepEqual(compiled.ignored_lines.map(line => [line.line_number, line.line, line.reason]), [
    [3, 'CHECKHUM', 'missing-original-checkhum-arguments'],
    [4, 'CHECKHUM 1', 'missing-original-checkhum-arguments'],
    [5, 'CHECKHUM 1 D10071', 'missing-original-checkhum-arguments'],
    [6, 'checkhum   1   EM002', 'missing-original-checkhum-arguments'],
    [7, 'CHECKHUM %ARG(0)', 'missing-original-checkhum-arguments'],
  ]);
  assert.equal(JSON.stringify(script), before);
  assert.deepEqual(compiled.name_lists, []);
});

test('CHECKHUM omission requires the supplied original minimum-argument proof', async () => {
  const script = parseExpandedScript('', ['[@_OnAcceptQuest(149)]', '#IF', 'CHECKHUM 1 D10071']);
  await assert.rejects(compileParserOmissions(script, checkHumParser(5), () => false), /CHECKHUM.*parser/);
});
