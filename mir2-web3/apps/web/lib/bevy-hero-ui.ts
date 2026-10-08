/** Original raw custody survives renderer lifetimes. No operation ledger lives here. */
import { createHeroSourceWitness, type HeroPlayerModel } from "./hero-player-ui";
export type HeroPhysical = Readonly<{ socket: object; connectionGeneration: number; sessionGeneration: number }>;
export type HeroScene = Readonly<{ sceneRevision: number; playerObjectId: number; mapFileName: string }>;
export type HeroScope = Omit<HeroPhysical, "socket"> & HeroScene & Readonly<{ runGeneration: number }>;
export type HeroSource = Readonly<{ scope: HeroScope; frameSequence: number; heroObjectId: number;
  heroGeneration: number; rustModelRevision: number }>;
export type VerifiedHeroOwner = Readonly<{ requestId: string; actor: string; producerScope: string; serverRevision: string }>;
export type HeroRawDelivery = HeroPhysical & Readonly<{ raw: string; sequence: number; receivedAtMs: number }>;
export type HeroRawHighWater = HeroPhysical & Readonly<{sequence:number}>;
export type HeroBootstrap = Readonly<{ source: HeroSource | null; acceptedFrameSequence: number; closed: boolean }>;
export type HeroSourceWitness = Readonly<{ source: HeroSource; witness: string }>;
export type HeroUiReady = HeroSourceWitness & Readonly<{acceptedFrameSequence:number;controlRevision:number;
  sinkGeneration:number;webLeaseToken:string;frame:number;modal:boolean;inputEnabled:boolean;
  inputRegions:readonly Readonly<{left:number;top:number;width:number;height:number}>[]}>;
