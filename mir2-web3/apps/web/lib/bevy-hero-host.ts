/** Display/input ownership only. Page retains the only Hero operation ledger. */
import {
  bindHeroUiControl, readHeroSourceWitness, readHeroUiFrame, readHeroUiStatus,
  sameHeroHighWater, sameHeroPhysical, sameHeroScope, sameHeroSource, supportsHeroSharedUi,
  type HeroInputRegion, type HeroRawHighWater, type HeroRawIngress, type HeroRuntime,
  type HeroScope, type HeroSource, type HeroSourceWitness, type HeroUiStatus,
} from "./bevy-hero-ui";
import {
  captureHeroActionBasis, heroActionBasisMatches,
  type HeroActionBasis, type HeroBasisWindows, type HeroCell, type HeroPlayerModel, type HeroUiAction,
} from "./hero-player-ui";

export type HeroPresentation = Readonly<{logicalWidth:number;logicalHeight:number;stageCssScale:number;touch:boolean}>;
export type HeroWindowEpochs = Readonly<{inventory:number;character:number;belt:number}>;
export type HeroUiOrigin = "inventory" | "character" | "belt";
export type HeroStamp = Readonly<{scope:HeroScope;heroObjectId:number;heroGeneration:number;hudGeneration:number;
  modelRevision:number;presentationRevision:number;windowEpochs:readonly [number,number,number]}>;
export type HeroHostRead = Readonly<{
  ingress:HeroRawIngress; model:HeroPlayerModel|null; windows:HeroBasisWindows; windowEpochs:HeroWindowEpochs;
  hudGeneration:number; presentation:HeroPresentation|null; pending:boolean; inputAllowed:boolean; eligible:boolean;
}>;
type HeroSharedProof = Readonly<{
  model:HeroPlayerModel; source:HeroSource; witness:string; highWater:HeroRawHighWater;
  sourceWindows:HeroBasisWindows; windowEpochs:HeroWindowEpochs; presentation:HeroPresentation;
  controlRevision:number; sinkGeneration:number; webLeaseToken:string; intentSequence:number; stamp:HeroStamp;
}>;
export type HeroSharedIntent = HeroSharedProof & (
  | Readonly<{type:"action";origin:HeroUiOrigin;action:HeroUiAction;actionBasis:HeroActionBasis;oldKey:number|null}>
  | Readonly<{type:"windows";windows:HeroBasisWindows}>
);
export type HeroPointerContext = Readonly<{
  webLeaseToken:string; sinkGeneration:number; stamp:HeroStamp; presentation:HeroPresentation;
  /** False only during an explicitly continuous, identical-data source refresh. */
  ready:boolean; inputRegions:readonly HeroInputRegion[];
}>;
export type HeroPointerEdge = Readonly<{
  context:HeroPointerContext;pointerId:number;phase:"down"|"move"|"up"|"cancel";
  x:number;y:number;button:0|2;control?:boolean;shift?:boolean;
}>;
export type HeroKeyEdge = Readonly<{
  key:string;code?:string;text?:string;control?:boolean;shift?:boolean;alt?:boolean;meta?:boolean;repeat?:boolean;
}>;
export type HeroHostState = Readonly<{active:boolean;transitioning:boolean;owned:boolean;worldBlocked:boolean;webLeaseToken:string|null;error:string|null}>;
export type HeroHostOptions = Readonly<{
  runtime:HeroRuntime;now:()=>number;isCurrent?:()=>boolean;read:()=>HeroHostRead;
  /** Must synchronously retire the old DOM token before returning a non-null owner. */
  onOwner:(webLeaseToken:string|null)=>void;
  /** True means accepted by Page's transport path; it is never an ACK. */
  onIntent:(intent:HeroSharedIntent)=>boolean;onState?:(state:HeroHostState)=>void;
}>;

const HEALTH_MS=500, ENABLE_TIMEOUT_MS=2000, MAX_INTENT_BYTES=65536;
const uint=(v:unknown,max=Number.MAX_SAFE_INTEGER):v is number=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0&&v<=max;
const positive=(v:unknown):v is number=>uint(v)&&v>0;
const row=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==="object"&&!Array.isArray(v);
const fields=(v:Record<string,unknown>,names:readonly string[])=>Object.keys(v).sort().join() === [...names].sort().join();
const bytes=(v:string)=>new TextEncoder().encode(v).byteLength;
const sameWindows=(a:HeroBasisWindows,b:HeroBasisWindows)=>a.inventoryOpen===b.inventoryOpen&&a.characterOpen===b.characterOpen
  &&a.characterPage===b.characterPage&&a.beltVisible===b.beltVisible&&a.beltVertical===b.beltVertical;
const sameOwner=(a:HeroPlayerModel["owner"],b:HeroPlayerModel["owner"])=>sameHeroPhysical(a,b)&&a.sceneRevision===b.sceneRevision&&a.playerObjectId===b.playerObjectId&&a.mapFileName===b.mapFileName;
const sameEpochs=(a:HeroWindowEpochs,b:HeroWindowEpochs)=>a.inventory===b.inventory&&a.character===b.character&&a.belt===b.belt;
const samePresentation=(a:HeroPresentation,b:HeroPresentation)=>a.logicalWidth===b.logicalWidth&&a.logicalHeight===b.logicalHeight
  &&a.stageCssScale===b.stageCssScale&&a.touch===b.touch;
