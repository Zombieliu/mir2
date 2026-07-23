import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

async function readJson(relativeUrl) {
  return JSON.parse(await readFile(new URL(relativeUrl, import.meta.url), "utf8"));
}

const [snapshot, deployment, registration, acceptance] = await Promise.all([
  readJson("../data/dubhe-node-testnet.json"),
  readJson("../../../onchain/deployments/obelisk-node-registry-testnet.json"),
  readJson("../../../docs/generated/gate13/testnet/active-registration.json"),
  readJson("../../../docs/generated/gate13/docker/gate13-acceptance.json")
]);

assert.deepEqual(snapshot.deployment, {
  network: deployment.network,
  packageId: deployment.packageId,
  registryId: deployment.registryId,
  publishTransaction: deployment.publish.transactionDigest,
  registeredNodeCount: deployment.verification.registryNodeCount,
  retiredNodeCount: deployment.verification.registryRetiredCount
});

assert.deepEqual(snapshot.registration, {
  nodeId: registration.nodeId,
  operatorSuiAddress: registration.operatorSuiAddress,
  publicKey: registration.publicKey,
  endpoint: registration.endpoint,
  failureDomain: registration.failureDomain,
  stakeMist: registration.stakeMist,
  maxSessions: registration.maxSessions,
  maxZones: registration.maxZones,
  keyGeneration: registration.keyGeneration,
  status: registration.status,
  transactionDigest: registration.finality.transactionDigest,
  checkpoint: registration.finality.checkpoint
});

assert.deepEqual(snapshot.acceptance, {
  generatedAtMs: acceptance.generatedAtMs,
  capacityCompletedCommands: acceptance.capacityCompletedCommands,
  capacityMaxSessionsPerZone: acceptance.capacityMaxSessionsPerZone,
  capacityP95LatencyMs: acceptance.capacityP95LatencyMs,
  capacityCertificateId: acceptance.capacityCertificateId,
  capacityCertificateIssuer: acceptance.capacityCertificateIssuer,
  capacityCertificateExpiresAtMs: acceptance.capacityCertificateExpiresAtMs,
  commonwareQuorum: acceptance.commonwareQuorum,
  commonwareFinalizedHeight: acceptance.commonwareFinalizedHeight,
  membershipEligible: acceptance.membershipEligible,
  rewardBatchId: acceptance.rewardBatchId,
  rewardMerkleRoot: acceptance.rewardMerkleRoot,
  rewardTotal: acceptance.rewardTotal
});

console.log("Dubhe Node testnet snapshot matches the authoritative Gate 13 evidence.");