export type HeroRuntime = {
  getMir2HeroUiAbiVersion?: () => number;
  activateMir2HeroIngress?: (scope: string) => boolean;
  pushMir2HeroRawFrame?: (scope: string, sequence: number, raw: string, receivedAtMs: number) => boolean;
  pushMir2HeroVerifiedOwnerFrame?: (scope: string, sequence: number, raw: string, receivedAtMs: number, expected: string) => boolean;
  withdrawMir2HeroIngress?: (scope: string) => boolean;
  getMir2HeroIngressCheckpoint?: (scope: string) => string | null | undefined;
  restoreMir2HeroIngressCheckpoint?: (scope: string, heldCheckpoint: string) => boolean;
  getMir2HeroUiStatus?: () => string;
  getMir2HeroActionBasisVersion?: () => number;
  getMir2HeroSourceWitnessVersion?: () => number;
  getMir2HeroSourceWitness?: () => string | null | undefined;
  setMir2HeroUiControlWithWitness?: (scope:string,control:string,witness:string) => boolean;
  setMir2HeroUiControl?: (scope:string,control:string) => boolean;
  setMir2HeroUiInputEdge?: (scope:string,edge:string) => boolean;
  setMir2HeroUiIntentSink?: (scope:string,sink:(raw:string)=>boolean) => number;
  clearMir2HeroUiIntentSink?: (scope:string,sinkGeneration:number) => boolean;
};
const safe = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const positive = (value: unknown): value is number => safe(value) && value > 0;
const row = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const fields = (value: Record<string, unknown>, names: readonly string[]) => Object.keys(value).sort().join() === [...names].sort().join();
const bytes = (value: string) => new TextEncoder().encode(value).byteLength;
const scopeKeys = ["runGeneration","connectionGeneration","sessionGeneration","sceneRevision","playerObjectId","mapFileName"] as const;
export function sameHeroPhysical(a: HeroPhysical | null, b: HeroPhysical | null): boolean {
  return !!a && !!b && a.socket === b.socket && a.connectionGeneration === b.connectionGeneration && a.sessionGeneration === b.sessionGeneration;
}
function validPhysical(value: HeroPhysical | null): value is HeroPhysical {
  return !!value && typeof value.socket === "object" && value.socket !== null
    && positive(value.connectionGeneration) && positive(value.sessionGeneration);
}
function validScene(value: HeroScene | null): value is HeroScene {
  return !!value && positive(value.sceneRevision) && positive(value.playerObjectId) && value.playerObjectId <= 0xffffffff
    && typeof value.mapFileName === "string" && value.mapFileName.length > 0 && bytes(value.mapFileName) <= 512 && !value.mapFileName.includes("\0");
}
export function sameHeroScope(a: HeroScope | null, b: HeroScope | null): boolean {
  return !!a && !!b && scopeKeys.every(key => a[key] === b[key]);
}
export function sameHeroSource(a:HeroSource|null,b:HeroSource|null):boolean {
  return !!a&&!!b&&sameHeroScope(a.scope,b.scope)&&a.frameSequence===b.frameSequence
    &&a.heroObjectId===b.heroObjectId&&a.heroGeneration===b.heroGeneration&&a.rustModelRevision===b.rustModelRevision;
}
export function sameHeroHighWater(a:HeroRawHighWater|null,b:HeroRawHighWater|null):boolean {
  return !!a&&!!b&&sameHeroPhysical(a,b)&&a.sequence===b.sequence;
}
function validScope(value: unknown): value is HeroScope {
  return row(value) && fields(value, scopeKeys) && positive(value.runGeneration) && positive(value.connectionGeneration)
    && positive(value.sessionGeneration) && validScene(value as unknown as HeroScene);
}
function sameScene(a: HeroScene | null, b: HeroScene | null): boolean {
  return !!a && !!b && a.sceneRevision === b.sceneRevision && a.playerObjectId === b.playerObjectId && a.mapFileName === b.mapFileName;
}
function validSource(value: unknown): value is HeroSource {
  return row(value) && fields(value,["scope","frameSequence","heroObjectId","heroGeneration","rustModelRevision"])
    && validScope(value.scope) && positive(value.frameSequence) && positive(value.heroObjectId) && value.heroObjectId <= 0xffffffff
    && positive(value.heroGeneration) && positive(value.rustModelRevision);
}
export function supportsHeroIngress(runtime: HeroRuntime | null): runtime is HeroRuntime {
  try { return runtime?.getMir2HeroUiAbiVersion?.() === 1 && [runtime.activateMir2HeroIngress,
    runtime.pushMir2HeroRawFrame,runtime.pushMir2HeroVerifiedOwnerFrame,runtime.withdrawMir2HeroIngress,
    runtime.getMir2HeroIngressCheckpoint,runtime.restoreMir2HeroIngressCheckpoint,runtime.getMir2HeroUiStatus]
    .every(fn => typeof fn === "function"); } catch { return false; }
}
/** Bootstrap accepts source before control exists. It does not prove UI Ready. */
export function readHeroBootstrap(runtime: HeroRuntime): HeroBootstrap | null {
  try { const raw=runtime.getMir2HeroUiStatus?.();
    if(typeof raw!=="string"||raw.length===0||raw.length>MAX_HERO_STATUS_BYTES||bytes(raw)>MAX_HERO_STATUS_BYTES)return null;
    const value: unknown = JSON.parse(raw);
    if (!row(value) || value.version !== 1 || !safe(value.acceptedFrameSequence) || typeof value.closed !== "boolean"
      || !(value.source === null || validSource(value.source))
      || value.source && (value.source as HeroSource).frameSequence > value.acceptedFrameSequence) return null;
    return { source:value.source as HeroSource | null, acceptedFrameSequence:value.acceptedFrameSequence,closed:value.closed };
  } catch { return null; }
}
const MAX_HERO_SOURCE_WITNESS_BYTES = 65_536;
const MAX_HERO_WITNESS_ENVELOPE_BYTES = 2 * MAX_HERO_SOURCE_WITNESS_BYTES + 4096;
const MAX_HERO_STATUS_BYTES = 2 * MAX_HERO_SOURCE_WITNESS_BYTES + 65_536;
/** Raw ingress capability alone cannot transfer shared Hero UI ownership. */
export function supportsHeroSharedUi(runtime:HeroRuntime|null):runtime is HeroRuntime {
  try { return supportsHeroIngress(runtime) && runtime?.getMir2HeroActionBasisVersion?.()===1
    && runtime.getMir2HeroSourceWitnessVersion?.()===1
    && [runtime.getMir2HeroSourceWitness,runtime.setMir2HeroUiControlWithWitness,
      runtime.setMir2HeroUiInputEdge,runtime.setMir2HeroUiIntentSink,runtime.clearMir2HeroUiIntentSink]
      .every(fn=>typeof fn==="function"); } catch { return false; }
}
function sourceOwnsHeroModel(source:HeroSource,model:HeroPlayerModel):boolean {
  const owner=model.owner,scope=source.scope;
  return !!owner&&typeof owner.socket==="object"&&owner.socket!==null
    &&source.heroObjectId===model.actor.objectId
    &&scope.connectionGeneration===owner.connectionGeneration&&scope.sessionGeneration===owner.sessionGeneration
    &&scope.playerObjectId===owner.playerObjectId&&scope.sceneRevision===owner.sceneRevision&&scope.mapFileName===owner.mapFileName;
}
/** The Web witness is derived from its own frozen model before inspecting Rust.
 * Source/cursor bracketing rejects reentry; no ABI string is adopted as Web data. */
