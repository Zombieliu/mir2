// Success-only evidence contract. A verified failed probe still fails admission.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { createReadStream } from 'node:fs';
import { base, project, samePath } from './input-lib.mjs';

export const REQUIRED_PROBE12_RUN_ID='1792908575146-40a66fff';
export const REQUIRED_PROBE12_CHILD='capacity-native10-v2-compact-halo-followup-12-diagnostics';
const realm='release-native-10-v2-compact-halo-feasibility-12';
const topic=path.join(project,'docs/generated/cell-p0-p1-completion-20261024');
const hex=s=>typeof s==='string'&&/^[a-f0-9]{64}$/.test(s);
const relative=s=>{if(typeof s!=='string'||!s||path.isAbsolute(s)||/^[A-Za-z]:/.test(s))throw Error('Invalid public relative path');const parts=s.split(/[\\/]/);if(parts.some(p=>!p||p==='.'||p==='..'||/^(?:private|store|accounts|credentials|secrets?)$/i.test(p)))throw Error('Forbidden public path component');return parts.join('/');};
async function under(root,rel){const safe=relative(rel),absolute=path.resolve(root,...safe.split('/')),rr=await fs.realpath(root),real=await fs.realpath(absolute);const from=path.relative(rr,real);if(!from||from==='..'||from.startsWith('..'+path.sep)||path.isAbsolute(from)||!samePath(real,absolute))throw Error('Redirected or escaped public evidence');return absolute;}
async function pin(file){const hash=crypto.createHash('sha256');let bytes=0;for await(const chunk of createReadStream(file)){bytes+=chunk.length;hash.update(chunk);}return{file,bytes,sha256:hash.digest('hex')};}
async function json(file){const stat=await fs.stat(file);if(!stat.isFile()||stat.size>2*1024*1024)throw Error('Public metadata exceeds the fixed2MiB read limit');return JSON.parse(await fs.readFile(file,'utf8'));}
function eqPin(actual,expected,label){if(!actual||!expected||!Number.isSafeInteger(expected.bytes)||expected.bytes<0||!hex(expected.sha256)||!samePath(actual.file??'',expected.file??expected.path??'')||actual.bytes!==expected.bytes||actual.sha256!==expected.sha256)throw Error('Independent probe12 '+label+' raw pin differs');}
function allTrue(values){return values.every(v=>v===true);}

