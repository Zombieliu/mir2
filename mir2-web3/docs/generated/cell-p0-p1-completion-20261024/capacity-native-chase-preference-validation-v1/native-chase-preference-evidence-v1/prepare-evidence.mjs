import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { BASE, DIRECTORY, SOURCES, OWN_FILES, CHILD, pinUnder, SOURCE_SHA, validateEvidence, sha } from './evidence-lib.mjs';
assert.equal(process.argv.slice(2).join(' '), '--prepare-explicit-evidence');
const validated = await validateEvidence();
const files = await Promise.all([...SOURCES, ...OWN_FILES.map(name => 'native-chase-preference-evidence-v1/' + name)].map(file => pinUnder(BASE, file)));
const receipt = { schemaVersion: 1, preparedAt: new Date().toISOString(), publicChild: CHILD, state: 'Explicit source/evidence list prepared; not copied or driver-frozen by this step.',
  files, fileCount: files.length, totalBytes: files.reduce((sum, file) => sum + file.bytes, 0), aggregateSha256: sha(JSON.stringify(files)), sourceSha256: SOURCE_SHA,
  fullActual: { passed: 200, failed: 0, elapsedMs: validated.full.elapsedMs, sourcePinsUnchanged: true },
  focusedActual: { passed: 13, failed: 0 }, original187ByteIdentical: true, mainEightRulesInverseExact: true, testRevisionTwoRulesInverseExact: true,
  rootRedCurrentGreen: { originalPassed: 4, originalFailed: 9, currentPassed: 13, finallyRestored: true },
  firstDriverFreezeRequiresActualNewCodeCommit: true, originalFailedRecordsPreserved: true, noTestsRerun: true, noRealmStarted: true, capacityAccepted: false };
const target = path.join(DIRECTORY, 'explicit-evidence-prepared-v1.json'); await fs.writeFile(target, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ prepared: await pinUnder(DIRECTORY, path.basename(target)), files: receipt.fileCount, bytes: receipt.totalBytes, sourceSha256: SOURCE_SHA, capacityAccepted: false }));
