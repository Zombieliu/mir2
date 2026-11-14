import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {dirname,extname,join,resolve} from 'node:path';
import {createServer} from 'node:http';
import {createHash} from 'node:crypto';
const root='E:/mir2-player-journey/mir2-web3/apps/web';
const phase=process.argv[2];assert(['before','after'].includes(phase));
const output=join('C:/r16-death-locale-20261114',phase);await mkdir(output,{recursive:true});
const require=createRequire(join(root,'package.json'));
const ts=require('typescript'),React=require('react'),{renderToStaticMarkup}=require('react-dom/server');
const {chromium}=createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json')('playwright');
const cache=new Map();
function load(file){
 if(cache.has(file))return cache.get(file);
 if(extname(file)==='.json')return require(file);
 const module={exports:{}};
 const code=ts.transpileModule(readFileSync(file,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.ReactJSX,esModuleInterop:true}}).outputText;
 new Function('require','module','exports',code)(name=>{
  if(!name.startsWith('.'))return require(name);
  const target=resolve(dirname(file),name);
  return load(extname(target)?target:existsSync(target+'.ts')?target+'.ts':target+'.tsx');
 },module,module.exports);cache.set(file,module.exports);return module.exports;
}
const {buildTranslator,languageFontFamily}=load(join(root,'lib/localization.ts'));
const {R16_NATIVE_LANGUAGES,nativeLanguageInfo,loadR16GameplayLocalization}=load(join(root,'lib/r16-native-localization.ts'));
const {browserText}=load(join(root,'lib/r16-browser-localization.ts'));
const pageBytes=await readFile(join(root,'app/page.tsx'));
const source=ts.createSourceFile('page.tsx',pageBytes.toString(),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const fn=source.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='DeathReviveOverlay');assert(fn);
let tag;
function visit(n){if(ts.isJsxSelfClosingElement(n)&&n.tagName.getText(source)==='DeathReviveOverlay')tag=n;ts.forEachChild(n,visit);}visit(source);assert(tag);
const fields=['title','message','reviveLabel','revivingLabel'];
const expressions=Object.fromEntries(fields.map(name=>{const prop=tag.attributes.properties.find(n=>ts.isJsxAttribute(n)&&n.name.text===name);assert(prop&&ts.isJsxExpression(prop.initializer));return[name,prop.initializer.expression.getText(source)];}));
const compiled=ts.transpileModule('export '+fn.getText(source),{fileName:'death.tsx',compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.ReactJSX}}).outputText;
const component={exports:{}};new Function('require','module','exports',compiled)(require,component,component.exports);
const {DeathReviveOverlay}=component.exports;
const propsFor=language=>Object.fromEntries(fields.map(name=>[name,new Function('t','return '+expressions[name])(buildTranslator(language))]));
const initial=Object.fromEntries(R16_NATIVE_LANGUAGES.map(code=>[code,propsFor(code)]));
await loadR16GameplayLocalization();
for(const code of R16_NATIVE_LANGUAGES)assert.deepEqual(propsFor(code),initial[code],'gameplay loading must not change death copy');
const keys=['ui.death.title','ui.death.message','ui.death.revive','ui.death.reviving'];
let nonEnglishLocalized=0;
for(const code of R16_NATIVE_LANGUAGES)for(const [i,key]of keys.entries()){
 const value=initial[code][fields[i]];assert(value.trim());
 if(code!=='en'&&value!==initial.en[fields[i]])nonEnglishLocalized++;
 if(phase==='after')assert.equal(value,browserText(code,key),code+'/'+key);
}
if(phase==='before')assert.equal(nonEnglishLocalized,0);else assert.equal(nonEnglishLocalized,36);
const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const css=readFileSync(join(root,'app/globals.css'),'utf8');
const extraFonts='@font-face{font-family:"Noto Sans SC";src:url("/original-ui/fonts/NotoSansCJKsc-Regular.otf")}@font-face{font-family:"Noto Sans TC";src:url("/original-ui/fonts/NotoSansTC.ttf")}';
const server=createServer(async(req,res)=>{try{
 const url=new URL(req.url,'http://localhost');
 if(url.pathname==='/globals.css'){res.setHeader('Content-Type','text/css');res.end(css+extraFonts);return;}
 if(url.pathname.startsWith('/original-ui/fonts/')&&/^[A-Za-z0-9.-]+$/.test(url.pathname.split('/').at(-1))){const body=await readFile(join(root,'public',url.pathname));res.end(body);return;}
 if(url.pathname!=='/case'){res.writeHead(404);res.end();return;}
 const language=url.searchParams.get('language');assert(R16_NATIVE_LANGUAGES.includes(language));const busy=url.searchParams.get('busy')==='true';
 const html=renderToStaticMarkup(React.createElement(DeathReviveOverlay,{...initial[language],busy,onRevive(){}}));
 res.setHeader('Content-Type','text/html;charset=utf-8');
 res.end('<!doctype html><html lang="'+escape(language)+'" dir="'+nativeLanguageInfo(language).dir+'"><head><meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="/globals.css"><style>body{font-family:'+languageFontFamily(language)+';margin:0;background:#29261e}</style></head><body>'+html+'</body></html>');
 }catch{res.writeHead(500);res.end('smoke error');}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base='http://127.0.0.1:'+server.address().port;
let browser;const rows=[],errors=[];
try{
 browser=await chromium.launch({channel:'msedge',headless:true});
 const page=await browser.newPage();page.on('pageerror',e=>errors.push(e.name));
 for(const width of [360,1024])for(const language of R16_NATIVE_LANGUAGES)for(const busy of [false,true]){
  await page.setViewportSize({width,height:540});await page.goto(base+'/case?language='+language+'&busy='+busy);await page.evaluate(()=>document.fonts.ready);
  const expected=initial[language],dialog=page.getByRole('dialog',{name:expected.title,exact:true}),button=page.getByTestId('town-revive-button');
  assert(await dialog.isVisible());assert.equal((await button.innerText()).trim(),busy?expected.revivingLabel:expected.reviveLabel);assert.equal(await button.isDisabled(),busy);
  assert((await dialog.innerText()).includes(expected.message));
  const bounds=await button.boundingBox(),body=await dialog.locator(':scope > div').boundingBox();assert(bounds&&body&&body.x>=0&&body.y>=0&&body.x+body.width<=width+1&&body.y+body.height<=540);
  const file=width+'-'+language+'-'+(busy?'busy':'ready')+'.png';await page.screenshot({path:join(output,file)});
  rows.push({language,width,busy,props:expected,bodyBounds:body,buttonBounds:bounds,visible:true,disabled:await button.isDisabled(),screenshot:file});
 }
 assert.equal(errors.length,0);
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
const result={scope:'Actual Page death component and its real translator props, rendered without gameplay state. Local font files and unchanged CSS; no Gateway, mutation, revive packet or full-game claim.',phase,passed:true,nonEnglishLocalized,views:rows.length,rows,browserClosed:true,serverClosed:true,pageErrors:errors,source:{pageSha256:createHash('sha256').update(pageBytes).digest('hex'),dictionarySha256:createHash('sha256').update(await readFile(join(root,'lib/r16-browser-localization.ts'))).digest('hex')}};
await writeFile(join(output,'receipt.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({passed:true,phase,views:rows.length,nonEnglishLocalized,pageErrors:errors.length,browserClosed:true}));