export function readHeroSourceWitness(runtime:HeroRuntime,model:HeroPlayerModel):HeroSourceWitness|null {
  try {
    if (!supportsHeroSharedUi(runtime)) return null;
    const expected=createHeroSourceWitness(model);if(expected===null)return null;
    const before=readHeroBootstrap(runtime);if(!before?.source||before.closed||!sourceOwnsHeroModel(before.source,model))return null;
    const raw=runtime.getMir2HeroSourceWitness?.();
    if(typeof raw!=="string"||raw.length===0||raw.length>MAX_HERO_WITNESS_ENVELOPE_BYTES
      ||bytes(raw)>MAX_HERO_WITNESS_ENVELOPE_BYTES)return null;
    const value:unknown=JSON.parse(raw),after=readHeroBootstrap(runtime);
    if(!row(value)||!fields(value,["version","source","witness"])||value.version!==1
      ||!validSource(value.source)||typeof value.witness!=="string"||value.witness.length===0
      ||bytes(value.witness)>MAX_HERO_SOURCE_WITNESS_BYTES||value.witness!==expected
      ||!after?.source||after.closed||after.acceptedFrameSequence!==before.acceptedFrameSequence
      ||!sameHeroSource(before.source,after.source)||!sameHeroSource(value.source,after.source)
      ||!sourceOwnsHeroModel(value.source,model)||!sourceOwnsHeroModel(after.source,model))return null;
    return Object.freeze({source:Object.freeze({...value.source,scope:Object.freeze({...value.source.scope})}),witness:expected});
  } catch { return null; }
}
/** Accepted control is only a binding request. Actual consumed paint is checked
 * separately by readHeroUiReady; this function never creates UI Ready. */
export function bindHeroUiControl(runtime:HeroRuntime,control:Readonly<Record<string,unknown>>,
  model:HeroPlayerModel):HeroSourceWitness|null {
  try {
    const binding=readHeroSourceWitness(runtime,model);
    if(!binding||!validSource(control.source)||!sameHeroSource(control.source,binding.source))return null;
    const raw=JSON.stringify(control);if(bytes(raw)>8192)return null;
    if(runtime.setMir2HeroUiControlWithWitness?.(JSON.stringify(binding.source.scope),raw,binding.witness)!==true)return null;
    const after=readHeroSourceWitness(runtime,model);
    return after&&sameHeroSource(after.source,binding.source)&&after.witness===binding.witness?binding:null;
  } catch { return null; }
}
/** Requires the exact witness installed by the actual Rust World consumer and
 * the subsequent ready frame. A getter or setter echo alone has no authority. */
