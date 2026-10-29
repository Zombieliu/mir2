// Authoring only. Produces pending v3 files with wx; never copies a bank,
// imports production code, freezes sources, starts a process or runs a load.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const directory = path.dirname(fileURLToPath(import.meta.url)), base = path.dirname(directory);
const previous = path.join(base, 'ordinary-capacity-bank-prepare-v2');
const sourcePrivate = path.join(previous, 'episode', 'private');
const assert = (value, message) => { if (!value) throw Error(message); };
assert(path.basename(directory) === 'ordinary-capacity-bank-prepare-v3', 'Fixed new v3 authoring directory required');
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = file => { const bytes = fs.readFileSync(file); return { file: path.resolve(file), bytes: bytes.length, sha256: digest(bytes) }; };
const json = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const write = (name, value) => fs.writeFileSync(path.join(directory, name), typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
const replace = (source, from, to) => { assert(source.split(from).length === 2, 'Unique authoring replacement missing: ' + from); return source.replace(from, to); };
const controls = ['.mir2-directory-durability', '.mir2-recovery-exclusive.lock', '.mir2-recovery-owner-v1.json', 'quarantine/.mir2-directory-durability'];
function controlPins(root) {
  const files = [], directories = [];
  const walk = current => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const file = path.join(current, entry.name), stat = fs.lstatSync(file);
      assert(!stat.isSymbolicLink(), 'Recovery symlink forbidden');
      if (stat.isDirectory()) { directories.push(path.relative(root, file).replaceAll('\\', '/')); walk(file); }
      else { assert(stat.isFile(), 'Regular recovery file required'); files.push(path.relative(root, file).replaceAll('\\', '/')); }
    }
  };
  walk(root);
  assert(JSON.stringify(files.sort()) === JSON.stringify([...controls].sort()) && JSON.stringify(directories.sort()) === JSON.stringify(['quarantine']), 'Only four normally closed recovery control files, no journal payloads, are permitted');
  return controls.map(relative => ({ relative, ...pin(path.join(root, relative)) }));
}
const oldPlan = json(path.join(previous, 'source-bank-plan-pending-v1.json'));
const sourceReceipt = pin(path.join(previous, 'episode', 'preparation-episode-receipt-v1.json')), receipt = json(sourceReceipt.file);
const sourceReport = receipt.rawOriginalReport, report = json(sourceReport.file);
assert(receipt.cleanupComplete === true && receipt.preparationComplete === false && receipt.coldBankEligible === false && receipt.sourceFirst3InventoryReuse === true && receipt.sourceOriginalFailedVerdictRetained === true, 'Actual v2 normally closed failed provenance required');
assert(report.ok === false && report.error === 'ordinary supplies: owner/map/health changed' && report.preparedCount === 4 && report.actorResults.length === 4 && report.preparedIndex.length === 4 && report.ordinaryPreparationEvidenceEligible === false, 'Original false v2 report with four prepared actors required');
const activeControls = controlPins(path.join(sourcePrivate, 'save-recovery'));
const archivedControls = controlPins(path.join(sourcePrivate, 'source-save-recovery-preserved'));
const relativeFiles = ['accounts.json', 'capacity-pool.json', 'capacity-auth-budget.json', 'recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt'];
const files = [...relativeFiles.map(relative => ({ relative, ...pin(path.join(sourcePrivate, relative)) })), ...activeControls.map(p => ({ ...p, relative: 'save-recovery/' + p.relative }))];
const pool = json(path.join(sourcePrivate, 'capacity-pool.json'));
assert(pool.accounts.length === 100 && pool.accounts.filter(a => a.state === 'ready').length === 5 && pool.accounts.filter(a => a.state === 'registered').length === 95, 'Entire actual v2 pool must remain five ready and 95 registered');
assert(Object.keys(json(path.join(sourcePrivate, 'accounts.json')).accounts).length === 101, 'Entire v2 Store including additional account is required');
const plan = { ...oldPlan, packageId: path.basename(directory), sourcePrivate,
  sourceReceipt, sourceReport, sourceCleanup: receipt.cleanupRecord, sourcePreparedIndex: receipt.preparedIndexRecord,
  sourceOwnedProcess: pin(path.join(previous, 'episode', 'owned-process-v1.json')),
  sourceFrozenManifest: receipt.manifestPin,
  sourceOriginalVerdict: { ok: false, error: report.error, preparedCount: 4, ordinaryPreparationEvidenceEligible: false },
  sourcePreparedCharacters: report.preparedIndex.map(r => ({ name: r.name, characterIndex: r.characterIndex, map: r.map, x: r.x, y: r.y, normalLogout: r.normalLogout })),
  sourceFailedCharacter: { name: report.failedActor.name, characterIndex: report.failedActor.characterIndex, notPrepared: true,
    catchTimeNormalLogoutFlag: report.failedActor.normalLogout, originalFailure: report.failedActor.error,
    scope: 'The fifth actor failed the original owner/map/health guard and remains not prepared. V3 may use only the production ordinary recovery/preparation path; no wrapper HP, stock, position or account-state edits.' },
  sourceFirst3InventoryReuse: true, sourcePreparedEligibilityInherited: false, sourceReadyCharacters: 5, sourceRegisteredAccounts: 95,
  sourceStoreTotalAccounts: 101, files, fileCount: files.length, totalBytes: files.reduce((total, item) => total + item.bytes, 0),
  priorRecoveryArchive: { directory: path.join(sourcePrivate, 'source-save-recovery-preserved'), retainInPlace: true, copiedToNewActive: false,
    files: archivedControls, scope: 'The earlier v1 recovery controls stay byte-identical in the v2 archive and are not part of the new 11-file copy. Only v2 current active controls are archived in v3.' },
  scope: 'Actual v2 failed after four normally prepared/saved actors. Preserve the complete 100-entry pool, entire 101-account Store, actual original ledger, four secret files and current four recovery controls. V3 starts preparedCount at zero and ordinarily processes all 100 actors. First four require fresh ordinary relogin stock reuse evidence. The fifth is a saved ready identity, never inherited preparation evidence. Original v1/v2 false results remain unchanged; they are provenance, not current source qualification.' };
