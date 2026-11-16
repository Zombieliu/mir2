import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
let ts;
try {
  ts = require("typescript");
} catch {
  ts = require("../node_modules/.ignored/typescript/lib/typescript.js");
}

const modulePath = new URL("../lib/crystal-chat-history.ts", import.meta.url);
const source = readFileSync(modulePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(modulePath),
  reportDiagnostics: true,
});

const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, [], "crystal-chat-history.ts must transpile without diagnostics");

const module = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  module.exports,
  module,
  (specifier) => {
    throw new Error(`Unexpected test import: ${specifier}`);
  },
);

const {
  CRYSTAL_CHAT_LINE_COUNT,
  CRYSTAL_CHAT_STYLES,
  CRYSTAL_CHAT_WIDTH_PX,
  CrystalChatHistory,
  CrystalChatType,
  wrapCrystalChatText,
} = module.exports;

assert.equal(CRYSTAL_CHAT_WIDTH_PX, 614);
assert.equal(CRYSTAL_CHAT_LINE_COUNT, 4);
assert.deepEqual(Object.values(CrystalChatType), Array.from({ length: 17 }, (_, index) => index));
assert.equal(Object.keys(CRYSTAL_CHAT_STYLES).length, 17);

const expectedStyles = [
  [CrystalChatType.Normal, "#FF000000", "#FFFFFFFF", "normal"],
  [CrystalChatType.Shout, "#FF000000", "#FFFFFF00", "shout"],
  [CrystalChatType.System, "#FFFFFFFF", "#FFFF0000", "system"],
  [CrystalChatType.Hint, "#FF006400", "#FFFFFFFF", "hint"],
  [CrystalChatType.Announcement, "#FFFFFFFF", "#FF0000FF", "announcement"],
  [CrystalChatType.Group, "#FFA52A2A", "#FFFFFFFF", "group"],
  [CrystalChatType.WhisperIn, "#FF00008B", "#FFFFFFFF", "whisper"],
  [CrystalChatType.WhisperOut, "#FF6495ED", "#FFFFFFFF", "whisper"],
  [CrystalChatType.Guild, "#FF008000", "#FFFFFFFF", "guild"],
  [CrystalChatType.Trainer, "#FF000000", "#FFFFFFFF", "normal"],
  [CrystalChatType.LevelUp, "#FF0000FF", "#FFE1B9FA", "announcement"],
  [CrystalChatType.System2, "#FFFFFFFF", "#FF8B0000", "system"],
  [CrystalChatType.Relationship, "#FFFF69B4", "#00000000", "relationship"],
  [CrystalChatType.Mentor, "#FF800080", "#FFFFFFFF", "mentor"],
  [CrystalChatType.Shout2, "#FFFFFFFF", "#FF008000", "shout"],
  [CrystalChatType.Shout3, "#FFFFFFFF", "#FF800080", "shout"],
  [CrystalChatType.LineMessage, "#FFFFFFFF", "#FF0000FF", "line"],
];

for (const [type, ForeColour, BackColour, Channel] of expectedStyles) {
  assert.deepEqual(CRYSTAL_CHAT_STYLES[type], { ForeColour, BackColour, Channel });
}

const measuredLengths = [];
const exactLengthMeasure = (text) => {
  measuredLengths.push(text.length);
  return text.length;
};
assert.deepEqual(wrapCrystalChatText("a".repeat(614), exactLengthMeasure), ["a".repeat(614)]);
assert.deepEqual(wrapCrystalChatText("b".repeat(615), exactLengthMeasure), ["b".repeat(615)]);
assert.equal(Math.max(...measuredLengths), 614, "the source loop does not measure the complete 615-char value");
assert.deepEqual(wrapCrystalChatText("c".repeat(616), (text) => text.length), [
  "c".repeat(614),
  "c".repeat(2),
]);

assert.deepEqual(wrapCrystalChatText("abcdef", (text) => text.length * 205), ["ab", "cd", "ef"]);

