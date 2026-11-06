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
export const DIRECT_ORDER_KEYS=[...ORDER_KEYS,'createdAtMs'];
export function validateDirectOrder(o,c) {
  exactKeys(o,DIRECT_ORDER_KEYS);
  validateOrder(Object.fromEntries(ORDER_KEYS.map(k=>[k,o[k]])),c);
  if(integer(o.createdAtMs)===0n||integer(o.createdAtMs)>=integer(o.expiresAtMs))throw Error('usdcDirectOrderInvalid');
  return o;
}
export function protobufTimestampMs(timestamp) {
  if(!timestamp||typeof timestamp.seconds!=='bigint'||!Number.isInteger(timestamp.nanos)||timestamp.nanos<0||timestamp.nanos>=1000000000||timestamp.nanos%1000000!==0)throw Error('usdcDirectTimestampUnavailable');
  const ms=timestamp.seconds*1000n+BigInt(timestamp.nanos/1000000);
  return integer(ms.toString()).toString();
}
const normalizeCoinType=v=>canonicalType(v).replace(/0x0{1,63}2::/g,'0x2::');
const SUI_COIN='0x2::coin::Coin<0x2::sui::SUI>';
const ownerAddress=o=>{
  if(!o||o.kind==null||o.kind===0||o.address==null)throw Error('usdcDirectObjectsUnavailable');
  if(o?.kind!==1)throw Error('usdcDirectObjectOwnerInvalid');return address(o.address);
};
function coinObject(objects,effect,side,type) {
  const version=effect[side+'Version'],expectedDigest=effect[side+'Digest'];
  if(typeof version!=='bigint'||version<=0n||typeof expectedDigest!=='string')throw Error('usdcDirectObjectsUnavailable');
  const matches=objects.filter(o=>o.objectId===effect.objectId&&o.version===version);
  if(matches.length!==1)throw Error('usdcDirectObjectsUnavailable');
  const object=matches[0];
  if(object.digest!==expectedDigest||normalizeCoinType(object.objectType)!==type||ownerAddress(object.owner)!==ownerAddress(effect[side+'Owner']))throw Error('usdcDirectObjectMismatch');
  if(!(object.contents?.value instanceof Uint8Array)||object.contents.value.length===0)throw Error('usdcDirectObjectsUnavailable');
  const reader=new Reader(object.contents.value);
  if(reader.address()!==effect.objectId)throw Error('usdcDirectObjectMismatch');
  const balance=integer(reader.u64());reader.done();
  if(object.balance!=null&&String(object.balance)!==balance.toString())throw Error('usdcDirectObjectMismatch');
  return {owner:ownerAddress(object.owner),balance,object};
}
function pureInput(data,argument,length) {
  if(argument?.$kind!=='Input'||!Number.isInteger(argument.Input))throw Error('usdcDirectCommandInvalid');
  const bytes=Buffer.from(data.inputs[argument.Input]?.Pure?.bytes??'','base64');
  if(bytes.length!==length)throw Error('usdcDirectCommandInvalid');return bytes;
}
function assertSuiAccumulator(effect,tx,data,o) {
  const write=effect.accumulatorWrite;
  if(effect.outputState!==4||normalizeCoinType(write?.accumulatorType)!=='0x2::balance::Balance<0x2::sui::SUI>')return false;
  if(data.gasData.payment?.length!==0||data.expiration?.ValidDuring?.chain!==GENESIS_DIGEST||address(write.address)!==o.payer||![1,2].includes(write.operation))throw Error('usdcDirectGasMismatch');
  const gas=tx.effects.gasUsed;
  if(!gas||gas.computationCost==null||gas.storageCost==null||gas.storageRebate==null||write.value==null)throw Error('usdcDirectObjectsUnavailable');
  const fee=integer(String(gas.computationCost))+integer(String(gas.storageCost))-integer(String(gas.storageRebate));
  const delta=integer(String(write.value))*(write.operation===1?1n:-1n);
  if(delta!==-fee)throw Error('usdcDirectGasMismatch');return true;
}
// A bounded reference analysis for native CoinWithBalance payments. Results
// may come only from these framework operations, never an application Move call.
function analyzeDirectCommands(data,o,c) {
  const inputs=new Map(),results=new Map(),deleted=new Set(),funding=new Set();
  let merchantTransfer=false,usesAccumulator=false,withdrawalMaximum=0n;
  for(const command of data.commands){
    if(command.$kind==='MoveCall'&&(!/^0x0*2$/.test(command.MoveCall?.package)||!['coin','balance'].includes(command.MoveCall.module)))throw Error('usdcDirectCommandInvalid');
  }
  function coin(argument,index,kind='coin') {
    let token;
    if(argument?.$kind==='Input'){
      const input=data.inputs[argument.Input],ref=input?.Object?.ImmOrOwnedObject;
      if(!ref)throw Error('usdcDirectUnsupported');
      if(!inputs.has(argument.Input))inputs.set(argument.Input,{kind:'coin',objectId:address(ref.objectId),spent:false});
      token=inputs.get(argument.Input);
    }else{
      const result=argument?.$kind==='Result'?argument.Result:argument?.$kind==='NestedResult'?argument.NestedResult?.[0]:null;
      const sub=argument?.$kind==='NestedResult'?argument.NestedResult?.[1]:0;
      if(!Number.isInteger(result)||result<0||result>=index||!Number.isInteger(sub)||sub<0)throw Error('usdcDirectUnsupported');
      const values=results.get(result);
      if(!values||argument.$kind==='Result'&&values.length!==1)throw Error('usdcDirectUnsupported');
      token=values[sub];
    }
    if(!token||token.kind!==kind||token.spent)throw Error('usdcDirectCommandInvalid');return token;
  }
  function consume(token,removeObject){
    token.spent=true;
    if(removeObject&&token.objectId){if(deleted.has(token.objectId))throw Error('usdcDirectCommandInvalid');deleted.add(token.objectId);}
  }
  function recipient(argument){
    const value=hex(pureInput(data,argument,32));
    if(value!==o.payer&&value!==o.merchant)throw Error('usdcDirectRecipientInvalid');
    merchantTransfer ||= value===o.merchant;return value;
  }
  for(let index=0;index<data.commands.length;index++){
    const command=data.commands[index];
    if(command.$kind==='SplitCoins'){
      coin(command.SplitCoins.coin,index);
      const amounts=command.SplitCoins.amounts;
      if(!Array.isArray(amounts)||!amounts.length||amounts.length>64)throw Error('usdcDirectCommandInvalid');
      for(const amount of amounts)pureInput(data,amount,8);
      results.set(index,amounts.map(()=>({kind:'coin',spent:false})));continue;
    }
    if(command.$kind==='MergeCoins'){
      const destination=coin(command.MergeCoins.destination,index),sources=command.MergeCoins.sources;
      if(!Array.isArray(sources)||!sources.length||sources.length>64)throw Error('usdcDirectCommandInvalid');
      for(const source of sources){const token=coin(source,index);if(token===destination)throw Error('usdcDirectCommandInvalid');consume(token,true);}
      continue;
    }
    if(command.$kind==='TransferObjects'){
      recipient(command.TransferObjects.address);
      const objects=command.TransferObjects.objects;
      if(!Array.isArray(objects)||!objects.length||objects.length>64)throw Error('usdcDirectCommandInvalid');
      for(const object of objects)consume(coin(object,index),false);
      continue;
    }
    if(command.$kind!=='MoveCall')throw Error('usdcDirectCommandInvalid');
    const call=command.MoveCall;
    // All other packages/modules (especially recharge::pay) are ineligible even
    // when their balance changes happen to resemble a direct payment.
    if(!['redeem_funds','from_balance','send_funds','destroy_zero'].includes(call.function)){
      if(call.typeArguments?.includes(c.coinType))throw Error('usdcDirectUnsupported');
      throw Error('usdcDirectCommandInvalid');
    }
    if(call.module==='balance'&&call.function!=='redeem_funds')throw Error('usdcDirectCommandInvalid');
    if(call.typeArguments?.length!==1||call.typeArguments[0]!==c.coinType||!Array.isArray(call.arguments))throw Error('usdcDirectCommandInvalid');
    if(call.function==='redeem_funds'){
      if(call.arguments.length!==1||call.arguments[0].$kind!=='Input')throw Error('usdcDirectCommandInvalid');
      const input=call.arguments[0].Input,withdrawal=data.inputs[input]?.FundsWithdrawal;
      if(!withdrawal||funding.has(input))throw Error('usdcDirectCommandInvalid');
      exactKeys(withdrawal,['reservation','typeArg','withdrawFrom']);
      exactKeys(withdrawal.reservation,['$kind','MaxAmountU64']);exactKeys(withdrawal.typeArg,['$kind','Balance']);exactKeys(withdrawal.withdrawFrom,['$kind','Sender']);
      if(withdrawal.reservation.$kind!=='MaxAmountU64'||integer(withdrawal.reservation.MaxAmountU64)===0n||
        withdrawal.typeArg.$kind!=='Balance'||withdrawal.typeArg.Balance!==c.coinType||withdrawal.withdrawFrom.$kind!=='Sender'||withdrawal.withdrawFrom.Sender!==true)throw Error('usdcDirectCommandInvalid');
      withdrawalMaximum+=integer(withdrawal.reservation.MaxAmountU64);
      if(withdrawalMaximum>U64_MAX)throw Error('usdcDirectCommandInvalid');
      usesAccumulator=true;funding.add(input);results.set(index,[{kind:call.module==='coin'?'coin':'balance',spent:false}]);continue;
    }
    if(call.function==='from_balance'){
      if(call.module!=='coin'||call.arguments.length!==1)throw Error('usdcDirectCommandInvalid');
      consume(coin(call.arguments[0],index,'balance'),false);results.set(index,[{kind:'coin',spent:false}]);continue;
    }
    if(call.function==='send_funds'){
      if(call.module!=='coin'||call.arguments.length!==2)throw Error('usdcDirectCommandInvalid');
      usesAccumulator=true;recipient(call.arguments[1]);consume(coin(call.arguments[0],index),true);continue;
    }
    if(call.module!=='coin'||call.arguments.length!==1)throw Error('usdcDirectCommandInvalid');
    consume(coin(call.arguments[0],index),true);
  }
  if(!merchantTransfer)throw Error('usdcDirectRecipientInvalid');
  for(let index=0;index<data.inputs.length;index++)if(data.inputs[index].$kind==='FundsWithdrawal'&&!funding.has(index))throw Error('usdcDirectUnsupported');
  return {deleted,usesAccumulator,withdrawalMaximum};
}
// Finalized Coin BCS deltas plus hash-bound accumulator writes independently
// corroborate balance_changes. A mere browser/indexer balance is insufficient.
export function assertDirectTransfer(tx,data,o,c) {
  if(data.sender!==o.payer||data.gasData?.owner!==o.payer)throw Error('usdcDirectPayerMismatch');
  if(!Array.isArray(data.inputs)||data.inputs.length>64||!Array.isArray(data.commands)||!data.commands.length||data.commands.length>32)throw Error('usdcDirectCommandInvalid');
  if((tx.events?.events?.length??0)!==0||tx.effects?.eventsDigest)throw Error('usdcDirectEventsRejected');
  const commands=analyzeDirectCommands(data,o,c),objects=tx.objects?.objects??[],effects=tx.effects?.changedObjects;
  if(!Array.isArray(objects)||objects.length>256||!Array.isArray(effects)||!effects.length||effects.length>128)throw Error('usdcDirectObjectsUnavailable');
  const seenObjects=new Set();
  for(const object of objects){const key=object.objectId+':'+String(object.version);if(seenObjects.has(key))throw Error('usdcDirectObjectsUnavailable');seenObjects.add(key);}
  const usdcCoin='0x2::coin::Coin<'+c.coinType+'>',seenEffects=new Set(),inputs=new Map(),deleted=new Set(),gasRefs=new Set((data.gasData.payment??[]).map(ref=>ref.objectId));
  let payer=0n,merchant=0n,seenUsdc=false,suiWrite=false,payerAccumulator=0n;
  for(const effect of effects){
    address(effect.objectId);if(seenEffects.has(effect.objectId))throw Error('usdcDirectObjectMismatch');seenEffects.add(effect.objectId);
    if(effect.outputState===4){
      const write=effect.accumulatorWrite;
      if(!write||write.value==null)throw Error('usdcDirectObjectsUnavailable');
      const type=normalizeCoinType(write.accumulatorType);
      if(type==='0x2::balance::Balance<'+c.coinType+'>'){
        if(!commands.usesAccumulator)throw Error('usdcDirectCommandInvalid');
        const owner=address(write.address);
        if(![1,2].includes(write.operation)||owner!==o.payer&&owner!==o.merchant)throw Error('usdcDirectRecipientInvalid');
        const delta=integer(String(write.value))*(write.operation===1?1n:-1n);
        if(owner===o.payer){payer+=delta;payerAccumulator+=delta;}else{if(delta<0n)throw Error('usdcDirectRecipientInvalid');merchant+=delta;}
        seenUsdc=true;continue;
      }
      if(assertSuiAccumulator(effect,tx,data,o)){if(suiWrite)throw Error('usdcDirectGasMismatch');suiWrite=true;continue;}
      throw Error('usdcDirectAssetMismatch');
    }
    const type=normalizeCoinType(effect.objectType);
    if(type!==usdcCoin&&type!==SUI_COIN)throw Error('usdcDirectAssetMismatch');
    if(effect.inputState!==1&&effect.inputState!==2)throw Error('usdcDirectObjectsUnavailable');
    const expectedDeletion=commands.deleted.has(effect.objectId),gasDeletion=type===SUI_COIN&&gasRefs.has(effect.objectId)&&effect.objectId!==data.gasData.payment[0]?.objectId;
    const removed=effect.outputState===1&&effect.inputState===2&&effect.idOperation===3&&(expectedDeletion||gasDeletion);
    if(effect.outputState!==2&&!removed)throw Error('usdcDirectObjectsUnavailable');
    if(expectedDeletion&&!removed)throw Error('usdcDirectObjectMismatch');
    if(expectedDeletion)deleted.add(effect.objectId);
    const before=effect.inputState===2?coinObject(objects,effect,'input',type):null;
    const after=removed?null:coinObject(objects,effect,'output',type);
    if(before){if(before.owner!==o.payer)throw Error('usdcDirectPayerMismatch');inputs.set(effect.objectId,before.object);}
    if(type===SUI_COIN){if(after&&after.owner!==o.payer)throw Error('usdcDirectAssetMismatch');continue;}
    seenUsdc=true;
    if(after&&after.owner!==o.payer&&after.owner!==o.merchant)throw Error('usdcDirectRecipientInvalid');
    payer-=before?.balance??0n;
    if(after?.owner===o.payer)payer+=after.balance;else if(after)merchant+=after.balance;
  }
  if(deleted.size!==commands.deleted.size)throw Error('usdcDirectObjectsUnavailable');
  if(payerAccumulator < -commands.withdrawalMaximum)throw Error('usdcPaymentBalanceMismatch');
  const referenced=new Set();
  for(const input of data.inputs){
    if(input.$kind==='Pure'||input.$kind==='FundsWithdrawal')continue;
    const ref=input.Object?.ImmOrOwnedObject,object=ref&&inputs.get(ref.objectId);
    if(!ref)throw Error('usdcDirectUnsupported');if(!object)throw Error('usdcDirectObjectsUnavailable');
    if(String(object.version)!==ref.version||object.digest!==ref.digest||normalizeCoinType(object.objectType)!==usdcCoin||referenced.has(ref.objectId))throw Error('usdcDirectInputAssetMismatch');referenced.add(ref.objectId);
  }
  for(const ref of data.gasData.payment??[]){
    const object=inputs.get(ref.objectId);if(!object)throw Error('usdcDirectObjectsUnavailable');
    if(String(object.version)!==ref.version||object.digest!==ref.digest||normalizeCoinType(object.objectType)!==SUI_COIN||referenced.has(ref.objectId))throw Error('usdcDirectGasMismatch');referenced.add(ref.objectId);
  }
  if(inputs.size!==referenced.size||!data.gasData.payment.length&&!suiWrite)throw Error('usdcDirectGasMismatch');
  if(!seenUsdc||payer!==-integer(o.amountMicros)||merchant!==integer(o.amountMicros))throw Error('usdcPaymentBalanceMismatch');
  assertPaymentBalanceChanges(tx,o,c);
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


// Ledger balance changes independently corroborate the exact native-coin
// transfer. Missing index data is pending, never permission to mint credits.
export function assertPaymentBalanceChanges(tx,o,c) {
  if(!Array.isArray(tx.balanceChanges)||!tx.balanceChanges.length)throw Error('usdcBalanceChangesUnavailable');
  let payer=0n,merchant=0n,seenPayer=false,seenMerchant=false;
  for(const change of tx.balanceChanges){
    if(canonicalType(change.coinType)!==c.coinType)continue;
    address(change.address);
    if(typeof change.amount!=='string'||! /^-?(0|[1-9][0-9]{0,19})$/.test(change.amount)||change.amount==='-0')throw Error('usdcBalanceChangeInvalid');
    const amount=BigInt(change.amount);if(amount>U64_MAX||amount< -U64_MAX)throw Error('usdcBalanceChangeInvalid');
    if(change.address===o.payer){payer+=amount;seenPayer=true;}
    else if(change.address===o.merchant){merchant+=amount;seenMerchant=true;}
    else if(amount!==0n)throw Error('usdcPaymentBalanceMismatch');
  }
  if(!seenPayer||!seenMerchant||payer!==-integer(o.amountMicros)||merchant!==integer(o.amountMicros))throw Error('usdcPaymentBalanceMismatch');
}