write('source-bank-plan-pending-v1.json', plan);
const config = { ...json(path.join(previous, 'prepare-config-pending-v1.json')), packageId: path.basename(directory),
  bankPlan: pin(path.join(directory, 'source-bank-plan-pending-v1.json')),
  sourceQualification: { expectedHead: null, rootReceipt: null, state: 'PENDING_NEW_ACTUAL_HEAD_AND_FRESH_ZERO_SKIP_ROOT_RECEIPT', oldV2ReportIsNotCurrentQualification: true },
  sourcePartialBank: { controlledAccounts: 100, readyCharacters: 5, registeredAccounts: 95, preparedActors: 4,
    failedFifthNotPrepared: true, sourceEligibilityInherited: false, first4MustBeOrdinarilyReloggedAndReuseActualStock: true,
    first4ReuseScope: 'Fresh before/after HP stock >=12, no hunting/harvest/pickup/sale/purchase; verify the actual raw ordinaryPreparation result.commandCounts, not absent summary fields.' },
  ordinaryRecovery: { performedBy: 'Current production runPreparationBank before equip/preparation', wrapperSendsRecoveryCommands: false,
    configurationOverrides: false, qualificationPending: true, scope: 'No new CLI option. New production held-HP recovery must be bound by the actual current HEAD and fresh Root receipt; native measurement maintenance remains unchanged.' },
  scope: 'Continue the complete raw v2 partial bank through normal login, production ordinary held-item recovery when required, and preparation only. All 100 actors are processed from zero new prepared count. No seed, Store/pool/stock/position edit, auth-ledger reset, loginNotBefore override, engine or measurement. Four old successes need fresh stock and no-economic-work evidence; old fifth failure remains originalfalse.' };