const itemLinkCompatibilityText = "ABCDE12345 <X/1> tail";
assert.deepEqual(
  wrapCrystalChatText(itemLinkCompatibilityText, (text) =>
    text === "ABCDE1" || text === "12345 <X" ? 615 : 0,
  ),
  ["ABCDE", "12345", "2345 <X/1> tail"],
  "item-link wrapping must preserve Crystal's relative newIndex behavior",
);

assert.throws(
  () => wrapCrystalChatText("abc", () => Number.NaN),
  /finite non-negative width/,
);
assert.throws(
  () => new CrystalChatHistory(undefined),
  /injected text measure function/,
);

const scrolling = new CrystalChatHistory((text) => text.length);
assert.equal(scrolling.LineCount, 4);
for (const text of ["one", "two", "three", "four"]) {
  scrolling.receiveChat(text, CrystalChatType.Normal);
}
assert.equal(scrolling.StartIndex, 0);
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["one", "two", "three", "four"]);

scrolling.receiveChat("five", CrystalChatType.Normal);
assert.equal(scrolling.StartIndex, 1, "a new line must follow when the source is at its four-line tail");
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["two", "three", "four", "five"]);

scrolling.home();
assert.equal(scrolling.StartIndex, 0);
scrolling.receiveChat("six", CrystalChatType.Normal);
assert.equal(scrolling.StartIndex, 0, "a scrolled-up source view must not follow new lines");
scrolling.up();
assert.equal(scrolling.StartIndex, 0);
scrolling.down();
assert.equal(scrolling.StartIndex, 1);
scrolling.end();
assert.equal(scrolling.StartIndex, 5, "Crystal End points at History.Count - 1, not Count - LineCount");
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["six"]);
scrolling.down();
assert.equal(scrolling.StartIndex, 5);
scrolling.up();
assert.equal(scrolling.StartIndex, 4);
assert.deepEqual(scrolling.VisibleHistory.map((line) => line.Text), ["five", "six"]);
scrolling.home();
assert.equal(scrolling.StartIndex, 0);

const multilineFollow = new CrystalChatHistory((text) => text.length);
for (const text of ["one", "two", "three", "four"]) {
  multilineFollow.receiveChat(text, CrystalChatType.Normal);
}
multilineFollow.receiveChat("x".repeat(616), CrystalChatType.Normal);
assert.equal(multilineFollow.StartIndex, 2, "auto-follow must advance by the number of wrapped lines");
assert.deepEqual(multilineFollow.VisibleHistory.map((line) => line.Text), [
  "three",
  "four",
  "x".repeat(614),
  "x".repeat(2),
]);

const filtered = new CrystalChatHistory((text) => text.length);
for (const type of Object.values(CrystalChatType)) {
  filtered.receiveChat(`type-${type}`, type);
}
assert.equal(filtered.FullHistory.length, 17);
filtered.setFilters({
  FilterNormalChat: true,
  FilterWhisperChat: true,
  FilterShoutChat: true,
  FilterSystemChat: true,
  FilterGroupChat: true,
  FilterGuildChat: true,
});
assert.deepEqual(filtered.History.map((line) => line.Type), [
  CrystalChatType.Hint,
  CrystalChatType.Announcement,
  CrystalChatType.Trainer,
  CrystalChatType.LevelUp,
  CrystalChatType.Relationship,
  CrystalChatType.Mentor,
]);
assert.ok(
  !filtered.History.some((line) => line.Type === CrystalChatType.LineMessage),
  "LineMessage must use FilterNormalChat",
);
assert.equal(filtered.FullHistory.length, 17, "filtering must not mutate FullHistory");
assert.equal(filtered.StartIndex, 5, "Update must clamp to History.Count - 1");

