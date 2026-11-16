import assert from 'node:assert/strict';
import { join } from 'node:path';
import { writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyPublicQuest({page,proof,output}){
 const before=JSON.stringify(proof.ws),frame=page.locator('.client-stage-frame');
 const pane=page.getByTestId('quest-log-window');
 const observations=[];
 const read=()=>page.evaluate(()=>{
  const runtime=window.__mir2BevyRuntime;
  const json=name=>{try{return JSON.parse(runtime?.[name]?.()??'null');}catch{return null;}};
  const q=json('getMir2QuestUiStatus'),h=json('getMir2HudUiStatus');
  let keyboard=null;try{keyboard=JSON.parse(document.querySelector('[data-belt-input-status]')?.getAttribute('data-belt-input-status')??'null')?.keyboard;}catch{}
  return {owner:document.querySelector('.client-stage-frame')?.getAttribute('data-quest-ui-owner'),
   canvasSelector:runtime?.getMir2RuntimeCanvasSelector?.()??null,
   focus:{tag:document.activeElement?.tagName,id:document.activeElement?.id},keyboard,
   quest:{ready:q?.ready,open:q?.questLogOpen,generation:q?.generation,error:q?.error},
   hud:{ready:h?.ready,navigation:h?.navigation},
   package:{backend:window.__mir2BevyRuntimeDebug?.selectedBackend,packageId:window.__mir2BevyRuntimeDebug?.packageId}};
 });
 const save=async label=>{const state=await read();observations.push({label,...state});
  await writeFile(join(output,'quest-owner-observations.json'),JSON.stringify(observations,null,2)+'\n');return state;};
 try{
  const initial=await save('before-q');
  if(initial.quest.ready&&initial.canvasSelector)await page.locator(initial.canvasSelector).focus();else await frame.focus();
  await page.keyboard.press('q');
  let mode=null;
  for(let i=0;i<120;i++){
   const state=await read();
   if(await pane.isVisible()){mode='react';break;}
   if(state.quest.ready&&state.quest.open===true){mode='bevy';break;}
   await delay(100);
  }
  const opened=await save('after-q');
  assert(mode,'configured Q must open either the current shared diary or React diary');
  assert.equal(await page.locator('html').getAttribute('lang'),'zh-TW');
  if(mode==='bevy'){
   await frame.screenshot({path:join(output,'quest-available-zh-TW.png')});
   if(opened.canvasSelector)await page.locator(opened.canvasSelector).focus();else await frame.focus();
   await page.keyboard.press('q');
   let closed=false;for(let i=0;i<100;i++){const state=await read();if(state.quest.ready&&state.quest.open===false){closed=true;break;}await delay(100);}
   const final=await save('after-close');assert(closed,'configured Q closes shared diary');
   assert.equal(JSON.stringify(proof.ws),before,'reading shared diary must not submit gameplay commands');
   return {passed:true,locale:'zh-TW',renderer:'ordinary-shared-native',questOpened:true,qClosed:true,
    owner:opened.owner,package:opened.package,navigation:final.hud.navigation,zeroWorldCommands:true,screenshot:'quest-available-zh-TW.png'};
  }
  await pane.locator('[data-quest-id="2110001"][data-quest-stage="available"]').click();
  const detail=pane.locator('[data-quest-detail="2110001"]'),content=await detail.innerText();
  const chapter=await detail.locator('[data-newcomer-v2-chapter]').innerText();
  const journey=await pane.getByTestId('newcomer-v2-journey-progress').innerText();
  assert(content.includes('首套巡邏裝備')&&content.includes('向助理簡報告')&&content.includes('0/1'));
  assert(chapter.includes('章節 1/6')&&chapter.includes('村莊起步')&&chapter.includes('本章 0/4'));
  assert(journey.includes('當前主線 0/26'));
  await frame.screenshot({path:join(output,'quest-available-zh-TW.png')});
  await page.keyboard.press('q');await pane.waitFor({state:'hidden',timeout:5000});
  await save('after-close');assert.equal(JSON.stringify(proof.ws),before,'reading diary must not submit gameplay commands');
  return {passed:true,locale:'zh-TW',renderer:'ordinary-react-fallback',questId:2110001,stage:'available',
   objective:'向助理簡報告',counter:'0/1',chapter,journey,zeroWorldCommands:true,screenshot:'quest-available-zh-TW.png'};
 }catch(error){
  await save('failure');await frame.screenshot({path:join(output,'quest-failure.png')});
  throw Object.assign(new Error('public diary observation failed'),{code:'PUBLIC_DIARY_OBSERVATION_FAILED'});
 }
}
