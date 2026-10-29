// Official faucet v3 proof-of-work for the explicitly invoked isolated test runner.
// https://faucet.sui.io/pow-spec/ ; no wallet signature or private key is sent.
import { argon2d } from '@noble/hashes/argon2';
import { Worker, isMainThread, workerData, parentPort } from 'node:worker_threads';
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

export function powHash(challenge, nonce) {
  const input = ['sui-faucet-pow/1', challenge.chainId, challenge.checkpointSeq,
    challenge.checkpointDigest, challenge.randomBytes, challenge.faucetAddress, challenge.recipient, String(nonce)].join('\n');
  return Buffer.from(argon2d(new TextEncoder().encode(input), new TextEncoder().encode('sui-faucet-pow-1'),
    { t: 1, m: 8192, p: 1, dkLen: 32, version: 19 }));
}
if (!isMainThread) {
  const { challenge, index, count, until } = workerData;
  for (let n = BigInt(index); Date.now() < until; n += BigInt(count)) {
    const hash = powHash(challenge, n);
    if (hash.readBigUInt64BE() < BigInt(challenge.threshold)) {
      parentPort.postMessage({ recipient: challenge.recipient, checkpointSeq: challenge.checkpointSeq,
        nonce: String(n), hashHex: hash.toString('hex') }); break;
    }
  }
  parentPort.close();
}
function http(url, file) {
  // curl honors the host's configured HTTPS proxy. Keep TLS verification enabled;
  // Windows best-effort revocation only tolerates an unreachable revocation server.
  const args = ['--silent', '--show-error', '--connect-timeout', '10', '--max-time', '25'];
  if (process.platform === 'win32') args.push('--ssl-revoke-best-effort');
  if (file) args.push('--header', 'Content-Type: application/json', '--data-binary', '@' + file);
  args.push(url);
  return JSON.parse(execFileSync(process.platform === 'win32' ? 'curl.exe' : 'curl', args, { encoding: 'utf8', maxBuffer: 1024 * 1024 }));
}
function check(c, recipient, genesis) {
  if (c.version !== 1 || c.domain !== 'sui-faucet-pow/1' || c.algorithm !== 'argon2d' || c.argon2Version !== 19
    || c.memorySize !== 8192 || c.iterations !== 1 || c.parallelism !== 1 || c.hashLength !== 32 || c.salt !== 'sui-faucet-pow-1'
    || c.network !== 'testnet' || c.chainId !== genesis || c.recipient !== recipient
    || !/^0x[0-9a-f]{64}$/.test(c.faucetAddress) || !/^[0-9]+$/.test(c.checkpointSeq)
    || BigInt(c.difficulty) < 2n || BigInt(c.difficulty) > 2n ** 48n || BigInt(c.threshold) !== (2n ** 64n) / BigInt(c.difficulty)
    || BigInt(c.amountMist) > 2000000000n) throw Error('Faucet challenge/domain/parameters/budget mismatch');
}
async function solve(challenge) {
  const count = 16, workers = [], until = Date.now() + 48000;
  try {
    return await new Promise((resolve, reject) => {
      let exited = 0;
      for (let index = 0; index < count; index++) {
        const worker = new Worker(new URL(import.meta.url), { workerData: { challenge, index, count, until } });
        workers.push(worker); worker.on('message', resolve); worker.on('error', reject);
        worker.on('exit', () => { if (++exited === count) reject(Error('No faucet PoW before freshness deadline; no payout requested')); });
      }
    });
  } finally { await Promise.all(workers.map(w => w.terminate())); }
}
export async function requestIsolatedTestnetGas(recipient, genesis, evidence) {
  const proofFile = join(evidence, 'faucet-proof.json'), responseFile = join(evidence, 'faucet-response.json');
  if (existsSync(responseFile)) {
    const result = JSON.parse(readFileSync(responseFile));
    if (result.status === 'success' || result.digest) return result;
    throw Error('Existing faucet result requires explicit recovery; no second payout');
  }
  let proof;
  if (existsSync(proofFile)) proof = JSON.parse(readFileSync(proofFile));
  else {
    // A failed search never submitted a payout and may fetch a new challenge.
    const challenge = http('https://faucet.testnet.sui.io/v3/challenge?recipient=' + recipient);
    check(challenge, recipient, genesis);
    writeFileSync(join(evidence, 'faucet-challenge.json'), JSON.stringify(challenge, null, 2));
    proof = await solve(challenge); writeFileSync(proofFile, JSON.stringify(proof, null, 2));
  }
  if (proof.recipient !== recipient) throw Error('Saved faucet proof recipient mismatch');
  const result = http('https://faucet.testnet.sui.io/v3/gas', proofFile);
  writeFileSync(responseFile, JSON.stringify(result, null, 2));
  if (result.status !== 'success' && !result.digest) throw Error('Faucet refused proof; retained exact request/result for recovery');
  return result;
}
