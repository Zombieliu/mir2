import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

class ElementTarget {
  constructor(inGame) { this.inGame=inGame; }
  closest(selector) { assert.equal(selector,'.client-stage-frame');return this.inGame?this:null; }
}
const code=ts.transpileModule(readFileSync(new URL('../lib/game-browser-defaults.ts',import.meta.url),'utf8'),{
  compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS},
}).outputText;
const module={exports:{}};
new Function('module','exports','Element',code)(module,module.exports,ElementTarget);
const {installGameBrowserDefaults}=module.exports;
function fixture() {
  const host=new EventTarget(),image=new ElementTarget(true),canvas=new ElementTarget(true),outside=new ElementTarget(false);
  const cleanup=installGameBrowserDefaults(host);
  const emit=(type,target=outside,button=2,cancelable=true,pointerType='mouse')=>{
    const event=new Event(type,{cancelable,bubbles:true});
    Object.defineProperty(event,'target',{value:target});Object.defineProperty(event,'button',{value:button});
    if(type.startsWith('pointer'))Object.defineProperty(event,'pointerType',{value:pointerType});
    host.dispatchEvent(event);return event;
  };
  return{host,image,canvas,outside,cleanup,emit};
}

test('right image and canvas presses and menus are cancelled inside the game',()=>{
  const f=fixture();try{for(const target of [f.image,f.canvas]){
    assert(f.emit('mousedown',target).defaultPrevented);assert(f.emit('contextmenu',target).defaultPrevented);
  }}finally{f.cleanup();}
});
test('a game press owns the context menu after mouseup outside',()=>{
  const f=fixture();try{
    f.emit('mousedown',f.image);assert.equal(f.emit('mouseup',f.outside).defaultPrevented,false);
    assert(f.emit('contextmenu',f.outside).defaultPrevented);
    assert.equal(f.emit('contextmenu',f.outside).defaultPrevented,false,'ownership consumed once');
  }finally{f.cleanup();}
});
test('an ordinary outside press and menu stay available',()=>{
  const f=fixture();try{assert.equal(f.emit('mousedown').defaultPrevented,false);assert.equal(f.emit('contextmenu').defaultPrevented,false);}finally{f.cleanup();}
});
test('a new outside right press replaces an earlier unconsumed game press',()=>{
  const f=fixture();try{f.emit('mousedown',f.image);f.emit('mousedown');assert.equal(f.emit('contextmenu').defaultPrevented,false);}finally{f.cleanup();}
});
test('keyboard context menu outside does not consume a still-held right press',()=>{
  const f=fixture();try{f.emit('mousedown',f.image);assert.equal(f.emit('contextmenu',f.outside,0).defaultPrevented,false);
    f.emit('mouseup');assert(f.emit('contextmenu').defaultPrevented);assert.equal(f.emit('contextmenu').defaultPrevented,false);
  }finally{f.cleanup();}
});
test('keyboard context menu inside does not consume the held right release outside',()=>{
  const f=fixture();try{f.emit('mousedown',f.image);assert(f.emit('contextmenu',f.image,0).defaultPrevented);
    f.emit('mouseup');assert(f.emit('contextmenu').defaultPrevented);
  }finally{f.cleanup();}
});
test('outside-to-inside release still protects the current game target',()=>{
  const f=fixture();try{f.emit('mousedown');f.emit('mouseup',f.canvas);assert(f.emit('contextmenu',f.canvas).defaultPrevented);}finally{f.cleanup();}
});
test('blur clears press ownership',()=>{
  const f=fixture();try{f.emit('mousedown',f.image);f.emit('blur');assert.equal(f.emit('contextmenu').defaultPrevented,false);}finally{f.cleanup();}
});
test('cleanup removes the guard and a later mount starts without stale ownership',()=>{
  const f=fixture();f.emit('mousedown',f.image);f.cleanup();
  assert.equal(f.emit('contextmenu',f.image).defaultPrevented,false);
  assert.equal(f.emit('mousedown',f.canvas).defaultPrevented,false);
  const next=installGameBrowserDefaults(f.host);try{assert.equal(f.emit('contextmenu').defaultPrevented,false);}finally{next();}
});
test('left and middle mouse entry points and pointerdown stay uncancelled',()=>{
  const f=fixture();try{for(const button of [0,1])assert.equal(f.emit('mousedown',f.image,button).defaultPrevented,false);
    assert.equal(f.emit('pointerdown',f.image).defaultPrevented,false);
    assert.equal(f.emit('pointerup',f.image).defaultPrevented,false);
  }finally{f.cleanup();}
});
test('Native pointer cancellation can suppress mousedown without losing game ownership',()=>{
  const f=fixture();f.host.addEventListener('pointerdown',event=>event.preventDefault());try{
    assert(f.emit('pointerdown',f.image).defaultPrevented);
    f.emit('pointerup');assert(f.emit('contextmenu').defaultPrevented);
  }finally{f.cleanup();}
});
test('a fresh outside mouse pointerdown replaces prior game ownership without mousedown',()=>{
  const f=fixture();try{f.emit('pointerdown',f.image);f.emit('pointerdown');assert.equal(f.emit('contextmenu').defaultPrevented,false);}finally{f.cleanup();}
});
test('unrelated touch pointerdown does not create a right-button lease',()=>{
  const f=fixture();try{f.emit('pointerdown',f.image,0,true,'touch');assert.equal(f.emit('contextmenu').defaultPrevented,false);}finally{f.cleanup();}
});
test('custom game handlers still receive the right mouse and menu events',()=>{
  const f=fixture(),received=[];try{
    f.host.addEventListener('mousedown',e=>received.push([e.type,e.defaultPrevented]));
    f.host.addEventListener('contextmenu',e=>received.push([e.type,e.defaultPrevented]));
    f.emit('mousedown',f.image);f.emit('contextmenu',f.image);
    assert.deepEqual(received,[['mousedown',true],['contextmenu',true]]);
  }finally{f.cleanup();}
});
test('non-Element targets remain ordinary outside targets',()=>{
  const f=fixture();try{assert.equal(f.emit('mousedown',f.host).defaultPrevented,false);assert.equal(f.emit('contextmenu',null).defaultPrevented,false);}finally{f.cleanup();}
});
test('a detached target from a game press retains release ownership',()=>{
  const f=fixture();try{f.emit('mousedown',f.image);f.image.inGame=false;assert(f.emit('contextmenu',f.image).defaultPrevented);}finally{f.cleanup();}
});
test('uncancelable input does not throw or stop custom handlers',()=>{
  const f=fixture();let received=0;f.host.addEventListener('contextmenu',()=>received++);try{
    assert.equal(f.emit('mousedown',f.image,2,false).defaultPrevented,false);
    assert.equal(f.emit('contextmenu',f.image,2,false).defaultPrevented,false);assert.equal(received,1);
  }finally{f.cleanup();}
});
test('actual Shell installs only in game and returns the shared guard cleanup',()=>{
  const source=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8');
  const ast=ts.createSourceFile('shell.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let callback;
  function visit(n){if(ts.isCallExpression(n)&&n.expression.getText(ast)==='useEffect'&&n.arguments[0]?.getText(ast).includes('installGameBrowserDefaults(window)')){assert(!callback);callback=n.arguments[0].getText(ast);}ts.forEachChild(n,visit);}visit(ast);assert(callback);
  const js=ts.transpileModule('const effect = '+callback+';', {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
  for(const screen of ['login','select','game']){const host={},cleanup=()=>{};let calls=0;
    const effect=new Function('screen','window','installGameBrowserDefaults',js+'\nreturn effect;')(screen,host,actual=>{assert.equal(actual,host);calls++;return cleanup;});
    assert.equal(effect(),screen==='game'?cleanup:undefined);assert.equal(calls,screen==='game'?1:0);
  }
});