const filteredFollow = new CrystalChatHistory((text) => text.length, {
  FilterNormalChat: true,
});
for (const text of ["one", "two", "three", "four"]) {
  filteredFollow.receiveChat(text, CrystalChatType.Hint);
}
filteredFollow.receiveChat("hidden", CrystalChatType.Normal);
assert.equal(filteredFollow.History.length, 4);
assert.equal(
  filteredFollow.StartIndex,
  1,
  "ReceiveChat advances before the newly received line is filtered, matching Crystal",
);

console.log("crystal chat history tests passed");


// Execute actual ChatFrame/FilterBar and its pure source declarations in a
// memory-only React/DOM seam. No renderer, WASM, browser or network is started.
import test from "node:test";
const panelsSource=readFileSync(new URL("../app/components/original-client-panels.tsx",import.meta.url),"utf8");
const panelsAst=ts.createSourceFile("chat-panels.tsx",panelsSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const chatDeclarations=["ChatFrame","ChatFilterBar","chatPrefixForFilter","formatChatMessageForFilter",
  "crystalChatLocalY","crystalChatDragCurrent","crystalChatHiddenFilters","playerFacingChatLines",
  "crystalChatTypeForDisplayLine","measureCrystalChatText","argbToCss","trimLogTimestamp","matchesChatVisibility"];
const chatVariables=["CHAT_MASK_CHANNELS","CHAT_FILTER_BUTTONS","VISIBLE_CHAT_FILTER_BUTTONS","CHAT_FILTER_PREFIX",
  "CHAT_OPTION_FILTER_BUTTONS","VISIBLE_CHAT_OPTION_FILTER_BUTTONS","crystalChatMeasureContext"];
const nodes=panelsAst.statements.filter(n=>ts.isFunctionDeclaration(n)&&chatDeclarations.includes(n.name?.text)
  ||ts.isVariableStatement(n)&&n.declarationList.declarations.some(d=>chatVariables.includes(d.name.getText(panelsAst))));
assert.equal(nodes.length,chatDeclarations.length+chatVariables.length,"complete actual Chat declarations only");
const chatJs=ts.transpileModule(nodes.map(n=>n.getText(panelsAst).replace(/^export /,"")).join("\n"),
  {compilerOptions:{target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.React}}).outputText;
function chatMemory() {
  const hooks=[],effects=[],effectDependencies=[],events=new Map(),held=[],calls=[],captures=new Set();
  let hookIndex=0;
  class MemoryElement {
    focus(){document.activeElement=this;}
    getBoundingClientRect(){return {top:100,width:316,height:34};}
    setPointerCapture(id){captures.add(id);}
    hasPointerCapture(id){return captures.has(id);}
    releasePointerCapture(id){captures.delete(id);}
    querySelectorAll(){return flatten(this).filter(n=>n.type==="button");}
    querySelector(){return this.querySelectorAll()[0]??null;}
  }
  const document={visibilityState:"visible",activeElement:null,createElement:()=>({getContext:()=>null}),
    addEventListener:(kind,fn)=>events.set("document:"+kind,fn),removeEventListener:kind=>events.delete("document:"+kind)};
  const window={addEventListener:(kind,fn)=>events.set("window:"+kind,fn),removeEventListener:kind=>events.delete("window:"+kind)};
  const React={createElement(type,props,...children){return Object.assign(new MemoryElement(),{type,props:props??{},children:children.flat(Infinity).filter(v=>v!=null&&v!==false)});}};
  function useRef(value){const index=hookIndex++;return hooks[index]??(hooks[index]={current:value});}
  function useState(initial){const index=hookIndex++;if(!(index in hooks))hooks[index]=typeof initial==="function"?initial():initial;
    return [hooks[index],next=>{hooks[index]=typeof next==="function"?next(hooks[index]):next;}];}
  const useEffect=(fn,dependencies)=>{effects.push(fn);effectDependencies.push(dependencies);};
  const compiled=new Function("React","useRef","useState","useEffect","document","window","HTMLElement",
    "CrystalChatHistory","CrystalChatType","IS_PLATINUM_176_PROFILE","ORIGINAL_UI","SpriteButton",
    "OriginalAudioSettingsControls","findCrystalGdiTextAsset","CrystalGdiTextImage",
    chatJs+"\nreturn {"+chatDeclarations.join(",")+"};")(React,useRef,useState,useEffect,document,window,MemoryElement,
      CrystalChatHistory,CrystalChatType,false,{game:{chatDialog:"legacy",chatCountBar:"legacybar",
        chatScrollButtons:{home:{},up:{},down:{},end:{},knob:{base:"knob"}},chatFilterButtons:{},chatControlBar:"control"}},
      "SpriteButton","AudioControls",()=>null,"GdiText");
  const source={},doc={version:1,epoch:1,open:false,size:0,lineCount:4,frameIndex:2221,countBarIndex:2012,
    top:671,height:68,controlTop:656,inputTop:54,track:7,knobTop:16,index:0,historyCount:6,appliedMask:0,draftMask:null};
  const ui={source,document:doc,observe:count=>calls.push(["observe",count]),
    scroll:(...v)=>calls.push(["scroll",...v]),drag:(...v)=>calls.push(["drag",...v]),
    resize:(...v)=>calls.push(["resize",...v]),open:(...v)=>calls.push(["open",...v]),
    editFilter:(...v)=>calls.push(["filter",...v]),editAll:(...v)=>calls.push(["all",...v]),
    editTransparent:(...v)=>calls.push(["transparent",...v]),apply:(...v)=>calls.push(["apply",...v]),
    cancel:(...v)=>calls.push(["cancel",...v]),defaults:(...v)=>calls.push(["defaults",...v])};
  const props={t:(_k,_a,f)=>f??_k,runtimeMessage:"",logs:Array.from({length:6},(_,i)=>({text:"Line"+i,tone:"chat",channel:"normal"})),
    chatMessage:"",hints:[],activeFilter:"all",hiddenFilters:[],expanded:true,showSettings:false,transparent:false,chatUi:ui,
    onInputHoldChange:value=>held.push(value),onChatMessageChange:()=>{},onSendChat:()=>assert.fail("display action cannot send chat"),
    onCloseSettings:()=>ui.cancel(ui.document.epoch),onToggleHiddenFilter:()=>assert.fail("shared filter must not mutate legacy"),
    onToggleAllHiddenFilters:()=>assert.fail("shared All must not mutate legacy"),onToggleTransparent:()=>assert.fail("shared transparency must not mutate legacy")};
  function flatten(n){return n&&typeof n==="object"?[n,...n.children.flatMap(flatten)]:[];}
  function render(patch={}) {
    Object.assign(props,patch);hookIndex=0;effects.length=0;effectDependencies.length=0;
    const tree=compiled.ChatFrame(props),all=flatten(tree);
    for(const n of all)if(n.props.ref)n.props.ref.current=n;
    return {tree,all,knob:all.find(n=>n.props.className==="chat-position-knob"),effects:effects.slice(),effectDependencies:effectDependencies.slice()};
  }
  function pointer(node,id,y=108,type="move"){
    const event={button:0,pointerId:id,clientY:y,currentTarget:node,preventDefault(){},stopPropagation(){}};
    node.props[type==="down"?"onPointerDown":type==="end"?"onPointerUp":"onPointerMove"](event);
  }
  return {compiled,ui,props,doc,document,window,events,held,calls,captures,render,pointer,flatten};
}
test("actual shared Chat geometry and leases reject malformed coordinates and foreign pointer/source/epoch",()=>{
  const m=chatMemory(),f=m.compiled;
  assert.equal(f.crystalChatLocalY(108,{top:100,width:316,height:34},68),16);
  for(const rect of [{top:100,width:0,height:34},{top:100,width:316,height:35},{top:NaN,width:316,height:34}])
    assert.equal(f.crystalChatLocalY(108,rect,68),null);
  assert.equal(f.crystalChatLocalY(Infinity,{top:100,width:316,height:34},68),null);
  const lease={source:m.ui.source,epoch:1,pointerId:9,grabY:0};
  assert.equal(f.crystalChatDragCurrent(lease,m.ui.source,1,9,true,false),true);
  for(const values of [[{},1,9,true,false],[m.ui.source,2,9,true,false],[m.ui.source,1,10,true,false],
    [m.ui.source,1,9,false,false],[m.ui.source,1,9,true,true]])
    assert.equal(f.crystalChatDragCurrent(lease,...values),false);
  const old=m.render();m.ui.document={...m.doc,epoch:2};m.render();
  m.pointer(old.knob,9,108,"down");assert.equal(m.captures.size,0);assert.deepEqual(m.held,[]);
});
test("actual Chat pointer capture sends Core local coordinates and retires on stale source cancel blur and hidden",()=>{
  for(const retire of ["epoch","source","cancel","blur","hidden","cleanup"]) {
    const m=chatMemory(),view=m.render();
    const cleanup=view.effects.map(fn=>fn()).filter(fn=>typeof fn==="function");
    m.pointer(view.knob,9,108,"down");assert.deepEqual(m.held,[true]);assert(m.captures.has(9));
    m.pointer(view.knob,10,110);assert.equal(m.calls.some(c=>c[0]==="drag"),false);
    m.pointer(view.knob,9,110);assert.deepEqual(m.calls.at(-1),["drag",20,0,1]);
    if(retire==="epoch"){m.ui.document={...m.doc,epoch:2};m.render();}
    if(retire==="source"){m.props.chatUi={...m.ui,source:{}};m.render();}
    if(retire==="cancel")view.knob.props.onPointerCancel({pointerId:9,currentTarget:view.knob,preventDefault(){},stopPropagation(){}});
    if(retire==="blur")m.events.get("window:blur")();
    if(retire==="hidden"){m.document.visibilityState="hidden";m.events.get("document:visibilitychange")();}
    if(retire==="cleanup")cleanup.forEach(fn=>fn());
    const before=m.calls.filter(c=>c[0]==="drag").length;m.pointer(view.knob,9,112);
    assert.equal(m.calls.filter(c=>c[0]==="drag").length,before,retire);
    assert.equal(m.held.at(-1),false,retire);
    assert.equal(m.captures.has(9),false,retire+" releases its actual capture");
  }
});
test("actual Chat renders only applied filters while settings buttons delegate captured draft epoch",()=>{
  const m=chatMemory();m.ui.document={...m.doc,open:true,epoch:7,draftMask:513};
  const view=m.render();assert.equal(view.all.filter(n=>n.props.className?.startsWith("chat-feed-line ")).length,4);
  const normal=view.all.find(n=>n.props["data-chat-option-filter"]==="normal");assert.equal(normal.props["data-chat-option-hidden"],true);
  normal.props.onClick();assert.deepEqual(m.calls.at(-1),["filter",0,true,7]);
  for(const kind of ["defaults","apply","cancel"]) {
    view.all.find(n=>n.props["data-chat-settings-action"]===kind).props.onClick();
    assert.deepEqual(m.calls.at(-1),[kind,7]);
  }
  const settings=view.all.find(n=>n.props.role==="dialog");settings.props.onKeyDown({key:"Escape",preventDefault(){},stopPropagation(){}});
  assert.deepEqual(m.calls.at(-1),["cancel",7]);
  const oldClose=view.all.find(n=>n.props.className==="chat-settings-close");
  m.ui.document={...m.ui.document,epoch:8,draftMask:0};m.render();
  for(const kind of ["defaults","apply","cancel"]) {
    view.all.find(n=>n.props["data-chat-settings-action"]===kind).props.onClick();
    assert.deepEqual(m.calls.at(-1),[kind,7],"old dialog must not borrow replacement epoch");
  }
  oldClose.props.onClick();assert.deepEqual(m.calls.at(-1),["cancel",7]);
  settings.props.onKeyDown({key:"Escape",preventDefault(){},stopPropagation(){}});
  assert.deepEqual(m.calls.at(-1),["cancel",7],"old Escape must not cancel new draft");
  normal.props.onClick();assert.deepEqual(m.calls.at(-1),["filter",0,true,7]);
  const committed=m.render({chatUi:{...m.ui,document:{...m.ui.document,open:false,appliedMask:1,draftMask:null}}});
  assert.equal(committed.all.filter(n=>n.props.className?.startsWith("chat-feed-line ")).length,0);
  committed.effects[0]();assert.deepEqual(m.calls.at(-1),["observe",0],"empty shared history does not invent six held rows");
});
test("actual Chat size projection selects supplied Core frames and button callbacks do not implement a JS cycle",()=>{
  const m=chatMemory();
  for(const [size,lines,frame,bar,top,height,controlTop,inputTop] of [[0,4,2221,2012,671,68,656,54],[1,7,2224,2013,623,116,608,102],[2,11,2227,2014,575,164,560,150]]) {
    const doc={...m.doc,size,lineCount:lines,frameIndex:frame,countBarIndex:bar,top,height,controlTop,inputTop,epoch:4};
    const view=m.render({chatUi:{...m.ui,document:doc}});
    assert.equal(view.tree.props.style.top,top);assert.equal(view.tree.props.style.height,height);
    assert.equal(view.all.find(n=>n.props.className==="chat-frame-bg").props.src,"/original-ui/Prguse/"+frame+".png");
    assert.equal(view.all.find(n=>n.props.className==="chat-count-bar").props.src,"/original-ui/Prguse/"+bar+".png");
    assert.equal(view.all.find(n=>n.props.className==="chat-textbox").props.style.top,inputTop);
    assert.equal(view.all.filter(n=>n.props.className?.startsWith("chat-feed-line ")).length,Math.min(lines,6));
    const barTree=m.compiled.ChatFilterBar({t:(_k,_a,f)=>f,activeFilter:"all",chatExpanded:true,showSettings:false,
      chatUi:{...m.ui,document:doc},onSelectFilter:()=>{},onRequestTrade:()=>{},onToggleExpanded:()=>m.ui.resize(doc.epoch),
      onToggleSettings:()=>m.ui.open(doc.epoch),onToggleReport:()=>{}});
    assert.equal(barTree.props.style.top,controlTop);
    m.flatten(barTree).find(n=>n.props.className==="chat-filter-button size").children[0].props.onClick();
    assert.deepEqual(m.calls.at(-1),["resize",4]);
  }
});

const gameSceneSource=readFileSync(new URL("../app/components/original-client-game-ui-scene.tsx",import.meta.url),"utf8");
const gameSceneAst=ts.createSourceFile("chat-game-scene.tsx",gameSceneSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const chatHostNodes=[];
function findChatHost(n) {
  if(ts.isFunctionDeclaration(n)&&["changeChatSettings","resizeChatWindow","changeChatPointerHold"].includes(n.name?.text))chatHostNodes.push(n);
  ts.forEachChild(n,findChatHost);
}
findChatHost(gameSceneAst);
assert.equal(chatHostNodes.length,3,"complete actual Chat scene event handlers");
const chatHostJs=ts.transpileModule(chatHostNodes.map(n=>n.getText(gameSceneAst)).join("\n"),
  {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
test("actual Chat host captures dialog epoch and synchronously fences a held pointer",()=>{
  const m=chatMemory(),hp=[],quest=[],map=[],heldRef={current:false},state=[];
  function host(open,flags={}) {
    return new Function("chatUi","chatUiEpoch","effectiveChatSettingsOpen","setShowChatSettings","setChatExpanded",
      "chatPointerHeldRef","setChatPointerHeld","onHpOrbModalChange","onQuestUiModalChange","onMapRouteModalChange",
      "showBigMap","showReportPanel","showSystemMenu","showSystemMenuFeaturePanel","showGameShop",
      "storageRentalPrompt","showDuraPanel","visibleDialog",chatHostJs+
      "\nreturn {changeChatSettings,resizeChatWindow,changeChatPointerHold};")(m.ui,m.ui.document.epoch,open,
        value=>state.push(["legacy",value]),value=>state.push(["expanded",value]),heldRef,value=>state.push(["held",value]),
        value=>hp.push(value),value=>quest.push(value),value=>map.push(value),Boolean(flags.map),false,false,null,false,
        flags.rental??null,Boolean(flags.dura),flags.npc??null);
  }
  const closed=host(false);closed.changeChatSettings(true);
  assert.deepEqual(m.calls.at(-1),["open",1]);assert.equal(hp.at(-1),true);assert.equal(quest.at(-1),true);
  assert.equal(map.at(-1),true,"opening settings synchronously blocks map routing");
  m.ui.document={...m.doc,open:true,epoch:4};
  const oldDialog=host(true);m.ui.document={...m.doc,open:true,epoch:8};
  oldDialog.changeChatSettings(false);assert.deepEqual(m.calls.at(-1),["cancel",4]);
  closed.changeChatSettings(true);assert.deepEqual(m.calls.at(-1),["open",1],"old open cannot borrow current epoch");
  closed.resizeChatWindow();assert.deepEqual(m.calls.at(-1),["resize",1],"old size callback cannot borrow current epoch");
  const plain=host(false);plain.changeChatPointerHold(true);
  assert.equal(heldRef.current,true);assert.equal(hp.at(-1),true);assert.equal(quest.at(-1),true);
  assert.equal(map.at(-1),true,"captured Chat pointer synchronously blocks map routing");
  plain.changeChatPointerHold(false);assert.equal(heldRef.current,false);assert.equal(hp.at(-1),false);assert.equal(quest.at(-1),false);
  assert.equal(map.at(-1),false,"released pointer restores an otherwise unblocked map");
  host(false,{map:true}).changeChatPointerHold(false);assert.equal(hp.at(-1),true);assert.equal(quest.at(-1),true);
  assert.equal(map.at(-1),false,"the routing map itself does not block its accepted route");
  host(false,{npc:{}}).changeChatPointerHold(false);assert.equal(hp.at(-1),true);assert.equal(quest.at(-1),false);
  assert.equal(map.at(-1),true,"NPC modal retains its map blocker after Chat release");
  host(true).changeChatPointerHold(false);assert.equal(hp.at(-1),true);assert.equal(quest.at(-1),true);
  assert.equal(map.at(-1),true,"settings retain their map blocker after Chat release");
  host(false,{rental:{}}).changeChatPointerHold(false);
  assert.equal(map.at(-1),true,"rental confirmation retains its map blocker after Chat release");
  assert.equal(state.some(row=>row[0]==="legacy"),false,"shared settings never mutate legacy draft state");
});

test("actual Chat observe follows epoch and the wrapped applied-filter history count",()=>{
  const m=chatMemory(),logs=[{text:"x".repeat(104),tone:"chat",channel:"normal"},
    {text:"visible",tone:"system",channel:"hint"},{text:"network only",tone:"network"}];
  const initial=m.render({logs});
  assert.deepEqual(initial.effectDependencies[0],[m.ui.source,3,0,1]);
  initial.effects[0]();assert.deepEqual(m.calls.at(-1),["observe",3],"count includes wrapped rows and excludes network-only logs");
  m.ui.document={...m.doc,epoch:2,historyCount:0};
  const retired=m.render();
  assert.deepEqual(retired.effectDependencies[0],[m.ui.source,3,0,2],"same source/count/mask must reobserve after Core retirement");
  retired.effects[0]();assert.deepEqual(m.calls.at(-1),["observe",3]);
  m.ui.document={...m.ui.document,appliedMask:1};
  const filtered=m.render();
  assert.deepEqual(filtered.effectDependencies[0],[m.ui.source,1,1,2]);
  filtered.effects[0]();assert.deepEqual(m.calls.at(-1),["observe",1],"applied filter removes both wrapped Normal rows");
});
