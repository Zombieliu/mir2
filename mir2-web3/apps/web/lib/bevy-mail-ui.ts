import {parseMailList,type NormalizedMail} from "./extended-server-packets";
import type {BevyQuestUiPresentation} from "./bevy-quest-ui";
import type {MailWirePayload,MailParcelRuntime,MailParcelSnapshot,MailParcelState,MailParcelDecision,MailSendSlotRuntime,MailSlotDecision,MailSlotState,MailSlotRaw,MailSlotOwner,MailSlotStream} from "./client-core-runtime";
export type MailOwner={connectionGeneration:number;sessionGeneration:number;ownerRevision:number;playerObjectId:number};
export type MailIdentity=MailOwner&{run:number;sceneRevision:number;hudGeneration:number};
export type MailIntent=MailIdentity&{sequence:number;modelRevision:number;presentationRevision:number;renderRevision:number;action:"close"|"compose"|"read"|"reply"|"delete"|"claim"|"lock";mailId:number|null;lock:boolean|null};
export type MailOutcome="confirmedSend"|"definitelyUnsent"|"outcomeUnknown";
type Region={left:number;top:number;width:number;height:number};
export type MailStatus=MailIdentity&{version:1;frame:number;ready:boolean;inputEnabled:boolean;modal:boolean;renderRevision:number;appliedRevision:number;appliedModelRevision:number;appliedPresentationRevision:number;inputRegions:Region[];error:string|null};
export type MailSnapshot=MailIdentity&{revision:number;modelRevision:number;presentationRevision:number;open:boolean;inputEnabled:boolean;presentation:BevyQuestUiPresentation|null;mail:NormalizedMail[]};
export type MailPointerContext=MailIdentity&{modelRevision:number;presentationRevision:number;renderRevision:number;modal:boolean;inputRegions:Region[];presentation:BevyQuestUiPresentation};
export type MailPointerEdge=MailIdentity&{sequence:number;modelRevision:number;presentationRevision:number;renderRevision:number;pointerId:number;phase:"down"|"move"|"up"|"cancel";x:number;y:number;button:0};
export type MailRuntime={getMir2MailUiCapabilities?:()=>string;resolveMir2MailUiRows?:(json:string)=>string|null|undefined;setMir2MailUiSnapshot?:(json:string)=>boolean;getMir2MailUiStatus?:()=>string;setMir2MailUiIntentSink?:(sink:(json:string)=>string)=>void;clearMir2MailUiIntentSink?:()=>void;setMir2MailUiPointerEdge?:(json:string)=>boolean};
export type MailInput={owner:MailOwner;sceneRevision:number;hudGeneration:number;eligible:boolean;open:boolean;rustOwns:boolean;presentation:BevyQuestUiPresentation|null;mail:unknown;mailboxOwner:MailOwner|null};
const object=(v:unknown):v is Record<string,unknown>=>Boolean(v)&&typeof v==="object"&&!Array.isArray(v);
const safe=(v:unknown):v is number=>Number.isSafeInteger(v)&&Number(v)>=0;
const ownerKeys=["connectionGeneration","sessionGeneration","ownerRevision","playerObjectId"] as const;
const identityKeys=[...ownerKeys,"run","sceneRevision","hudGeneration"] as const;
export function sameMailOwner(a:MailOwner|null|undefined,b:MailOwner|null|undefined){return Boolean(a&&b&&ownerKeys.every(k=>a[k]===b[k]));}
function validOwner(v:MailOwner){return ownerKeys.every(k=>safe(v[k]))&&v.connectionGeneration>0&&v.sessionGeneration>0&&v.playerObjectId>0&&v.playerObjectId<=0xffffffff;}
function sameIdentity(a:MailIdentity,b:MailIdentity){return identityKeys.every(k=>a[k]===b[k]);}
function validIdentity(v:MailIdentity){return validOwner(v)&&[v.run,v.sceneRevision,v.hudGeneration].every(n=>safe(n)&&n>0);}
export function mailContentKey(row:NormalizedMail){return JSON.stringify([row.mailId,row.senderName,row.message,row.gold,row.items.map(i=>[i.uniqueId,i.itemIndex,i.uniqueId===null?[i.name,i.key]:i.key,i.count,i.currentDura,i.maxDura,i.soulBoundId,i.gemCount,i.identified,i.cursed])]);}
export function fitsMail(p:BevyQuestUiPresentation|null){return Boolean(p&&[p.logicalWidth,p.logicalHeight,p.stageCssScale].every(Number.isFinite)&&p.logicalWidth>=1024&&p.logicalWidth<=16384&&p.logicalHeight>=768&&p.logicalHeight<=16384&&p.stageCssScale>0&&p.stageCssScale<=16&&(!p.touch||p.stageCssScale*16>=44));}
export function supportsMail(r:MailRuntime|null){try{const v=JSON.parse(r?.getMir2MailUiCapabilities?.()??"null");return Boolean(v&&Object.keys(v).sort().join()==="compiled,mailIntentAbiVersion,mailPageAbiVersion,schemaVersion,startup"&&v.schemaVersion===1&&v.mailPageAbiVersion===1&&v.mailIntentAbiVersion===1&&v.compiled===true&&v.startup===true&&[r?.resolveMir2MailUiRows,r?.setMir2MailUiSnapshot,r?.getMir2MailUiStatus,r?.setMir2MailUiIntentSink,r?.clearMir2MailUiIntentSink,r?.setMir2MailUiPointerEdge].every(f=>typeof f==="function"));}catch{return false;}}
export function parseMailIntent(json:string):MailIntent|null {try{const v=JSON.parse(json);if(!object(v)||!validIdentity(v as unknown as MailIdentity)||![v.sequence,v.modelRevision,v.presentationRevision,v.renderRevision].every(n=>safe(n)&&n>0)||!['close','compose','read','reply','delete','claim','lock'].includes(String(v.action))||!(v.mailId===null||safe(v.mailId)&&v.mailId>0)||!(v.action==='lock'?typeof v.lock==='boolean':v.lock===null)||Object.keys(v).sort().join()!==[...identityKeys,"sequence","modelRevision","presentationRevision","renderRevision","action","mailId","lock"].sort().join())return null;return v as MailIntent;}catch{return null;}}
export function readMailStatus(r:MailRuntime):MailStatus|null {try{const v=JSON.parse(r.getMir2MailUiStatus?.()??"null");if(!object(v)||!validIdentity(v as unknown as MailIdentity)||v.version!==1||![v.frame,v.renderRevision,v.appliedRevision,v.appliedModelRevision,v.appliedPresentationRevision].every(safe)||![v.ready,v.inputEnabled,v.modal].every(n=>typeof n==="boolean")||v.error!==null||!Array.isArray(v.inputRegions)||v.inputRegions.length>2||!v.inputRegions.every(r=>object(r)&&[r.left,r.top,r.width,r.height].every(n=>typeof n==="number"&&Number.isFinite(n))&&Number(r.width)>0&&Number(r.height)>0))return null;return v as MailStatus;}catch{return null;}}
type Allocator={readonly version:1;readonly last:number;allocate:()=>number};
export function burnMailRun(){const key="__mir2MailRunV1";
  if(!Object.hasOwn(globalThis,key)){let last=0;Object.defineProperty(globalThis,key,{value:Object.freeze({version:1,get last(){return last;},allocate(){if(last>=Number.MAX_SAFE_INTEGER)throw Error("Mail run exhausted");return ++last;}}),writable:false,configurable:false});}
  const slot=Object.getOwnPropertyDescriptor(globalThis,key),a=slot?.value as Allocator|undefined;
  if(!slot||slot.writable!==false||slot.configurable!==false||!a||!Object.isFrozen(a)||a.version!==1||!safe(a.last)||typeof a.allocate!=="function")throw Error("Mail run invalid");
  const before=a.last,next=a.allocate();if(!safe(next)||next!==before+1||a.last!==next)throw Error("Mail run rollback");return next;
}
function owners(){const realm=globalThis as typeof globalThis&{__mir2MailRuntimeOwnersV1?:WeakMap<object,number>};return realm.__mir2MailRuntimeOwnersV1??=new WeakMap();}
type Options={runtime:MailRuntime;isCurrent:()=>boolean;read:()=>MailInput;now:()=>number;onState:(ready:boolean)=>void;onIntent:(intent:MailIntent)=>MailOutcome};
/** Presentation transport only. Interaction, pagination and reader geometry remain in Rust. */
export class MailHost {
  readonly run=burnMailRun();private stopped=false;private active=false;private revision=0;private modelRevision=0;private presentationRevision=0;
  private modelKey="";private presentationKey="";private sent:MailSnapshot|null=null;private frame=-1;private frameAt=0;private seen=0;private proof:MailIntent|null=null;
  private prepared=false;private everVisible=false;private closing:MailSnapshot|null=null;private withdrawalFrame=-1;
  constructor(private options:Options){if(!supportsMail(options.runtime))return;owners().set(options.runtime,this.run);try{options.runtime.setMir2MailUiIntentSink?.(json=>{
    const i=parseMailIntent(json);if(!i||i.sequence<=this.seen||!this.qualify(i))return JSON.stringify({outcome:"definitelyUnsent"});this.seen=i.sequence;this.proof=i;
    let outcome:MailOutcome;try{outcome=this.options.onIntent(i);}catch{outcome="outcomeUnknown";}finally{this.proof=null;}
    return JSON.stringify({outcome:["confirmedSend","definitelyUnsent","outcomeUnknown"].includes(outcome)?outcome:"outcomeUnknown"});
  });}catch{this.stop();}}
  private owns(){return owners().get(this.options.runtime)===this.run;}
  private statusMatches(v:MailStatus|null,input=true){const s=this.sent;return Boolean(s&&v&&v.ready&&(!input||v.inputEnabled)&&sameIdentity(s,v)&&v.appliedRevision===s.revision&&v.appliedModelRevision===s.modelRevision&&v.appliedPresentationRevision===s.presentationRevision);}
  tick(){if(this.stopped)return;try{
    if(!this.owns()||!this.options.isCurrent()){this.stop();return;}
    const input=this.options.read(),mail=parseMailList(input.mail);
    if(!supportsMail(this.options.runtime)||!validOwner(input.owner)||!input.mailboxOwner||!sameMailOwner(input.owner,input.mailboxOwner)||!mail||!safe(input.sceneRevision)||input.sceneRevision===0||!safe(input.hudGeneration)||input.hudGeneration===0){this.withdraw();return;}
    const open=input.open&&input.eligible&&fitsMail(input.presentation),mk=JSON.stringify(mail),pk=JSON.stringify([input.owner,input.sceneRevision,input.hudGeneration,input.presentation,open]);
    const status=readMailStatus(this.options.runtime);if(status&&status.frame!==this.frame){this.frame=status.frame;this.frameAt=this.options.now();}
    if(mk!==this.modelKey||pk!==this.presentationKey){this.prepared=false;}
    if(!open)this.prepared=false;
    const inputEnabled=open&&this.prepared&&input.rustOwns;
    if(mk!==this.modelKey||pk!==this.presentationKey||!this.sent||this.sent.inputEnabled!==inputEnabled){this.proof=null;if(mk!==this.modelKey)++this.modelRevision;if(pk!==this.presentationKey||this.sent?.inputEnabled!==inputEnabled)++this.presentationRevision;
      this.modelKey=mk;this.presentationKey=pk;const s:MailSnapshot={...input.owner,run:this.run,sceneRevision:input.sceneRevision,hudGeneration:input.hudGeneration,
        revision:++this.revision,modelRevision:this.modelRevision,presentationRevision:this.presentationRevision,open,inputEnabled,presentation:fitsMail(input.presentation)?input.presentation:null,mail};
      if(![s.revision,s.modelRevision,s.presentationRevision].every(safe)||!this.options.runtime.setMir2MailUiSnapshot?.(JSON.stringify(s))){this.withdraw();return;}this.sent=s;}
    const current=readMailStatus(this.options.runtime),fresh=this.options.now()-this.frameAt<=500;
    if(this.statusMatches(current,false)&&fresh&&open)this.prepared=true;
    else if(current&&this.sent&&current.appliedRevision===this.sent.revision&&!current.ready)this.prepared=false;
    if(!fresh)this.prepared=false;
    this.active=this.statusMatches(current)&&fresh;if(this.active)this.everVisible=true;this.options.onState(this.prepared);
  }catch{this.withdraw();}}
  private qualify(i:MailIntent){const s=this.sent;if(this.stopped||!this.active||!s||!this.owns()||!this.options.isCurrent()||!sameIdentity(s,i)||i.modelRevision!==s.modelRevision||i.presentationRevision!==s.presentationRevision)return false;
    const live=this.options.read(),mail=parseMailList(live.mail),status=readMailStatus(this.options.runtime);
    if(!live.eligible||!live.open||!live.mailboxOwner||!sameMailOwner(live.mailboxOwner,live.owner)||!sameMailOwner(s,live.owner)||live.sceneRevision!==s.sceneRevision||live.hudGeneration!==s.hudGeneration||JSON.stringify(mail)!==this.modelKey||JSON.stringify([live.owner,live.sceneRevision,live.hudGeneration,live.presentation,true])!==this.presentationKey||!this.statusMatches(status)||status?.renderRevision!==i.renderRevision||this.options.now()-this.frameAt>500)return false;
    if(i.action==="close"||i.action==="compose")return i.mailId===null;
    const row=mail?.find(m=>m.mailId===i.mailId);return Boolean(row&&(i.action!=="reply"||row.canReply)&&(i.action!=="delete"||!row.locked)&&(i.action!=="claim"||!row.locked&&!row.collected&&(row.gold>0||row.items.length>0))&&(i.action!=="lock"||row.gold===0&&row.items.length===0&&i.lock===!row.locked));
  }
  allows(i:MailIntent){return this.proof!==null&&JSON.stringify(i)===JSON.stringify(this.proof)&&this.qualify(i);}
  claim(i:MailIntent){if(!this.allows(i))return false;this.proof=null;return true;}
  pointerContext():MailPointerContext|null {const s=this.sent,v=readMailStatus(this.options.runtime),live=this.options.read();if(!s?.presentation||!this.active||!this.statusMatches(v)||!this.owns()||!this.options.isCurrent()||!live.open||!live.eligible||!sameMailOwner(s,live.owner)||live.sceneRevision!==s.sceneRevision||live.hudGeneration!==s.hudGeneration||!live.mailboxOwner||!sameMailOwner(live.mailboxOwner,live.owner)||JSON.stringify(parseMailList(live.mail))!==this.modelKey||JSON.stringify([live.owner,live.sceneRevision,live.hudGeneration,live.presentation,true])!==this.presentationKey||this.options.now()-this.frameAt>500)return null;
    return{...identity(s),modelRevision:s.modelRevision,presentationRevision:s.presentationRevision,renderRevision:v!.renderRevision,modal:v!.modal,inputRegions:v!.inputRegions,presentation:s.presentation};}
  pointer(e:MailPointerEdge){if(e.phase==="cancel")this.proof=null;else if(!this.pointerContext())return false;try{return this.options.runtime.setMir2MailUiPointerEdge?.(JSON.stringify(e))===true;}catch{this.withdraw();return false;}}
  isWithdrawn(requirePublished=false){if(!requirePublished&&(!this.everVisible||!this.owns()||!supportsMail(this.options.runtime)))return true;const v=readMailStatus(this.options.runtime),s=this.sent??this.closing;
    return Boolean(v&&s&&sameIdentity(v,s)&&!v.inputEnabled&&(v.appliedRevision>=s.revision||v.frame>this.withdrawalFrame&&!v.ready));}
  withdraw(){this.active=false;this.prepared=false;this.proof=null;this.options.onState(false);const s=this.sent;this.withdrawalFrame=readMailStatus(this.options.runtime)?.frame??this.withdrawalFrame;this.sent=null;if(s&&this.owns())try{this.closing={...s,revision:++this.revision,open:false,inputEnabled:false};this.options.runtime.setMir2MailUiSnapshot?.(JSON.stringify(this.closing));}catch{/* retired runtime */}}
  stop(){this.withdraw();this.stopped=true;if(this.owns()){owners().delete(this.options.runtime);try{this.options.runtime.clearMir2MailUiIntentSink?.();}catch{/* retired */}}}
}
function identity(s:MailIdentity):MailIdentity{return{run:s.run,connectionGeneration:s.connectionGeneration,sessionGeneration:s.sessionGeneration,ownerRevision:s.ownerRevision,sceneRevision:s.sceneRevision,hudGeneration:s.hudGeneration,playerObjectId:s.playerObjectId};}
export class MailPointerRouter {
  held:{context:MailPointerContext;pointerId:number;invalid:boolean}|null=null;private sequence=0;
  matches(c:MailPointerContext|null){const h=this.held;return Boolean(h&&c&&sameIdentity(h.context,c)&&h.context.modelRevision===c.modelRevision&&h.context.presentationRevision===c.presentationRevision&&h.context.renderRevision===c.renderRevision);}
  contains(c:MailPointerContext,x:number,y:number){return c.inputRegions.some(r=>x>=r.left&&y>=r.top&&x<r.left+r.width&&y<r.top+r.height);}
  down(c:MailPointerContext,pointerId:number,x:number,y:number){if(this.held||!this.contains(c,x,y))return null;this.held={context:c,pointerId,invalid:false};return this.edge("down",pointerId,x,y);}
  edge(phase:MailPointerEdge["phase"],pointerId:number,x:number,y:number):MailPointerEdge|null{const h=this.held;if(!h||h.pointerId!==pointerId)return null;if(!this.contains(h.context,x,y))h.invalid=true;
    const e:MailPointerEdge={...identity(h.context),modelRevision:h.context.modelRevision,presentationRevision:h.context.presentationRevision,renderRevision:h.context.renderRevision,sequence:++this.sequence,pointerId,phase:h.invalid?"cancel":phase,x,y,button:0};
    if(!safe(e.sequence)){this.held=null;return null;}if(phase==="up"||phase==="cancel"||h.invalid)this.held=null;return e;}
  cancel(){const h=this.held;return h?this.edge("cancel",h.pointerId,0,0):null;}
}
export type MailCommandType="readMail"|"collectParcel"|"deleteMail"|"sendMail"|"lockMail";
export type MailDraftState={to:string;subject:string;body:string;goldText:string;items:string[];attachmentUniqueIds?:number[];stamped?:boolean};
export type MailComposeProof={owner:MailOwner;key:string;body:string;serial:string;incarnation:string;generation:string;stream:MailSlotStream};
export type MailQuoteProof={owner:MailOwner;token:number;body:string};
export type MailLockProof={owner:MailOwner;body:string};
/** Transport ownership only; selection, stamp and cost policy stay in Rust. */
export class MailParcelController {
  owner:MailOwner|null=null;snapshot:MailParcelSnapshot|null=null;state:MailParcelState|null=null;error:string|null=null;
  private proof:MailQuoteProof|null=null;
  private costConnection:number|null=null;
  private locks=new Set<MailLockProof>();
  constructor(private bridge:MailParcelRuntime){}
  call(input:Record<string,unknown>):MailParcelDecision {
    try{const r=this.bridge.transact(input);if(r.ok){this.state=r.state;this.error=null;if(!r.state.pendingQuote)this.costConnection=null;}else this.error=r.error;return r;}
    catch{this.error='Shared parcel rules are unavailable; draft kept';return{ok:false,error:this.error};}
  }
  sync(owner:MailOwner|null,snapshot:MailParcelSnapshot|null,gold:number,now:number):MailParcelDecision {
    this.owner=owner?{...owner}:null;this.snapshot=snapshot;
    return this.call({action:'sync',owner:this.owner,snapshot,gold,now});
  }
  quote(owner:MailOwner,now:number):{proof:MailQuoteProof;payload:Record<string,unknown>}|null {
    if(!sameMailOwner(owner,this.owner))return null;const r=this.call({action:'quote',now});if(!r.ok||!r.quote||r.token===null)return null;
    const payload={...r.quote,type:'mailCost'},proof={owner:{...owner},token:r.token,body:JSON.stringify(payload)};this.proof=proof;return{proof,payload};
  }
  enter(proof:MailQuoteProof,body:string):boolean {
    if(this.proof!==proof||!sameMailOwner(this.owner,proof.owner)||proof.body!==body)return false;
    const entered=this.call({action:'enter',token:proof.token}).ok;if(entered)this.costConnection=proof.owner.connectionGeneration;return entered;
  }
  finish(proof:MailQuoteProof){if(this.proof===proof){this.call({action:'finish',token:proof.token});this.proof=null;}}
  /** Page has verified the received connection and synced owner/raw to null. */
  retireCost(connectionGeneration:number,cost:unknown):boolean {
    if(this.owner!==null||this.snapshot!==null||this.error||!this.state?.pendingQuote||this.costConnection!==connectionGeneration
      ||!Number.isSafeInteger(connectionGeneration)||connectionGeneration<=0||typeof cost!=='number'||!Number.isSafeInteger(cost)||cost<0||cost>0xffffffff)return false;
    return this.call({action:'cost',cost}).ok;
  }
  lock(owner:MailOwner,uniqueId:number,locked:boolean):{proof:MailLockProof;payload:Record<string,unknown>}|null {
    if(!sameMailOwner(owner,this.owner)||!Number.isSafeInteger(uniqueId)||uniqueId<=0||locked&&!this.state?.blockedUniqueIds.includes(uniqueId))return null;
    const payload={type:'mailLockedItem',uniqueId,locked},proof={owner:{...owner},body:JSON.stringify(payload)};this.locks.add(proof);return{proof,payload};
  }
  enterLock(proof:MailLockProof,body:string){
    if(!this.locks.delete(proof)||!sameMailOwner(this.owner,proof.owner)||proof.body!==body)return false;
    const command=JSON.parse(body) as {uniqueId:number;locked:boolean};
    return !command.locked||Boolean(this.snapshot?.items.some(i=>i.uniqueId===command.uniqueId&&i.container===0)&&this.state?.blockedUniqueIds.includes(command.uniqueId));
  }
  retireLock(proof:MailLockProof){this.locks.delete(proof);}
  allowsSend(owner:MailOwner,payload:MailWirePayload):boolean {
    const s=this.state;if(!sameMailOwner(this.owner,owner)||!s||this.error||s.reviewRequired)return false;
    const ids=payload.itemsIdx.filter(id=>id!==0);
    if(JSON.stringify(ids)!==JSON.stringify(s.attachmentUniqueIds)||payload.stamped!==s.stamped)return false;
    return payload.gold===0&&ids.length===0&&!payload.stamped||this.snapshot!==null&&s.quoteReady;
  }
}
const slotOwnerOf=(owner:MailOwner):MailSlotOwner=>({connectionGeneration:String(owner.connectionGeneration),sessionGeneration:String(owner.sessionGeneration),ownerRevision:String(owner.ownerRevision),playerObjectId:String(owner.playerObjectId)});
const sameSlotOwner=(a:MailSlotOwner|null,b:MailSlotOwner)=>Boolean(a&&ownerKeys.every(key=>a[key]===b[key]));
const slotRawOf=(draft:MailDraftState):MailSlotRaw=>({to:draft.to,subject:draft.subject,body:draft.body,goldText:draft.goldText,items:[...draft.items],attachmentUniqueIds:(draft.attachmentUniqueIds??[]).map(String),stamped:draft.stamped??false,attachmentUniqueIdsPresent:draft.attachmentUniqueIds!==undefined,stampedPresent:draft.stamped!==undefined});
function slotDraftOf(raw:MailSlotRaw):MailDraftState{
 // UIDs are old numeric DTO fields. The new bridge independently refuses any
 // selected UID that cannot be represented losslessly on that existing wire.
 const ids=raw.attachmentUniqueIds.map(id=>{const value=BigInt(id);if(value>9007199254740991n)throw Error('Mail UID is unavailable on this wire');return Number(value);});
 return{to:raw.to,subject:raw.subject,body:raw.body,goldText:raw.goldText,items:[...raw.items],...(raw.attachmentUniqueIdsPresent?{attachmentUniqueIds:ids}:{}),...(raw.stampedPresent?{stamped:raw.stamped}:{})};
}
/** Thin presentation adapter. The only occupied flight and clock live in Core. */
export class MailComposeSender {
  private facade:MailSendSlotRuntime|null=null;private mounted:MailSendSlotRuntime|null=null;private incarnation:string|null=null;
  private offline:{owner:MailOwner;key:string;draft:MailDraftState}|null=null;private localNotice:string|null=null;
  constructor(private readSlot:()=>MailSendSlotRuntime|null=()=>null){}
  private call(input:Record<string,unknown>):MailSlotDecision|null{try{return this.acquire()?.transact(input)??null;}catch{this.localNotice='Shared mail send slot is unavailable; draft kept';return null;}}
  private acquire():MailSendSlotRuntime|null{
    this.facade??=this.readSlot();const facade=this.facade;if(!facade)return null;
    if(this.mounted!==facade){this.mounted=facade;const result=facade.transact({op:'mount'});this.incarnation=result.ok&&result.matched?result.state.incarnation:null;
      if(this.incarnation&&this.offline){const saved=this.offline;facade.transact({op:'sync',incarnation:this.incarnation,owner:slotOwnerOf(saved.owner)});facade.transact({op:'remember',incarnation:this.incarnation,key:saved.key,draft:slotRawOf(saved.draft)});this.offline=null;}}
    return facade;
  }
  private state():MailSlotState|null{const result=this.call({op:'status'});return result?.ok?result.state:null;}
  get notice():string|null{return this.localNotice??this.state()?.notice??null;}
  sync(owner:MailOwner|null){const valid=owner&&validOwner(owner)?owner:null;this.acquire();
    if(this.incarnation){this.call({op:'sync',incarnation:this.incarnation,owner:valid?slotOwnerOf(valid):null});}
    else{this.facade?.syncUnavailableOwner(valid?slotOwnerOf(valid):null);if(this.offline&&!sameMailOwner(this.offline.owner,valid))this.offline=null;}
  }
  observeOpen(source:object,current:object|null,isOpen:boolean){try{return this.acquire()?.observeOpen(source,current,isOpen)??false;}catch{return false;}}
  beginPresentation(owner:MailOwner):string|null{this.sync(owner);const result=this.call({op:'mount'});if(result?.ok&&result.matched){this.incarnation=result.state.incarnation;return this.incarnation;}return null;}
  remember(owner:MailOwner,key:string,draft:MailDraftState){if(!validOwner(owner))return false;this.sync(owner);const state=this.state();if(state?.flight)return false;
    if(!state||!this.incarnation){this.offline={owner:{...owner},key,draft:{...draft,items:[...draft.items],...(draft.attachmentUniqueIds?{attachmentUniqueIds:[...draft.attachmentUniqueIds]}:{})}};this.facade?.retainUnavailableDraft(slotOwnerOf(owner),key,slotRawOf(draft));this.localNotice='Shared mail send slot is unavailable; draft kept';return true;}
    const result=this.call({op:'remember',incarnation:this.incarnation,key,draft:slotRawOf(draft)});if(result?.ok&&result.matched){this.localNotice=null;return true;}return false;}
  rememberRaw(owner:MailOwner,key:string,raw:MailSlotRaw){return this.remember(owner,key,slotDraftOf(raw));}
  /** Accepted authoritative parcel changes must dirty a pending full draft too. */
  syncParcelDraft(owner:MailOwner,attachmentUniqueIds:readonly number[],stamped:boolean){
    if(!validOwner(owner))return false;this.sync(owner);const state=this.state();
    if(!state||!this.incarnation){const draft=this.draft(owner);return draft?this.remember(owner,'parcel-unavailable',{...draft,attachmentUniqueIds:[...attachmentUniqueIds],stamped}):false;}
    if(!sameSlotOwner(state.owner,slotOwnerOf(owner))||!state.draft||state.key===null)return false;
    const result=this.call({op:'remember',incarnation:this.incarnation,key:state.key,draft:{...state.draft,attachmentUniqueIds:attachmentUniqueIds.map(String),stamped,attachmentUniqueIdsPresent:true,stampedPresent:true}});
    return Boolean(result?.ok&&result.matched);
  }
  draft(owner:MailOwner|null){const state=this.state();if(state&&owner&&validOwner(owner)&&sameSlotOwner(state.owner,slotOwnerOf(owner))&&state.draft)return slotDraftOf(state.draft);
    const retained=owner&&validOwner(owner)?this.facade?.unavailableDraft(slotOwnerOf(owner)):null;if(retained)return slotDraftOf(retained);
    return sameMailOwner(this.offline?.owner,owner)&&this.offline?{...this.offline.draft,items:[...this.offline.draft.items],...(this.offline.draft.attachmentUniqueIds?{attachmentUniqueIds:[...this.offline.draft.attachmentUniqueIds]}:{})}:null;}
  /** Read-only exact Core proof for the common editor; never seeds another clock. */
  composeSnapshot(owner:MailOwner|null){const state=this.state();if(!state||!owner||!validOwner(owner)||!sameSlotOwner(state.owner,slotOwnerOf(owner))||
    !state.draft||!state.generation||!state.incarnation||!state.key||this.incarnation!==state.incarnation)return null;
    // Status and the optional projection come from the same cached Core facade in one synchronous stack.
    let gold:number|null=null;try{gold=this.facade?.draftGold?.()??null;}catch{/* An absent or old getter cannot paint an invented amount. */}
    return{raw:state.draft,generation:state.generation,incarnation:state.incarnation,key:state.key,flight:state.flight,gold};}
  pending(owner:MailOwner|null){const state=this.state();return Boolean(state&&owner&&validOwner(owner)&&sameSlotOwner(state.owner,slotOwnerOf(owner))&&state.flight);}
  reserve(owner:MailOwner,key:string,payload:MailWirePayload):MailComposeProof|null{
    this.sync(owner);if(!this.incarnation){this.localNotice='Shared mail send slot is unavailable; draft kept';return null;}
    const body=JSON.stringify({...payload,type:'sendMail'}),result=this.call({op:'reserve',incarnation:this.incarnation,key,payload:{...payload,itemsIdx:payload.itemsIdx.map(String)},body});
    if(!result?.ok||!result.matched||!result.state.flight){this.localNotice='Mail send authorization unavailable; draft kept';return null;}const f=result.state.flight;this.localNotice=null;
    return{owner:{...owner},key,body,serial:f.token,incarnation:f.incarnation,generation:f.generation,stream:{...f.stream}};
  }
  private proofCall(op:string,proof:MailComposeProof,body=proof.body){return this.call({op,incarnation:proof.incarnation,token:proof.serial,stream:proof.stream,body});}
  allows(proof:MailComposeProof){const result=this.proofCall('allows',proof);return Boolean(result?.ok&&result.matched);}
  enterSocket(proof:MailComposeProof,body:string,source?:object){const stream=source?this.acquire()?.streamFor(source):null;
    if(!stream||stream.run!==proof.stream.run||stream.connection!==proof.stream.connection)return false;const result=this.proofCall('enter',proof,body);return Boolean(result?.ok&&result.matched);}
  finish(proof:MailComposeProof,outcome:MailOutcome):MailOutcome{const result=this.proofCall('cancel',proof);if(result?.ok&&result.matched){this.localNotice=null;return 'definitelyUnsent';}
    if(!result?.ok)return 'outcomeUnknown';const flight=result.state.flight;if(flight?.token===proof.serial&&flight.entered){const actual=outcome==='definitelyUnsent'?'outcomeUnknown':outcome;if(actual==='outcomeUnknown')this.localNotice='Mail delivery is unknown; waiting for the server';return actual;}
    return outcome;}
  cancelUnsent(){const state=this.state(),f=state?.flight;if(f&&!f.entered&&f.incarnation===this.incarnation)this.proofCall('cancel',{owner:{connectionGeneration:0,sessionGeneration:0,ownerRevision:0,playerObjectId:0},key:f.key,body:f.body,serial:f.token,incarnation:f.incarnation,generation:f.generation,stream:f.stream});}
  acknowledge(_owner:MailOwner,result:number,source?:object):'success'|'failure'|'retired'|null{
    if(!source||(result!==1&&result!==-1))return null;const stream=this.acquire()?.streamFor(source);if(!stream)return null;
    const answer=this.call({op:'ack',incarnation:this.incarnation,stream,result});if(answer?.ok&&(answer.completion==='success'||answer.completion==='failure'))this.localNotice=null;return answer?.ok?answer.completion:null;
  }
  retireAcknowledgement(connectionGeneration:number,result:unknown,source?:object):'retired'|null{
    const state=this.state();
    if(!source||!Number.isSafeInteger(connectionGeneration)||connectionGeneration<=0||(result!==1&&result!==-1)||state?.owner!==null||state.flight?.owner.connectionGeneration!==String(connectionGeneration))return null;
    const answer=this.acknowledge({connectionGeneration,sessionGeneration:0,ownerRevision:0,playerObjectId:0},result,source);return answer==='retired'?'retired':null;
  }
  error(message:string){if(!this.state()?.flight)this.localNotice=message;}
}
export type MailSendProof={owner:MailOwner;serial:number;revision:number;commandType:MailCommandType;mailId:number|null;content:string|null;rust:MailIntent|null;lock:boolean|null;compose:MailComposeProof|null};
/** One Page wire owner, including compatibility actions. These are local proofs, never wire ACKs. */
export class MailDispatcher {
  readonly composer:MailComposeSender;
  private serial=0;private revision=0;private pending=new Set<MailSendProof>();
  constructor(private read:()=>{owner:MailOwner|null;mail:unknown;enabled:boolean},readSlot:()=>MailSendSlotRuntime|null=()=>null){this.composer=new MailComposeSender(readSlot);}
  prepare(commandType:MailCommandType,mailId:number|null,rust:MailIntent|null=null,compose:MailComposeProof|null=null):MailSendProof|null{const live=this.read(),mail=parseMailList(live.mail);
    if(!live.enabled||!live.owner||!validOwner(live.owner)||!mail||this.serial>=Number.MAX_SAFE_INTEGER||this.pending.size>=64)return null;
    const row=mailId===null?null:mail.find(m=>m.mailId===mailId);if(mailId!==null&&!row||commandType!=="sendMail"&&mailId===null||commandType==="collectParcel"&&row&&(row.collected||row.locked||row.gold===0&&row.items.length===0)||commandType==="deleteMail"&&row?.locked)return null;
    if(commandType==="lockMail"&&(!row||row.gold>0||row.items.length>0))return null;
    if(commandType==="sendMail"&&(!compose||!sameMailOwner(compose.owner,live.owner)||!this.composer.allows(compose)))return null;
    const p:MailSendProof={owner:{...live.owner},serial:++this.serial,revision:this.revision,commandType,mailId,content:row?mailContentKey(row):null,rust,lock:commandType==="lockMail"&&row?!row.locked:null,compose};this.pending.add(p);return p;}
  allows(p:MailSendProof){const live=this.read(),mail=parseMailList(live.mail),row=mail?.find(m=>m.mailId===p.mailId);
    if(!this.pending.has(p)||p.revision!==this.revision||!live.enabled||!live.owner||!sameMailOwner(live.owner,p.owner)||!mail)return false;
    if(p.mailId!==null&&(!row||mailContentKey(row)!==p.content))return false;
    if(p.commandType==="collectParcel"&&(!row||row.locked||row.collected||row.gold===0&&row.items.length===0))return false;
    if(p.commandType==="deleteMail"&&(!row||row.locked))return false;
    if(p.commandType==="lockMail"&&(!row||row.gold>0||row.items.length>0||p.lock!==!row.locked))return false;
    return p.commandType==="sendMail"?Boolean(p.compose&&this.composer.allows(p.compose)):Boolean(row);
  }
  claim(p:MailSendProof){if(!this.allows(p))return false;this.pending.delete(p);return true;}
  retire(p:MailSendProof){this.pending.delete(p);}
  withdraw(){this.pending.clear();this.composer.cancelUnsent();if(this.revision<Number.MAX_SAFE_INTEGER)++this.revision;else this.serial=Number.MAX_SAFE_INTEGER;}
}
