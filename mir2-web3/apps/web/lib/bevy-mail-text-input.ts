/** Thin browser input transport for the shared Rust mail editor. No text policy lives here. */
export type ComposeOwner={run:number;connectionGeneration:number;sessionGeneration:number;ownerRevision:number;sceneRevision:number;hudGeneration:number;playerObjectId:number};
export type ComposeProof={owner:ComposeOwner;incarnation:number;draftEpoch:number;draftGeneration:string;editorRevision:number;focusGeneration:number;presentationRevision:number;layoutRevision:number;sequence:number};
export type ComposeRaw={to:string;subject:string;body:string;goldText:string;items:string[];attachmentUniqueIds:string[];stamped:boolean;attachmentUniqueIdsPresent:boolean;stampedPresent:boolean};
export type ComposeRect={left:number;top:number;width:number;height:number};
export type ComposeCaret=ComposeRect&{anchorUtf16:number;caretUtf16:number;editorRevision:number;layoutRevision:number};
export type ComposeStatus={version:1;owner:ComposeOwner;ready:boolean;inputEnabled:boolean;modal:boolean;proof:ComposeProof|null;inputRegions:ComposeRect[];caret:ComposeCaret|null;scroll:[number,number];error:string|null;
  textTarget:'body'|'recipient'|'gold'|null;promptText:string|null;promptViewport:ComposeRect|null;bodyViewport:ComposeRect|null};
export type ComposeTextOperation=
  |{op:'focus';focused:boolean}|{op:'selection';anchorUtf16:number;caretUtf16:number}
  |{op:'insert'|'replaceBody';text:string}|{op:'key';key:'left'|'right'|'up'|'down'|'home'|'end'|'backspace'|'delete'|'enter'|'selectAll';control:boolean;shift:boolean}
  |{op:'pointer';x:number;y:number;extend:boolean}|{op:'wheel';upwardPixels:number}
  |{op:'imeStart'|'imeCancel';compositionId:number}|{op:'imePreview';compositionId:number;text:string;cursorUtf16:null|[number,number]}|{op:'imeCommit';compositionId:number;text:string}
  |{op:'clipboardRequest';requestId:number;kind:'copy'|'cut'|'paste'}|{op:'clipboardResult';requestId:number;success:boolean;text:string}|{op:'submit'}
  |{op:'promptReplace';target:'recipient'|'gold';text:string};
export type ComposeIntent={proof:ComposeProof;action:'edit'|'focus'|'clipboardRequest'|'submit'|'rejected'|'close'|'cancel'|'recipient'|'gold'|'stamp'|'slot'|'recipientSubmit'|'recipientCancel'|'feedback'|'promptEdit'|'goldConfirm'|'goldCancel';baseRaw:ComposeRaw;bodyMutation:string|null;recipientMutation:string|null;requestId:number|null;clipboardKind:'copy'|'cut'|'paste'|null;selectedText:string|null;error:string|null;
  promptMutation:{target:'recipient'|'gold';text:string}|null;promptValue:number|null};
export type ComposePointerEdge={proof:ComposeProof;pointerId:number;phase:'down'|'move'|'up'|'cancel';x:number;y:number;button:0;shift:boolean};
export type ComposeRuntime={getMir2MailComposeUiCapabilities?:()=>string;setMir2MailComposeUiSnapshot?:(json:string)=>boolean;getMir2MailComposeUiStatus?:()=>string;setMir2MailTextEdge?:(json:string)=>boolean;setMir2MailComposeUiActionEdge?:(json:string)=>boolean;setMir2MailComposeUiPointerEdge?:(json:string)=>boolean;setMir2MailComposeUiIntentSink?:(sink:(json:string)=>void)=>void;clearMir2MailComposeUiIntentSink?:()=>void};
export type ComposePresentation={logicalWidth:number;logicalHeight:number;stageCssScale:number;touch:boolean};
export type ComposeParcelCell={uniqueId:number;image:number|null;countLabel:string};
export type ComposeParcel={stamped:boolean;stampAvailable:boolean;quoteReady:boolean;postage:number|null;quoteError:string|null;cells:ComposeParcelCell[]};
export type ComposeSnapshot={owner:ComposeOwner;revision:number;incarnation:number;draftEpoch:number;draftGeneration:string;presentationRevision:number;layoutRevision:number;open:boolean;inputEnabled:boolean;kind:'letter'|'parcel';presentation:ComposePresentation;raw:ComposeRaw;gold:number;parcel:ComposeParcel;notice:string|null;recipientPrompt:string|null;feedback:string|null;
  goldPrompt:{draft:string;maxAmount:number;amount:number|null}|null};
