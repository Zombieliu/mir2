import type { CrystalTooltipItem, CrystalTooltipDocument } from "./shared-item-tooltip";
import manifest from "./generated/client_core_runtime.json";
import type { RegistrationDraft, ChangePasswordDraft } from "./client-login-runtime";
import { createSharedChatUi, searchSharedMapRoute, readSharedCashPreview, turnSharedCashPreview, readSharedNpcRepairQuote, readSharedPresentationItemTooltip,
  type PresentationWasmModule, type MapRouteWasmModule, type ChatUiRuntime, type MapRouteInput, type MapRouteDecision,
  type CashPreviewInput, type CashPreviewLayerDocument, type NpcRepairQuoteInput, type NpcRepairQuote } from "./client-presentation-runtime";
export type { ChatUiDocument, ChatUiControls, ChatUiRuntime, MapRouteInput, MapRouteDecision,
  CashPreviewInput, CashPreviewLayerDocument, NpcRepairQuoteInput, NpcRepairQuote } from "./client-presentation-runtime";

export type AuthValidation = { ok: true; birthDateBinary?: string } |
  { ok: false; code: number; field: string; error: string };
export type AuthUiRuntime = {
  validateRegistration(fields: RegistrationDraft): AuthValidation;
  validatePassword(fields: ChangePasswordDraft): AuthValidation;
  keys(): string; reshuffle(): string;
  edit(value: string, key: string, account: boolean, deleting: boolean): string;
  dispose(): void;
};

// These are UI labels for the shared Rust error codes, not local validators.
const authErrors: ReadonlyArray<readonly [string, string]> = [
  ["", ""], ["accountId", "account ID must be 3-15 alphanumeric characters"],
  ["password", "password must be 5-15 alphanumeric characters"],
  ["confirmPassword", "password confirmation does not match"],
  ["confirmPassword", "password confirmation must be 5-15 alphanumeric characters"],
  ["userName", "user name must be at most 20 characters"],
  ["secretQuestion", "secret question must be at most 30 characters"],
  ["secretAnswer", "secret answer must be at most 30 characters"],
  ["emailAddress", "email address is not acceptable"],
  ["birthDate", "birth date must use YYYY-MM-DD"],
  ["oldPassword", "current password must be 5-15 alphanumeric characters"],
  ["newPassword", "new password must be 5-15 alphanumeric characters"],
  ["confirmPassword", "new password confirmation does not match"],
  ["confirmPassword", "new password confirmation must be 5-15 alphanumeric characters"],
];
function authValidationError(code: number): AuthValidation {
  if (!Number.isInteger(code) || code < 1 || code >= authErrors.length) {
    throw Error("Invalid shared authentication response");
  }
  return { ok: false, code, field: authErrors[code][0], error: authErrors[code][1] };
}
function createAuthUiRuntime(module: WasmModule, seed: bigint): AuthUiRuntime {
  if (module.auth_ui_abi_version?.() !== 1 || !module.AuthUiBridge
    || !module.auth_ui_validate_registration || !module.auth_ui_validate_password) {
    throw Error("Shared account controls are unavailable; reload the client");
  }
  const bridge = new module.AuthUiBridge(seed);
  let disposed = false;
  function live() { if (disposed) throw Error("Account controls were retired"); }
  function fields(values: string[]) {
    live();
    if (!values.every(value => slotUtf8(value, 4096))) throw Error("Invalid account field encoding");
  }
  function keys(value: unknown): string {
    if (typeof value !== "string" || value.length !== 36 || new Set(value).size !== 36
      || !/^[A-Z0-9]+$/.test(value)) throw Error("Invalid shared keyboard response");
    return value;
  }
  return {
    validateRegistration(f) {
      const values = [f.accountId, f.password, f.confirmPassword, f.userName,
        f.birthDate, f.secretQuestion, f.secretAnswer, f.emailAddress];
      fields(values);
      const result = module.auth_ui_validate_registration!(...values as [string,string,string,string,string,string,string,string]);
      if (/^e(?:[1-9]|1[0-3])$/.test(result)) return authValidationError(Number(result.slice(1)));
      if (!/^(?:0|[1-9][0-9]{0,18})$/.test(result) || BigInt(result) > 3155378975999999999n) {
        throw Error("Invalid shared birth date response");
      }
      return { ok: true, birthDateBinary: result };
    },
    validatePassword(f) {
      fields([f.accountId, f.oldPassword, f.newPassword, f.confirmPassword]);
      const code = module.auth_ui_validate_password!(f.accountId, f.oldPassword, f.newPassword, f.confirmPassword);
      return code === 0 ? { ok: true } : authValidationError(code);
    },
    keys() { live(); return keys(bridge.keys()); },
    reshuffle() { live(); return keys(bridge.reshuffle()); },
    edit(value, key, account, deleting) {
      fields([value, key]);
      const result = bridge.edit(value, key, account, deleting);
      if (!slotUtf8(result, 4096)) throw Error("Invalid shared keyboard edit response");
      return result;
    },
    dispose() { if (!disposed) { disposed = true; bridge.free(); } },
  };
}

export type QuestActionInput = {
  action: "accept" | "finish";
  profile: "crystal" | "newcomer-v1" | "newcomer-v2";
  status: "notStarted" | "inProgress" | "readyToTurnIn" | "other" | null;
  acceptNpcIndex?: number | null;
  finishNpcIndex?: number | null;
  selectableRewardIndices: number[];
  selectedRewardIndex?: number | null;
  dialogNpcIndex?: number | null;
  dialogActionOffered: boolean;
  pending: boolean;
};

