import fs from 'node:fs/promises';
import path from 'node:path';
import { root, project, driverFrozenFile, read, sha, git } from './input-lib.mjs';

// Finalize only after the new nine-input freeze and clean committed driver exist.
export const EXPECTED_DRIVER_SOURCE_COMMIT = '0998105c30f35502b8b569b82fb7942a5d356c89';
export const EXPECTED_DRIVER_SOURCE_SHA256 = '8c29818d96fc7d0974d248fe7b4f140812385216cc98b04a0712fecfe25385b7';
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
export async function verifyDriverReportContract(report) {
  if (!/^[a-f0-9]{40}$/.test(EXPECTED_DRIVER_SOURCE_COMMIT) || !/^[a-f0-9]{64}$/.test(EXPECTED_DRIVER_SOURCE_SHA256) ||
      !EXPECTED_NATIVE_PATROL_PLANNING || typeof EXPECTED_NATIVE_PATROL_PLANNING.scope !== 'string') throw Error('Patrol driver template has not been finalized after source freeze/commit');
  const frozen = await read(driverFrozenFile);
  if (frozen.files?.length !== 9 || frozen.sourceSha256 !== EXPECTED_DRIVER_SOURCE_SHA256 || sha(JSON.stringify(frozen.files)) !== frozen.sourceSha256 ||
      JSON.stringify(frozen.nativePatrolPlanning) !== JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING) ||
      git(['rev-parse','HEAD']).toString('utf8').trim() !== EXPECTED_DRIVER_SOURCE_COMMIT) throw Error('Exact final patrol driver source identity differs');
  const file = path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs'), source = await fs.readFile(file, 'utf8');
  const sourcePin = frozen.files.find(row => row.file === 'apps/web/scripts/playtest-capacity-soak.mjs');
  if (!sourcePin || Buffer.byteLength(source) !== sourcePin.bytes || sha(Buffer.from(source)) !== sourcePin.sha256 ||
      sha(Buffer.from(source.replace(/\r\n/g,'\n'))) !== sha(git(['show',EXPECTED_DRIVER_SOURCE_COMMIT+':mir2-web3/apps/web/scripts/playtest-capacity-soak.mjs']))) throw Error('Final patrol declaration is not bound to raw/Git source');
  const declaration = source.match(/Object\.defineProperty\(report, 'nativePatrolPlanning', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumFallbackCornerTargets: (\d+), pathExpansionLimit: (\d+),\s*selectionRule: '([^'\r\n]+)',\s*primingRule: '([^'\r\n]+)' \}\) \}\)/);
  const coordination = source.match(/Object\.defineProperty\(report, 'navigationCoordination', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumHintCellsPerConnection: NAVIGATION_HINT_MAX_CELLS, blockedOpportunityRule: '([^'\r\n]+)' \}\) \}\)/);
  const maximumCells = source.match(/export const NAVIGATION_HINT_MAX_CELLS = (\d+);/);
  if (!declaration || !coordination || maximumCells?.[1] !== '7' ||
      JSON.stringify({scope:declaration[1],maximumFallbackCornerTargets:Number(declaration[2]),pathExpansionLimit:Number(declaration[3]),selectionRule:declaration[4],primingRule:declaration[5]}) !== JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING) ||
      JSON.stringify({scope:coordination[1],maximumHintCellsPerConnection:7,blockedOpportunityRule:coordination[2]}) !== JSON.stringify(EXPECTED_NAVIGATION_COORDINATION)) throw Error('Exact immutable report declarations do not match the final pinned source');
  if (JSON.stringify(report.navigationCoordination) !== JSON.stringify(EXPECTED_NAVIGATION_COORDINATION) ||
      JSON.stringify(report.nativePatrolPlanning) !== JSON.stringify(EXPECTED_NATIVE_PATROL_PLANNING)) throw Error('Report declaration differs from the reviewed exact patrol driver source');
}
