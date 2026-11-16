"use client";
import {useEffect,useMemo,useRef,useState} from "react";
import {BevyHeroHost,type HeroHostOptions,type HeroHostState,type HeroPointerEdge,type HeroKeyEdge,type HeroSharedIntent} from "./bevy-hero-host";
import type {HeroRuntime} from "./bevy-hero-ui";

export type UseBevyHeroUiOptions=Omit<HeroHostOptions,"runtime"|"now"|"isCurrent"|"onState"> & Readonly<{
  requested:boolean;runtimeGeneration:number;runtimeRef:{current:HeroRuntime|null};
}>;
const inactive:HeroHostState=Object.freeze({active:false,transitioning:false,owned:false,worldBlocked:false,webLeaseToken:null,error:null});
/** Owns only the renderer host. The Page's raw ingress, source clock, original
 * receipt stream and operation ledger deliberately outlive this effect. */
export function useBevyHeroUi(options:UseBevyHeroUiOptions){
  const latest=useRef(options);latest.current=options;
  const host=useRef<BevyHeroHost|null>(null);
  const [state,setState]=useState<HeroHostState>(inactive);
  useEffect(()=>{
    const runtime=latest.current.runtimeRef.current;
    if(!options.requested||!runtime){setState(inactive);return;}
    let live=true;
    const current:BevyHeroHost=new BevyHeroHost({runtime,now:()=>performance.now(),
      isCurrent:()=>latest.current.requested&&latest.current.runtimeRef.current===runtime,
      read:()=>{const value=latest.current.read();return {...value,eligible:value.eligible&&document.visibilityState==="visible"&&document.hasFocus()};},
      onOwner:token=>{if(host.current===current)latest.current.onOwner(token);},
      onIntent:intent=>live&&host.current===current&&latest.current.onIntent(intent),
      onState:next=>{if(live&&host.current===current)setState(next);},
    });
    host.current=current;current.tick();
    const timer=window.setInterval(()=>current.tick(),50),pause=()=>current.withdraw();
    window.addEventListener("blur",pause);window.addEventListener("resize",pause);document.addEventListener("visibilitychange",pause);
    return()=>{
      live=false;window.clearInterval(timer);
      window.removeEventListener("blur",pause);window.removeEventListener("resize",pause);document.removeEventListener("visibilitychange",pause);
      // Expected-sink cleanup and synchronous DOM token revocation still run;
      // only a replacement effect is permitted to install its new ownership.
      current.stop();if(host.current===current)host.current=null;
    };
  },[options.requested,options.runtimeGeneration]);
  const methods=useMemo(()=>({
    peek:()=>host.current?.peek()??inactive,
    tick:()=>host.current?.tick(),withdraw:()=>host.current?.withdraw(),
    isSharedOwner:()=>host.current?.isSharedOwner()??false,
    blocksWorld:()=>host.current?.blocksWorld()??false,
    pointerContext:()=>host.current?.pointerContext()??null,
    pointer:(edge:HeroPointerEdge)=>host.current?.pointer(edge)??false,
    key:(edge:HeroKeyEdge)=>host.current?.key(edge)??false,
    allows:(intent:HeroSharedIntent)=>host.current?.allows(intent)??false,
    claimCurrent:(intent:HeroSharedIntent)=>host.current?.claimCurrent(intent)??false,
  }),[]);
  // JSX reads owned/worldBlocked. Live methods belong to event/claim paths.
  return {...state,...methods};
}