write('prepare-config-pending-v1.json', config);
let copy = fs.readFileSync(path.join(previous, 'copy-source-bank-v1.mjs'), 'utf8');
copy = replace(copy, "'ordinary-capacity-bank-prepare-v1','episode','private'", "'ordinary-capacity-bank-prepare-v2','episode','private'");
copy = replace(copy, "'ActualV1 partial source bank required'", "'ActualV2 partial source bank required'");
copy = replace(copy, "assert(receipt.cleanupComplete===true", "assert(receipt.packageId==='ordinary-capacity-bank-prepare-v2'&&receipt.sourceFirst3InventoryReuse===true&&receipt.sourceOriginalFailedVerdictRetained===true&&receipt.cleanupComplete===true");
copy = replace(copy, "report.error==='ordinary supplies: only live Deer may be hunted'&&report.preparedCount===3&&report.actorResults.length===3&&report.preparedIndex.length===3", "report.error==='ordinary supplies: owner/map/health changed'&&report.preparedCount===4&&report.actorResults.length===4&&report.preparedIndex.length===4");
copy = replace(copy, "'Original3 prepared actors or saved index differ'", "'Original4 prepared actors or saved index differ'");
copy = replace(copy, "'Failed fourth actor cannot become prepared evidence'", "'Failed fifth actor cannot become prepared evidence'");
copy = replace(copy, "pool.accounts.filter(a=>a.state==='ready').length===4&&pool.accounts.filter(a=>a.state==='registered').length===96", "pool.accounts.filter(a=>a.state==='ready').length===5&&pool.accounts.filter(a=>a.state==='registered').length===95");
copy = replace(copy, "assert(pool.schemaVersion===1", "assert(plan.sourceReadyCharacters===pool.accounts.filter(a=>a.state==='ready').length&&plan.sourceRegisteredAccounts===pool.accounts.filter(a=>a.state==='registered').length,'Plan pool state metadata differs from actual pinned pool');assert(pool.schemaVersion===1");
copy = replace(copy, "const get=n=>JSON.parse", "const prior=plan.priorRecoveryArchive;assert(prior&&same(prior.directory,path.join(SOURCE,'source-save-recovery-preserved'))&&prior.retainInPlace===true&&prior.copiedToNewActive===false&&prior.files.length===4,'Old v2 archive must remain in place');const priorTree=recoveryTree(prior.directory);assert(JSON.stringify(priorTree.files)===JSON.stringify(controls.sort())&&JSON.stringify(priorTree.directories)===JSON.stringify(['quarantine'])&&JSON.stringify(prior.files.map(p=>p.relative).sort())===JSON.stringify(controls.sort()),'Old archive controls changed');for(const p of prior.files){assert(same(p.file,path.join(prior.directory,p.relative)),'Prior archive path differs');checked(p)}const get=n=>JSON.parse");
copy = replace(copy, 'sourceReadyCharacters:4,sourcePreparedCount:3', 'sourceReadyCharacters:5,sourcePreparedCount:4');
copy = replace(copy, 'sourceFailedFourthPromoted:false', 'sourceFailedFifthPromoted:false');
copy = replace(copy, 'sourceRecoveryPreserved:true,activeRecoveryInitiallyEmpty:true', 'sourceRecoveryPreserved:true,sourcePriorRecoveryArchiveRetainedInPlace:true,priorRecoveryArchiveCopiedToActive:false,activeRecoveryInitiallyEmpty:true');
write('copy-source-bank-v1.mjs', copy);
let freezer = fs.readFileSync(path.join(previous, 'freeze-prepare-v1.mjs'), 'utf8');
freezer = replace(freezer, 'readyCharacters:4,sourcePreparedCount:3', 'readyCharacters:5,sourcePreparedCount:4');
write('freeze-prepare-v1.mjs', freezer);
let library = fs.readFileSync(path.join(previous, 'source-lib-v1.mjs'), 'utf8');
library = replace(library, "['/playtest-capacity-prepare.mjs','/playtest-capacity-soak.mjs','/capacity-ordinary-supplies.mjs','/quest-agent/protocol-funding.mjs']", "['/playtest-capacity-prepare.mjs','/playtest-capacity-prepare.test.mjs','/playtest-capacity-soak.mjs','/playtest-capacity-soak.test.mjs','/capacity-ordinary-supplies.mjs','/capacity-ordinary-supplies.test.mjs','/quest-agent/protocol-funding.mjs']");
write('source-lib-v1.mjs', library);
write('owned-child-v1.mjs', fs.readFileSync(path.join(previous, 'owned-child-v1.mjs'), 'utf8'));
let runner = fs.readFileSync(path.join(previous, 'run-owned-prepare-v1.mjs'), 'utf8');
runner = replace(runner, "'Only the reviewed fresh100 single-worker preparation is allowed'", "'Only the reviewed continued100 single-worker preparation is allowed'");
runner = replace(runner, 'Original100pool, fullStore, four secrets and actual25+5/901000ms ledger remain intact.', 'Original100pool, fullStore, four secrets and actual25+5/901000ms ledger remain intact. All100 start with zero newly prepared actors; production ordinary recovery is used without wrapper commands.');
const oldReuse = "const sourceFirst3InventoryReuse=bankCopyReceipt.sourcePreparedCharacters.length===3&&bankCopyReceipt.sourcePreparedCharacters.every(expected=>(report?.actorResults??[]).some(r=>r.name===expected.name&&r.characterIndex===expected.characterIndex&&r.before?.hpStock>=12&&r.after?.hpStock>=12&&r.purchased===0&&r.soldVenison===0&&r.funding===null&&r.normalLogout===true));";
const newReuse = `async function first4InventoryReuseEvidence(actualReport,expected){
 const proofs=new Map(),duplicates=new Set();
 if(!actualReport||expected.length!==4||!Array.isArray(actualReport.actorResults)||typeof actualReport.samplesPath!=='string')return{passed:false,actors:[],scope:'No actual completed report/samples for fresh four-actor reuse proof'};
 const sampleFile=path.resolve(actualReport.samplesPath);assert(sampleFile.startsWith(path.resolve(root,'reports')+path.sep),'Original samples escaped owned reports');
 const wanted=new Set(expected.map(r=>r.name));const input=createReadStream(sampleFile);const lines=createInterface({input,crlfDelay:Infinity});
 for await(const line of lines){if(!line.trim())continue;const row=JSON.parse(line);if(row.type!=='ordinaryPreparation'||!wanted.has(row.actor))continue;if(proofs.has(row.actor))duplicates.add(row.actor);const r=row.result,counts=r?.commandCounts;
  const completeCounts=Boolean(counts&&typeof counts==='object'&&!Array.isArray(counts)&&Object.values(counts).every(n=>Number.isSafeInteger(n)&&n>=0)&&Object.values(counts).reduce((n,value)=>n+value,0)===r.commands);
  const sentNone=types=>completeCounts&&types.every(type=>!Object.hasOwn(counts,type)||counts[type]===0);
  const identity=expected.find(r=>r.name===row.actor),summary=actualReport.actorResults.find(r=>r.name===row.actor&&r.characterIndex===identity.characterIndex);
  const fresh=Boolean(r?.before&&r?.after&&r.before.ownerName===row.actor&&r.after.ownerName===row.actor&&r.before.map==='0'&&r.after.map==='0'&&r.before.ownerId===r.after.ownerId&&Number.isSafeInteger(r.before.sequence)&&Number.isSafeInteger(r.after.sequence)&&r.after.sequence>r.before.sequence&&Number.isFinite(r.before.monotonicMs)&&Number.isFinite(r.after.monotonicMs)&&r.after.monotonicMs>=r.before.monotonicMs);
  const passed=Boolean(summary&&summary.normalLogout===true&&summary.before?.hpStock>=12&&summary.after?.hpStock>=12&&summary.purchased===0&&summary.soldVenison===0&&summary.funding===null&&r?.status==='ready'&&fresh&&r.before.hpStock>=12&&r.after.hpStock>=12&&r.purchased===0&&r.soldVenison===0&&r.funding===null&&sentNone(['attack','attackV2','harvest','pickUp','sellItem','buyItem']));
  proofs.set(row.actor,{name:row.actor,characterIndex:identity.characterIndex,passed,actualCompleteModuleCommandCounts:completeCounts,freshBeforeAfter:fresh,beforeHpStock:r?.before?.hpStock??null,afterHpStock:r?.after?.hpStock??null,beforeSequence:r?.before?.sequence??null,afterSequence:r?.after?.sequence??null,noHuntingHarvestPickupSalePurchase:sentNone(['attack','attackV2','harvest','pickUp','sellItem','buyItem']),purchased:r?.purchased??null,soldVenison:r?.soldVenison??null,funding:r?.funding??null});
 }
 const actors=expected.map(r=>proofs.get(r.name)??{name:r.name,characterIndex:r.characterIndex,passed:false});return{passed:duplicates.size===0&&actors.every(r=>r.passed),actors,duplicateOrdinaryPreparationRows:duplicates.size,scope:'Actual raw ordinaryPreparation result commandCounts must exist and sum to commands; absent members mean unsent types only within that complete module-owned count object. No missing summary commandCounts is treated as zero. Fresh ordinary-login before/after stock >=12; no hunting, harvest, pickup, sale or purchase. No inherited readiness.'};
}
let sourceFirst4InventoryReuseEvidence;try{sourceFirst4InventoryReuseEvidence=await first4InventoryReuseEvidence(report,bankCopyReceipt.sourcePreparedCharacters)}catch{executionError??='Original first-four inventory reuse evidence unavailable';sourceFirst4InventoryReuseEvidence={passed:false,actors:[],failure:'Original first-four inventory reuse evidence unavailable'}}const sourceFirst4InventoryReuse=sourceFirst4InventoryReuseEvidence.passed;`;
runner = replace(runner, oldReuse, newReuse);
runner = replace(runner, 'preparedIndexValid&&sourceFirst3InventoryReuse&&report?.preparedCount', 'preparedIndexValid&&sourceFirst4InventoryReuse&&report?.preparedCount');
runner = replace(runner, 'sourcePreparedEligibilityInherited:false,sourceFirst3InventoryReuse,seederInvoked:false', 'sourcePreparedEligibilityInherited:false,sourceFirst4InventoryReuse,sourceFirst4InventoryReuseEvidence,seederInvoked:false');
write('run-owned-prepare-v1.mjs', runner);
console.log(JSON.stringify({packageId:path.basename(directory),state:'PENDING_NEW_ROOT_HEAD_RECEIPT_NOT_FROZEN_NOT_RUN',sourceFiles:11,sourcePreparedCount:4,sourceReadyCharacters:5,sourceRegisteredAccounts:95,originalFailureRetained:true,bankCopied:false,freezeExecuted:false,loadStarted:false,capacityAccepted:false,p0Accepted:false},null,2));
