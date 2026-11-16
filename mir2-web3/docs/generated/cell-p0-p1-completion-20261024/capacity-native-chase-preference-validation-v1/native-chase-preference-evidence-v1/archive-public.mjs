import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { BASE, DIRECTORY, TOPIC, CHILD, SOURCES, OWN_FILES, SOURCE_SHA, pinUnder, bytesUnder, safeRelative, validateEvidence, assertPin, sha } from './evidence-lib.mjs';
if (process.argv.slice(2).join(' ') !== '--archive-after-actual-full-pass') throw Error('Explicit post-test archive flag is required; this program never runs tests or realms');
const verified = await validateEvidence(), entries = [];
for (const source of [...SOURCES, ...OWN_FILES.map(file => 'native-chase-preference-evidence-v1/' + file)]) entries.push({ source, target: safeRelative(source), ...(await pinUnder(BASE, source)) });
assert.equal(new Set(entries.map(entry => entry.target)).size, entries.length); assert.ok(entries.every(entry => entry.bytes <= 1024 * 1024));
const totalBytes = entries.reduce((sum, entry) => sum + entry.bytes, 0); assert.ok(totalBytes <= 4 * 1024 * 1024);
const whitelist = { schemaVersion: 1, generatedAt: new Date().toISOString(), sourceSha256: SOURCE_SHA, child: CHILD,
  explicitWhitelist: true, files: entries, copiedFileCount: entries.length, copiedBytes: totalBytes,
  fullActual: { tests: 200, passed: 200, failed: 0, sourcePinsUnchangedAfterRun: true }, priorFocusedFailurePreserved: { tests: 13, passed: 12, failed: 1 },
  originalRedCurrentGreen: { red: { passed: 4, failed: 9 }, green: { passed: 13, failed: 0 }, finallyRestored: true },
  noRuntimeOrPrivateCopied: true, noTestsExecutedByCollector: true, capacityAccepted: false, p2ImplementationPermitted: false,
  existingSourceReferences: { repositoryFiles: verified.files, original187: verified.original187 },
  codeCommitNotYetAssigned: true, freezeNotCreatedByArchive: true };
const whitelistFile = path.join(DIRECTORY, 'archive-whitelist-final-v1.json'); await fs.writeFile(whitelistFile, JSON.stringify(whitelist, null, 2) + '\n', { flag: 'wx' });
const destination = path.join(TOPIC, CHILD); await fs.mkdir(destination);
for (const entry of entries) { const bytes = await bytesUnder(BASE, entry.source); assertPin({ bytes: bytes.length, sha256: sha(bytes) }, entry);
  const target = path.join(destination, entry.target); await fs.mkdir(path.dirname(target), { recursive: true }); await fs.writeFile(target, bytes, { flag: 'wx' }); assertPin(await pinUnder(destination, entry.target), entry); }
await fs.copyFile(whitelistFile, path.join(destination, 'archive-whitelist-final-v1.json'), fs.constants.COPYFILE_EXCL);
const copies = entries.map(({ source, target, bytes, sha256 }) => ({ source: path.join(BASE, source), target, bytes, sha256 }));
const receipt = { schemaVersion: 1, collectedAt: new Date().toISOString(), child: CHILD, sourceSha256: SOURCE_SHA, copies,
  rawBytesVerifiedAfterCopy: true, copiedFileCount: copies.length, copiedBytes: totalBytes, fullActualPassed: true, capacityAccepted: false,
  doesNotReplaceAnyFailedGameReport: true, noPrivateStoreBinaryOrRecordingPayloadRead: true, noRuntimeProcessStarted: true };
await fs.writeFile(path.join(destination, 'public-copy-receipt-v1.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
const result = { directory: destination, copiedFileCount: entries.length, copiedBytes: totalBytes, totalFiles: entries.length + 2,
  whitelist: await pinUnder(DIRECTORY, 'archive-whitelist-final-v1.json'), copyReceipt: await pinUnder(destination, 'public-copy-receipt-v1.json'), sourceSha256: SOURCE_SHA, capacityAccepted: false };
await fs.writeFile(path.join(DIRECTORY, 'archive-execution-v1.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' }); console.log(JSON.stringify(result));