export type QuestActionDecision = {
  eligible: boolean;
  source?: "diary" | "npcDialog";
  npcIndex?: number;
  rejection?: "pending" | "profileDisabled" | "missingEndpoint" | "actionUnavailable"
    | "wrongStatus" | "rewardSelectionRequired" | "invalidInput";
};

export type EquipmentOperation = {
  kind: "equip" | "remove";
  grid: "inventory" | "belt" | "storage";
  uniqueId: number;
  to: number;
};

export type EquipmentPlacement = {
  uniqueId: number | null;
  container: 0 | 1 | 2 | 3 | 4;
  slot: number;
};

/** Crystal whole-array capacity, including six belt cells. */
export type EquipmentSnapshot = {
  capacity: number;
  storageCapacity?: number;
  placements: EquipmentPlacement[];
};

export type EquipmentBridgeResult = {
  ok: boolean;
  error?: string;
  pending: number;
  matched?: boolean;
  released?: boolean | number;
  reserved?: boolean;
  ready?: boolean;
  barriers?: number;
};

export type EquipmentPendingRuntime = {
  replaceSnapshot(snapshot: EquipmentSnapshot): EquipmentBridgeResult;
  reserve(operation: EquipmentOperation): EquipmentBridgeResult;
  acknowledge(operation: EquipmentOperation, success: boolean): EquipmentBridgeResult;
  releaseUnsent(operation: EquipmentOperation): EquipmentBridgeResult;
  contains(operation: EquipmentOperation): EquipmentBridgeResult;
  hasInstance(uniqueId: number): EquipmentBridgeResult;
  status(): EquipmentBridgeResult;
};

export type ClientCoreRuntime = {
  /** Optional ABI; older bundles keep their existing capabilities. */
  createAuthUi(seed: bigint): AuthUiRuntime;
  createChatUi(mask: number): ChatUiRuntime;
  searchMapRoute(input: MapRouteInput): MapRouteDecision;
  readCashPreviewLayers(input: CashPreviewInput): CashPreviewLayerDocument | null;
  turnCashPreview(direction: number, right: boolean): number | null;
  readNpcRepairQuote(input: NpcRepairQuoteInput): NpcRepairQuote | null;
  readItemTooltip(item: CrystalTooltipItem, player: Readonly<Record<string, unknown>>, nowMs?: number): CrystalTooltipDocument | null;
  resolveQuestAction(input: QuestActionInput): QuestActionDecision;
  /** Additive capability; old Quest-only bundles throw only when requested. */
  createEquipmentPendingLedger(): EquipmentPendingRuntime;
  normalizeMailMessage(message: string): MailMessageDecision;
  prepareMailSend(input: MailSendInput): MailSendDecision;
  createMailParcelLedger(): MailParcelRuntime;
  /** One cached Core-owned send slot; remounts never construct another flight. */
  getMailSendSlot(): MailSendSlotRuntime;
  getNpcGoldBuyAttemptSlot(): NpcGoldBuyAttemptSlotRuntime;
};

export type MailSendInput = {recipient:string;message:string;gold:number;attachmentUniqueIds:number[];stamped:boolean};
export type MailWirePayload = {name:string;message:string;gold:number;itemsIdx:number[];stamped:boolean};
export type MailMessageDecision = {ok:true;message:string}|{ok:false;error:string};
export type MailSendDecision = {ok:true;payload:MailWirePayload}|{ok:false;error:string};
export type MailParcelSnapshot={bagCapacity:number;items:Array<{uniqueId:number|null;container:number;slot:number;stamp:boolean;pricing:null|{templateIndex:number;itemType:number;shape:number;templatePrice:number;templateDurability:number;quantity:number;currentDura:number|null;maxDura:number|null;addedStats:Array<{stat:number;value:number}>}}>};
export type MailParcelState={attachmentUniqueIds:number[];stamped:boolean;slotLimit:number;blockedUniqueIds:number[];postage:number|null;pendingQuote:boolean;quoteReady:boolean;reviewRequired:boolean;notice:string|null};
export type MailParcelDecision={ok:false;error:string}|{ok:true;state:MailParcelState;locks:Array<{uniqueId:number;locked:boolean}>;quote:{gold:number;itemsIdx:number[];stamped:boolean}|null;token:number|null};
export type MailParcelRuntime={transact(input:Record<string,unknown>):MailParcelDecision};