export type ComposeInput=Omit<ComposeSnapshot,'owner'|'revision'|'presentationRevision'|'layoutRevision'|'inputEnabled'>&{owner:Omit<ComposeOwner,'run'>;eligible:boolean};

const keys=(v:Record<string,unknown>,expected:readonly string[])=>Object.keys(v).sort().join('\0')===[...expected].sort().join('\0');
const record=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const counter=(v:unknown,zero=false):v is number=>Number.isSafeInteger(v)&&Number(v)>=(zero?0:1);
const decimal=(v:unknown):v is string=>typeof v==='string'&&/^(?:[1-9][0-9]{0,19})$/.test(v)&&BigInt(v)<=18446744073709551615n;
const utf8=(v:unknown,max:number):v is string=>typeof v==='string'&&!/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(v)&&new TextEncoder().encode(v).length<=max;
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const rect=(v:unknown):v is ComposeRect=>record(v)&&keys(v,['left','top','width','height'])&&finite(v.left)&&finite(v.top)&&finite(v.width)&&finite(v.height)&&v.width>0&&v.height>0;
const owner=(v:unknown):v is ComposeOwner=>record(v)&&keys(v,['run','connectionGeneration','sessionGeneration','ownerRevision','sceneRevision','hudGeneration','playerObjectId'])&&
  ['run','connectionGeneration','sessionGeneration','sceneRevision','hudGeneration','playerObjectId'].every(k=>counter(v[k]))&&counter(v.ownerRevision,true)&&Number(v.playerObjectId)<=0xffffffff;
export function sameComposeOwner(a:ComposeOwner,b:ComposeOwner){return a.run===b.run&&a.connectionGeneration===b.connectionGeneration&&
  a.sessionGeneration===b.sessionGeneration&&a.ownerRevision===b.ownerRevision&&a.sceneRevision===b.sceneRevision&&
  a.hudGeneration===b.hudGeneration&&a.playerObjectId===b.playerObjectId;}
