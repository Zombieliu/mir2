"use client";
import {useEffect,useRef,useState} from "react";
import {MailHost,burnMailRun,type MailInput,type MailRuntime,type MailIntent,type MailOutcome,type MailPointerEdge} from "./bevy-mail-ui";
import {MailComposeHost,MailTextClipboard,type ClipboardGesture,type ComposeInput,type ComposeIntent,type ComposeRuntime,type ComposeTextOperation} from "./bevy-mail-text-input";
type Options={requested:boolean;runtimeGeneration:number;runtimeRef:{current:MailRuntime|null};read:()=>MailInput;onIntent:(intent:MailIntent)=>MailOutcome;
  compose?:{read:()=>ComposeInput|null;onIntent:(intent:ComposeIntent,host:MailComposeHost)=>void}};
export function useBevyMailUi(options:Options){
  const latest=useRef(options);latest.current=options;const host=useRef<MailHost|null>(null);const mounted=useRef(false);
  const composeHost=useRef<MailComposeHost|null>(null),clipboard=useRef<MailTextClipboard|null>(null);
  const retiring=useRef<{runtime:MailRuntime;host:MailHost;timer:number}|null>(null);
  const [ready,setReady]=useState(false),[withdrawn,setWithdrawn]=useState(true);
  const [composeReady,setComposeReady]=useState(false),[composeRequested,setComposeRequested]=useState(false),[composeWithdrawn,setComposeWithdrawn]=useState(true);
  const [composeText,setComposeText]=useState<ReturnType<MailComposeHost['textContext']>>(null);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;const prior=retiring.current;if(prior){window.clearInterval(prior.timer);prior.host.stop();retiring.current=null;}};},[]);
  useEffect(()=>{
    const runtime=latest.current.runtimeRef.current;
    const releaseRetired=()=>{const prior=retiring.current;if(prior&&latest.current.runtimeRef.current!==prior.runtime){window.clearInterval(prior.timer);prior.host.stop();retiring.current=null;}return !retiring.current;};
    if(!options.requested||!runtime){setReady(false);setWithdrawn(!runtime||releaseRetired());return;}
    let live=true,current:MailHost;
    const observe=()=>{releaseRetired();const prior=retiring.current;if(prior&&(prior.host.isWithdrawn(true)||current.isWithdrawn(true))){window.clearInterval(prior.timer);prior.host.stop();retiring.current=null;}
      if(live)setWithdrawn(!retiring.current&&current.isWithdrawn());};
    try{current=new MailHost({runtime,isCurrent:()=>latest.current.runtimeRef.current===runtime&&latest.current.requested,read:()=>latest.current.read(),now:()=>performance.now(),onState:v=>{if(live)setReady(v);},onIntent:i=>latest.current.onIntent(i)});}catch{setReady(false);setWithdrawn(releaseRetired());return;}
    host.current=current;current.tick();observe();const timer=window.setInterval(()=>{current.tick();observe();},50),cancel=()=>{current.withdraw();observe();};
    window.addEventListener("blur",cancel);window.addEventListener("resize",cancel);document.addEventListener("visibilitychange",cancel);
    return()=>{
      live=false;window.clearInterval(timer);window.removeEventListener("blur",cancel);window.removeEventListener("resize",cancel);document.removeEventListener("visibilitychange",cancel);current.withdraw();if(host.current===current)host.current=null;
      if(!mounted.current||latest.current.runtimeRef.current!==runtime||current.isWithdrawn(true)){current.stop();if(mounted.current&&!host.current)setWithdrawn(true);return;}
      const prior=retiring.current;if(prior){window.clearInterval(prior.timer);prior.host.stop();}
      const pending={runtime,host:current,timer:0};retiring.current=pending;setWithdrawn(false);
      pending.timer=window.setInterval(()=>{
        if(retiring.current!==pending){window.clearInterval(pending.timer);return;}
        const replacement=host.current;
        if(latest.current.runtimeRef.current!==runtime||current.isWithdrawn(true)||replacement?.isWithdrawn(true)){
          window.clearInterval(pending.timer);current.stop();retiring.current=null;if(mounted.current)setWithdrawn(replacement?.isWithdrawn()??true);
        }
      },50);
    };
  },[options.requested,options.runtimeGeneration]);
  useEffect(()=>{
    const runtime=latest.current.runtimeRef.current as (MailRuntime&ComposeRuntime)|null;
    if(!options.requested||!runtime||!latest.current.compose){setComposeReady(false);setComposeRequested(false);setComposeWithdrawn(true);setComposeText(null);return;}
    let live=true;const current=new MailComposeHost(burnMailRun(),{
      runtime,isCurrent:()=>latest.current.runtimeRef.current===runtime&&latest.current.requested,
      read:()=>latest.current.compose?.read()??null,onIntent:(intent,h)=>latest.current.compose?.onIntent(intent,h),
    });
    const clip=new MailTextClipboard(current.edges,()=>current.readTextState(),{readText:()=>navigator.clipboard.readText()});
    composeHost.current=current;clipboard.current=clip;
    const observe=()=>{current.tick();if(live){setComposeReady(current.ready);setComposeRequested(current.requested);setComposeWithdrawn(current.withdrawn);setComposeText(current.textContext());}};
    observe();const timer=window.setInterval(observe,50),cancel=()=>{current.withdraw();clip.clear();observe();};
    window.addEventListener('blur',cancel);window.addEventListener('resize',cancel);document.addEventListener('visibilitychange',cancel);
    return()=>{live=false;window.clearInterval(timer);window.removeEventListener('blur',cancel);window.removeEventListener('resize',cancel);document.removeEventListener('visibilitychange',cancel);
      current.stop();clip.clear();if(composeHost.current===current)composeHost.current=null;if(clipboard.current===clip)clipboard.current=null;};
  },[options.requested,options.runtimeGeneration]);
  return{ready,withdrawn,withdraw:()=>{host.current?.withdraw();setWithdrawn(!retiring.current&&(host.current?.isWithdrawn()??true));},allows:(i:MailIntent)=>host.current?.allows(i)??false,claim:(i:MailIntent)=>host.current?.claim(i)??false,
    pointerContext:()=>host.current?.pointerContext()??null,pointer:(e:MailPointerEdge)=>host.current?.pointer(e)??false,
    composeReady,composeRequested,composeWithdrawn,composeText,composeWithdraw:()=>composeHost.current?.withdraw(),
    composeBusy:()=>composeHost.current?.edges.pendingCount??0,
    composeEdge:(operation:ComposeTextOperation)=>composeHost.current?.edges.enqueue(operation)??false,
    composeAction:(action:string)=>composeHost.current?.action(action)??false,
    composeClipboard:(kind:'copy'|'cut'|'paste',gesture:ClipboardGesture)=>clipboard.current?.gesture(kind,gesture)??false,
    composeClipboardIntent:(intent:ComposeIntent)=>clipboard.current?.onRequest(intent),
    composePointer:(pointerId:number,phase:'down'|'move'|'up'|'cancel',x:number,y:number,shift:boolean)=>
      composeHost.current?.pointer(pointerId,phase,x,y,shift)??false,
    composeCancelPointer:()=>composeHost.current?.cancelPointer(),
    composeHost:()=>composeHost.current};
}