export type MailSlotStream={run:string;connection:string};
export type MailSlotOwner={connectionGeneration:string;sessionGeneration:string;ownerRevision:string;playerObjectId:string};
export type MailSlotRaw={to:string;subject:string;body:string;goldText:string;items:string[];attachmentUniqueIds:string[];stamped:boolean;attachmentUniqueIdsPresent:boolean;stampedPresent:boolean};
export type MailSlotPayload={name:string;message:string;gold:number;itemsIdx:string[];stamped:boolean};
export type MailSlotFlight={token:string;stream:MailSlotStream;owner:MailSlotOwner;ownerTag:string;incarnation:string;generation:string;key:string;draft:MailSlotRaw;payload:MailSlotPayload;body:string;entered:boolean;retired:boolean};
export type MailSlotState={stream:MailSlotStream|null;incarnation:string|null;owner:MailSlotOwner|null;ownerTag:string|null;generation:string|null;key:string|null;draft:MailSlotRaw|null;flight:MailSlotFlight|null;notice:string|null};
export type MailSlotDecision={ok:false;error:string}|{ok:true;matched:boolean;completion:'success'|'failure'|'retired'|null;state:MailSlotState};
export type MailSendSlotRuntime={transact(input:Record<string,unknown>):MailSlotDecision;observeOpen(source:object,current:object|null,isOpen:boolean):boolean;streamFor(source:object):MailSlotStream|null;
 syncUnavailableOwner(owner:MailSlotOwner|null):void;retainUnavailableDraft(owner:MailSlotOwner,key:string,draft:MailSlotRaw):void;unavailableDraft(owner:MailSlotOwner):MailSlotRaw|null;
 /** Optional read-only projection; older ABI-1 bundles can still send through transact. */
 draftGold?():number|null};


export type NpcGoldBuyAttemptPhase = 'queued'|'bound'|'entered'|'flushed'|'unknown'|'definitelyUnsent';
export type NpcGoldBuyTransportTicket = Readonly<{transport:string;body:string}>;
export type NpcGoldBuyAttemptState = Readonly<{authorityRevision:string;canReserve:boolean;
 flight:Readonly<{token:string;authorityRevision:string;ticket:NpcGoldBuyTransportTicket|null;phase:NpcGoldBuyAttemptPhase}>|null;
 lastPhase:NpcGoldBuyAttemptPhase|null}>;
export type NpcGoldBuyAttemptDecision = {ok:false;error:string}|{ok:true;matched:boolean;state:NpcGoldBuyAttemptState};
/** Core owns purchase state. Producer and socket identities only bind the host. */
export type NpcGoldBuyAttemptSlotRuntime = {
 attachProducer(source:object):boolean;
 withdrawProducer(source:object):boolean;
 transact(source:object,input:Record<string,unknown>):NpcGoldBuyAttemptDecision;
 transportFor(source:object,socket:object,current:object|null,isOpen:boolean):string|null;
};

type WasmEquipmentPendingBridge = {
  replace_snapshot(input: string): string;
  reserve(input: string): string;
  acknowledge(input: string): string;
  release_unsent(input: string): string;
  contains(input: string): string;
  has_instance(input: string): string;
  status(): string;
};

type WasmModule = {
  default(options: { module_or_path: URL }): Promise<unknown>;
  client_core_abi_version(): number;
  auth_ui_abi_version?: () => number;
  auth_ui_validate_registration?: (account: string, password: string, confirm: string,
    name: string, birthDate: string, question: string, answer: string, email: string) => string;
  auth_ui_validate_password?: (account: string, oldPassword: string, newPassword: string, confirm: string) => number;
  AuthUiBridge?: new (seed: bigint) => { keys(): string; reshuffle(): string;
    edit(value: string, key: string, account: boolean, deleting: boolean): string; free(): void };
  resolve_quest_action(input: string): string;
  equipment_pending_abi_version?: () => number;
  EquipmentPendingBridge?: new () => WasmEquipmentPendingBridge;
  mail_compose_abi_version?:()=>number;
  normalize_mail_message?:(input:string)=>string;
  prepare_mail_send?:(input:string)=>string;
  mail_parcel_abi_version?:()=>number;
  MailParcelBridge?:new()=>{transact(input:string):string};
  mail_send_slot_abi_version?:()=>number;
  MailSendSlotBridge?:new()=>{transact(input:string):string;draft_gold?:()=>number|null|undefined};
  npc_gold_buy_attempt_abi_version?:()=>number;
  NpcGoldBuyAttemptBridge?:new()=>{transact(input:string):string};
};

/** Rust String limits are UTF-8 bytes, with no unpaired UTF-16 surrogates. */
function slotUtf8(v:unknown,limit:number):v is string{
 if(typeof v!=='string')return false;let bytes=0;
 for(const scalar of v){const point=scalar.codePointAt(0)!;if(point>=0xd800&&point<=0xdfff)return false;
  bytes+=point<=0x7f?1:point<=0x7ff?2:point<=0xffff?3:4;if(bytes>limit)return false;}
 return true;
}
function slotFields(v:unknown,names:string[]):v is Record<string,unknown>{return typeof v==='object'&&v!==null&&!Array.isArray(v)&&Object.keys(v).sort().join()===names.slice().sort().join();}
function slotDecimal(v:unknown,positive=true):v is string{return typeof v==='string'&&/^(0|[1-9][0-9]{0,19})$/.test(v)&&BigInt(v)<=18446744073709551615n&&(!positive||v!=='0');}
function slotStream(v:unknown):v is MailSlotStream{return slotFields(v,['run','connection'])&&slotDecimal(v.run)&&slotDecimal(v.connection);}
function slotOwner(v:unknown):v is MailSlotOwner{return slotFields(v,['connectionGeneration','sessionGeneration','ownerRevision','playerObjectId'])&&slotDecimal(v.connectionGeneration)&&slotDecimal(v.sessionGeneration)&&slotDecimal(v.ownerRevision,false)&&slotDecimal(v.playerObjectId);}
function slotRaw(v:unknown):v is MailSlotRaw{return slotFields(v,['to','subject','body','goldText','items','attachmentUniqueIds','stamped','attachmentUniqueIdsPresent','stampedPresent'])
 &&slotUtf8(v.to,1024)&&slotUtf8(v.subject,4096)&&slotUtf8(v.body,8192)&&slotUtf8(v.goldText,256)
 &&Array.isArray(v.items)&&v.items.length<=5&&v.items.every(x=>slotUtf8(x,1024))
 &&Array.isArray(v.attachmentUniqueIds)&&v.attachmentUniqueIds.length<=5&&v.attachmentUniqueIds.every(x=>slotDecimal(x)&&BigInt(x)<=9007199254740991n)&&new Set(v.attachmentUniqueIds).size===v.attachmentUniqueIds.length
 &&typeof v.stamped==='boolean'&&typeof v.attachmentUniqueIdsPresent==='boolean'&&typeof v.stampedPresent==='boolean'
 &&(v.attachmentUniqueIdsPresent||v.attachmentUniqueIds.length===0)&&(v.stampedPresent||!v.stamped);}