export function validComposeRaw(v:unknown):v is ComposeRaw{
  return record(v)&&keys(v,['to','subject','body','goldText','items','attachmentUniqueIds','stamped','attachmentUniqueIdsPresent','stampedPresent'])&&
    utf8(v.to,1024)&&utf8(v.subject,4096)&&utf8(v.body,8192)&&utf8(v.goldText,256)&&
    Array.isArray(v.items)&&v.items.length<=5&&v.items.every(i=>utf8(i,1024))&&
    Array.isArray(v.attachmentUniqueIds)&&v.attachmentUniqueIds.length<=5&&v.attachmentUniqueIds.every(i=>decimal(i)&&BigInt(i)<=9007199254740991n)&&
    new Set(v.attachmentUniqueIds).size===v.attachmentUniqueIds.length&&typeof v.stamped==='boolean'&&
    typeof v.attachmentUniqueIdsPresent==='boolean'&&typeof v.stampedPresent==='boolean'&&
    (v.attachmentUniqueIdsPresent||v.attachmentUniqueIds.length===0)&&(v.stampedPresent||v.stamped===false);
}
export function validComposeProof(v:unknown):v is ComposeProof{
  return record(v)&&keys(v,['owner','incarnation','draftEpoch','draftGeneration','editorRevision','focusGeneration','presentationRevision','layoutRevision','sequence'])&&owner(v.owner)&&
    ['incarnation','draftEpoch','editorRevision','focusGeneration','presentationRevision','layoutRevision','sequence'].every(k=>counter(v[k]))&&decimal(v.draftGeneration);
}
export function supportsComposeUi(runtime:ComposeRuntime|null):runtime is Required<ComposeRuntime>{
  try{const v=JSON.parse(runtime?.getMir2MailComposeUiCapabilities?.()??'null');return record(v)&&keys(v,['schemaVersion','mailComposeUiAbiVersion','textAdapterAbiVersion','compiled','startup'])&&
    v.schemaVersion===1&&v.mailComposeUiAbiVersion===1&&v.textAdapterAbiVersion===1&&v.compiled===true&&v.startup===true&&
    [runtime?.setMir2MailComposeUiSnapshot,runtime?.getMir2MailComposeUiStatus,runtime?.setMir2MailTextEdge,runtime?.setMir2MailComposeUiActionEdge,runtime?.setMir2MailComposeUiPointerEdge,runtime?.setMir2MailComposeUiIntentSink,runtime?.clearMir2MailComposeUiIntentSink].every(f=>typeof f==='function');
  }catch{return false;}
}
export function readComposeStatus(runtime:ComposeRuntime):ComposeStatus|null{
  try{const v=JSON.parse(runtime.getMir2MailComposeUiStatus?.()??'null');if(!record(v)||!keys(v,['version','owner','ready','inputEnabled','modal','proof','inputRegions','caret','scroll','error','textTarget','promptText','promptViewport','bodyViewport'])||v.version!==1||!owner(v.owner)||
    typeof v.ready!=='boolean'||typeof v.inputEnabled!=='boolean'||typeof v.modal!=='boolean'||!(v.proof===null||validComposeProof(v.proof))||
    !Array.isArray(v.inputRegions)||v.inputRegions.length>16||!v.inputRegions.every(rect)||
    !(v.caret===null||record(v.caret)&&keys(v.caret,['left','top','width','height','anchorUtf16','caretUtf16','editorRevision','layoutRevision'])&&
      ['left','top','width','height'].every(k=>finite((v.caret as Record<string,unknown>)[k]))&&
      ['anchorUtf16','caretUtf16'].every(k=>counter((v.caret as Record<string,unknown>)[k],true))&&
      ['editorRevision','layoutRevision'].every(k=>counter((v.caret as Record<string,unknown>)[k])))||
    !Array.isArray(v.scroll)||v.scroll.length!==2||!v.scroll.every(finite)||!(v.error===null||utf8(v.error,1024))||
    !(v.textTarget===null||['body','recipient','gold'].includes(String(v.textTarget)))||!(v.promptText===null||utf8(v.promptText,1024))||
    !(v.promptViewport===null||rect(v.promptViewport))||!(v.bodyViewport===null||rect(v.bodyViewport))||
    (v.proof!==null&&!sameComposeOwner(v.proof.owner,v.owner)))return null;
    return v as ComposeStatus;
  }catch{return null;}
}
export function parseComposeIntent(json:string):ComposeIntent|null{
  try{if(!utf8(json,32768))return null;const v=JSON.parse(json);if(!record(v)||!keys(v,['proof','action','baseRaw','bodyMutation','recipientMutation','requestId','clipboardKind','selectedText','error','promptMutation','promptValue'])||
    !validComposeProof(v.proof)||!validComposeRaw(v.baseRaw)||!['edit','focus','clipboardRequest','submit','rejected','close','cancel','recipient','gold','stamp','slot','recipientSubmit','recipientCancel','feedback','promptEdit','goldConfirm','goldCancel'].includes(String(v.action))||
    !(v.bodyMutation===null||utf8(v.bodyMutation,8192))||!(v.recipientMutation===null||utf8(v.recipientMutation,1024))||
    !(v.requestId===null||counter(v.requestId,true))||!(v.clipboardKind===null||['copy','cut','paste'].includes(String(v.clipboardKind)))||
    !(v.selectedText===null||utf8(v.selectedText,8192))||!(v.error===null||utf8(v.error,1024))||
    !(v.promptMutation===null||record(v.promptMutation)&&keys(v.promptMutation,['target','text'])&&['recipient','gold'].includes(String(v.promptMutation.target))&&
      utf8(v.promptMutation.text,v.promptMutation.target==='recipient'?1024:256))||
    !(v.promptValue===null||counter(v.promptValue,true)&&Number(v.promptValue)<=0xffffffff))return null;
    return v as ComposeIntent;
  }catch{return null;}
}
export function validComposeSnapshot(v:unknown):v is ComposeSnapshot{
  if(!record(v)||!keys(v,['owner','revision','incarnation','draftEpoch','draftGeneration','presentationRevision','layoutRevision','open','inputEnabled','kind','presentation','raw','gold','parcel','notice','recipientPrompt','feedback','goldPrompt'])||
    !owner(v.owner)||!['revision','incarnation','draftEpoch','presentationRevision','layoutRevision'].every(k=>counter(v[k]))||!decimal(v.draftGeneration)||
    typeof v.open!=='boolean'||typeof v.inputEnabled!=='boolean'||!['letter','parcel'].includes(String(v.kind))||!validComposeRaw(v.raw)||
    !record(v.presentation)||!keys(v.presentation,['logicalWidth','logicalHeight','stageCssScale','touch'])||
    !['logicalWidth','logicalHeight','stageCssScale'].every(k=>finite((v.presentation as Record<string,unknown>)[k]))||
    Number(v.presentation.stageCssScale)<=0||typeof v.presentation.touch!=='boolean'||!counter(v.gold,true)||Number(v.gold)>0xffffffff||
    !record(v.parcel)||!keys(v.parcel,['stamped','stampAvailable','quoteReady','postage','quoteError','cells'])||
    !['stamped','stampAvailable','quoteReady'].every(k=>typeof (v.parcel as Record<string,unknown>)[k]==='boolean')||
    !(v.parcel.postage===null||counter(v.parcel.postage,true)&&Number(v.parcel.postage)<=0xffffffff)||
    !(v.parcel.quoteError===null||utf8(v.parcel.quoteError,1024))||!Array.isArray(v.parcel.cells)||v.parcel.cells.length>5||
    !v.parcel.cells.every((c:unknown)=>record(c)&&keys(c,['uniqueId','image','countLabel'])&&counter(c.uniqueId)&&
      (c.image===null||counter(c.image,true)&&Number(c.image)<=65535)&&utf8(c.countLabel,1024))||
    !(v.notice===null||utf8(v.notice,8192))||!(v.recipientPrompt===null||utf8(v.recipientPrompt,1024))||
    !(v.feedback===null||utf8(v.feedback,8192))||
    [v.recipientPrompt,v.feedback,v.goldPrompt].filter(p=>p!==null).length>1||
    !(v.goldPrompt===null||record(v.goldPrompt)&&keys(v.goldPrompt,['draft','maxAmount','amount'])&&utf8(v.goldPrompt.draft,256)&&
      counter(v.goldPrompt.maxAmount,true)&&Number(v.goldPrompt.maxAmount)<=0xffffffff&&
      (v.goldPrompt.amount===null||counter(v.goldPrompt.amount,true)&&Number(v.goldPrompt.amount)<=Number(v.goldPrompt.maxAmount))))return false;
  return utf8(JSON.stringify(v),32768);
}
export function composeLandscape(p:ComposePresentation){const cssWidth=p.logicalWidth*p.stageCssScale,cssHeight=p.logicalHeight*p.stageCssScale;
  return Number.isFinite(cssWidth)&&Number.isFinite(cssHeight)&&cssWidth>=600&&cssHeight>=320&&cssWidth>cssHeight&&p.stageCssScale>0&&p.stageCssScale<=16;
}
export function composeStagePoint(rect:ComposeRect,p:ComposePresentation,clientX:number,clientY:number){
  if(!rect.width||!rect.height||!finite(clientX)||!finite(clientY)||!finite(p.logicalWidth)||!finite(p.logicalHeight))return null;
  return{x:(clientX-rect.left)*p.logicalWidth/rect.width,y:(clientY-rect.top)*p.logicalHeight/rect.height};
}
export function sameComposeRaw(a:ComposeRaw,b:ComposeRaw){return a.to===b.to&&a.subject===b.subject&&a.body===b.body&&a.goldText===b.goldText&&
  a.stamped===b.stamped&&a.attachmentUniqueIdsPresent===b.attachmentUniqueIdsPresent&&a.stampedPresent===b.stampedPresent&&
  a.items.length===b.items.length&&a.items.every((item,i)=>item===b.items[i])&&
  a.attachmentUniqueIds.length===b.attachmentUniqueIds.length&&a.attachmentUniqueIds.every((id,i)=>id===b.attachmentUniqueIds[i]);}