export async function verifyProbe12CollectedEvidence({report,reportPin,rawPin,cleanup,provenance,sources,scene,binding,monitor}){
 if(report.runId!==REQUIRED_PROBE12_RUN_ID||report.ok!==true)throw Error('Only the exact successful compact12 run is eligible; failed12 or later probes cannot substitute');
 const directory=await under(topic,REQUIRED_PROBE12_CHILD+'/public-verification.json').then(f=>path.dirname(f));
 const names={report:`${realm}/reports/${REQUIRED_PROBE12_RUN_ID}.report.json`,reportProvenance:`${realm}/report-provenance.json`,catalog:`collect-${realm}-public-artifact-catalog.json`,publicCopy:'public-copy-provenance.json',publicVerification:'public-verification.json',collectionVerification:'collection-verification.json',projection:`${realm}/reports/${REQUIRED_PROBE12_RUN_ID}.raw-projection.json`};
 const paths={},pins={};for(const[key,rel]of Object.entries(names)){paths[key]=await under(directory,rel);pins[key]=await pin(paths[key]);}
 const publicReport=await json(paths.report),catalog=await json(paths.catalog),copy=await json(paths.publicCopy),verification=await json(paths.publicVerification),collection=await json(paths.collectionVerification);
 if(pins.report.bytes!==reportPin.bytes||pins.report.sha256!==reportPin.sha256||JSON.stringify(publicReport)!==JSON.stringify(report))throw Error('Collected compact12 report bytes differ from the original');
 if(verification.schemaVersion!==1||verification.runId!==report.runId||!allTrue([verification.reportOk,verification.originalVerdictPreserved,verification.normalProcessCleanup,verification.normalOwnedCleanup,verification.diagnosticsComplete])||verification.capacityAccepted!==false||verification.rawVerifierAccepted!==false||verification.rawBytes!==rawPin.bytes||verification.rawSha256!==rawPin.sha256||verification.backendSourceCommit!==binding.backendSourceCommit||verification.driverSourceCommit!==binding.driverSourceCommit)throw Error('Completed independent public verification does not bind a successful compact12');
 if(!Array.isArray(copy.files)||!Array.isArray(catalog.files)||copy.files.length!==catalog.files.length+1||verification.originalCopiesVerified!==copy.files.length)throw Error('Completed compact12 public-copy inventory differs');
 for(const row of copy.files){const prefixed=relative(row.path);if(!prefixed.startsWith(REQUIRED_PROBE12_CHILD+'/'))throw Error('Wrong compact12 public child');const destination=await under(directory,prefixed.slice(REQUIRED_PROBE12_CHILD.length+1)),source=await under(base,row.source);eqPin({...await pin(destination),file:source},{file:source,bytes:row.bytes,sha256:row.sha256},'public-copy');}
 const run=catalog.run;
 if(!run||run.runId!==report.runId||run.backendSourceCommit!==binding.backendSourceCommit||run.driverSourceCommit!==binding.driverSourceCommit||run.driverSourceSha256!==sources.driverSourceSha256||run.gatewaySha256!==binding.release.binary.sha256||run.backendInputCount!==41||run.sceneSourceSha256!==scene.sourceSha256||run.scene10Sha256!==scene.probeScene.sha256||run.scene100Sha256!==scene.formalScene.sha256||!allTrue([run.reportOk,run.normalProcessCleanup,run.normalOwnedCleanup,run.diagnosticsComplete])||run.capacityAccepted!==false||run.rawVerifierAccepted!==false)throw Error('Collected compact12 catalog source/scene/verdict differs');
 const manifestFile=await under(directory,'raw-artifact-manifest.json'),manifest=await json(manifestFile),entry=manifest.files?.[0];
 if(manifest.schemaVersion!==1||manifest.files?.length!==1||entry.runId!==report.runId||entry.report!==REQUIRED_PROBE12_CHILD+'/'+names.report||entry.projection!==REQUIRED_PROBE12_CHILD+'/'+names.projection||entry.path!==`${realm}/reports/${report.runId}.samples.jsonl`||entry.bytes!==rawPin.bytes||entry.sha256!==rawPin.sha256)throw Error('Compact12 raw manifest/projection/report linkage differs');
 // The independent verifier has a separately named, pinned original receipt.
 const receiptFile=await under(base,'probe12-independent-verification-v1/final-verification.json'),receipt=await json(receiptFile);
 if(receipt.schemaVersion!==1||receipt.runId!==report.runId||!allTrue([receipt.independentVerificationPassed,receipt.probePassed,receipt.formalPreconditionOnly])||receipt.capacityAccepted!==false||receipt.rawVerifierAccepted!==false)throw Error('Independent compact12 receipt must verify a successful probe without promoting capacity');
 const identity=receipt.identities;
 if(!identity||identity.backendSourceCommit!==binding.backendSourceCommit||identity.backendInputs!==41||identity.gatewaySha256!==binding.release.binary.sha256||identity.driverSourceCommit!==binding.driverSourceCommit||identity.driverSourceSha256!==sources.driverSourceSha256||identity.scene10Sha256!==scene.probeScene.sha256||identity.scene100Sha256!==scene.formalScene.sha256||identity.sceneSourceSha256!==scene.sourceSha256||identity.sourceConfigSha256!==sources.sourceConfigSha256)throw Error('Independent compact12 receipt source/scene/config identity differs');
 eqPin(reportPin,receipt.pins?.report,'report');eqPin(rawPin,receipt.pins?.raw,'raw');
 for(const key of ['reportProvenance','catalog','publicCopy','publicVerification','collectionVerification','projection'])eqPin(pins[key],receipt.pins?.[key],key);
 const monitorFile=await under(base,realm+'/monitor-history.jsonl'),monitorPin=await pin(monitorFile);eqPin(monitorPin,receipt.pins?.monitor,'monitor');
 const rootCleanupFile=await under(base,'probe12-root-independent-cleanup-v1.json'),rootCleanupPin=await pin(rootCleanupFile);eqPin(rootCleanupPin,receipt.pins?.rootCleanup,'root-cleanup');
 const expected=[['stage-2',300,5],['stage-10',300,5],['soak',360,6]];
 if(JSON.stringify(receipt.phaseChecks?.map(p=>p.name))!==JSON.stringify(expected.map(p=>p[0])))throw Error('Independent compact12 phase order differs');
 for(const[name,seconds,windows]of expected){const c=receipt.phaseChecks.find(p=>p.name===name),phase=report.stages.find(p=>p.name===name);if(c.configuredSeconds!==seconds||c.measuredSeconds!==phase.measuredSeconds||c.completeWindowCount!==windows||c.requiredCompleteWindows!==windows||!allTrue([c.rawReportWindowsEqual,c.activityCounterReplayEqual,c.strictGatesPassed]))throw Error('Independent compact12 complete windows or replay differs');}
 if(!allTrue([receipt.cleanup?.allSevenTrue,receipt.cleanup?.gatewayExit0,receipt.cleanup?.monitorExit0,receipt.monitor?.rawStreamValidated,receipt.monitor?.complete,receipt.monitor?.noStops])||receipt.monitor.standaloneHistorySHA!==monitorPin.sha256||!Array.isArray(receipt.monitor.phaseBrackets)||JSON.stringify(receipt.monitor.phaseBrackets)!==JSON.stringify(monitor.phaseBrackets)||receipt.HP?.notContinuousHpCoverage!==true||!Array.isArray(receipt.HP.observedStatuses)||typeof receipt.HP.successProofChecked!=='boolean')throw Error('Independent compact12 cleanup/monitor/HP evidence boundary differs');
 if(cleanup.report?.runId!==report.runId||provenance.runId!==report.runId||collection.runId!==report.runId)throw Error('Collected compact12 run linkage differs');
 return{schemaVersion:1,runId:report.runId,publicCollectionVerified:true,independentVerificationPassed:true,probePassed:true,capacityAccepted:false,rawVerifierAccepted:false,formalPreconditionOnly:true,files:[...Object.values(pins),await pin(manifestFile),await pin(receiptFile),monitorPin,rootCleanupPin]};
}
