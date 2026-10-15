import type { SocialReplyOwner } from "./social-incoming-replies";
import type { RankingQuery } from "./social-parity-actions";
import type { RankingInspectAdmissionInput, RankingInspectAdmissionPlan, RankingPlayerInspect } from "./shared-ranking-inspect";

type RankingPage = Readonly<{rankType:number; rankIndex?:number; onlineOnly:boolean;
  entries:readonly Readonly<{playerId:number; name:string}>[]}>;
export type RankingInspectSource = Readonly<{owner:SocialReplyOwner|null; core:object|null;
  readAdmission:((input:RankingInspectAdmissionInput)=>RankingInspectAdmissionPlan|null)|null;
  page:RankingPage|null; query:RankingQuery; opened:boolean; pending:boolean}>;
export type RankingInspectProof = Readonly<{objectId:number; expectedName:string; owner:SocialReplyOwner}>;

const physicalOwner = (a:SocialReplyOwner|null,b:SocialReplyOwner|null) => !!a && !!b
  && a.socket===b.socket && a.connectionGeneration===b.connectionGeneration
  && a.sessionGeneration===b.sessionGeneration && a.playerObjectId===b.playerObjectId;
const sameOwner = (a:SocialReplyOwner|null,b:SocialReplyOwner|null) => physicalOwner(a,b)
  && a!.sceneRevision===b!.sceneRevision && a!.mapFileName===b!.mapFileName;
const sameSource = (a:RankingInspectSource|null,b:RankingInspectSource) => !!a && sameOwner(a.owner,b.owner)
  && a.core===b.core && a.readAdmission===b.readAdmission && a.page===b.page
  && a.opened===b.opened && a.pending===b.pending && a.query.rankType===b.query.rankType
  && a.query.rankIndex===b.query.rankIndex && a.query.onlineOnly===b.query.onlineOnly;

/** The legacy reply carries a name, not a request ID. Keep one entered request
 * until its actual reply drains or its physical connection is retired. */
export class RankingInspectRequests {
  private source:RankingInspectSource|null=null;
  private revision=0;
  private exhausted=false;
  private reading=false;
  private nextReadyMs=0;
  private lastClock:number|null=null;
  private active:{proof:RankingInspectProof; revision:number; nowMs:number;
    nextReadyMs:number; entered:boolean; displayAllowed:boolean}|null=null;
  get pending():boolean { return this.active!==null; }

  observe(source:RankingInspectSource):void {
    if (sameSource(this.source,source)) return;
    if (this.revision===Number.MAX_SAFE_INTEGER) this.exhausted=true;
    else this.revision++;
    const previous=this.source;
    this.source=Object.freeze({...source,query:Object.freeze({...source.query}),
      owner:source.owner?Object.freeze({...source.owner}):null});
    if (this.active) {
      if (!this.active.entered) this.active=null;
      else this.active.displayAllowed=false;
    }
    // Only an explicit new physical owner resets the clock. A hidden/closed
    // surface must not wash an entered request or restore an old send proof.
    if (previous?.owner && source.owner && !physicalOwner(previous.owner,source.owner)) {
      this.active=null; this.nextReadyMs=0; this.lastClock=null;
    }
  }

  private clock(nowMs:number):boolean {
    if (!Number.isSafeInteger(nowMs)||nowMs<0) return false;
    if (this.lastClock!==null && nowMs<this.lastClock) {
      if (this.active && !this.active.entered) this.active=null;
      if (this.active) this.active.displayAllowed=false;
      return false;
    }
    this.lastClock=nowMs;
    return true;
  }

  reserve(readSource:()=>RankingInspectSource,playerId:number,name:string,nowMs:number):RankingInspectProof|null {
    if (this.reading||this.exhausted||!this.clock(nowMs)) return null;
    const source=readSource(); this.observe(source);
    if (this.active||!source.owner||!source.core||!source.readAdmission
      ||typeof name!=="string"||name.length===0||name.length>256||name.includes("\0")) return null;
    const page=source.page;
    const row=page?.entries.filter(entry=>entry.playerId===playerId && entry.name===name);
    if (!page||!row||row.length!==1||page.entries.filter(entry=>entry.playerId===playerId).length!==1) return null;
    const ready=page.rankType===source.query.rankType && page.rankIndex===source.query.rankIndex
      && page.onlineOnly===source.query.onlineOnly;
    const revision=this.revision,read=source.readAdmission;
    this.reading=true;
    let plan:RankingInspectAdmissionPlan|null=null;
    try { plan=read.call(source.core,{version:1,opened:source.opened,rankingsReady:ready,
      pending:source.pending,playerId,nowMs,nextReadyMs:this.nextReadyMs}); }
    finally { this.reading=false; }
    this.observe(readSource());
    if (revision!==this.revision||this.exhausted||!plan?.ok||plan.objectId!==playerId
      ||!Number.isSafeInteger(plan.nextReadyMs)||plan.nextReadyMs<0) return null;
    const proof=Object.freeze({objectId:plan.objectId,expectedName:name,owner:Object.freeze({...source.owner})});
    this.nextReadyMs=plan.nextReadyMs;
    this.active={proof,revision,nowMs,nextReadyMs:plan.nextReadyMs,entered:false,displayAllowed:true};
    return proof;
  }

  claim(proof:RankingInspectProof,source:RankingInspectSource,command:Record<string,unknown>,nowMs:number):boolean {
    this.observe(source);
    const active=this.active;
    if (!this.clock(nowMs)||!active||this.active!==active||active.proof!==proof||active.entered
      ||active.revision!==this.revision||this.exhausted||!sameOwner(proof.owner,source.owner)
      ||nowMs<active.nowMs||this.nextReadyMs!==active.nextReadyMs
      ||Object.keys(command).length!==4||command.type!=="inspect"||command.objectId!==proof.objectId
      ||command.ranking!==true||command.hero!==false) return false;
    active.entered=true;
    return true;
  }

  cancelDefinitelyUnsent(proof:RankingInspectProof):void {
    if (this.active?.proof===proof && !this.active.entered) this.active=null;
  }

  expectedName(owner:SocialReplyOwner|null):string|null {
    const active=this.active;
    return active?.entered && physicalOwner(active.proof.owner,owner) ? active.proof.expectedName : null;
  }

  receive(info:RankingPlayerInspect,source:RankingInspectSource):boolean {
    this.observe(source);
    const active=this.active;
    if (!active?.entered||!physicalOwner(active.proof.owner,source.owner)
      ||info.name!==active.proof.expectedName||info.isHero!==false) return false;
    this.active=null;
    return active.displayAllowed && active.revision===this.revision && sameOwner(active.proof.owner,source.owner);
  }

  closeDisplay():void {
    if (this.active?.entered) this.active.displayAllowed=false;
    else this.active=null;
  }

  retireConnection():void {
    this.active=null; this.source=null; this.nextReadyMs=0; this.lastClock=null;
    if (this.revision===Number.MAX_SAFE_INTEGER) this.exhausted=true;
    else this.revision++;
  }
}
