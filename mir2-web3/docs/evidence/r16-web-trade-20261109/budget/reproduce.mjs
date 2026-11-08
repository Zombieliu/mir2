import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';

// External audit evidence only. Reads real source, evaluates the real TS
// modules in memory, writes one new external receipt, never opens a socket.
const [repoArgument, outputArgument] = process.argv.slice(2);
if (!repoArgument || !outputArgument || process.argv.length !== 4)
  throw new Error('usage: node reproduce.mjs <repo-root> <new-external-receipt.json>');
const repo = path.resolve(repoArgument);
const output = path.resolve(outputArgument);
const relativeOutput = path.relative(import.meta.dirname, output);
if (!relativeOutput || relativeOutput.startsWith('..') || path.isAbsolute(relativeOutput))
  throw new Error('receipt must stay in this external audit directory');

const sourcePins = {};
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
async function source(relative) {
  const text = await fs.readFile(path.join(repo, relative), 'utf8');
  sourcePins[relative] = sha256(text);
  return text;
}
const nativePath = 'apps/game-client/client-bevy/src/social.rs';
const native = await source(nativePath);
const nativeLimit = Number(native.match(/pub const MAX_SOCIAL_PENDING: usize = (\d+);/)?.[1]);
const enumBody = native.slice(native.indexOf('pub enum SocialPendingOperation {')).split(/\r?\n}\r?\n/, 1)[0];
const nativeVariants = [...enumBody.matchAll(/^    ([A-Z][A-Za-z]+)(?:\s*\{|\(|,)/gm)].map(match => match[1]);
if (nativeLimit !== 8 || nativeVariants.length !== 20)
  throw new Error('Native contract changed; re-audit rather than silently update expectations');

const require = createRequire(pathToFileURL(path.join(repo, 'apps/web/scripts/test-group-operations.mjs')));
const ts = require('typescript');
const texts = new Map();
for (const name of ['group-operations', 'guild-rank-operations', 'social-parity-actions'])
  texts.set(name, await source(`apps/web/lib/${name}.ts`));
const cache = new Map();
function load(name) {
  if (cache.has(name)) return cache.get(name);
  if (!texts.has(name)) throw new Error('unexpected runtime import');
  const compiled = ts.transpileModule(texts.get(name), {
    fileName: name + '.ts', reportDiagnostics: true,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true },
  });
  if ((compiled.diagnostics ?? []).some(item => item.category === ts.DiagnosticCategory.Error))
    throw new Error('source did not transpile');
  const module = { exports: {} }; cache.set(name, module.exports);
  new Function('module', 'exports', 'require', compiled.outputText)(module, module.exports,
    id => load(id.replace(/^\.\//, '')));
  return module.exports;
}
const { GroupOperations, groupOperationCommand } = load('group-operations');
const { GuildRankOperations } = load('guild-rank-operations');
const owner = { socket: {}, connectionGeneration: 1, sessionGeneration: 2, sceneRevision: 3,
  playerObjectId: 1000, mapFileName: '0' };
const groupSource = { owner, playerName: 'Leader', members: [] };
const guildSource = { owner, scope: {}, guildName: 'Guild', actor: 'Leader', ownRankIndex: 0,
  permissions: ['CanChangeRank'], ranks: [
    { index: 0, name: 'Leader', permissions: ['CanChangeRank'] },
    { index: 2, name: 'Members', permissions: ['CanRecruit'] },
  ] };
const group = new GroupOperations();
const guild = new GuildRankOperations();
const entered = [];
for (let index = 0; index < nativeLimit; index++) {
  const proof = group.reserve('addMember', 'Absent' + index, groupSource);
  entered.push(Boolean(proof && group.claim(proof, groupSource, groupOperationCommand(proof))));
}
const sameQueueNinthRejected = group.reserve('addMember', 'Ninth', groupSource) === null;
const guildProof = guild.reservePermission(2, 'CanStoreItem', true, guildSource, 1000);
const ninthGuildEntered = Boolean(guildProof && guild.claim(guildProof, guildSource, guildProof.command, 1000));
const combinedUnresolved = entered.filter(Boolean).length + Number(ninthGuildEntered);
const report = {
  schemaVersion: 1, audit: 'r16-cross-social-budget', status: 'open',
  parityPassed: combinedUnresolved <= nativeLimit,
  evidenceKind: 'actual-typescript-module-memory-reproduction',
  nativeContract: { file: nativePath, maxPending: nativeLimit, variants: nativeVariants,
    rustExecuted: false, comparison: 'source constant and existing mixed-operation regression' },
  fixture: { syntheticOnly: true, samePhysicalOwner: true, groupNames: 'Absent0..Absent7',
    groupEmpty: true, guildOwnRankIndex: 0, guildTargetRankIndex: 2,
    guildPermission: 'CanStoreItem', enabled: true, clock: 1000 },
  observed: { groupEntered: entered.filter(Boolean).length, sameQueueNinthRejected,
    ninthGuildPermissionEntered: ninthGuildEntered, combinedUnresolved,
    noAcknowledgementsSupplied: true },
  expectedNativeAdmission: { afterEightPending: false },
  limits: { pageHostExecuted: false, socketSendEntered: false, rustExecuted: false,
    productFilesWritten: 0, servicesStarted: 0, networkConnections: 0,
    runtimeFinallyNote: 'Page permission finally clears its UI/throttle row; this does not acknowledge a server operation.' },
  sourcePins,
};
await fs.writeFile(output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ status: report.status, parityPassed: report.parityPassed,
  observed: report.observed, nativeLimit, sourcePins }));
// This is intentionally a failing parity receipt, not an inverted always-green
// product test. Preserve the failure even though it reproduces the defect.
process.exitCode = report.parityPassed ? 0 : 1;