const windowKeys=["inventoryOpen","characterOpen","characterPage","beltVisible","beltVertical"] as const;
const stampKeys=["scope","heroObjectId","heroGeneration","hudGeneration","modelRevision","presentationRevision","windowEpochs"] as const;
function copyWindows(value:unknown,strict=false):HeroBasisWindows|null {
  if(!row(value)||strict&&!fields(value,windowKeys)||typeof value.inventoryOpen!=="boolean"
    ||typeof value.characterOpen!=="boolean"||typeof value.beltVisible!=="boolean"||typeof value.beltVertical!=="boolean"
    ||!["equipment","status","state","skills"].includes(String(value.characterPage)))return null;
  return Object.freeze({inventoryOpen:value.inventoryOpen,characterOpen:value.characterOpen,
    characterPage:value.characterPage as HeroBasisWindows["characterPage"],beltVisible:value.beltVisible,beltVertical:value.beltVertical});
}
export function heroPresentationFits(value:HeroPresentation|null):value is HeroPresentation {
  return !!value&&uint(value.logicalWidth,16384)&&value.logicalWidth>=1024&&uint(value.logicalHeight,16384)&&value.logicalHeight>=768
    &&typeof value.touch==="boolean"&&Number.isFinite(value.stageCssScale)&&value.stageCssScale>0&&value.stageCssScale<=16
    &&(!value.touch||value.stageCssScale*14>=44&&value.stageCssScale*8>=14);
}
function copyPlain<T>(value:T,depth=0):T {
  if(depth>64)throw Error("Hero model nesting exceeds bound");
  if(Array.isArray(value))return Object.freeze(value.map(v=>copyPlain(v,depth+1))) as T;
  if(row(value))return Object.freeze(Object.fromEntries(Object.entries(value).map(([k,v])=>[k,copyPlain(v,depth+1)]))) as T;
  if(value===null||value===undefined||typeof value==="string"||typeof value==="boolean"||typeof value==="number"&&Number.isFinite(value))return value;
  throw Error("Hero model is not immutable data");
}
function frozenModel(model:HeroPlayerModel):HeroPlayerModel {
  // The external socket is an identity, never cloned, serialized or frozen.
  const {owner,...data}=model;
  return Object.freeze({...copyPlain(data),owner:Object.freeze({...owner})});
}
function sourceShape(value:unknown):value is HeroSource {
  if(!row(value)||!fields(value,["scope","frameSequence","heroObjectId","heroGeneration","rustModelRevision"])
    ||!row(value.scope)||!fields(value.scope,["runGeneration","connectionGeneration","sessionGeneration","sceneRevision","playerObjectId","mapFileName"]))return false;
  const s=value.scope;
  return [s.runGeneration,s.connectionGeneration,s.sessionGeneration,s.sceneRevision,s.playerObjectId,
    value.frameSequence,value.heroObjectId,value.heroGeneration,value.rustModelRevision].every(positive)
    &&uint(s.playerObjectId,0xffffffff)&&uint(value.heroObjectId,0xffffffff)
    &&typeof s.mapFileName==="string"&&s.mapFileName.length>0&&bytes(s.mapFileName)<=512&&!s.mapFileName.includes("\0");
}
function sameStamp(a:HeroStamp,b:HeroStamp):boolean {
  return sameHeroScope(a.scope,b.scope)&&a.heroObjectId===b.heroObjectId&&a.heroGeneration===b.heroGeneration
    &&a.hudGeneration===b.hudGeneration&&a.modelRevision===b.modelRevision&&a.presentationRevision===b.presentationRevision
    &&a.windowEpochs.every((n,i)=>n===b.windowEpochs[i]);
}
function stampMatches(value:Record<string,unknown>,stamp:HeroStamp):boolean {
  return row(value.scope)&&sourceShape({scope:value.scope,frameSequence:1,heroObjectId:stamp.heroObjectId,heroGeneration:1,rustModelRevision:1})
    &&Array.isArray(value.windowEpochs)&&value.windowEpochs.length===3&&value.windowEpochs.every(v=>uint(v))
    &&sameStamp(value as unknown as HeroStamp,stamp);
}
const originOpen=(origin:HeroUiOrigin,windows:HeroBasisWindows)=>origin==="inventory"?windows.inventoryOpen:origin==="character"?windows.characterOpen:windows.beltVisible;
function cell(value:unknown):HeroCell|null {
  return row(value)&&fields(value,["grid","slot"])&&["HeroInventory","HeroEquipment","Inventory"].includes(String(value.grid))&&uint(value.slot,0xffffffff)
    ?Object.freeze({grid:value.grid as HeroCell["grid"],slot:value.slot}):null;
}
/** Rust Remove.to is explicitly null for automatic placement; the TS planner
 * uses absence. No other missing field, coercion or guessed action is accepted. */