export function readHeroUiReady(runtime:HeroRuntime,binding:HeroSourceWitness,model:HeroPlayerModel,
  controlRevision:number,webLeaseToken:string,sinkGeneration:number):HeroUiReady|null {
  try {
    if(!supportsHeroSharedUi(runtime)||!positive(controlRevision)||!positive(sinkGeneration)
      ||typeof webLeaseToken!=="string"||!webLeaseToken||bytes(webLeaseToken)>512||webLeaseToken.includes("\0")
      ||!validSource(binding.source)||typeof binding.witness!=="string"
      ||createHeroSourceWitness(model)!==binding.witness)return null;
    const before=readHeroSourceWitness(runtime,model);
    if(!before||!sameHeroSource(before.source,binding.source)||before.witness!==binding.witness)return null;
    const raw=runtime.getMir2HeroUiStatus?.();
    if(typeof raw!=="string"||raw.length===0||raw.length>MAX_HERO_STATUS_BYTES||bytes(raw)>MAX_HERO_STATUS_BYTES)return null;
    const value:unknown=JSON.parse(raw);
    if(!row(value)||value.version!==1||value.closed!==false||value.ready!==true
      ||typeof value.inputEnabled!=="boolean"||typeof value.modal!=="boolean"
      ||!validSource(value.source)||!validSource(value.appliedSource)
      ||!sameHeroSource(value.source,binding.source)||!sameHeroSource(value.appliedSource,binding.source)
      ||value.appliedWitness!==binding.witness||value.controlRevision!==controlRevision
      ||value.webLeaseToken!==webLeaseToken||value.sinkGeneration!==sinkGeneration||!positive(value.frame)
      ||!safe(value.acceptedFrameSequence)||value.acceptedFrameSequence<binding.source.frameSequence
      ||!Array.isArray(value.receiptFrames)||value.receiptFrames.length>4096||!value.receiptFrames.every(positive)
      ||!Array.isArray(value.inputRegions)||value.inputRegions.length>256)return null;
    const regions:Readonly<{left:number;top:number;width:number;height:number}>[]=[];
    for(const region of value.inputRegions){
      if(!row(region)||!fields(region,["left","top","width","height"])
        ||![region.left,region.top,region.width,region.height].every(n=>typeof n==="number"&&Number.isFinite(n))
        ||(region.width as number)<=0||(region.height as number)<=0)return null;
      regions.push(Object.freeze({left:region.left as number,top:region.top as number,
        width:region.width as number,height:region.height as number}));
    }
    const after=readHeroSourceWitness(runtime,model);
    if(!after||!sameHeroSource(after.source,binding.source)||after.witness!==binding.witness)return null;
    return Object.freeze({...binding,acceptedFrameSequence:value.acceptedFrameSequence,
      controlRevision,sinkGeneration,webLeaseToken,frame:value.frame,modal:value.modal,
      inputEnabled:value.inputEnabled,inputRegions:Object.freeze(regions)});
  } catch { return null; }
}
const decimal = (value: unknown): value is string => typeof value === "string" && /^(?:0|[1-9][0-9]{0,19})$/.test(value)
  && BigInt(value) <= 18446744073709551615n;
const opaque = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{64}$/.test(value) && /[1-9a-f]/.test(value);
export function validVerifiedHeroOwner(value: unknown): value is VerifiedHeroOwner {
  return row(value) && fields(value,["requestId","actor","producerScope","serverRevision"])
    && decimal(value.requestId) && value.requestId !== "0" && opaque(value.actor) && opaque(value.producerScope)
    && decimal(value.serverRevision) && value.serverRevision !== "18446744073709551615";
}
/** Only call after the established verified owner application succeeds. Metadata
 * strings are compared; this never projects or re-encodes the parsed snapshot. */
