"use client";
import {useEffect,useRef,useState} from "react";
import {CombatHost,supportsCombat,type CombatRuntime,type CombatOwner,type CombatRaw,type CombatFacts,type CombatProof,type CombatAction,type CombatUiChannel} from "./bevy-combat-input";
type Options={runtimeGeneration:number;runtimeRef:{current:CombatRuntime|null};read:()=>{owner:CombatOwner|null;facts:CombatFacts|null};readBaseline:()=>{snapshot:CombatRaw;owner:CombatOwner}|null;onWire:(proof:CombatProof,body:string)=>"confirmedSend"|"definitelyUnsent"|"outcomeUnknown";onApproach:(x:number,y:number)=>void;onClear:()=>void};
/** Event edges and lifetime only. Gameplay decisions execute in the common Rust controller. */
export function useBevyCombatInput(options:Options){
 const latest=useRef(options);latest.current=options;const host=useRef<CombatHost|null>(null);const [ready,setReady]=useState(false);
 useEffect(()=>{const runtime=latest.current.runtimeRef.current;if(!runtime){setReady(false);return;}let live=true;let current:CombatHost;
  try{current=new CombatHost({runtime,isCurrent:()=>latest.current.runtimeRef.current===runtime,read:()=>latest.current.read(),now:()=>performance.now(),onReady:v=>{if(live)setReady(v);},onWire:(p,b)=>latest.current.onWire(p,b),onApproach:(x,y)=>latest.current.onApproach(x,y),onClear:()=>latest.current.onClear()});}catch{setReady(false);return;}
  host.current=current;const baseline=latest.current.readBaseline();if(baseline)current.observeSnapshot(baseline.snapshot,baseline.owner);current.tick();const timer=window.setInterval(()=>current.tick(),50);
  const cancel=()=>{current.clearUiHeld();current.withdraw();setReady(false);latest.current.onClear();};window.addEventListener("blur",cancel);window.addEventListener("resize",cancel);document.addEventListener("visibilitychange",cancel);
  return()=>{live=false;window.clearInterval(timer);window.removeEventListener("blur",cancel);window.removeEventListener("resize",cancel);document.removeEventListener("visibilitychange",cancel);current.stop();if(host.current===current)host.current=null;};
 },[options.runtimeGeneration]);
 return{ready,fresh:()=>host.current?.fresh()??false,setUiHeld:(channel:CombatUiChannel,token:object,held:boolean)=>host.current?.setUiHeld(channel,token,held)??false,hasUiHeld:()=>host.current?.hasUiHeld()??false,supported:()=>supportsCombat(latest.current.runtimeRef.current),edge:(a:CombatAction)=>host.current?.edge(a)??false,allows:(p:CombatProof)=>host.current?.allows(p)??false,claim:(p:CombatProof,b:string)=>host.current?.claim(p,b)??false,withdraw:()=>host.current?.withdraw(),observeSnapshot:(s:CombatRaw,o:CombatOwner)=>host.current?.observeSnapshot(s,o)??false,observePacket:(p:string,v:Record<string,unknown>,o:CombatOwner)=>host.current?.observePacket(p,v,o)??false};
}