export function parseHeroSemanticAction(value:unknown):HeroUiAction|null {
  if(!row(value)||typeof value.kind!=="string")return null;
  const pair=()=>fields(value,["kind","from","to"])&&uint(value.from,0xffffffff)&&uint(value.to,0xffffffff);
  switch(value.kind){
    case "move":case "equip":case "transfer":case "takeBack":return pair()?Object.freeze({kind:value.kind,from:value.from as number,to:value.to as number}):null;
    case "remove":return fields(value,["kind","from","to"])&&uint(value.from,0xffffffff)&&(value.to===null||uint(value.to,0xffffffff))
      ?Object.freeze({kind:"remove",from:value.from,...(value.to===null?{}:{to:value.to})}):null;
    case "merge":{const from=cell(value.from),to=cell(value.to);return fields(value,["kind","from","to"])&&from&&to?Object.freeze({kind:"merge",from,to}):null;}
    case "use":return fields(value,["kind","slot","confirmed"])&&uint(value.slot,0xffffffff)&&typeof value.confirmed==="boolean"
      ?Object.freeze({kind:"use",slot:value.slot,confirmed:value.confirmed}):null;
    case "autoPotValue":return fields(value,["kind","stat","value"])&&(value.stat===12||value.stat===13)&&uint(value.value,99)
      ?Object.freeze({kind:"autoPotValue",stat:value.stat,value:value.value}):null;
    case "autoPotItem":return fields(value,["kind","grid","slot"])&&(value.grid==="HeroHpItem"||value.grid==="HeroMpItem")&&(value.slot===null||uint(value.slot,0xffffffff))
      ?Object.freeze({kind:"autoPotItem",grid:value.grid,slot:value.slot}):null;
    case "magicKey":return fields(value,["kind","spell","key"])&&typeof value.spell==="string"&&value.spell.length>0&&bytes(value.spell)<=256&&!value.spell.includes("\0")
      &&(value.key===0||uint(value.key,24)&&value.key>=17)?Object.freeze({kind:"magicKey",spell:value.spell,key:value.key}):null;
    default:return null;
  }
}
function navigationAllowed(before:HeroBasisWindows,after:HeroBasisWindows):boolean {
  const changed=windowKeys.filter(key=>before[key]!==after[key]);if(changed.length!==1)return false;
  switch(changed[0]){
    case "inventoryOpen":return before.inventoryOpen&&!after.inventoryOpen;
    case "characterOpen":return before.characterOpen&&!after.characterOpen;
    case "beltVisible":return before.beltVisible&&!after.beltVisible;
    case "characterPage":return before.characterOpen;
    case "beltVertical":return before.beltVisible;
  }
  return false;
}

type Clock={control:number;model:number;presentation:number;input:number};
const runtimeClocks=new WeakMap<HeroRuntime,Clock>();let nextLease=0;
function clockFor(runtime:HeroRuntime):Clock {let c=runtimeClocks.get(runtime);if(!c){c={control:0,model:0,presentation:0,input:0};runtimeClocks.set(runtime,c);}return c;}
function advance(clock:Clock,key:keyof Clock):number {const n=clock[key]+1;if(!positive(n))throw Error("Hero UI counter exhausted");clock[key]=n;return n;}
function leaseToken():string {if(!positive(++nextLease))throw Error("Hero owner counter exhausted");return "hero-shared:"+nextLease;}
type Snapshot=Readonly<{model:HeroPlayerModel;binding:HeroSourceWitness;highWater:HeroRawHighWater;windows:HeroBasisWindows;
  epochs:HeroWindowEpochs;hudGeneration:number;presentation:HeroPresentation;pending:boolean;ingress:HeroRawIngress}>;
type Control=Readonly<{source:HeroSource;controlRevision:number;webLeaseToken:string;hudGeneration:number;webModelRevision:number;
  presentationRevision:number;windowEpochs:readonly [number,number,number];windows:HeroBasisWindows;presentation:HeroPresentation;
  inGame:boolean;hostVisible:boolean;inputEnabled:boolean;pending:boolean}>;
type Bound=Readonly<{snapshot:Snapshot;control:Control;stamp:HeroStamp}>;
type Installation={scope:HeroScope;generation:number;token:string};
type HeldPointer={pointerId:number;button:0|2;context:HeroPointerContext;lineage:number};
function interactionEqual(a:Snapshot,b:Snapshot):boolean {
  return sameHeroScope(a.binding.source.scope,b.binding.source.scope)&&a.binding.source.heroObjectId===b.binding.source.heroObjectId
    &&a.binding.source.heroGeneration===b.binding.source.heroGeneration&&a.binding.witness===b.binding.witness
    &&a.model.sourceKey===b.model.sourceKey&&sameHeroPhysical(a.model.owner,b.model.owner)
    &&sameWindows(a.windows,b.windows)&&sameEpochs(a.epochs,b.epochs)&&a.hudGeneration===b.hudGeneration
    &&samePresentation(a.presentation,b.presentation)&&a.pending===b.pending;
}
function controlDataEqual(a:Snapshot,b:Snapshot):boolean {return interactionEqual(a,b)&&sameHeroSource(a.binding.source,b.binding.source);}