function slotPayload(v:unknown):v is MailSlotPayload{return slotFields(v,['name','message','gold','itemsIdx','stamped'])&&typeof v.name==='string'&&typeof v.message==='string'
 &&typeof v.gold==='number'&&Number.isSafeInteger(v.gold)&&v.gold>=0&&v.gold<=0xffffffff&&typeof v.stamped==='boolean'
 &&Array.isArray(v.itemsIdx)&&v.itemsIdx.length===5&&v.itemsIdx.every(x=>slotDecimal(x,false)&&BigInt(x)<=9007199254740991n);}
function slotFlight(v:unknown):v is MailSlotFlight{return slotFields(v,['token','stream','owner','ownerTag','incarnation','generation','key','draft','payload','body','entered','retired'])
 &&slotDecimal(v.token)&&slotStream(v.stream)&&slotOwner(v.owner)&&slotDecimal(v.ownerTag)&&slotDecimal(v.incarnation)&&slotDecimal(v.generation)
 &&slotUtf8(v.key,4096)&&slotRaw(v.draft)&&slotPayload(v.payload)&&slotUtf8(v.body,32768)&&typeof v.entered==='boolean'&&typeof v.retired==='boolean';}
function mailSendSlotRequest(v:unknown):boolean{
 if(typeof v!=='object'||v===null||Array.isArray(v))return false;const r=v as Record<string,unknown>;
 switch(r.op){case 'status':case 'mount':case 'open':return slotFields(r,['op']);
 case 'sync':return slotFields(r,['op','incarnation','owner'])&&slotDecimal(r.incarnation)&&(r.owner===null||slotOwner(r.owner));
 case 'remember':return slotFields(r,['op','incarnation','key','draft'])&&slotDecimal(r.incarnation)&&slotUtf8(r.key,4096)&&slotRaw(r.draft);
 case 'reserve':return slotFields(r,['op','incarnation','key','payload','body'])&&slotDecimal(r.incarnation)&&slotUtf8(r.key,4096)&&slotPayload(r.payload)&&slotUtf8(r.body,32768);
 case 'allows':case 'cancel':case 'enter':return slotFields(r,['op','incarnation','token','stream','body'])&&slotDecimal(r.incarnation)&&slotDecimal(r.token)&&slotStream(r.stream)&&slotUtf8(r.body,32768);
 case 'ack':return slotFields(r,['op','incarnation','stream','result'])&&(r.incarnation===null||slotDecimal(r.incarnation))&&slotStream(r.stream)&&(r.result===1||r.result===-1);
 default:return false;}
}
function mailSendSlotResult(json:string):MailSlotDecision{
 const r:unknown=JSON.parse(json);if(JSON.stringify(r)!==json)throw Error('Noncanonical mail send-slot response');
 if(slotFields(r,['ok','error'])&&r.ok===false&&typeof r.error==='string')return r as MailSlotDecision;
 if(!slotFields(r,['ok','matched','completion','state'])||r.ok!==true||typeof r.matched!=='boolean'||![null,'success','failure','retired'].includes(r.completion as null|string))throw Error('Invalid mail send-slot response');
 const s=r.state;if(!slotFields(s,['stream','incarnation','owner','ownerTag','generation','key','draft','flight','notice'])
 ||!(s.stream===null||slotStream(s.stream))||!(s.incarnation===null||slotDecimal(s.incarnation))||!(s.owner===null||slotOwner(s.owner))
 ||!(s.ownerTag===null||slotDecimal(s.ownerTag))||!(s.generation===null||slotDecimal(s.generation))||!(s.key===null||slotUtf8(s.key,4096))
 ||!(s.draft===null||slotRaw(s.draft))||!(s.flight===null||slotFlight(s.flight))||!(s.notice===null||typeof s.notice==='string'))throw Error('Invalid mail send-slot state');
 return r as MailSlotDecision;
}
/** Socket identity is a platform binding only. Core allocates every stream. */
function createMailSendSlotRuntime(module:WasmModule):MailSendSlotRuntime{
 const streams=new WeakMap<object,MailSlotStream>();let bridge:{transact(input:string):string;draft_gold?:()=>number|null|undefined}|null=null;
 // Passive draft display only when the new capability is absent. This cache
 // has no token, generation, transport state, allocation or send authority.
 let unavailableOwner:MailSlotOwner|null=null,unavailableRaw:MailSlotRaw|null=null;
 const sameOwner=(a:MailSlotOwner|null,b:MailSlotOwner|null)=>Boolean(a&&b&&['connectionGeneration','sessionGeneration','ownerRevision','playerObjectId'].every(k=>a[k as keyof MailSlotOwner]===b[k as keyof MailSlotOwner]));
 const unavailable=():MailSlotDecision=>({ok:false,error:'Shared mail send slot is unavailable; draft kept'});
 try{if(module.mail_send_slot_abi_version?.()===1&&typeof module.MailSendSlotBridge==='function'){
  const candidate=new module.MailSendSlotBridge();const status=mailSendSlotResult(candidate.transact(JSON.stringify({op:'status'})));if(status.ok)bridge=candidate;
 }}catch{/* Independent missing capability does not reject Quest/equipment/old compose loading. */}
 const call=(input:Record<string,unknown>):MailSlotDecision=>{if(!mailSendSlotRequest(input))throw Error('Invalid mail send-slot request');const json=JSON.stringify(input);if(!slotUtf8(json,65536))throw Error('Invalid mail send-slot request');if(!bridge)return unavailable();return mailSendSlotResult(bridge.transact(json));};
 return{transact(input){if(input.op==='open')throw Error('Mail stream requires a current socket OPEN');return call(input);},
  draftGold(){try{const value=bridge?.draft_gold?.();return typeof value==='number'&&Number.isSafeInteger(value)&&!Object.is(value,-0)&&value>=0&&value<=0xffffffff?value:null;}catch{return null;}},
  observeOpen(source,current,isOpen){if(source!==current||!isOpen)return false;const prior=streams.get(source);
   if(prior){const state=call({op:'status'});return Boolean(state.ok&&state.state.stream?.run===prior.run&&state.state.stream.connection===prior.connection);}
   const result=call({op:'open'});if(!result.ok||!result.matched||!result.state.stream)return false;streams.set(source,{...result.state.stream});return true;},
  streamFor(source){const stream=streams.get(source);return stream?{...stream}:null;},
  syncUnavailableOwner(owner){if(bridge)return;if(owner!==null&&!slotOwner(owner))throw Error('Invalid unavailable draft owner');if(!sameOwner(unavailableOwner,owner)){unavailableOwner=owner?{...owner}:null;unavailableRaw=null;}},
  retainUnavailableDraft(owner,_key,draft){if(bridge)return;if(!slotOwner(owner)||!slotRaw(draft))throw Error('Invalid unavailable draft');unavailableOwner={...owner};unavailableRaw={...draft,items:[...draft.items],attachmentUniqueIds:[...draft.attachmentUniqueIds]};},
  unavailableDraft(owner){return !bridge&&sameOwner(unavailableOwner,owner)&&unavailableRaw?{...unavailableRaw,items:[...unavailableRaw.items],attachmentUniqueIds:[...unavailableRaw.attachmentUniqueIds]}:null;}};
}