export function sameComposeScope(a:ComposeProof,b:ComposeProof){return sameComposeOwner(a.owner,b.owner)&&a.incarnation===b.incarnation&&a.draftEpoch===b.draftEpoch&&a.focusGeneration===b.focusGeneration&&a.presentationRevision===b.presentationRevision&&a.layoutRevision===b.layoutRevision;}
export function sameComposeAuthority(a:ComposeProof,b:ComposeProof){return sameComposeScope(a,b)&&a.draftGeneration===b.draftGeneration&&a.editorRevision===b.editorRevision;}

/** JS string indices are UTF-16. Rust editor offsets are UTF-8 byte offsets. */
export function utf16ToUtf8(text:string,index:number):number|null{
  if(!utf8(text,8192)||!counter(index,true)||index>text.length||index>0&&index<text.length&&/^[\uDC00-\uDFFF]$/.test(text[index]))return null;
  return new TextEncoder().encode(text.slice(0,index)).length;
}
export function utf8ToUtf16(text:string,offset:number):number|null{
  if(!utf8(text,8192)||!counter(offset,true))return null;let bytes=0,units=0;
  for(const scalar of text){if(bytes===offset)return units;bytes+=new TextEncoder().encode(scalar).length;units+=scalar.length;if(bytes>offset)return null;}
  return bytes===offset?units:null;
}
type Read=()=>{status:ComposeStatus|null;raw:ComposeRaw|null};
type QueueEntry={operation:ComposeTextOperation;scope:ComposeProof;base:ComposeRaw};
type InFlight=QueueEntry&{sentSequence:number;ownRaw:ComposeRaw|null;ownGeneration:string|null};
/** Browser input buffering only. One Rust edge is in flight; Rust owns all editing rules. */
export class MailTextEdgeQueue{
  private pending:QueueEntry[]=[];private inFlight:InFlight|null=null;private highwater=0;
  constructor(private runtime:ComposeRuntime,private read:Read,private onAdmit?:(proof:ComposeProof,raw:ComposeRaw)=>void,
    private allocate?:(proof:ComposeProof)=>number|null){}
  get pendingCount(){return this.pending.length+(this.inFlight?1:0);}
  clear(){this.pending=[];this.inFlight=null;}
  enqueue(operation:ComposeTextOperation){const {status,raw}=this.read(),proof=status?.proof;
    if(operation.op==='imePreview'&&(!utf8(operation.text,8192)||
      !(operation.cursorUtf16===null||Array.isArray(operation.cursorUtf16)&&operation.cursorUtf16.length===2&&
        operation.cursorUtf16[0]<=operation.cursorUtf16[1]&&
        utf16ToUtf8(operation.text,operation.cursorUtf16[0])!==null&&
        utf16ToUtf8(operation.text,operation.cursorUtf16[1])!==null)))return false;
    if(!status?.ready||!status.inputEnabled||!proof||!raw||!validComposeRaw(raw))return false;
    const first=this.inFlight?.scope??this.pending[0]?.scope;
    if(first&&!sameComposeScope(first,proof)){this.clear();return false;}
    if(!this.inFlight&&first&&(!sameComposeAuthority(first,proof)||first.sequence!==proof.sequence||
      !sameComposeRaw(this.pending[0].base,raw)))this.clear();
    const last=this.pending[this.pending.length-1]?.operation;
    if(last&&operation.op==='imePreview'&&last.op==='imePreview'&&last.compositionId===operation.compositionId)
      this.pending[this.pending.length-1]={operation,scope:proof,base:raw};
    else if(last&&operation.op==='promptReplace'&&last.op==='promptReplace'&&last.target===operation.target)
      this.pending[this.pending.length-1]={operation,scope:proof,base:raw};
    else this.pending.push({operation,scope:proof,base:raw});
    this.tick();return true;
  }
  /** Called only after the Page sink has synchronously accepted a Core full-raw edit. */
  ownEdit(raw:ComposeRaw,generation:string){if(this.inFlight&&validComposeRaw(raw)&&decimal(generation)){this.inFlight.ownRaw=raw;this.inFlight.ownGeneration=generation;}}
  tick(){const {status,raw}=this.read(),proof=status?.proof;if(!status?.ready||!status.inputEnabled||!proof||!raw)return;
    if(this.inFlight){const f=this.inFlight;if(!sameComposeScope(f.scope,proof)){this.clear();return;}
      if(proof.sequence<f.sentSequence)return;
      if(f.ownRaw){if(!sameComposeRaw(raw,f.ownRaw)||proof.draftGeneration!==f.ownGeneration){this.clear();return;}}
      else if(!sameComposeRaw(raw,f.base)||proof.draftGeneration!==f.scope.draftGeneration){this.clear();return;}
      this.inFlight=null;
      // Only a terminal edge actually admitted by Rust may carry queued browser input
      // onto a new proof. A refused setter has no such provenance.
      for(const entry of this.pending){entry.scope=proof;entry.base=raw;}
    }
    const next=this.pending[0];if(!next)return;
    if(!sameComposeAuthority(next.scope,proof)||next.scope.sequence!==proof.sequence||!sameComposeRaw(next.base,raw)){
      this.clear();return;
    }
    const sequence=this.allocate?this.allocate(proof):Math.max(this.highwater,proof.sequence)+1;if(sequence===null||!Number.isSafeInteger(sequence))return;
    const edge={proof:{...proof,sequence},operation:next.operation};
    try{if(this.runtime.setMir2MailTextEdge?.(JSON.stringify(edge))!==true)return;}catch{return;}
    this.highwater=sequence;this.pending.shift();this.inFlight={...next,scope:proof,base:raw,sentSequence:sequence,ownRaw:null,ownGeneration:null};
    this.onAdmit?.(edge.proof,raw);
  }
}

