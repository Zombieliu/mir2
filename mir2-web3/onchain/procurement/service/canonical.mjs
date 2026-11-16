import { createHash, createPrivateKey, createPublicKey, sign, verify } from 'node:crypto';

export const INTENT_DOMAIN = 'MIR2_PROCUREMENT_INTENT_V1';
export const ORACLE_DOMAIN = 'MIR2_PROCUREMENT_ORACLE_V1';
export const ZERO = `0x${'00'.repeat(32)}`;
export const hex = b => `0x${Buffer.from(b).toString('hex')}`;
export const hash = b => createHash('sha256').update(b).digest();
export function bytes(value, length = 32) {
  if (typeof value !== 'string' || !new RegExp(`^0x[0-9a-f]{${length * 2}}$`).test(value)) throw Error(`Expected canonical ${length}-byte hex`);
  return Buffer.from(value.slice(2), 'hex');
}
export function u64(value) {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value)) throw Error('Expected canonical u64 decimal string');
  const n = BigInt(value);
  if (n > 18446744073709551615n) throw Error('u64 overflow');
  const b = Buffer.alloc(8); b.writeBigUInt64LE(n); return b;
}
function u32(value) {
  if (!Number.isInteger(value) || value < 0 || value > 4294967295) throw Error('Invalid u32');
  const b = Buffer.alloc(4); b.writeUInt32LE(value); return b;
}
function vector(b) {
  if (b.length >= 128) throw Error('v1 vectors must be shorter than 128 bytes');
  return Buffer.concat([Buffer.from([b.length]), b]);
}
function exactKeys(value, keys) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).sort().join() !== [...keys].sort().join()) throw Error('Unknown or missing signed fields');
}
const intentKeys = ['domain','network_hash','package_id','dapp_storage_id','operation','order_id','fill_id','buyer_ref','seller_ref','template_id','conditions_hash','quantity','price_gold','expires_at_ms','rule_version','nonce','valid_until_ms','actor_public_key'];
export function intentBytes(i) {
  exactKeys(i, intentKeys);
  if (i.domain !== INTENT_DOMAIN || ![1,2,3].includes(i.operation) || i.template_id !== 245 || i.quantity !== '1') throw Error('Unsupported procurement v1 intent');
  if (BigInt(i.price_gold) < 1n || BigInt(i.price_gold) > 4294967295n) throw Error('Invalid game gold amount');
  return Buffer.concat([vector(Buffer.from(i.domain)), vector(bytes(i.network_hash)), bytes(i.package_id), bytes(i.dapp_storage_id), Buffer.from([i.operation]),
    ...['order_id','fill_id','buyer_ref','seller_ref'].map(k => vector(bytes(i[k]))), u32(i.template_id), vector(bytes(i.conditions_hash)),
    ...['quantity','price_gold','expires_at_ms','rule_version'].map(k => u64(i[k])), vector(bytes(i.nonce)), u64(i.valid_until_ms), vector(bytes(i.actor_public_key))]);
}
export function proofBytes(p) {
  exactKeys(p, ['domain','intent_hash','asset_lock_id','asset_instance_hash','oracle_key_id','valid_until_ms','fence']);
  if (p.domain !== ORACLE_DOMAIN) throw Error('Invalid oracle domain');
  return Buffer.concat([vector(Buffer.from(p.domain)), ...['intent_hash','asset_lock_id','asset_instance_hash'].map(k => vector(bytes(p[k]))),
    ...['oracle_key_id','valid_until_ms','fence'].map(k => u64(p[k]))]);
}
export function revokeBytes(config, orderId, nextFence, validUntilMs, nonce) {
  return Buffer.concat([vector(Buffer.from('MIR2_PROCUREMENT_REVOKE_V1')), vector(bytes(config.networkHash)), bytes(config.packageId), bytes(config.dappStorageId),
    vector(bytes(orderId)), u64(nextFence), u64(validUntilMs), vector(bytes(nonce)), u64(config.oracleKeyId)]);
}
const privatePrefix = Buffer.from('302e020100300506032b657004220420','hex');
const publicPrefix = Buffer.from('302a300506032b6570032100','hex');
export function privateKey(seed) { return createPrivateKey({key:Buffer.concat([privatePrefix,bytes(seed)]),format:'der',type:'pkcs8'}); }
export function publicFromSeed(seed) { return hex(createPublicKey(privateKey(seed)).export({format:'der',type:'spki'}).subarray(-32)); }
export function signHash(body, seed) { return hex(sign(null,hash(body),privateKey(seed))); }
export function verifyHash(body, signature, publicKey) {
  return verify(null, hash(body), createPublicKey({key:Buffer.concat([publicPrefix,bytes(publicKey)]),format:'der',type:'spki'}),bytes(signature,64));
}
export function validateAttestation(i, p, actorSignature, oracleSignature, config, nowMs) {
  const ib = intentBytes(i), pb = proofBytes(p);
  if (i.network_hash !== config.networkHash || i.package_id !== config.packageId || i.dapp_storage_id !== config.dappStorageId
    || p.oracle_key_id !== config.oracleKeyId || p.intent_hash !== hex(hash(ib))) throw Error('Attestation domain mismatch');
  if (BigInt(nowMs) >= BigInt(i.valid_until_ms) || BigInt(nowMs) >= BigInt(p.valid_until_ms)
    || BigInt(p.valid_until_ms) > BigInt(i.valid_until_ms) || BigInt(i.valid_until_ms) > BigInt(i.expires_at_ms)) throw Error('Attestation expired or deadline invalid');
  if (!verifyHash(ib,actorSignature,i.actor_public_key) || !verifyHash(pb,oracleSignature,config.oraclePublicKey)) throw Error('Invalid actor or oracle signature');
  return { intentHash: hex(hash(ib)), proofHash: hex(hash(pb)) };
}