function npcAttemptFields(value:unknown,names:string[]):value is Record<string,unknown> {
 if(typeof value!=='object'||value===null||Array.isArray(value))return false;
 const prototype=Object.getPrototypeOf(value);
 if(prototype!==Object.prototype&&prototype!==null||Object.getOwnPropertySymbols(value).length)return false;
 const descriptors=Object.getOwnPropertyDescriptors(value);
 return Object.keys(descriptors).sort().join()===names.slice().sort().join()
  &&Object.values(descriptors).every(field=>field.enumerable&&'value' in field);
}
function npcAttemptTicket(value:unknown):value is NpcGoldBuyTransportTicket {
 return npcAttemptFields(value,['transport','body'])&&slotDecimal(value.transport)&&slotUtf8(value.body,4096);
}
function npcAttemptRequest(value:unknown):boolean {
 if(typeof value!=='object'||value===null)return false;
 const op=Object.getOwnPropertyDescriptor(value,'op');if(!op||!('value' in op))return false;
 const r=value as Record<string,unknown>;
 switch(op.value){
 case 'status':case 'reserve':return npcAttemptFields(r,['op']);
 case 'observe':return npcAttemptFields(r,['op','authority'])&&slotUtf8(r.authority,1048576)&&r.authority.length>0;
 case 'availability':return npcAttemptFields(r,['op','available'])&&typeof r.available==='boolean';
 case 'rejectUnpublished':return npcAttemptFields(r,['op','token'])&&slotDecimal(r.token);
 case 'bind':case 'allows':case 'enter':return npcAttemptFields(r,['op','token','ticket'])&&slotDecimal(r.token)&&npcAttemptTicket(r.ticket);
 case 'receipt':return npcAttemptFields(r,['op','token','ticket','outcome'])&&slotDecimal(r.token)&&npcAttemptTicket(r.ticket)
  &&['definitelyUnsent','flushed','unknown'].includes(r.outcome as string);
 default:return false;
 }
}
/** Only the private actual-OPEN-socket binding may submit this operation. */
function npcAttemptConnectionRequest(value:unknown):boolean {
 return npcAttemptFields(value,['op','run','connection'])&&value.op==='connection'
  &&slotDecimal(value.run)&&slotDecimal(value.connection);
}
function npcAttemptResult(json:string):NpcGoldBuyAttemptDecision {
 const value:unknown=JSON.parse(json);
 if(JSON.stringify(value)!==json)throw Error('Noncanonical NPC purchase response');
 if(npcAttemptFields(value,['ok','error'])&&value.ok===false&&typeof value.error==='string')return value as NpcGoldBuyAttemptDecision;
 if(!npcAttemptFields(value,['ok','matched','state'])||value.ok!==true||typeof value.matched!=='boolean')throw Error('Invalid NPC purchase response');
 const state=value.state,phases=['queued','bound','entered','flushed','unknown','definitelyUnsent'];
 if(!npcAttemptFields(state,['authorityRevision','canReserve','flight','lastPhase'])||!slotDecimal(state.authorityRevision,false)
  ||typeof state.canReserve!=='boolean'||!(state.lastPhase===null||phases.includes(state.lastPhase as string)))throw Error('Invalid NPC purchase state');
 const flight=state.flight;
 if(flight!==null){
  if(!npcAttemptFields(flight,['token','authorityRevision','ticket','phase'])||!slotDecimal(flight.token)||!slotDecimal(flight.authorityRevision)
   ||(!['entered','flushed','unknown'].includes(flight.phase as string)
    ?flight.authorityRevision!==state.authorityRevision:BigInt(flight.authorityRevision)>BigInt(state.authorityRevision))
   ||!phases.includes(flight.phase as string)||state.canReserve
   ||state.lastPhase!==flight.phase||(flight.phase==='queued'?flight.ticket!==null:!npcAttemptTicket(flight.ticket)))throw Error('Invalid NPC purchase flight');
  if(flight.ticket!==null)Object.freeze(flight.ticket);Object.freeze(flight);
 }
 Object.freeze(state);return value as NpcGoldBuyAttemptDecision;
}
/** One Core instance per document/content version, including remount and HMR. */
function persistentNpcGoldBuyAttemptSlot(module:WasmModule,documentOwner:object,version:string):NpcGoldBuyAttemptSlotRuntime {
 const key=Symbol.for('mir2.clientCore.npcGoldBuyAttempt.v1');
 const facadeContract='npcGoldBuyConnectionBarrier.v1';
 const descriptor=Object.getOwnPropertyDescriptor(documentOwner,key);
 if(descriptor){
  if(!('value' in descriptor)||!(npcAttemptFields(descriptor.value,['version','runtime','poison','facadeContract'])
    ||npcAttemptFields(descriptor.value,['version','runtime','poison']))
   ||typeof descriptor.value.version!=='string'||typeof descriptor.value.poison!=='function')throw Error('Invalid persistent NPC purchase slot');
  const holder=descriptor.value as {version:string;runtime:NpcGoldBuyAttemptSlotRuntime;poison:()=>void;facadeContract?:unknown};
  if(holder.facadeContract!==facadeContract){
   // Retain and disable the existing holder/Core; never construct another slot
   // or return a pre-connection facade merely because content versions match.
   try{holder.poison();}finally{throw Error('Incompatible persistent NPC purchase facade; reload the client');}
  }
  if(holder.version!==version)holder.poison();
  return holder.runtime;
 }
 let bridge:{transact(input:string):string}|null=null,poisoned=false,active:object|null=null;
 const known=new WeakSet<object>(),retired=new WeakSet<object>(),transports=new WeakMap<object,string>();let lastTransport=0n;
 const unavailable=():NpcGoldBuyAttemptDecision=>({ok:false,error:'Shared NPC purchase state is unavailable; reload the client'});
 const poison=()=>{if(poisoned)return;poisoned=true;
  try{bridge?.transact(JSON.stringify({op:'availability',available:false}));}catch{/* Preserve the existing Core flight. */}
 };
 try{if(module.npc_gold_buy_attempt_abi_version?.()===1&&typeof module.NpcGoldBuyAttemptBridge==='function'){
  const candidate=new module.NpcGoldBuyAttemptBridge();
  if(npcAttemptResult(candidate.transact(JSON.stringify({op:'status'}))).ok)bridge=candidate;
 }}catch{poisoned=true;}
 const call=(input:Record<string,unknown>,trustedConnection=false):NpcGoldBuyAttemptDecision=>{
  if(!(trustedConnection?npcAttemptConnectionRequest(input):npcAttemptRequest(input)))throw Error('Invalid NPC purchase request');
  const json=JSON.stringify(input);if(!slotUtf8(json,6*(1048576+4096)+256))throw Error('Oversized NPC purchase request');
  if(poisoned||!bridge)return unavailable();
  try{return npcAttemptResult(bridge.transact(json));}catch{poison();return unavailable();}
 };
 const runtime:NpcGoldBuyAttemptSlotRuntime=Object.freeze({
  attachProducer(source:object){
   if(poisoned||!bridge||retired.has(source))return false;
   if(active===source)return true;
   const result=call({op:'availability',available:false});if(!result.ok||!result.matched)return false;
   if(active)retired.add(active);active=source;known.add(source);return true;
  },
  withdrawProducer(source:object){
   if(active!==source)return false;
   active=null;retired.add(source);const result=call({op:'availability',available:false});return result.ok&&result.matched;
  },
  transact(source:object,input:Record<string,unknown>){
   if(!npcAttemptRequest(input))throw Error('Invalid NPC purchase request');
   if(!known.has(source)||(active!==source&&input.op!=='receipt'&&input.op!=='status'))return unavailable();
   return call(input);
  },
  transportFor(source:object,socket:object,current:object|null,isOpen:boolean){
   if(poisoned||!bridge||active!==source||socket!==current||!isOpen)return null;
   const prior=transports.get(socket);
   // A previously known socket can never roll the trusted connection backward.
   if(prior&&BigInt(prior)<lastTransport)return null;
   if(!prior&&lastTransport===18446744073709551615n){poison();return null;}
   const identity=prior??(++lastTransport).toString();
   if(!prior)transports.set(socket,identity);
   const observed=call({op:'connection',run:'1',connection:identity},true);
   return observed.ok&&observed.matched&&!poisoned&&active===source
    &&transports.get(socket)===identity&&lastTransport.toString()===identity?identity:null;
  },
 });
 Object.defineProperty(documentOwner,key,{value:Object.freeze({version,runtime,poison,facadeContract}),enumerable:false,writable:false,configurable:false});
 return runtime;
}