export type ClipboardGesture={setData:(format:string,text:string)=>void;getData:(format:string)=>string;preventDefault:()=>void};
export type MailClipboardPort={readText:()=>Promise<string>};
type ClipboardCapture={id:number;kind:'copy'|'cut'|'paste';proof:ComposeProof;raw:ComposeRaw;anchor:number;caret:number;selected:string|null;copySucceeded:boolean;pasteText:string|null};
/** Browser clipboard effects are fenced to the exact published Rust selection. */
export class MailTextClipboard {
  private nextId=0;private latest:ClipboardCapture|null=null;
  constructor(private edges:MailTextEdgeQueue,private read:Read,private port:MailClipboardPort){}
  clear(){this.latest=null;}
  private capture(kind:ClipboardCapture['kind']):ClipboardCapture|null{
    const {status,raw}=this.read(),proof=status?.proof,caret=status?.caret;
    if(!status?.ready||!status.inputEnabled||status.textTarget!=='body'||!proof||!caret||!raw||caret.editorRevision!==proof.editorRevision||caret.layoutRevision!==proof.layoutRevision||
      utf16ToUtf8(raw.body,caret.anchorUtf16)===null||utf16ToUtf8(raw.body,caret.caretUtf16)===null||this.nextId>=Number.MAX_SAFE_INTEGER)return null;
    const start=Math.min(caret.anchorUtf16,caret.caretUtf16),end=Math.max(caret.anchorUtf16,caret.caretUtf16);
    return{id:++this.nextId,kind,proof,raw,anchor:caret.anchorUtf16,caret:caret.caretUtf16,
      selected:kind==='paste'?null:raw.body.slice(start,end),copySucceeded:false,pasteText:null};
  }
  private current(c:ClipboardCapture,requestSequence?:number){const {status,raw}=this.read(),proof=status?.proof,caret=status?.caret;
    return Boolean(this.latest===c&&status?.ready&&status.inputEnabled&&proof&&caret&&raw&&sameComposeScope(c.proof,proof)&&
      c.proof.draftGeneration===proof.draftGeneration&&c.proof.editorRevision===proof.editorRevision&&
      (requestSequence===undefined||proof.sequence<=requestSequence)&&
      caret.anchorUtf16===c.anchor&&caret.caretUtf16===c.caret&&sameComposeRaw(raw,c.raw));
  }
  gesture(kind:'copy'|'cut'|'paste',event:ClipboardGesture):boolean{
    const capture=this.capture(kind);if(!capture)return false;this.latest=capture;
    if(kind==='paste'){try{capture.pasteText=event.getData('text/plain');}catch{capture.pasteText=null;}event.preventDefault();}
    else {try{event.setData('text/plain',capture.selected??'');capture.copySucceeded=true;}catch{capture.copySucceeded=false;}
      event.preventDefault();}
    if(!this.edges.enqueue({op:'clipboardRequest',requestId:capture.id,kind})){this.latest=null;return false;}
    return true;
  }
  /** Invoked by the Rust intent sink. Old/reordered completions cannot consume a newer request. */
  async onRequest(intent:ComposeIntent){const c=this.latest;if(!c||intent.action!=='clipboardRequest'||intent.requestId!==c.id||intent.clipboardKind!==c.kind||
    !sameComposeAuthority(c.proof,intent.proof)||intent.proof.sequence<=c.proof.sequence||
    !sameComposeRaw(c.raw,intent.baseRaw)||!this.current(c,intent.proof.sequence))return;
    if(c.kind==='paste'&&c.pasteText===null){try{c.pasteText=await this.port.readText();}catch{c.pasteText=null;}}
    if(!this.current(c,intent.proof.sequence))return;
    const selectedMatches=c.kind==='paste'?intent.selectedText===null:intent.selectedText===c.selected;
    const success=selectedMatches&&(c.kind==='paste'?c.pasteText!==null:c.copySucceeded);
    const text=success?(c.kind==='paste'?c.pasteText??'':c.selected??''):'';
    this.latest=null;
    this.edges.enqueue({op:'clipboardResult',requestId:c.id,success,text});
  }
}

