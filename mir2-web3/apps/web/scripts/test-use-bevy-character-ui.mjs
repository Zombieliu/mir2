import assert from "node:assert/strict";import test from "node:test";import {readFileSync} from "node:fs";import ts from "typescript";
function hook(){let effect,cleaned=0;const hosts=[];
 class Host {constructor(options){this.options=options;hosts.push(this);}tick(){if(!this.options.isCurrent())this.stop();}withdraw(){cleaned++;}stop(){cleaned++;}pointerContext(){return null;}pointer(){return false;}allows(){return false;}}
 const fakeReact={useRef:v=>({current:v}),useState:v=>[v,()=>{}],useEffect:fn=>{effect=fn;}};
 const m={exports:{}};const deps={react:fakeReact,"./bevy-character-ui":{CharacterHost:Host},"./bevy-character-model":{parseStateItemMetadata:()=>null}};
 new Function("exports","module","require",ts.transpileModule(readFileSync(new URL("../lib/use-bevy-character-ui.ts",import.meta.url),"utf8"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m,p=>deps[p]);
 return {run:m.exports.useBevyCharacterUi,get effect(){return effect;},hosts,get cleaned(){return cleaned;}};
}
test("hook checks captured runtime identity and cleanup retires it despite ref replacement",()=>{
 const saved={window:globalThis.window,document:globalThis.document,fetch:globalThis.fetch};let tick;
 globalThis.window={setInterval:fn=>{tick=fn;return 1;},clearInterval:()=>{},addEventListener:()=>{},removeEventListener:()=>{}};
 globalThis.document={addEventListener:()=>{},removeEventListener:()=>{}};globalThis.fetch=()=>new Promise(()=>{});
 try{const h=hook(),ref={current:{}};h.run({requested:true,runtimeGeneration:1,runtimeRef:ref,read:()=>({}),onIntent:()=>true});const cleanup=h.effect();
  assert.equal(h.hosts[0].options.isCurrent(),true);ref.current=null;tick();assert.equal(h.hosts[0].options.isCurrent(),false);assert.equal(h.cleaned,1);ref.current={};cleanup();assert.equal(h.cleaned,2);
 }finally{Object.assign(globalThis,saved);}
});