export function verifiedHeroOwnerFromFrame(raw: string): VerifiedHeroOwner | null {
  try { const value: unknown = JSON.parse(raw);
    if (!row(value) || !fields(value,["type","protocolVersion","requestId","reply","snapshot","authority"])
      || value.type !== "npcPurchaseOwner" || value.protocolVersion !== 1 || !row(value.reply) || !row(value.snapshot)
      || !row(value.authority) || !fields(value.authority,["actor","producerScope","serverRevision"])) return null;
    const expected = {requestId:value.requestId,actor:value.authority.actor,
      producerScope:value.authority.producerScope,serverRevision:value.authority.serverRevision};
    return validVerifiedHeroOwner(expected) ? Object.freeze(expected) : null;
  } catch { return null; }
}
function nextCounter(key: "__mir2HeroRawSequence" | "__mir2HeroIngressRun"): number {
  const global = globalThis as typeof globalThis & { __mir2HeroRawSequence?: number; __mir2HeroIngressRun?: number };
  const n = (global[key] ?? 0) + 1;
  if (!positive(n)) throw Error("Hero source counter exhausted");
  global[key] = n; return n;
}
type Pending = Readonly<{ delivery: HeroRawDelivery; completed: boolean; expected: VerifiedHeroOwner | null; scene: HeroScene | null; charge: number }>;
type Held = Readonly<{ physical: HeroPhysical; scope: HeroScope; cursor: number; raw: string }>;
const MAX_RAW_BYTES = 16 * 1024 * 1024, MAX_TAIL_BYTES = 32 * 1024 * 1024, MAX_TAIL_FRAMES = 4096;
const MAX_HELD_BYTES = 64 * 1024 * 1024;
/** Page-owned original-frame tail. Failed capture/replay leaves React ownership.
 * Tail and checkpoint are separate bounds; they do not cap the entire runtime. */
