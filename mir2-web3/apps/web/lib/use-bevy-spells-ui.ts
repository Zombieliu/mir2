"use client";
import {useEffect,useRef,useState} from "react";
import {SpellsHost,parseSpellsIconMetadata,type SpellsInput,type SpellsRuntime,type SpellsIntent,type SpellsProof,type SpellsOutcome,type SpellsOwner,type SpellsRawSnapshot,type SpellsPointerEdge,type SpellsIconMetadata} from "./bevy-spells-ui";
type Options={requested:boolean;runtimeGeneration:number;runtimeRef:{current:SpellsRuntime|null};read:(metadata:SpellsIconMetadata|null)=>SpellsInput;readBaseline?:()=>{snapshot:SpellsRawSnapshot;owner:SpellsOwner}|null;onIntent:(intent:SpellsIntent,proof:SpellsProof)=>SpellsOutcome};
export function useBevySpellsUi(options:Options){
  const latest=useRef(options);latest.current=options;
  const host=useRef<SpellsHost|null>(null),metadata=useRef<SpellsIconMetadata|null>(null);
  const [ready,setReady]=useState(false);
  useEffect(()=>{
    const runtime=latest.current.runtimeRef.current;if(!options.requested||!runtime){setReady(false);return;}
    let live=true;let current:SpellsHost;
    const abort=new AbortController();
    try{current=new SpellsHost({runtime,isCurrent:()=>latest.current.runtimeRef.current===runtime&&latest.current.requested,read:()=>latest.current.read(metadata.current),now:()=>performance.now(),onState:v=>{if(live)setReady(v);},onIntent:(i,p)=>latest.current.onIntent(i,p)});}catch{setReady(false);return;}
    host.current=current;metadata.current=null;
    const baseline=latest.current.readBaseline?.();if(baseline)current.observeSnapshot(baseline.snapshot,baseline.owner);
    void fetch("/api/original-ui-meta?library=MagIcon2",{signal:abort.signal,cache:"no-store"}).then(r=>r.ok?r.json():null).then(v=>{if(live){metadata.current=parseSpellsIconMetadata(v);current.tick();}}).catch(()=>{if(live){metadata.current=null;current.tick();}});
    current.tick();const timer=window.setInterval(()=>current.tick(),50);
    const cancel=()=>{current.withdraw();setReady(false);};
    window.addEventListener("blur",cancel);window.addEventListener("resize",cancel);document.addEventListener("visibilitychange",cancel);
    return()=>{live=false;abort.abort();window.clearInterval(timer);window.removeEventListener("blur",cancel);window.removeEventListener("resize",cancel);document.removeEventListener("visibilitychange",cancel);current.stop();if(host.current===current)host.current=null;};
  },[options.requested,options.runtimeGeneration]);
  return{ready,withdraw:()=>host.current?.withdraw(),pointerContext:()=>host.current?.pointerContext()??null,pointer:(e:SpellsPointerEdge)=>host.current?.pointer(e)??false,allows:(p:SpellsProof)=>host.current?.allows(p)??false,
    observeSnapshot:(s:SpellsRawSnapshot,o:SpellsOwner)=>host.current?.observeSnapshot(s,o)??false,observePacket:(p:string,v:Record<string,unknown>,o:SpellsOwner)=>host.current?.observePacket(p,v,o)??false};
}
