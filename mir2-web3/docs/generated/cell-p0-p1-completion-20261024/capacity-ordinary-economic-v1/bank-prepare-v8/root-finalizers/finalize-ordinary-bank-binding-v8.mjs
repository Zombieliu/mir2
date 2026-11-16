import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [directory, repository, head, nodeReceiptFile, expectedNodeSha, expectedBindingSha, expectedConfigSha] = process.argv.slice(2);
if (!path.isAbsolute(directory ?? '') || !path.isAbsolute(repository ?? '') || !path.isAbsolute(nodeReceiptFile ?? '') ||
    !/^[a-f0-9]{40}$/.test(head ?? '') || ![expectedNodeSha, expectedBindingSha, expectedConfigSha].every(x => /^[a-f0-9]{64}$/.test(x ?? ''))) {
  throw Error('Absolute package/repository/Root receipt, actual HEAD, and three reviewed SHA256 values required');
}
const {pin, read, backendBinding, validateFocusedReceipt, assert} = await import(pathToFileURL(path.join(directory, 'source-lib-v1.mjs')));
const nodePin = pin(nodeReceiptFile);
assert(nodePin.sha256 === expectedNodeSha, 'Root qualification receipt changed');
const focused = validateFocusedReceipt(nodePin, repository, head);
const bindingFile = path.join(directory, 'backend-release-binding-pending-v1.json');
const configFile = path.join(directory, 'prepare-config-pending-v1.json');
const before = {binding: pin(bindingFile), config: pin(configFile)};
assert(before.binding.sha256 === expectedBindingSha && before.config.sha256 === expectedConfigSha, 'Reviewed pending binding/config changed');
const binding = read(before.binding), config = read(before.config);
assert(binding.finalized !== true && binding.executionSourceCommit == null && binding.currentJsSourceCommit == null,
  'Only an unfinalized package with no claimed current JS qualification may be finalized');
assert(config.sourceQualification.driverSourceCommit == null && config.sourceQualification.executionSourceCommit == null,
  'Pending config already claims a current JS source');
const backend = backendBinding(repository, head);
assert(backend.sourceCount === 55 && backend.reusedRustTestsPassed === 32 && backend.rawAndGitObjectsExact === true &&
  backend.newRustBuildPerformed === false, 'Actual original V6 complete-source reuse failed');
binding.state = 'FINAL_REUSED_V6_RELEASE_AND_CURRENT_JS_BOUND';
binding.finalized = true;
binding.executionSourceCommit = head;
binding.currentJsSourceCommit = head;
binding.newRootJsReceipt = nodePin;
config.sourceQualification = {...config.sourceQualification, state:'ACTUAL_CURRENT_HELPER_JS_QUALIFIED_WITH_EXACT_REUSED_V6_BACKEND',
  driverSourceCommit:head, executionSourceCommit:head, newRootJsReceipt:nodePin};
fs.copyFileSync(bindingFile, path.join(directory, 'backend-release-binding-before-root-final-v1.json'), fs.constants.COPYFILE_EXCL);
fs.copyFileSync(configFile, path.join(directory, 'prepare-config-before-root-final-v1.json'), fs.constants.COPYFILE_EXCL);
fs.writeFileSync(bindingFile, JSON.stringify(binding, null, 2) + '\n');
fs.writeFileSync(configFile, JSON.stringify(config, null, 2) + '\n');
const receipt = {schemaVersion:1, expectedHead:head,
  scope:'Current helper JS fresh selected qualification; original V6 backend/Rust32 reused after55 raw/GitLF/object/closure equality. No new Rust build/tests or capacity acceptance.',
  before, after:{binding:pin(bindingFile), config:pin(configFile)}, node:nodePin, actualFreshNodeTestsPassed:focused.actualTestsPassed,
  reusedBackendCommit:backend.backendSourceCommit, reusedRelease:backend.releasePin, reusedRustReceipt:backend.reusedRustReceipt,
  reusedRustTestsPassed:32, backendSourcesExact:55, newRustBuildPerformed:false, freshRustTestsPerformed:false, capacityAccepted:false, p0Accepted:false};
const receiptFile = path.join(directory, 'root-final-binding-receipt-v1.json');
fs.writeFileSync(receiptFile, JSON.stringify(receipt, null, 2) + '\n', {flag:'wx'});
console.log(JSON.stringify({receipt:pin(receiptFile), config:receipt.after.config, node:nodePin, actualFreshNodeTestsPassed:focused.actualTestsPassed}));