export class HeroRawIngress {
  private physical: HeroPhysical | null = null;
  private scene: HeroScene | null = null;
  private runtime: HeroRuntime | null = null;
  private scope: HeroScope | null = null;
  private run = 0;
  private tail: Pending[] = [];
  private tailBytes = 0;
  private forwarded = 0;
  private held: Held | null = null;
  private latestCaptured: HeroRawDelivery | null = null;
  private clock = 0;
  private closed = false;
  private bound = false;
  private rendererFault = false;
  capture(raw: unknown, physical: HeroPhysical | null, receivedAtMs: number): HeroRawDelivery | null {
    if (this.closed || !validPhysical(physical) || !sameHeroPhysical(physical,this.physical)) return null;
    if (typeof raw !== "string" || !Number.isFinite(receivedAtMs) || receivedAtMs < this.clock
      || receivedAtMs > Number.MAX_SAFE_INTEGER || raw.length > MAX_RAW_BYTES || bytes(raw) > MAX_RAW_BYTES) {
      this.closed = true; return null;
    }
    const charge=bytes(raw);
    if(this.tail.length>=MAX_TAIL_FRAMES||this.tailBytes+charge>MAX_TAIL_BYTES){this.closed=true;return null;}
    try { const delivery = Object.freeze({...physical,raw,sequence:nextCounter("__mir2HeroRawSequence"),receivedAtMs});
      // Reserve before Page application: a reentrant message cannot overtake it.
      this.tail.push(Object.freeze({delivery,completed:false,expected:null,scene:null,charge}));this.tailBytes+=charge;
      this.clock = receivedAtMs; this.latestCaptured = delivery; return delivery;
    } catch { this.closed = true; return null; }
  }
  offer(delivery: HeroRawDelivery, expected: VerifiedHeroOwner | null = null): boolean {
    if (this.closed || !sameHeroPhysical(delivery,this.physical) || !positive(delivery.sequence)
      || !Number.isFinite(delivery.receivedAtMs) || expected !== null && !validVerifiedHeroOwner(expected)) return false;
    const index=this.tail.findIndex(p=>p.delivery===delivery);
    if(delivery.sequence<=this.forwarded||index<0||this.tail[index].completed)return false;
    this.tail[index]=Object.freeze({...this.tail[index],completed:true,
      expected:expected ? Object.freeze({...expected}) : null,scene:this.scene});
    this.flush(); return !this.closed;
  }
  sync(runtime: HeroRuntime | null, physical: HeroPhysical | null, scene: HeroScene | null): void {
    if (!validPhysical(physical)) { this.retire(); return; }
    const physicalChanged = !sameHeroPhysical(physical,this.physical);
    if (physicalChanged) {
      // A new scene directly activates the same receiver; withdraw would lose
      // its private HeroInformation bootstrap and original cooldown anchors.
      this.tail=[];this.tailBytes=0;this.held=null;this.forwarded=0;this.closed=false;this.rendererFault=false;
      if (physicalChanged && !sameHeroPhysical(this.latestCaptured,physical)) this.latestCaptured=null;
    }
    if (physicalChanged) this.scene=null;
    this.physical = Object.freeze({...physical});
    if(validScene(scene)&&!sameScene(scene,this.scene)
      &&this.tail.some(p=>!p.completed&&p.delivery.sequence!==this.latestCaptured?.sequence)){
      // A nested scene replacement cannot relabel an unfinished earlier raw frame.
      this.closed=true;
    }
    if (validScene(scene)) this.scene=Object.freeze({...scene});
    this.bound = supportsHeroIngress(runtime) && validScene(scene);
    if (!supportsHeroIngress(runtime) || !validScene(scene)) {
      if (this.runtime) this.saveCheckpoint();
      return;
    }
    const runtimeChanged = runtime !== this.runtime;
    if (runtimeChanged) {
      this.saveCheckpoint();this.run=nextCounter("__mir2HeroIngressRun");this.runtime=runtime;this.scope=null;this.rendererFault=false;
    }
    if (runtimeChanged) {
      this.forwarded=0;
      const held=this.held;
      const initial=held && sameHeroPhysical(held.physical,physical) ? held.scope
        : this.tail.find(p=>p.scene!==null)?.scene ?? scene;
      if (!this.activateScene(initial)) return;
      if (held && sameHeroPhysical(held.physical,physical)) {
        try {
          if (!runtime.restoreMir2HeroIngressCheckpoint?.(JSON.stringify(this.scope),held.raw)) {this.rendererFault=true;return;}
          this.forwarded=held.cursor;
        } catch {this.rendererFault=true;return;}
      }
    }
    this.flush();this.saveCheckpoint();
  }
  private activateScene(scene: HeroScene): boolean {
    if(!this.runtime||!this.physical||this.closed||this.rendererFault)return false;
    const scope: HeroScope={runGeneration:this.run,connectionGeneration:this.physical.connectionGeneration,
      sessionGeneration:this.physical.sessionGeneration,sceneRevision:scene.sceneRevision,
      playerObjectId:scene.playerObjectId,mapFileName:scene.mapFileName};
    if(sameHeroScope(scope,this.scope))return true;
    try {
      if(this.runtime.activateMir2HeroIngress?.(JSON.stringify(scope))!==true){this.rendererFault=true;return false;}
      this.scope=Object.freeze(scope);return true;
    } catch {this.rendererFault=true;return false;}
  }
  private flush(): void {
    if (this.closed || this.rendererFault || !this.bound || !this.runtime || !this.scene) return;
    try { for (const pending of this.tail) {
      const d=pending.delivery;if(d.sequence<=this.forwarded)continue;
      if(!pending.completed)return;
      if(!sameHeroPhysical(d,this.physical)){this.closed=true;return;}
      if(!this.activateScene(pending.scene ?? this.scene))return;
      const scope=JSON.stringify(this.scope);
      const accepted=pending.expected
        ? this.runtime.pushMir2HeroVerifiedOwnerFrame?.(scope,d.sequence,d.raw,d.receivedAtMs,JSON.stringify(pending.expected))
        : this.runtime.pushMir2HeroRawFrame?.(scope,d.sequence,d.raw,d.receivedAtMs);
      if(accepted!==true){this.rendererFault=true;return;}
      this.forwarded=d.sequence;
    }
      this.activateScene(this.scene);
    } catch {this.rendererFault=true;}
  }
  private saveCheckpoint(): void {
    if(this.closed||this.rendererFault||!this.runtime||!this.scope||!this.physical
      ||this.scope.connectionGeneration!==this.physical.connectionGeneration
      ||this.scope.sessionGeneration!==this.physical.sessionGeneration)return;
    try {
      const before=readHeroBootstrap(this.runtime);
      const raw=this.runtime.getMir2HeroIngressCheckpoint?.(JSON.stringify(this.scope));
      const after=readHeroBootstrap(this.runtime);
      if(!before||!after||before.closed||after.closed||before.acceptedFrameSequence!==after.acceptedFrameSequence
        || typeof raw!=="string"||raw.length===0||raw.length>MAX_HELD_BYTES||bytes(raw)>MAX_HELD_BYTES
        || before.acceptedFrameSequence!==this.forwarded)return;
      this.held=Object.freeze({physical:this.physical,scope:this.scope,cursor:after.acceptedFrameSequence,raw});
      this.tail=this.tail.filter(p=>p.delivery.sequence>after.acceptedFrameSequence);
      this.tailBytes=this.tail.reduce((n,p)=>n+p.charge,0);
    } catch { /* Keep the last successful original string and its exact tail. */ }
  }
  detachRenderer(expected: HeroRuntime | null): void {
    // Effect cleanup cannot withdraw a replacement runtime or clear Page custody.
    if(expected===this.runtime)this.saveCheckpoint();
  }
  reject(delivery: HeroRawDelivery | null): void {
    if (delivery && sameHeroPhysical(delivery,this.physical)) this.closed=true;
  }
  retire(): void {
    if(this.runtime&&this.scope){try{this.runtime.withdrawMir2HeroIngress?.(JSON.stringify(this.scope));}catch{}}
    this.physical=null;this.scene=null;this.scope=null;this.tail=[];this.tailBytes=0;
    this.held=null;this.forwarded=0;this.latestCaptured=null;this.closed=false;this.bound=false;this.runtime=null;this.run=0;this.rendererFault=false;
  }
  bootstrap(): HeroBootstrap | null {
    const value=this.runtime?readHeroBootstrap(this.runtime):null;
    return this.bound&&!this.closed&&!this.rendererFault&&this.scope&&sameScene(this.scope,this.scene)&&value&&!value.closed
      && (value.source===null||sameHeroScope(value.source.scope,this.scope))?value:null;
  }
  /** Exact captured cursor is independent of forwarding and the last Hero change. */
  highWater():HeroRawHighWater|null {
    const captured=this.latestCaptured;
    return !this.closed&&!this.rendererFault&&captured&&sameHeroPhysical(captured,this.physical)
      ?Object.freeze({socket:captured.socket,connectionGeneration:captured.connectionGeneration,sessionGeneration:captured.sessionGeneration,sequence:captured.sequence}):null;
  }
  caughtUp(): boolean {
    const value=this.bootstrap(),captured=this.latestCaptured;
    return !!value&&!!captured&&sameHeroPhysical(captured,this.physical)
      && value.acceptedFrameSequence>=captured.sequence&&this.tail.every(p=>p.completed&&p.delivery.sequence<=this.forwarded);
  }
  status(): Readonly<{closed:boolean;tailFrames:number;tailBytes:number;forwarded:number;scope:HeroScope|null}> {
    return {closed:this.closed||this.rendererFault,tailFrames:this.tail.length,tailBytes:this.tailBytes,forwarded:this.forwarded,scope:this.scope};
  }
}