export class BevyHeroHost {
  private readonly clock:Clock;
  private stopped=false;private ticking=false;private shared=false;private installation:Installation|null=null;private bound:Bound|null=null;
  private state:HeroHostState=Object.freeze({active:false,transitioning:false,owned:false,worldBlocked:false,webLeaseToken:null,error:null});
  private lineage=0;private contexts=new WeakMap<HeroPointerContext,number>();private issued=new WeakSet<HeroSharedIntent>();
  private held:HeldPointer|null=null;private intentHigh=0;private pendingEdgeFrame:number|null=null;
  private lastReady:Readonly<{status:HeroUiStatus;lineage:number;at:number}>|null=null;private continuous=false;
  private lastFrame=0;private advanced=false;private lastAdvanceAt=0;private lastNow=0;private enabledAt=0;
  private contentKey:string|null=null;private geometryKey:string|null=null;private modelRevision=0;private presentationRevision=0;
  constructor(private readonly options:HeroHostOptions){this.clock=clockFor(options.runtime);}

  /** Pure cached render projection; it never samples time, source or runtime. */
  peek():HeroHostState {return this.state;}
  private now():number {const now=this.options.now();if(!Number.isFinite(now)||now<0||now<this.lastNow)throw Error("Hero UI clock unavailable");this.lastNow=now;return now;}
  private pulse():boolean {
    const now=this.now(),frame=readHeroUiFrame(this.options.runtime);
    if(!frame)return false;
    this.clock.input=Math.max(this.clock.input,frame.acceptedInputSequence);this.clock.control=Math.max(this.clock.control,frame.controlRevision);
    if(frame.frame<this.lastFrame)throw Error("Hero UI frame regressed");
    if(frame.frame>this.lastFrame){if(this.lastFrame>0)this.advanced=true;this.lastFrame=frame.frame;this.lastAdvanceAt=now;}
    if(this.pendingEdgeFrame!==null&&frame.frame>this.pendingEdgeFrame){this.pendingEdgeFrame=null;this.emitProjection();}
    return this.advanced&&now-this.lastAdvanceAt<=HEALTH_MS;
  }
  private factsMatch(snapshot:Snapshot,ignorePending:boolean,exactHighWater:boolean):boolean {
    if(this.stopped||this.options.isCurrent?.()===false)return false;
    const read=this.options.read(),windows=copyWindows(read.windows);
    return !!read.eligible&&!!read.inputAllowed&&!!read.model&&!!windows&&!!read.windowEpochs&&heroPresentationFits(read.presentation)
      &&sameOwner(read.model.owner,snapshot.model.owner)&&read.model.sourceKey===snapshot.model.sourceKey
      &&sameWindows(windows,snapshot.windows)&&sameEpochs(read.windowEpochs,snapshot.epochs)
      &&read.hudGeneration===snapshot.hudGeneration&&samePresentation(read.presentation,snapshot.presentation)
      &&(ignorePending||read.pending===snapshot.pending)&&read.ingress===snapshot.ingress
      &&(!exactHighWater||sameHeroHighWater(read.ingress.highWater(),snapshot.highWater));
  }
  private frameFresh():boolean {return this.advanced&&this.lastFrame>0&&this.now()-this.lastAdvanceAt<=HEALTH_MS;}
  private readSnapshot():Snapshot|null {
    if(this.stopped||this.options.isCurrent?.()===false)return null;
    const read=this.options.read(),windows=copyWindows(read.windows);
    if(!read.eligible||!read.inputAllowed||!read.model||!windows||!positive(read.hudGeneration)
      ||!(windows.inventoryOpen||windows.characterOpen||windows.beltVisible)||!heroPresentationFits(read.presentation)
      ||typeof read.pending!=="boolean"||!read.windowEpochs||![read.windowEpochs.inventory,read.windowEpochs.character,read.windowEpochs.belt].every(v=>uint(v)))return null;
    const high=read.ingress.highWater();if(!high||!read.ingress.caughtUp()||!sameHeroPhysical(high,read.model.owner))return null;
    const model=frozenModel(read.model),binding=readHeroSourceWitness(this.options.runtime,model),bootstrap=read.ingress.bootstrap();
    if(!binding||!bootstrap?.source||bootstrap.closed||!sameHeroSource(bootstrap.source,binding.source)
      ||bootstrap.acceptedFrameSequence<high.sequence||!sameHeroHighWater(high,read.ingress.highWater())||!read.ingress.caughtUp())return null;
    const snapshot=Object.freeze({model,binding,highWater:Object.freeze({...high}),windows,epochs:Object.freeze({...read.windowEpochs}),
      hudGeneration:read.hudGeneration,presentation:Object.freeze({...read.presentation}),pending:read.pending,ingress:read.ingress});
    return this.factsMatch(snapshot,false,true)?snapshot:null;
  }
  private liveMatches(snapshot:Snapshot,ignorePending:boolean,exactHighWater:boolean):Snapshot|null {
    const now=this.readSnapshot();if(!now||!sameHeroSource(snapshot.binding.source,now.binding.source)||snapshot.binding.witness!==now.binding.witness
      ||snapshot.model.sourceKey!==now.model.sourceKey||!sameHeroPhysical(snapshot.model.owner,now.model.owner)
      ||snapshot.hudGeneration!==now.hudGeneration||!sameWindows(snapshot.windows,now.windows)||!sameEpochs(snapshot.epochs,now.epochs)
      ||!samePresentation(snapshot.presentation,now.presentation)||!ignorePending&&snapshot.pending!==now.pending
      ||exactHighWater&&!sameHeroHighWater(snapshot.highWater,now.highWater))return null;
    return now;
  }
  private emitProjection():void {this.emitState(this.state.active,this.state.transitioning,this.state.error);}
  private emitState(active:boolean,transitioning:boolean,error:string|null=null):void {
    // Cached render data only: never call read, now, WASM or withdrawal here.
    const owned=this.shared&&!this.stopped&&this.installation!==null,current=this.bound,previous=this.lastReady;
    const exact=previous&&current&&previous.lineage===this.lineage
      &&sameHeroSource(previous.status.source,current.snapshot.binding.source)
      &&previous.status.controlRevision===current.control.controlRevision;
    const displayed=exact||this.continuous&&previous?.lineage===this.lineage?previous?.status:null;
    const worldBlocked=owned&&(!displayed||displayed.modal||this.held!==null||this.pendingEdgeFrame!==null);
    const next=Object.freeze({active,transitioning,owned,worldBlocked,webLeaseToken:owned?this.installation?.token??null:null,error});
    if(Object.keys(next).some(key=>next[key as keyof HeroHostState]!==this.state[key as keyof HeroHostState])){
      this.state=next;this.options.onState?.(next);
    }
  }
  private makeControl(snapshot:Snapshot,inputEnabled:boolean):Bound {
    const install=this.installation;if(!install)throw Error("Hero sink missing");
    const content=JSON.stringify([snapshot.binding.source.scope,snapshot.binding.source.heroObjectId,snapshot.binding.source.heroGeneration,
      snapshot.binding.witness,snapshot.model.sourceKey]);
    const geometry=JSON.stringify(snapshot.presentation);
    if(content!==this.contentKey){this.contentKey=content;this.modelRevision=advance(this.clock,"model");}
    if(geometry!==this.geometryKey){this.geometryKey=geometry;this.presentationRevision=advance(this.clock,"presentation");}
    const epochs=Object.freeze([snapshot.epochs.inventory,snapshot.epochs.character,snapshot.epochs.belt]) as readonly [number,number,number];
    const control:Control=Object.freeze({source:snapshot.binding.source,controlRevision:advance(this.clock,"control"),webLeaseToken:install.token,
      hudGeneration:snapshot.hudGeneration,webModelRevision:this.modelRevision,presentationRevision:this.presentationRevision,
      windowEpochs:epochs,windows:snapshot.windows,presentation:snapshot.presentation,inGame:true,hostVisible:true,inputEnabled,pending:snapshot.pending});
    const stamp:HeroStamp=Object.freeze({scope:control.source.scope,heroObjectId:control.source.heroObjectId,heroGeneration:control.source.heroGeneration,
      hudGeneration:control.hudGeneration,modelRevision:control.webModelRevision,presentationRevision:control.presentationRevision,windowEpochs:epochs});
    return Object.freeze({snapshot,control,stamp});
  }
  private setControl(snapshot:Snapshot,inputEnabled:boolean):boolean {
    const install=this.installation,bound=this.makeControl(snapshot,inputEnabled);
    if(!bindHeroUiControl(this.options.runtime,bound.control,snapshot.model)||this.installation!==install
      ||!this.liveMatches(snapshot,false,true))return false;
    this.bound=bound;return true;
  }
  private start(snapshot:Snapshot):boolean {
    const install:Installation={scope:snapshot.binding.source.scope,generation:0,token:leaseToken()};
    this.installation=install;this.intentHigh=0;this.lineage++;this.lastReady=null;this.continuous=false;this.held=null;this.pendingEdgeFrame=null;
    let generation:number|undefined;
    try {generation=this.options.runtime.setMir2HeroUiIntentSink?.(JSON.stringify(install.scope),raw=>this.acceptIntent(raw,install));}catch{return false;}
    if(this.installation!==install||!positive(generation))return false;
    install.generation=generation;
    return this.setControl(snapshot,false);
  }
  /** Called after completed Page delivery as well as by the 50 ms fallback ticker. */
  tick():void {
    if(this.stopped||this.ticking)return;this.ticking=true;
    try {
      const snapshot=this.readSnapshot();if(!snapshot){this.withdraw("Hero source or presentation unavailable");return;}
      const healthy=this.pulse();
      if(this.installation&&!sameHeroScope(this.installation.scope,snapshot.binding.source.scope))this.withdraw();
      if(!this.installation){if(!supportsHeroSharedUi(this.options.runtime)||!this.start(snapshot)){this.withdraw("Hero control unavailable");return;}}
      const old=this.bound;if(!old){this.withdraw("Hero control unavailable");return;}
      if(!controlDataEqual(old.snapshot,snapshot)){
        if(sameHeroScope(old.snapshot.binding.source.scope,snapshot.binding.source.scope)
          &&(snapshot.binding.source.frameSequence<old.snapshot.binding.source.frameSequence
            ||snapshot.binding.source.rustModelRevision<old.snapshot.binding.source.rustModelRevision)){
          this.withdraw("Hero source regressed");return;
        }
        const hadReady=this.lastReady!==null;
        const continuity=this.shared&&hadReady&&interactionEqual(old.snapshot,snapshot)&&old.control.inputEnabled;
        if(!continuity){this.lineage++;this.held=null;this.lastReady=null;this.pendingEdgeFrame=null;}
        this.continuous=continuity;
        if(!this.setControl(snapshot,this.shared)){this.withdraw("Hero source changed during control");return;}
        if(this.shared&&!continuity&&hadReady)this.enabledAt=this.now();
      }else{
        // Unrelated raw deliveries still advance the final-claim high-water proof.
        this.bound=Object.freeze({...old,snapshot});
      }
      const current=this.bound!,install=this.installation!;
      const status=readHeroUiStatus(this.options.runtime,current.snapshot.binding,current.snapshot.model,
        current.control.controlRevision,install.token,install.generation);
      if(!healthy||!this.frameFresh()){
        if(this.shared||this.advanced&&this.now()-this.lastAdvanceAt>HEALTH_MS){this.withdraw("Hero renderer is not advancing");return;}
        this.emitState(false,true);return;
      }
      if(!this.shared){
        if(!status?.prepared||status.ready||status.inputEnabled){this.emitState(false,true);return;}
        this.shared=true;this.enabledAt=this.now();
        this.emitState(false,true); // The following flushSync must already see owned=true.
        this.options.onOwner(install.token);
        if(this.stopped||this.installation!==install||!this.liveMatches(current.snapshot,false,true)||!this.frameFresh()
          ||!this.setControl(current.snapshot,true)){
          this.withdraw("Hero owner changed during handoff");return;
        }
        this.emitState(false,true);return;
      }
      if(status?.ready&&status.inputEnabled){
        this.lastReady=Object.freeze({status,lineage:this.lineage,at:this.now()});this.continuous=false;this.emitState(true,false);return;
      }
      if(this.continuous&&(!this.lastReady||this.now()-this.lastReady.at>HEALTH_MS)){this.withdraw("Hero continuous refresh expired");return;}
      if(this.now()-this.enabledAt>ENABLE_TIMEOUT_MS&&!this.continuous){this.withdraw("Hero ready handshake expired");return;}
      this.emitState(false,true);
    }catch{this.withdraw("Hero host unavailable");}finally{this.ticking=false;}
  }
  private currentStatus():HeroUiStatus|null {
    const current=this.bound,install=this.installation;
    if(!this.shared||!current||!install||!this.liveMatches(current.snapshot,true,false)||!this.pulse())return null;
    const status=readHeroUiStatus(this.options.runtime,current.snapshot.binding,current.snapshot.model,
      current.control.controlRevision,install.token,install.generation);
    if(status?.ready&&status.inputEnabled&&this.bound===current&&this.installation===install&&this.factsMatch(current.snapshot,true,false)&&this.frameFresh()){
      this.lastReady=Object.freeze({status,lineage:this.lineage,at:this.now()});this.continuous=false;this.emitState(true,false);return status;
    }
    return null;
  }
  private continuousStatus():HeroUiStatus|null {
    const current=this.bound,ready=this.lastReady;
    return this.shared&&this.continuous&&current&&ready&&ready.lineage===this.lineage
      &&this.liveMatches(current.snapshot,false,false)&&this.pulse()&&this.now()-ready.at<=HEALTH_MS
      &&this.bound===current&&this.factsMatch(current.snapshot,false,false)&&this.frameFresh()?ready.status:null;
  }
  isSharedOwner():boolean {
    try {
      if(!this.shared||this.stopped)return false;
      if(this.options.isCurrent?.()===false||!this.pulse()){this.withdraw("Hero renderer is not advancing");return false;}
      const read=this.options.read();
      if(!read.eligible||!read.inputAllowed||!read.model||!heroPresentationFits(read.presentation)
        ||!this.bound||!sameHeroPhysical(read.model.owner,this.bound.snapshot.model.owner)){
        this.withdraw("Hero owner unavailable");return false;
      }
      return true;
    }catch{this.withdraw("Hero owner unavailable");return false;}
  }
  blocksWorld():boolean {
    if(!this.isSharedOwner())return false;
    try {const status=this.currentStatus()??this.continuousStatus();return !status||status.modal||this.held!==null||this.pendingEdgeFrame!==null;}
    catch{this.withdraw("Hero input unavailable");return false;}
  }
  pointerContext():HeroPointerContext|null {
    this.tick();if(!this.isSharedOwner()||!this.bound||!this.installation)return null;
    try {
      const ready=this.currentStatus(),status=ready??this.continuousStatus();if(!status)return null;
      const context=Object.freeze({webLeaseToken:this.installation.token,sinkGeneration:this.installation.generation,
        stamp:this.bound.stamp,presentation:this.bound.snapshot.presentation,ready:!!ready,inputRegions:status.inputRegions});
      this.contexts.set(context,this.lineage);return context;
    }catch{this.withdraw("Hero pointer unavailable");return null;}
  }
  private input(phase:string,pointerId:number,x:number,y:number,button:0|2,key:string,text:string,control:boolean,shift:boolean):boolean {
    const current=this.bound,install=this.installation;if(!current||!install||!this.frameFresh())return false;
    const sequence=advance(this.clock,"input"),frame=this.lastFrame;
    const accepted=this.options.runtime.setMir2HeroUiInputEdge?.(JSON.stringify(install.scope),JSON.stringify({sinkGeneration:install.generation,
      controlRevision:current.control.controlRevision,webLeaseToken:install.token,edge:{stamp:current.stamp,sequence,pointerId,phase,x,y,button,key,text,control,shift}}))===true;
    if(accepted&&(phase!=="move"||this.held!==null)){this.pendingEdgeFrame=frame;this.emitProjection();}
    return accepted;
  }
  pointer(edge:HeroPointerEdge):boolean {
    try {
      if(!edge||!uint(edge.pointerId)||!["down","move","up","cancel"].includes(edge.phase)||![0,2].includes(edge.button)
        ||![edge.x,edge.y].every(v=>Number.isFinite(v)&&Math.abs(v)<=16384))return false;
      const held=this.held,terminal=edge.phase==="up"||edge.phase==="cancel";
      const owned=held?.pointerId===edge.pointerId&&held.context===edge.context;
      if(this.contexts.get(edge.context)!==this.lineage||!this.installation||!this.bound
        ||edge.context.webLeaseToken!==this.installation.token||edge.context.sinkGeneration!==this.installation.generation
        ||!sameStamp(edge.context.stamp,this.bound.stamp))return false;
      const ready=this.currentStatus(),status=ready??(owned?this.continuousStatus():null);
      if(!status)return false;
      if(held&&!owned)return false;
      if(terminal&&!owned)return false;
      if(edge.phase==="down"){
        if(!ready||held||!status.inputRegions.some(r=>edge.x>=r.left&&edge.y>=r.top&&edge.x<r.left+r.width&&edge.y<r.top+r.height))return false;
        this.held={pointerId:edge.pointerId,button:edge.button,context:edge.context,lineage:this.lineage};
      }
      if(owned&&edge.button!==held!.button)return false;
      if(!this.input(edge.phase,edge.pointerId,edge.x,edge.y,edge.button,"","",edge.control===true,edge.shift===true)){
        // Queue refusal can mean World reset or an expired lease. Never move a
        // terminal edge onto a replacement owner merely to clear its gesture.
        this.withdraw("Hero pointer lease rejected");return false;
      }
      if(terminal)this.held=null;
      this.emitProjection();return true;
    }catch{this.withdraw("Hero pointer unavailable");return false;}
  }
  key(edge:HeroKeyEdge):boolean {
    try {
      const status=this.currentStatus();if(!status||!edge||typeof edge.key!=="string")return false;
      const escape=edge.key==="Escape",waiting=this.pendingEdgeFrame!==null;
      const captures=escape?(status.capturesEscape||this.held!==null||waiting):(status.modal||waiting);
      if(!captures)return false; // Enqueue acceptance is not a keyboard-consumption signal.
      if(edge.alt||edge.meta)return true;
      if(escape&&edge.repeat)return true;
      const key=edge.control&&(edge.code==="KeyA"||edge.key.toLowerCase()==="a")?"KeyA":edge.key;
      const text=edge.control?"":edge.text??([...edge.key].length===1?edge.key:"");
      if(bytes(key)>64||bytes(text)>32)return true;
      if(!this.input("key",0,0,0,0,key,text,edge.control===true,edge.shift===true))this.withdraw("Hero key lease rejected");
      return true;
    }catch{this.withdraw("Hero key unavailable");return false;}
  }
  private acceptIntent(raw:string,install:Installation):boolean {
    try {
      if(this.stopped||this.installation!==install||!positive(install.generation)||!this.shared
        ||typeof raw!=="string"||raw.length===0||raw.length>MAX_INTENT_BYTES||bytes(raw)>MAX_INTENT_BYTES)return false;
      const value:unknown=JSON.parse(raw),current=this.bound;if(!current||!row(value)
        ||!fields(value,["version","source","controlRevision","webLeaseToken","intent"])||value.version!==1
        ||!sourceShape(value.source)||!sameHeroSource(value.source,current.snapshot.binding.source)
        ||value.controlRevision!==current.control.controlRevision||value.webLeaseToken!==install.token||!row(value.intent))return false;
      const body=value.intent;if(!positive(body.intentSequence)||body.intentSequence<=this.intentHigh||!stampMatches(body,current.stamp))return false;
      // Every admitted envelope burns its sequence, including denied/failed sends.
      this.intentHigh=body.intentSequence;
      const status=this.currentStatus(),snapshot=this.readSnapshot();
      if(!status||!snapshot||!this.liveMatches(current.snapshot,true,false))return false;
      const proof:HeroSharedProof=Object.freeze({model:snapshot.model,source:current.snapshot.binding.source,witness:current.snapshot.binding.witness,
        highWater:snapshot.highWater,sourceWindows:snapshot.windows,windowEpochs:snapshot.epochs,presentation:snapshot.presentation,
        controlRevision:current.control.controlRevision,sinkGeneration:install.generation,webLeaseToken:install.token,
        intentSequence:body.intentSequence,stamp:current.stamp});
      let intent:HeroSharedIntent;
      if(body.type==="windows"){
        const windows=copyWindows(body.windows,true);
        if(!fields(body,[...stampKeys,"intentSequence","type","windows"])||!windows||!navigationAllowed(snapshot.windows,windows))return false;
        intent=Object.freeze({...proof,type:"windows",windows});
      }else if(body.type==="action"){
        if(!fields(body,[...stampKeys,"intentSequence","type","origin","action","oldKey","basis"])||snapshot.pending
          ||!["inventory","character","belt"].includes(String(body.origin)))return false;
        const origin=body.origin as HeroUiOrigin,action=parseHeroSemanticAction(body.action);
        if(!action||!originOpen(origin,snapshot.windows)||origin==="belt"&&(action.kind!=="use"||action.slot>1)
          ||!heroActionBasisMatches(body.basis,snapshot.model,action,snapshot.windows))return false;
        const expectedOldKey=action.kind==="magicKey"?snapshot.model.magics.find(m=>m.spell===action.spell)?.key:null;
        if(body.oldKey!==expectedOldKey)return false;
        const actionBasis=captureHeroActionBasis(snapshot.model,action,snapshot.windows);if(!actionBasis)return false;
        intent=Object.freeze({...proof,type:"action",origin,action,actionBasis,oldKey:expectedOldKey as number|null});
      }else return false;
      this.issued.add(intent);
      if(!this.allows(intent))return false;
      return this.options.onIntent(intent)===true;
    }catch{return false;}
  }
  /** Idempotent proof check: Page still owns reserve -> claim -> send and settlement. */
  claimCurrent(intent:HeroSharedIntent):boolean {return this.allows(intent);}
  allows(intent:HeroSharedIntent):boolean {
    try {
      const current=this.bound,install=this.installation;
      if(!this.issued.has(intent)||!this.shared||this.stopped||!current||!install
        ||intent.webLeaseToken!==install.token||intent.sinkGeneration!==install.generation
        ||intent.controlRevision!==current.control.controlRevision||!sameHeroSource(intent.source,current.snapshot.binding.source)
        ||intent.witness!==current.snapshot.binding.witness||!sameStamp(intent.stamp,current.stamp))return false;
      const snapshot=this.readSnapshot();if(!snapshot||!sameHeroHighWater(intent.highWater,snapshot.highWater)
        ||!sameHeroSource(intent.source,snapshot.binding.source)||intent.witness!==snapshot.binding.witness
        ||intent.model.sourceKey!==snapshot.model.sourceKey||!sameHeroPhysical(intent.model.owner,snapshot.model.owner)
        ||!sameWindows(intent.sourceWindows,snapshot.windows)||!sameEpochs(intent.windowEpochs,snapshot.epochs)
        ||!samePresentation(intent.presentation,snapshot.presentation)||!this.currentStatus())return false;
      // A reservation made by this very callback changes pending. Only the
      // original Page ledger can distinguish its own reservation at final claim.
      const valid=intent.type==="windows"?navigationAllowed(snapshot.windows,intent.windows)
        :originOpen(intent.origin,snapshot.windows)&&heroActionBasisMatches(intent.actionBasis,snapshot.model,intent.action,snapshot.windows);
      return valid&&this.bound===current&&this.installation===install&&this.shared&&this.factsMatch(snapshot,true,true)&&this.frameFresh();
    }catch{return false;}
  }
  withdraw(error:string|null=null):void {
    const install=this.installation,wasShared=this.shared;
    this.installation=null;this.bound=null;this.shared=false;this.lineage++;this.held=null;this.lastReady=null;this.continuous=false;this.pendingEdgeFrame=null;
    if(install){
      // Clear only this installation. Never withdraw/retire the permanent raw ingress.
      // Writing an old control here could overwrite a newer installation even
      // if the generation-checked clear subsequently refused it.
      try {this.options.runtime.clearMir2HeroUiIntentSink?.(JSON.stringify(install.scope),install.generation);}catch{/* renderer may already be gone */}
    }
    this.emitState(false,false,error);
    if(wasShared)try {this.options.onOwner(null);}catch{/* state remains locally revoked */}
  }
  stop():void {if(this.stopped)return;this.stopped=true;this.withdraw();}
}