function mailResult(json:string):Record<string,unknown> {
  const value:unknown=JSON.parse(json);
  if(typeof value!=="object"||value===null||Array.isArray(value))throw Error("Invalid shared mail response");
  const result=value as Record<string,unknown>;
  if(typeof result.ok!=="boolean"||result.ok===false&&(typeof result.error!=="string"||Object.keys(result).sort().join()!=="error,ok"))throw Error("Invalid shared mail response");
  return result;
}

function equipmentResult(json: string): EquipmentBridgeResult {
  const value: unknown = JSON.parse(json);
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("Invalid equipment ledger response");
  }
  const result = value as Record<string, unknown>;
  if (typeof result.ok !== "boolean" || !Number.isSafeInteger(result.pending)
    || (result.pending as number) < 0) {
    throw new Error("Invalid equipment ledger response");
  }
  return result as EquipmentBridgeResult;
}

function parcelResult(json:string):MailParcelDecision {
 const r=mailResult(json);if(!r.ok)return r as MailParcelDecision;
 const s=r.state as MailParcelState|null;
 const ids=(v:unknown):v is number[]=>Array.isArray(v)&&v.length<=1024&&v.every(n=>Number.isSafeInteger(n)&&n>0)&&new Set(v).size===v.length;
 const uint=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0&&v<=0xffffffff;
 if(Object.keys(r).sort().join()!=='locks,ok,quote,state,token'||!s||typeof s!=='object'||Array.isArray(s)||Object.keys(s).sort().join()!=='attachmentUniqueIds,blockedUniqueIds,notice,pendingQuote,postage,quoteReady,reviewRequired,slotLimit,stamped'
  ||!ids(s.attachmentUniqueIds)||s.attachmentUniqueIds.length>5||!ids(s.blockedUniqueIds)||![s.stamped,s.pendingQuote,s.quoteReady,s.reviewRequired].every(v=>typeof v==='boolean')
  ||s.slotLimit!==(s.stamped?5:1)||s.attachmentUniqueIds.length>s.slotLimit||!(s.postage===null||uint(s.postage))||!(s.notice===null||typeof s.notice==='string')
  ||!Array.isArray(r.locks)||r.locks.length>1024||!r.locks.every(v=>typeof v==='object'&&v!==null&&!Array.isArray(v)&&Object.keys(v).sort().join()==='locked,uniqueId'&&Number.isSafeInteger(v.uniqueId)&&v.uniqueId>0&&typeof v.locked==='boolean')
  ||!(r.token===null||typeof r.token==='number'&&Number.isSafeInteger(r.token)&&r.token>0))throw Error('Invalid parcel ledger response');
 if(r.quote!==null){const q=r.quote as Record<string,unknown>;if(typeof q!=='object'||Array.isArray(q)||Object.keys(q).sort().join()!=='gold,itemsIdx,stamped'||!uint(q.gold)||typeof q.stamped!=='boolean'||!Array.isArray(q.itemsIdx)||q.itemsIdx.length!==5||!q.itemsIdx.every(n=>Number.isSafeInteger(n)&&n>=0)||r.token===null)throw Error('Invalid parcel quote response');}
 return r as MailParcelDecision;
}

