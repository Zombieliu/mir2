import fs from 'node:fs/promises';
import path from 'node:path';
import { root, project, driverFrozenFile, read, sha, git } from './input-lib.mjs';

// Finalize only after the new nine-input freeze and clean committed driver exist.
export const EXPECTED_DRIVER_SOURCE_COMMIT = '6e05932000740abc88997db9496c57b301e0a4a9';
export const EXPECTED_DRIVER_SOURCE_SHA256 = 'c7928e91e0528132c470ad1d87ffd63656eb59b2ce35b602b5e84f4603434298';
export const EXPECTED_NATIVE_PATROL_PLANNING = Object.freeze({
  scope: 'Native primed patrol fallback only; original fast path and at most four existing corner targets remain.',
  maximumFallbackCornerTargets: 4, pathExpansionLimit: 4000,
  selectionRule: 'Prefer the first existing collision-aware path with two legal collinear Run steps; otherwise retain the original first Walk.',
  primingRule: 'Only while native Run priming is active; original cadence, transfer/coverage checks and offered-opportunity accounting remain.'
});
export const EXPECTED_NAVIGATION_COORDINATION = Object.freeze({
  scope: 'Native tool-local conservative union of original AOI obstacles, active controlled owner receipts and one in-flight movement hint per connection; never authoritative occupancy, snapshots or AOI.',
  maximumHintCellsPerConnection: 7,
  blockedOpportunityRule: 'A conflicting prepared move is still an offered navigation-blocked/native opportunity; original cadence, completion and correction gates remain.'
});
export const EXPECTED_MEASUREMENT_CLOCK = Object.freeze({
  scope: 'All measurement phases; seconds retains configured duration and measuredSeconds is actual elapsed wall time.',
  minimumCompleteWindowMs: 60000,
  completeWindowCountRule: 'At least floor(configured seconds / 60) complete windows, each starting at the previous actual close, plus the full configured phase duration.',
  durationRule: 'Activity and rate assessment use actual measuredSeconds; no short window is relabelled as sixty seconds or duplicated after a stall.',
  partialWindowRule: 'The final partial minute remains in whole-phase and continuous assessment, without counting as a complete rolling window.'
});
export const EXPECTED_NATIVE_HP_MAINTENANCE = Object.freeze({
  "scope": "Already-held ordinary HP potions only; no MP, purchase, grants or revival.",
  "hpThreshold": 0.6,
  "reuseDelayMs": 6000,
  "evidenceDeadlineMs": 2000,
  "commandBudgetPerSecond": 3,
  "rule": "Fresh matching UseItem success plus complete-inventory consumption or exact HP increase within the same deadline; consumption is not proof of healing.",
  "activityRule": "Independent auxiliary promise; native opportunities continue and HP maintenance never credits movement, attack or native completion."
});
export const EXPECTED_NATIVE_ATTACK_WITNESS = Object.freeze({
  "scope": "New native attack witness selection; the original full AOI membership range remains16.",
  "maximumOwnerAndTargetDistance": 6,
  "rule": "Choose the nearest active living same-map observer that actually sees both attacker and target; reselect immediately before send. No witness leaves the native opportunity incomplete.",
  "damageRule": "Original dual-end damage/health evidence,900ms deadline and ambiguous-target quarantine remain unchanged."
});
export async function verifyDriverReportContract(report) {
  if (!/^[a-f0-9]{40}$/.test(EXPECTED_DRIVER_SOURCE_COMMIT) || !/^[a-f0-9]{64}$/.test(EXPECTED_DRIVER_SOURCE_SHA256) ||
      !EXPECTED_NATIVE_PATROL_PLANNING || typeof EXPECTED_NATIVE_PATROL_PLANNING.scope !== 'string') throw Error('Patrol driver template has not been finalized after source freeze/commit');
  const frozen = await read(driverFrozenFile);
  if (frozen.files?.length !== 9 || frozen.sourceSha256 !== EXPECTED_DRIVER_SOURCE_SHA256 || sha(JSON.stringify(frozen.files)) !== frozen.sourceSha256 ||
      JSON.stringify(frozen.nativePatrolPlanning) !== JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING) ||
      JSON.stringify(frozen.measurementClock) !== JSON.stringify(EXPECTED_MEASUREMENT_CLOCK) ||
      git(['rev-parse','HEAD']).toString('utf8').trim() !== EXPECTED_DRIVER_SOURCE_COMMIT) throw Error('Exact final patrol driver source identity differs');
  const file = path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs'), source = await fs.readFile(file, 'utf8');
  const sourcePin = frozen.files.find(row => row.file === 'apps/web/scripts/playtest-capacity-soak.mjs');
  if (!sourcePin || Buffer.byteLength(source) !== sourcePin.bytes || sha(Buffer.from(source)) !== sourcePin.sha256 ||
      sha(Buffer.from(source.replace(/\r\n/g,'\n'))) !== sha(git(['show',EXPECTED_DRIVER_SOURCE_COMMIT+':mir2-web3/apps/web/scripts/playtest-capacity-soak.mjs']))) throw Error('Final patrol declaration is not bound to raw/Git source');
  const declaration = source.match(/Object\.defineProperty\(report, 'nativePatrolPlanning', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumFallbackCornerTargets: (\d+), pathExpansionLimit: (\d+),\s*selectionRule: '([^'\r\n]+)',\s*primingRule: '([^'\r\n]+)' \}\) \}\)/);
  const coordination = source.match(/Object\.defineProperty\(report, 'navigationCoordination', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumHintCellsPerConnection: NAVIGATION_HINT_MAX_CELLS, blockedOpportunityRule: '([^'\r\n]+)' \}\) \}\)/);
  const clock = source.match(/Object\.defineProperty\(report, 'measurementClock', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*minimumCompleteWindowMs: (\d+),\s*completeWindowCountRule: '([^'\r\n]+)',\s*durationRule: '([^'\r\n]+)',\s*partialWindowRule: '([^'\r\n]+)' \}\) \}\)/);
  if(!clock||JSON.stringify({scope:clock[1],minimumCompleteWindowMs:Number(clock[2]),completeWindowCountRule:clock[3],durationRule:clock[4],partialWindowRule:clock[5]})!==JSON.stringify(EXPECTED_MEASUREMENT_CLOCK))throw Error('Measurement clock declaration differs from the final pinned source');
  const maximumCells = source.match(/export const NAVIGATION_HINT_MAX_CELLS = (\d+);/);
  if (!declaration || !coordination || maximumCells?.[1] !== '7' ||
      JSON.stringify({scope:declaration[1],maximumFallbackCornerTargets:Number(declaration[2]),pathExpansionLimit:Number(declaration[3]),selectionRule:declaration[4],primingRule:declaration[5]}) !== JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING) ||
      JSON.stringify({scope:coordination[1],maximumHintCellsPerConnection:7,blockedOpportunityRule:coordination[2]}) !== JSON.stringify(EXPECTED_NAVIGATION_COORDINATION)) throw Error('Exact immutable report declarations do not match the final pinned source');
  const hp = source.match(/Object\.defineProperty\(report, 'nativeHpMaintenance', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*hpThreshold: (\.6), reuseDelayMs: NATIVE_HP_REUSE_MS, evidenceDeadlineMs: NATIVE_HP_EVIDENCE_MS,\s*commandBudgetPerSecond: (\d+), rule: '([^'\r\n]+)',\s*activityRule: '([^'\r\n]+)' \}\) \}\)/);
  const witness = source.match(/Object\.defineProperty\(report, 'nativeAttackWitness', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumOwnerAndTargetDistance: stableFanoutRange\('attack', policy\),\s*rule: '([^'\r\n]+)',\s*damageRule: '([^'\r\n]+)' \}\) \}\)/);
  if(!hp||!witness||!source.includes('const NATIVE_HP_REUSE_MS = 6000, NATIVE_HP_EVIDENCE_MS = 2000;')||!source.includes("if (policy?.activityMode === 'native') return kind === 'chat' ? 8 : 6;")||
    JSON.stringify({scope:hp[1],hpThreshold:Number(hp[2]),reuseDelayMs:6000,evidenceDeadlineMs:2000,commandBudgetPerSecond:Number(hp[3]),rule:hp[4],activityRule:hp[5]})!==JSON.stringify(EXPECTED_NATIVE_HP_MAINTENANCE)||
    JSON.stringify({scope:witness[1],maximumOwnerAndTargetDistance:6,rule:witness[2],damageRule:witness[3]})!==JSON.stringify(EXPECTED_NATIVE_ATTACK_WITNESS))throw Error('HP/witness immutable metadata differs from the exact final source');
  for(const [key,value] of [['nativeHpMaintenance',EXPECTED_NATIVE_HP_MAINTENANCE],['nativeAttackWitness',EXPECTED_NATIVE_ATTACK_WITNESS]])if(JSON.stringify(frozen[key])!==JSON.stringify(value)||JSON.stringify(report[key])!==JSON.stringify(value))throw Error('HP/witness frozen/report declaration differs: '+key);
  if(JSON.stringify(report.measurementClock)!==JSON.stringify(EXPECTED_MEASUREMENT_CLOCK))throw Error('Report measured-window declaration differs from the exact source');
  if (JSON.stringify(report.navigationCoordination) !== JSON.stringify(EXPECTED_NAVIGATION_COORDINATION) ||
      JSON.stringify(report.nativePatrolPlanning) !== JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING)) throw Error('Report declaration differs from the reviewed exact patrol driver source');
}
