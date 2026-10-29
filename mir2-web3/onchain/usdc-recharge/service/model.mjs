import { createHash } from 'node:crypto';

export const COIN_TYPE = '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC';
export const CHAIN_IDENTIFIER = '4c78adac';
export const GENESIS_DIGEST = '69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD';
export const NETWORK_HASH = '0x'+createHash('sha256').update(CHAIN_IDENTIFIER).digest('hex');
export const hex = b => '0x'+Buffer.from(b).toString('hex');
export const U64_MAX = (1n<<64n)-1n;
export function exactKeys(v, keys) {
  if(!v || typeof v!=='object' || Array.isArray(v) || Object.keys(v).sort().join()!==[...keys].sort().join()) throw Error('usdcFieldsInvalid');
}
export function address(v) {
  if(typeof v!=='string'||!/^0x[0-9a-f]{64}$/.test(v)||/^0x0{64}$/.test(v))throw Error('usdcAddressInvalid');return v;
}
export function hash32(v) { address(v);return v; }
export function integer(v) {
  if(typeof v!=='string'||!/^(0|[1-9][0-9]{0,19})$/.test(v)||BigInt(v)>U64_MAX)throw Error('usdcIntegerInvalid');return BigInt(v);
}
export function digest(v) {
  if(typeof v!=='string'||!/^[1-9A-HJ-NP-Za-km-z]{43,44}$/.test(v))throw Error('usdcDigestInvalid');return v;
}
export function canonicalType(v) {
  if(typeof v!=='string')throw Error('usdcCoinTypeInvalid');return v.startsWith('0x')?v:'0x'+v;
}
export function validateConfig(c) {
  exactKeys(c,['enabled','environmentTag','network','chainIdentifier','networkHash','fullnodeUrl','packageId','configId','adminCapId','issuerAddress','merchantAddress','coinType','gasBudgetMist']);
  if(c.enabled!==true||c.environmentTag!=='isolated-test'||c.network!=='testnet'||c.chainIdentifier!==CHAIN_IDENTIFIER||c.networkHash!==NETWORK_HASH
    ||!['https://fullnode.testnet.sui.io:443','https://fullnode.testnet.sui.io','https://fullnode.testnet.sui.io:443/','https://fullnode.testnet.sui.io/'].includes(c.fullnodeUrl)
    ||c.coinType!==COIN_TYPE)throw Error('usdcTestnetConfigurationInvalid');
  for(const k of ['packageId','configId','adminCapId','issuerAddress','merchantAddress'])address(c[k]);
  if(integer(c.gasBudgetMist)===0n||integer(c.gasBudgetMist)>50000000n)throw Error('usdcGasBudgetInvalid');return c;
}
export const ORDER_KEYS=['orderRef','accountRef','snapshotHash','payer','merchant','amountMicros','expiresAtMs'];
export function validateOrder(o,c) {
  exactKeys(o,ORDER_KEYS);for(const k of ['orderRef','accountRef','snapshotHash'])hash32(o[k]);
  address(o.payer);address(o.merchant);
  if(o.merchant!==c.merchantAddress||o.payer===o.merchant||integer(o.amountMicros)===0n||integer(o.expiresAtMs)===0n)throw Error('usdcOrderInvalid');return o;
}
// Deliberately independent of the procurement decoder: payer/package/event BCS
// must not be discarded when verifying a real payment.
export class Reader {
  constructor(b){this.b=Buffer.from(b);this.i=0;}
  take(n){if(!Number.isSafeInteger(n)||n<0||this.i+n>this.b.length)throw Error('usdcBcsTruncated');const r=this.b.subarray(this.i,this.i+n);this.i+=n;return r;}
  u8(){return this.take(1)[0];}
  bool(){const v=this.u8();if(v>1)throw Error('usdcBcsBooleanInvalid');return !!v;}
  u64(){return this.take(8).readBigUInt64LE().toString();}
  length(){let n=0,s=0;for(let i=0;i<5;i++){const b=this.u8();n+=(b&127)*2**s;if(!(b&128)){if(i&&b===0||n>4096)throw Error('usdcBcsLengthInvalid');return n;}s+=7;}throw Error('usdcBcsLengthInvalid');}
  vector(){return this.take(this.length());}
  hash(){const v=this.vector();if(v.length!==32)throw Error('usdcBcsHashInvalid');return hex(v);}
  address(){return hex(this.take(32));}
  string(){const b=this.vector();if([...b].some(x=>x>127))throw Error('usdcBcsAsciiInvalid');return b.toString('ascii');}
  done(){if(this.i!==this.b.length)throw Error('usdcBcsTrailing');}
}
function common(r) {
  return {configId:r.address(),networkHash:r.hash(),orderRef:r.hash(),accountRef:r.hash(),snapshotHash:r.hash(),
    payer:r.address(),merchant:r.address(),coinType:canonicalType(r.string()),amountMicros:r.u64(),expiresAtMs:r.u64()};
}
export function decodeInvoice(b) {const r=new Reader(b),o={invoiceId:r.address(),...common(r),paid:r.bool(),paidAtMs:r.u64()};r.done();return o;}
export function decodeConfig(b) {const r=new Reader(b),o={configId:r.address(),adminCapId:r.address(),issuerAddress:r.address(),networkHash:r.hash(),coinType:canonicalType(r.string()),merchantAddress:r.address(),publishingEnabled:r.bool()};r.done();return o;}
export function decodeEvent(b,kind) {
  const r=new Reader(b),o={invoiceId:r.address(),...common(r)};
  if(kind==='created'){o.issuer=r.address();o.createdAtMs=r.u64();}
  else if(kind==='paid')o.paidAtMs=r.u64();else throw Error('usdcEventKindInvalid');
  r.done();return o;
}
export function assertTerms(invoice,o,c) {
  for(const k of ORDER_KEYS)if(invoice[k]!==o[k])throw Error('usdcInvoiceTermsMismatch');
  if(invoice.configId!==c.configId||invoice.networkHash!==c.networkHash||invoice.coinType!==c.coinType)throw Error('usdcInvoiceDomainMismatch');
  if(invoice.paid===false&&invoice.paidAtMs!=='0'||invoice.paid===true&&(integer(invoice.paidAtMs)>=integer(invoice.expiresAtMs)))throw Error('usdcInvoiceStateInvalid');
  return invoice;
}
export function finalized(tx,expectedDigest) {
  if(!tx||tx.digest!==expectedDigest||typeof tx.effects?.status?.success!=='boolean'||tx.checkpoint==null)return false;
  digest(tx.digest);integer(String(tx.checkpoint));return true;
}
export function matchingEvent(tx,c,kind,o,invoiceId) {
  const expected=c.packageId+'::recharge::'+(kind==='created'?'InvoiceCreated':'PaymentReceived');
  const events=tx.events?.events??[];let found=[];
  for(let index=0;index<events.length;index++){
    const e=events[index];if(e.eventType!==expected)continue;
    const v=decodeEvent(e.contents?.value??[],kind);if(v.invoiceId!==invoiceId&&invoiceId!=null)continue;
    assertTerms(v,o,c);
    if(e.sender!==tx.transaction?.sender||e.sender!==(kind==='created'?c.issuerAddress:o.payer))throw Error('usdcEventSenderMismatch');
    // Package metadata is the top-level PTB caller, while eventType is the
    // defining module. Require direct calls to our frozen payment package.
    if(e.packageId!==c.packageId||e.module!=='recharge')throw Error('usdcEventPackageMismatch');
    if(kind==='created'&&v.issuer!==c.issuerAddress)throw Error('usdcEventIssuerMismatch');
    found.push({...v,eventSequence:String(index)});
  }
  if(found.length!==1)throw Error('usdcPaymentEventMissingOrAmbiguous');return found[0];
}
export function objectEffect(tx,c,id,created=false) {
  const hits=(tx.effects?.changedObjects??[]).filter(v=>v.objectId===id&&v.objectType===c.packageId+'::recharge::Invoice'
    &&v.outputState===2&&(!created||v.idOperation===2));
  if(hits.length!==1)throw Error('usdcInvoiceEffectMissing');return hits[0];
}
