import type { BevyQuestUiPresentation } from "./bevy-quest-ui";
import type { HudPlayer } from "./bevy-hud-ui";
export type SpellsOwner = {connectionGeneration:number;sessionGeneration:number;ownerRevision:number;playerObjectId:number};
export type SpellsIdentity = SpellsOwner & {requestRun:number;hudGeneration:number};
export type SpellsRequest = {requestId:number;spell:string;key:number;oldKey:number};
export type SpellsIntent = SpellsIdentity & SpellsRequest & {skillId:number;modelRevision:number;presentationRevision:number;renderRevision:number};
export type SpellsProof = SpellsIntent;
export type SpellsOutcome = "confirmedSend"|"definitelyUnsent"|"outcomeUnknown";
export type SpellsTiming = {spell:string;sequence:number;observedAtMs:number;delayMs:number|null};
export type SpellsRawSnapshot = {playerObjectId:number|null;knownSkills:unknown[];skillKeyAck?:unknown};
type Receipt={serial:number;learned:unknown[];skillKeyAck:SpellsRequest&{accepted:boolean}};
export type SpellsSnapshot = SpellsIdentity & {revision:number;modelRevision:number;presentationRevision:number;open:boolean;inputEnabled:boolean;
  presentation:BevyQuestUiPresentation|null;player:HudPlayer;learned:unknown[];receipts:Receipt[];timing:SpellsTiming[];nowMs:number;iconMetadata:SpellsIconMetadata|null};
export type SpellsIconMetadata={version:number;frames:Array<{index:number;x:number;y:number;width:number;height:number}>};
type Region={left:number;top:number;width:number;height:number};
export type SpellsStatus = SpellsIdentity & {version:1;frame:number;ready:boolean;inputEnabled:boolean;modal:boolean;pending:boolean;renderRevision:number;
  appliedRevision:number;appliedModelRevision:number;appliedPresentationRevision:number;inputRegions:Region[];error:string|null};
export type SpellsPointerContext=SpellsIdentity & {modelRevision:number;presentationRevision:number;renderRevision:number;modal:boolean;inputRegions:Region[];presentation:BevyQuestUiPresentation};
export type SpellsPointerEdge=SpellsIdentity & {sequence:number;modelRevision:number;presentationRevision:number;renderRevision:number;pointerId:number;phase:"down"|"move"|"up"|"cancel";x:number;y:number;button:0};
export type SpellsRuntime={getMir2SpellsUiCapabilities?:()=>string;setMir2SpellsUiSnapshot?:(json:string)=>boolean;getMir2SpellsUiStatus?:()=>string;
  setMir2SpellsUiIntentSink?:(sink:(json:string)=>string)=>void;clearMir2SpellsUiIntentSink?:()=>void;setMir2SpellsUiPointerEdge?:(json:string)=>boolean};