let pendingRuntime: Promise<ClientCoreRuntime> | null = null;
let loadAttempt = 0;

/** Independent of Bevy: compatibility and touch clients load the same policy. */
export function loadClientCoreRuntime(): Promise<ClientCoreRuntime> {
  if (pendingRuntime) return pendingRuntime;
  pendingRuntime = (async () => {
    if (typeof window === "undefined") throw new Error("Client core requires a browser");
    const base = new URL(`/client-core/${manifest.version}/`, window.location.origin);
    const moduleUrl = new URL("mir2_platform_web.js", base);
    // Browsers remember a failed module import for the life of the document.
    // A user retry must issue a fresh import while keeping the content version.
    const attempt = loadAttempt++;
    if (attempt > 0) moduleUrl.searchParams.set("retry", String(attempt));
    const module = await import(/* webpackIgnore: true */ moduleUrl.href) as WasmModule;
    await module.default({ module_or_path: new URL("mir2_platform_web_bg.wasm", base) });
    if (module.client_core_abi_version() !== manifest.abiVersion) {
      throw new Error("Client core version mismatch; reload the client");
    }
    // Presentation rules are a separate small package so compatibility/touch
    // clients use the same Rust policy without loading a renderer.
    const presentationRelease = (manifest as typeof manifest & { presentation?: {
      abiVersion: number; version: string; sourceSha256: string;
    } }).presentation;
    if (!presentationRelease || presentationRelease.abiVersion !== 1
      || !/^[a-f0-9]{64}$/.test(presentationRelease.version)
      || presentationRelease.sourceSha256 !== manifest.sourceSha256) {
      throw new Error("Client presentation package mismatch; rebuild the client");
    }
    const presentationBase = new URL(`/client-core/${presentationRelease.version}/`, window.location.origin);
    const presentationUrl = new URL("mir2_platform_web.js", presentationBase);
    if (attempt > 0) presentationUrl.searchParams.set("retry", String(attempt));
    const presentation = await import(/* webpackIgnore: true */ presentationUrl.href) as
      PresentationWasmModule & MapRouteWasmModule & { default: WasmModule["default"]; client_presentation_abi_version: () => number };
    await presentation.default({ module_or_path: new URL("mir2_platform_web_bg.wasm", presentationBase) });
    if (presentation.client_presentation_abi_version() !== presentationRelease.abiVersion) {
      throw new Error("Client presentation version mismatch; reload the client");
    }
    let mailSendSlot:MailSendSlotRuntime|null=null;
    return {
      createAuthUi(seed: bigint) { return createAuthUiRuntime(module, seed); },
      createChatUi(mask: number) { return createSharedChatUi(presentation, mask); },
      searchMapRoute(input: MapRouteInput) { return searchSharedMapRoute(presentation, input); },
      readCashPreviewLayers(input: CashPreviewInput) { return readSharedCashPreview(presentation, input); },
      turnCashPreview(direction: number, right: boolean) { return turnSharedCashPreview(presentation, direction, right); },
      readNpcRepairQuote(input: NpcRepairQuoteInput) { return readSharedNpcRepairQuote(presentation, input); },
      readItemTooltip(item: CrystalTooltipItem, player: Readonly<Record<string, unknown>>, nowMs?: number) {
        return readSharedPresentationItemTooltip(presentation, item, player, nowMs);
      },
      getMailSendSlot():MailSendSlotRuntime {return mailSendSlot??=createMailSendSlotRuntime(module);},
      getNpcGoldBuyAttemptSlot():NpcGoldBuyAttemptSlotRuntime {return persistentNpcGoldBuyAttemptSlot(module,document,manifest.version);},
      resolveQuestAction(input: QuestActionInput): QuestActionDecision {
        return JSON.parse(module.resolve_quest_action(JSON.stringify(input))) as QuestActionDecision;
      },
      createEquipmentPendingLedger(): EquipmentPendingRuntime {
        if (module.equipment_pending_abi_version?.() !== 1 || !module.EquipmentPendingBridge) {
          throw new Error("Shared equipment ledger is unavailable in this client core");
        }
        const bridge = new module.EquipmentPendingBridge();
        return {
          replaceSnapshot: (snapshot) => equipmentResult(bridge.replace_snapshot(JSON.stringify(snapshot))),
          reserve: (operation) => equipmentResult(bridge.reserve(JSON.stringify(operation))),
          acknowledge: (operation, success) => equipmentResult(bridge.acknowledge(JSON.stringify({ operation, success }))),
          releaseUnsent: (operation) => equipmentResult(bridge.release_unsent(JSON.stringify(operation))),
          contains: (operation) => equipmentResult(bridge.contains(JSON.stringify(operation))),
          hasInstance: (uniqueId) => equipmentResult(bridge.has_instance(JSON.stringify({ uniqueId }))),
          status: () => equipmentResult(bridge.status()),
        };
      },
      normalizeMailMessage(message: string): MailMessageDecision {
        if(module.mail_compose_abi_version?.()!==1||!module.normalize_mail_message) {
          return {ok:false,error:"Shared mail rules are unavailable; reload the client"};
        }
        const result=mailResult(module.normalize_mail_message(JSON.stringify({message})));
        if(result.ok&&(typeof result.message!=="string"||Object.keys(result).sort().join()!=="message,ok"))throw Error("Invalid shared mail message response");
        return result as MailMessageDecision;
      },
      prepareMailSend(input: MailSendInput): MailSendDecision {
        if(module.mail_compose_abi_version?.()!==1||!module.prepare_mail_send) {
          return {ok:false,error:"Shared mail rules are unavailable; reload the client"};
        }
        const result=mailResult(module.prepare_mail_send(JSON.stringify(input)));
        if(result.ok){
          const payload=result.payload as MailWirePayload|null;
          if(Object.keys(result).sort().join()!=="ok,payload"||typeof payload!=="object"||payload===null||Array.isArray(payload)
            ||Object.keys(payload).sort().join()!=="gold,itemsIdx,message,name,stamped"||typeof payload.name!=="string"||typeof payload.message!=="string"
            ||!Number.isSafeInteger(payload.gold)||payload.gold<0||payload.gold>0xffffffff||typeof payload.stamped!=="boolean"
            ||!Array.isArray(payload.itemsIdx)||payload.itemsIdx.length!==5||!payload.itemsIdx.every(n=>Number.isSafeInteger(n)&&n>=0))throw Error("Invalid shared mail send response");
        }
        return result as MailSendDecision;
      },
      createMailParcelLedger():MailParcelRuntime {
        if(module.mail_parcel_abi_version?.()!==1||!module.MailParcelBridge)throw Error('Shared parcel rules are unavailable; reload the client');
        const bridge=new module.MailParcelBridge();
        return{transact(input:Record<string,unknown>){return parcelResult(bridge.transact(JSON.stringify(input)));}};
      },
    };
  })().catch((error: unknown) => {
    pendingRuntime = null;
    throw error;
  });
  return pendingRuntime;
}