type HostOptions={runtime:ComposeRuntime;isCurrent:()=>boolean;read:()=>ComposeInput|null;onIntent:(intent:ComposeIntent,host:MailComposeHost)=>void};
type ComposeRuntimeOwner={run:number;host:MailComposeHost};
function composeOwners(){const realm=globalThis as typeof globalThis&{__mir2MailComposeRuntimeOwnersV1?:WeakMap<object,ComposeRuntimeOwner>};
  return realm.__mir2MailComposeRuntimeOwnersV1??=new WeakMap<object,ComposeRuntimeOwner>();}
/** Independent C2 painter ingress. Legacy inbox ABI and its capability remain untouched. */
export class MailComposeHost {
  private current:ComposeSnapshot|null=null;private key='';private revision=0;private presentationRevision=0;private layoutRevision=0;
  private active=false;private stopped=false;private seen=0;private everVisible=false;
  private admitted=new Map<number,{proof:ComposeProof;raw:ComposeRaw}>();
  private sequenceHighwater=0;private held:{pointerId:number;proof:ComposeProof;raw:ComposeRaw}|null=null;
  readonly edges:MailTextEdgeQueue;
  constructor(readonly run:number,private options:HostOptions){
    this.edges=new MailTextEdgeQueue(options.runtime,()=>({status:this.status(),raw:this.current?.raw??null}),
      (proof,raw)=>this.admitted.set(proof.sequence,{proof,raw}),proof=>this.allocate(proof));
    if(!counter(run)||!supportsComposeUi(options.runtime))return;
    const prior=composeOwners().get(options.runtime);if(prior&&prior.run>=run)return;
    composeOwners().set(options.runtime,{run,host:this});
    try{options.runtime.setMir2MailComposeUiIntentSink?.(json=>{const intent=parseComposeIntent(json);
      if(!intent||intent.proof.sequence<=this.seen||!this.qualify(intent))return;
      this.seen=intent.proof.sequence;this.admitted.delete(intent.proof.sequence);
      try{this.options.onIntent(intent,this);}finally{this.tick();}
    });}catch{this.stop();}
  }
  private owns(){const owner=composeOwners().get(this.options.runtime);return owner?.run===this.run&&owner.host===this;}
  private status(){return this.owns()?readComposeStatus(this.options.runtime):null;}
  private allocate(proof:ComposeProof){const next=Math.max(this.sequenceHighwater,proof.sequence)+1;
    if(!this.owns()||!Number.isSafeInteger(next))return null;this.sequenceHighwater=next;return next;}
  private qualify(intent:ComposeIntent){const sent=this.current,admitted=this.admitted.get(intent.proof.sequence);
    if(!sent||!this.owns())return false;const live=this.options.read();
    return Boolean(this.active&&admitted&&live&&this.options.isCurrent()&&sameComposeAuthority(admitted.proof,intent.proof)&&
      admitted.proof.sequence===intent.proof.sequence&&
      sameComposeRaw(intent.baseRaw,admitted.raw)&&sameComposeRaw(intent.baseRaw,sent.raw)&&
      sameComposeOwner({run:this.run,...live.owner},sent.owner));
  }
  tick(){if(this.stopped)return;if(!this.owns()){this.stop();return;}
    if(!supportsComposeUi(this.options.runtime)||!this.options.isCurrent()){this.withdraw();return;}
    const input=this.options.read();if(!input||!input.eligible||!input.open||!validComposeRaw(input.raw)||!composeLandscape(input.presentation)){this.withdraw();return;}
    const id={run:this.run,...input.owner};const present=JSON.stringify([id,input.kind,input.presentation,input.open]);
    if(present!==this.key){this.key=present;this.presentationRevision++;this.layoutRevision++;this.edges.clear();this.admitted.clear();this.held=null;}
    const {eligible:ignoredEligible,...payload}=input;void ignoredEligible;
    const snap:ComposeSnapshot={...payload,owner:id,revision:this.revision+1,presentationRevision:this.presentationRevision,layoutRevision:this.layoutRevision,inputEnabled:true};
    if(!validComposeSnapshot(snap)){this.withdraw();return;}
    const prior=this.current;if(!prior||JSON.stringify({...prior,revision:0})!==JSON.stringify({...snap,revision:0})){
      if(!counter(snap.revision)||this.options.runtime.setMir2MailComposeUiSnapshot?.(JSON.stringify(snap))!==true){this.withdraw();return;}
      this.revision=snap.revision;this.current=snap;
    }
    const status=this.status();this.active=Boolean(status?.ready&&status.inputEnabled&&status.proof&&
      this.current&&sameComposeOwner(status.owner,this.current.owner)&&status.proof.incarnation===this.current.incarnation&&status.proof.draftEpoch===this.current.draftEpoch&&
      status.proof.draftGeneration===this.current?.draftGeneration&&status.proof.presentationRevision===this.current?.presentationRevision&&
      status.proof.layoutRevision===this.current?.layoutRevision);
    if(status?.proof)for(const sequence of this.admitted.keys())if(sequence<=status.proof.sequence)this.admitted.delete(sequence);
    if(this.active)this.everVisible=true;this.edges.tick();
  }
  get ready(){return this.owns()&&this.active;}
  get requested(){return this.owns()&&this.current!==null;}
  get withdrawn(){return !this.owns()||!this.everVisible||this.current===null&&!this.status()?.inputEnabled;}
  readTextState(){return{status:this.status(),raw:this.current?.raw??null};}
  textContext(){const status=this.status();return this.active&&this.current&&status?{status,raw:this.current.raw,presentation:this.current.presentation,pendingEdges:this.edges.pendingCount}:null;}
  action(action:string){const status=this.status(),proof=status?.proof;
    if(!this.owns()||!this.active||!proof)return false;const sequence=this.allocate(proof);if(!sequence)return false;
    const edge={proof:{...proof,sequence},action};try{if(this.options.runtime.setMir2MailComposeUiActionEdge?.(JSON.stringify(edge))!==true)return false;
      if(this.current)this.admitted.set(sequence,{proof:edge.proof,raw:this.current.raw});return true;}catch{return false;}
  }
  pointer(pointerId:number,phase:ComposePointerEdge['phase'],x:number,y:number,shift:boolean){
    const status=this.status(),proof=status?.proof,raw=this.current?.raw,held=this.held;
    if(!this.owns()||!this.active||!status?.modal||!proof||!raw||!counter(pointerId,true)||!finite(x)||!finite(y))return false;
    if(phase==='down'){
      if(held||!status.inputRegions.some(r=>x>=r.left&&y>=r.top&&x<r.left+r.width&&y<r.top+r.height))return false;
      this.held={pointerId,proof,raw};
    }else if(!held||held.pointerId!==pointerId||!sameComposeAuthority(held.proof,proof)||!sameComposeRaw(held.raw,raw)){this.held=null;return false;}
    const sequence=this.allocate(proof);if(!sequence){this.held=null;return false;}
    const edge:ComposePointerEdge={proof:{...proof,sequence},pointerId,phase,x,y,button:0,shift};
    try{if(this.options.runtime.setMir2MailComposeUiPointerEdge?.(JSON.stringify(edge))!==true){this.held=null;return false;}
      this.admitted.set(sequence,{proof:edge.proof,raw});if(phase==='up'||phase==='cancel')this.held=null;return true;
    }catch{this.held=null;return false;}
  }
  cancelPointer(){const held=this.held;if(!held)return;this.pointer(held.pointerId,'cancel',0,0,false);this.held=null;}
  withdraw(){this.active=false;this.edges.clear();this.admitted.clear();this.held=null;this.key='';const old=this.current;this.current=null;
    if(old&&this.owns()&&this.revision<Number.MAX_SAFE_INTEGER){const closing={...old,revision:++this.revision,open:false,inputEnabled:false};try{this.options.runtime.setMir2MailComposeUiSnapshot?.(JSON.stringify(closing));}catch{/* retired module */}}
  }
  stop(){this.withdraw();this.stopped=true;if(this.owns()){composeOwners().delete(this.options.runtime);
      try{this.options.runtime.clearMir2MailComposeUiIntentSink?.();}catch{/* retired module */}}}
}