export type SpellsInput={owner:SpellsOwner;hudGeneration:number;eligible:boolean;open:boolean;player:HudPlayer|null;presentation:BevyQuestUiPresentation|null;iconMetadata:SpellsIconMetadata|null};
const safe=(v:unknown):v is number=>Number.isSafeInteger(v)&&Number(v)>=0;
const object=(v:unknown):v is Record<string,unknown>=>Boolean(v)&&typeof v==="object"&&!Array.isArray(v);
const ownerKeys=["connectionGeneration","sessionGeneration","ownerRevision","playerObjectId"] as const;
const identityKeys=[...ownerKeys,"requestRun","hudGeneration"] as const;
const requestKeys=["requestId","spell","key","oldKey"] as const;
const BLOCK=2**22,MAX_RUN=2**31-1;
type SpellsRunAllocator={readonly version:1;readonly last:number;readonly poisoned:boolean;allocate:()=>number};
function createSpellsRunAllocator(initial=0):SpellsRunAllocator {
  if(!safe(initial)||initial>MAX_RUN)throw Error("Invalid Spells request namespace");
  let last=initial,poisoned=false;
  return Object.freeze({version:1 as const,get last(){return last;},get poisoned(){return poisoned;},allocate(){
    if(poisoned||last>=MAX_RUN){poisoned=true;throw Error("Spells request namespace exhausted");}
    const next=last+1;if(!safe(next)||next>MAX_RUN){poisoned=true;throw Error("Spells request namespace overflow");}
    last=next;return next;
  }});
}
export function requestRunOf(id:unknown):number|null{if(!safe(id)||id===0||id%BLOCK===0)return null;const run=Math.floor(id/BLOCK);return run>=1&&run<=MAX_RUN?run:null;}
export function burnSpellsRun():number {
  const key="__mir2SpellsRequestNamespaceV1";
  if(!Object.hasOwn(globalThis,key))Object.defineProperty(globalThis,key,{value:createSpellsRunAllocator(),writable:false,configurable:false});
  const slot=Object.getOwnPropertyDescriptor(globalThis,key),state=slot?.value as SpellsRunAllocator|undefined;
  // A writable legacy record cannot prove a burned high-water mark across HMR.
  if(!slot||slot.writable!==false||slot.configurable!==false||!state||!Object.isFrozen(state)||state.version!==1
    ||state.poisoned||!safe(state.last)||typeof state.allocate!=="function")throw Error("Spells request namespace exhausted/invalid");
  const before=state.last,next=state.allocate();
  if(!safe(next)||next!==before+1||next>MAX_RUN||state.last!==next)throw Error("Spells request namespace invalid allocation");
  return next;
}
function owners(){const realm=globalThis as typeof globalThis&{__mir2SpellsRuntimeOwnersV1?:WeakMap<object,number>};return realm.__mir2SpellsRuntimeOwnersV1??=new WeakMap();}
export function sameSpellsOwner(a:SpellsOwner,b:SpellsOwner){return ownerKeys.every(k=>a[k]===b[k]);}
function sameIdentity(a:SpellsIdentity,b:SpellsIdentity){return identityKeys.every(k=>a[k]===b[k]);}
function validOwner(v:Record<string,unknown>){return ownerKeys.every(k=>safe(v[k]))&&Number(v.connectionGeneration)>0&&Number(v.sessionGeneration)>0&&Number(v.playerObjectId)>0&&Number(v.playerObjectId)<=0xffffffff;}
function validIdentity(v:Record<string,unknown>){return validOwner(v)&&safe(v.hudGeneration)&&v.hudGeneration>0&&safe(v.requestRun)&&v.requestRun>=1&&v.requestRun<=MAX_RUN;}
function request(v:unknown):v is SpellsRequest&Record<string,unknown>{return object(v)&&requestKeys.every(k=>Object.hasOwn(v,k))&&requestRunOf(v.requestId)!==null&&typeof v.spell==="string"&&v.spell.length>0&&v.spell.length<=128&&safe(v.key)&&v.key<=16&&safe(v.oldKey)&&v.oldKey<=16;}
export function parseSpellsIntent(json:string):SpellsIntent|null{try{const v:unknown=JSON.parse(json);if(!object(v)||!validIdentity(v)||!request(v)||!safe(v.skillId)||v.skillId>0xffffffff||![v.modelRevision,v.presentationRevision,v.renderRevision].every(safe)||requestRunOf(v.requestId)!==v.requestRun||Object.keys(v).sort().join()!==[...identityKeys,...requestKeys,"skillId","modelRevision","presentationRevision","renderRevision"].sort().join())return null;return v as SpellsIntent;}catch{return null;}}
export function supportsSpells(r:SpellsRuntime|null){try{const c=JSON.parse(r?.getMir2SpellsUiCapabilities?.()??"null");return Boolean(c&&Object.keys(c).sort().join()==="compiled,schemaVersion,spellsIntentAbiVersion,spellsPageAbiVersion,startup"&&c.schemaVersion===1&&c.spellsPageAbiVersion===1&&c.spellsIntentAbiVersion===1&&c.compiled===true&&c.startup===true&&[r?.setMir2SpellsUiSnapshot,r?.getMir2SpellsUiStatus,r?.setMir2SpellsUiIntentSink,r?.clearMir2SpellsUiIntentSink,r?.setMir2SpellsUiPointerEdge].every(f=>typeof f==="function"));}catch{return false;}}
export function fitsSpells(p:BevyQuestUiPresentation|null){return Boolean(p&&[p.logicalWidth,p.logicalHeight,p.stageCssScale].every(Number.isFinite)&&p.logicalWidth>=1024&&p.logicalWidth<=16384&&p.logicalHeight>=768&&p.logicalHeight<=16384&&p.stageCssScale>0&&p.stageCssScale<=16&&(!p.touch||p.stageCssScale*32>=44&&p.stageCssScale*13>=44&&p.stageCssScale*14>=44));}
export function parseSpellsIconMetadata(value:unknown):SpellsIconMetadata|null{if(!object(value)||!safe(value.version)||!Array.isArray(value.frames)||value.frames.length>512)return null;const ids=new Set<number>();const frames:SpellsIconMetadata["frames"]=[];for(const raw of value.frames){if(!object(raw)||!safe(raw.index)||raw.index>511||ids.has(raw.index)||![raw.x,raw.y].every(n=>Number.isInteger(n)&&Number(n)>=-2147483648&&Number(n)<=2147483647)||![raw.width,raw.height].every(n=>safe(n)&&n<=65535))return null;ids.add(raw.index);frames.push({index:raw.index,x:Number(raw.x),y:Number(raw.y),width:Number(raw.width),height:Number(raw.height)});}return{version:value.version,frames};}
export function readSpellsStatus(r:SpellsRuntime):SpellsStatus|null{try{const v:unknown=JSON.parse(r.getMir2SpellsUiStatus?.()??"null");if(!object(v)||!validIdentity(v)||v.version!==1||![v.frame,v.renderRevision,v.appliedRevision,v.appliedModelRevision,v.appliedPresentationRevision].every(safe)||![v.ready,v.inputEnabled,v.modal,v.pending].every(n=>typeof n==="boolean")||!(v.error===null||typeof v.error==="string")||!Array.isArray(v.inputRegions)||!v.inputRegions.every(r=>object(r)&&[r.left,r.top,r.width,r.height].every(n=>typeof n==="number"&&Number.isFinite(n))&&Number(r.width)>0&&Number(r.height)>0))return null;return v as SpellsStatus;}catch{return null;}}
type Options={runtime:SpellsRuntime;isCurrent:()=>boolean;read:()=>SpellsInput;now:()=>number;onState:(ready:boolean)=>void;onIntent:(intent:SpellsIntent,proof:SpellsProof)=>SpellsOutcome};
/** Thin transport/ownership mailbox. All skill interaction state remains in Rust. */
export class SpellsHost {
  readonly run=burnSpellsRun();private stopped=false;private active=false;private revision=0;private modelRevision=0;private presentationRevision=0;
  private sent:SpellsSnapshot|null=null;private owner:SpellsOwner|null=null;private raw:unknown[]|null=null;private rawOwner:SpellsOwner|null=null;
  private receipts:Receipt[]=[];private serial=0;private overflow=false;private modelKey="";private presentationKey="";private frame=-1;private frameAt=0;
  private castSequence=0;private timing=new Map<string,SpellsTiming>();private pending=new Map<number,{intent:SpellsIntent;owner:SpellsOwner}>();
  private requestCounter=0;
  constructor(private options:Options){try{if(supportsSpells(options.runtime)){owners().set(options.runtime,this.run);options.runtime.setMir2SpellsUiIntentSink?.(json=>{
    const i=parseSpellsIntent(json);if(!i||!this.allows(i)||i.requestId%BLOCK<=this.requestCounter||this.pending.has(i.requestId)||this.pending.size>=64)return JSON.stringify({outcome:"definitelyUnsent"});
    this.requestCounter=i.requestId%BLOCK;
    this.pending.set(i.requestId,{intent:i,owner:{...this.owner!}});
    let outcome:SpellsOutcome;try{outcome=this.options.onIntent(i,{...i});}catch{outcome="outcomeUnknown";}
    if(outcome==="definitelyUnsent")this.pending.delete(i.requestId);
    if(!["confirmedSend","definitelyUnsent","outcomeUnknown"].includes(outcome))outcome="outcomeUnknown";
    return JSON.stringify({outcome});
  });}}catch{this.stop();}}
  private owns(){return owners().get(this.options.runtime)===this.run;}
  private qualify(status:SpellsStatus|null){const s=this.sent;return Boolean(s&&status&&status.ready&&status.inputEnabled&&status.error===null&&sameIdentity(s,status)&&status.appliedRevision===s.revision&&status.appliedModelRevision===s.modelRevision&&status.appliedPresentationRevision===s.presentationRevision);}
  /** Called before React/movement coalescing, with connection + confirmed session captured by Page. */
  observeSnapshot(snapshot:SpellsRawSnapshot,owner:SpellsOwner){
    if(this.stopped||!this.owns()||!this.options.isCurrent()||!validOwner(owner))return false;
    const live=this.options.read();if(!sameSpellsOwner(owner,live.owner))return false;
    if(snapshot.playerObjectId!==owner.playerObjectId)return false;
    if(!Array.isArray(snapshot.knownSkills)||snapshot.knownSkills.length>512){
      // A current capture failed: old rows cannot regain authority on the next timer tick.
      // Pending receipts and burned request watermarks survive until a complete capture.
      this.raw=null;this.withdraw();return false;
    }
    if(this.rawOwner&&!sameSpellsOwner(this.rawOwner,owner)){this.receipts=[];this.timing.clear();this.pending.clear();this.raw=null;this.overflow=false;}
    this.rawOwner={...owner};this.raw=JSON.parse(JSON.stringify(snapshot.knownSkills));
    const a=snapshot.skillKeyAck;
    if(request(a)&&object(a)&&typeof a.accepted==="boolean"&&requestRunOf(a.requestId)===this.run){
      const p=this.pending.get(a.requestId);if(p&&sameSpellsOwner(p.owner,owner)&&requestKeys.every(k=>p.intent[k]===a[k])){
        if(this.receipts.length>=64){this.overflow=true;this.withdraw();return false;}
        this.receipts.push({serial:++this.serial,learned:JSON.parse(JSON.stringify(this.raw)),skillKeyAck:{requestId:a.requestId,spell:a.spell,key:a.key,oldKey:a.oldKey,accepted:a.accepted}});this.pending.delete(a.requestId);
      }
    }
    this.tick();return true;
  }
  observePacket(packet:string,payload:Record<string,unknown>,owner:SpellsOwner){
    if(this.stopped||!this.rawOwner||!sameSpellsOwner(owner,this.rawOwner)||!sameSpellsOwner(owner,this.options.read().owner)||!this.owns()||!this.options.isCurrent())return false;
    const spell=payload.spell;if(typeof spell!=="string"||!spell||spell.length>128)return false;
    if(payload.objectId!==undefined&&Number(payload.objectId)!==owner.playerObjectId)return false;
    const at=Math.floor(this.options.now());if(!safe(at))return false;
    if(packet==="MagicCast"||packet==="Magic"){
      if(this.castSequence===Number.MAX_SAFE_INTEGER||this.timing.size>=512&&!this.timing.has(spell)){this.overflow=true;this.withdraw();return false;}
      const old=this.timing.get(spell);this.timing.set(spell,{spell,sequence:++this.castSequence,observedAtMs:at,delayMs:old?.delayMs??null});
    }else if(packet==="MagicDelay"){
      if(!safe(payload.delay)||payload.delay>0xffffffff)return false;
      const old=this.timing.get(spell);this.timing.set(spell,old?{...old,delayMs:payload.delay}:{spell,sequence:0,observedAtMs:at,delayMs:payload.delay});
    }else return false;
    this.tick();return true;
  }
  tick(){if(this.stopped)return;try{
    if(!this.owns()||!this.options.isCurrent()){this.stop();return;}
    const input=this.options.read();
    if(!supportsSpells(this.options.runtime)||!validOwner(input.owner)||!input.player||!this.raw||!this.rawOwner||!sameSpellsOwner(input.owner,this.rawOwner)||this.overflow){this.withdraw();return;}
    const open=input.eligible&&input.open&&fitsSpells(input.presentation);
    const modelKey=JSON.stringify([this.raw,input.player,[...this.timing.values()]]),presentationKey=JSON.stringify([input.owner,input.hudGeneration,input.presentation,input.iconMetadata,open]);
    const status=readSpellsStatus(this.options.runtime);
    if(status&&status.frame!==this.frame){this.frame=status.frame;this.frameAt=this.options.now();}
    // Timing is sampled every tick without inventing a model revision or cast sequence.
    if(modelKey!==this.modelKey||presentationKey!==this.presentationKey||this.receipts.length||!this.sent){
      if(modelKey!==this.modelKey)++this.modelRevision;if(presentationKey!==this.presentationKey)++this.presentationRevision;
      this.modelKey=modelKey;this.presentationKey=presentationKey;this.owner={...input.owner};
      const s:SpellsSnapshot={...input.owner,requestRun:this.run,hudGeneration:input.hudGeneration,revision:++this.revision,modelRevision:this.modelRevision,presentationRevision:this.presentationRevision,
        open,inputEnabled:open,presentation:input.presentation,player:input.player,learned:this.raw,receipts:this.receipts,timing:[...this.timing.values()],nowMs:Math.floor(this.options.now()),iconMetadata:input.iconMetadata};
      if(!this.options.runtime.setMir2SpellsUiSnapshot?.(JSON.stringify(s))){this.withdraw();return;}
      this.sent=s;this.receipts=[];
    }
    this.active=this.qualify(readSpellsStatus(this.options.runtime))&&this.options.now()-this.frameAt<=500;
    this.options.onState(this.active);
  }catch{this.withdraw();}}
  allows(i:SpellsProof){if(!this.owns()||!this.options.isCurrent()||this.stopped||!this.active||!this.sent||!this.owner||!sameIdentity(i,this.sent)||i.modelRevision!==this.sent.modelRevision||i.presentationRevision!==this.sent.presentationRevision||requestRunOf(i.requestId)!==this.run)return false;
    const rows=this.raw??[],identified=rows.filter((row,index)=>object(row)&&(Object.hasOwn(row,"id")?row.id:index)===i.skillId);
    if(identified.length!==1||!object(identified[0])||identified[0].spell!==i.spell||rows.filter(row=>object(row)&&row.spell===i.spell).length!==1)return false;
    const live=this.options.read(),status=readSpellsStatus(this.options.runtime);
    return Boolean(live.eligible&&live.open&&sameSpellsOwner(live.owner,this.owner)&&fitsSpells(live.presentation)&&live.hudGeneration===this.sent.hudGeneration&&JSON.stringify([this.raw,live.player,[...this.timing.values()]])===this.modelKey&&JSON.stringify([live.owner,live.hudGeneration,live.presentation,live.iconMetadata,true])===this.presentationKey&&this.options.now()-this.frameAt<=500&&this.qualify(status)&&status?.renderRevision===i.renderRevision);
  }
  pointerContext():SpellsPointerContext|null{const s=this.sent,status=readSpellsStatus(this.options.runtime),live=this.options.read();if(!s?.presentation||!this.active||!this.qualify(status)||!this.owns()||!this.options.isCurrent()||!live.eligible||!live.open||!this.owner||!sameSpellsOwner(this.owner,live.owner)||this.options.now()-this.frameAt>500)return null;return{...this.identity(s),modelRevision:s.modelRevision,presentationRevision:s.presentationRevision,renderRevision:status!.renderRevision,modal:status!.modal,inputRegions:status!.inputRegions,presentation:s.presentation};}
  private identity(s:SpellsIdentity):SpellsIdentity{return{requestRun:s.requestRun,connectionGeneration:s.connectionGeneration,sessionGeneration:s.sessionGeneration,ownerRevision:s.ownerRevision,hudGeneration:s.hudGeneration,playerObjectId:s.playerObjectId};}
  pointer(e:SpellsPointerEdge){try{if(e.phase!=="cancel"&&!this.pointerContext())return false;return this.options.runtime.setMir2SpellsUiPointerEdge?.(JSON.stringify(e))===true;}catch{this.withdraw();return false;}}
  withdraw(){this.active=false;this.options.onState(false);const s=this.sent;this.sent=null;this.owner=null;if(s&&this.owns())try{this.options.runtime.setMir2SpellsUiSnapshot?.(JSON.stringify({...s,revision:++this.revision,open:false,inputEnabled:false,receipts:[]}));}catch{/* Captured runtime has retired. */}}
  stop(){this.withdraw();this.stopped=true;if(this.owns()){owners().delete(this.options.runtime);try{this.options.runtime.clearMir2SpellsUiIntentSink?.();}catch{/* Retired. */}}}
}
export class SpellsPointerRouter {
  held:{context:SpellsPointerContext;pointerId:number;invalid:boolean}|null=null;private sequence=0;
  matches(c:SpellsPointerContext|null){const h=this.held;return Boolean(h&&c&&sameIdentity(h.context,c)&&h.context.modelRevision===c.modelRevision&&h.context.presentationRevision===c.presentationRevision&&h.context.renderRevision===c.renderRevision);}
  contains(c:SpellsPointerContext,x:number,y:number){return c.inputRegions.some(r=>x>=r.left&&y>=r.top&&x<r.left+r.width&&y<r.top+r.height);}
  down(c:SpellsPointerContext,pointerId:number,x:number,y:number){if(this.held||!this.contains(c,x,y))return null;this.held={context:c,pointerId,invalid:false};return this.edge("down",pointerId,x,y);}
  edge(phase:SpellsPointerEdge["phase"],pointerId:number,x:number,y:number):SpellsPointerEdge|null{const h=this.held;if(!h||h.pointerId!==pointerId)return null;if(!this.contains(h.context,x,y))h.invalid=true;const c=h.context;const wire:SpellsPointerEdge={requestRun:c.requestRun,connectionGeneration:c.connectionGeneration,sessionGeneration:c.sessionGeneration,ownerRevision:c.ownerRevision,hudGeneration:c.hudGeneration,playerObjectId:c.playerObjectId,modelRevision:c.modelRevision,presentationRevision:c.presentationRevision,renderRevision:c.renderRevision,sequence:++this.sequence,pointerId,phase:h.invalid?"cancel":phase,x,y,button:0};if(phase==="up"||phase==="cancel"||h.invalid)this.held=null;return wire;}
  cancel(){const h=this.held;return h?this.edge("cancel",h.pointerId,0,0):null;}
}
