import {writeFile} from 'node:fs/promises';

// Passive CDP observation, including the Worker's actual upstream requests.
// No throttling, interception, cache changes, headers, cookies or bodies.
export async function observeNetwork(context, output) {
 const browser=context.browser();if(!browser)throw Error('Owned Chromium browser handle unavailable');
 const root=await browser.newBrowserCDPSession(),rows=[],sessions=new Map(),rpc=new Map(),targets=new Map(),active=new Map(),errors=[];
 let serial=0,stopped=false,maxActive=0;
 const wanted=url=>/\/(?:original-ui|original-map|bevy-runtime|mir2\/v\/|bootstrap\/)/.test(url.pathname);
 function event(owner,method,p){
  if(stopped)return;
  const key=owner+':'+p.requestId;
  if(method==='Network.requestWillBeSent'){
   const u=new URL(p.request.url);if(p.request.method!=='GET'||!wanted(u))return;
   const row={owner,path:u.origin+u.pathname,type:p.type,at:p.timestamp,epochMs:p.wallTime*1000,initialPriority:p.request.initialPriority,
    activeAtStart:active.size,sameOriginAtStart:[...active.values()].filter(r=>r.origin===u.origin).length,origin:u.origin};
   active.set(key,row);targets.set(key,row);rows.push(row);maxActive=Math.max(maxActive,active.size);
  }else{
   const row=targets.get(key);if(!row)return;
   if(method==='Network.requestWillBeSentExtraInfo')row.extraInfo={observedAt:Date.now(),connectTiming:p.connectTiming};
   if(method==='Network.resourceChangedPriority')row.priorityChanges=[...(row.priorityChanges??[]),{at:p.timestamp,priority:p.newPriority}].slice(-5);
   if(method==='Network.responseReceived'){
    const r=p.response;Object.assign(row,{headersAt:p.timestamp,status:r.status,protocol:r.protocol,serviceWorker:r.fromServiceWorker,
     fromDiskCache:r.fromDiskCache,connectionId:r.connectionId,connectionReused:r.connectionReused,timing:r.timing,
     headers:Object.fromEntries(Object.entries(r.headers).filter(([k])=>/^(content-(?:type|length|encoding|range)|cf-cache-status|x-mir2-edge-cache|etag)$/i.test(k)))});
   }
   if(method==='Network.dataReceived'){
    row.firstDataAt??=p.timestamp;row.maxDataGapMs=Math.max(row.maxDataGapMs??0,(p.timestamp-(row.lastDataAt??row.headersAt??row.at))*1000);
    row.lastDataAt=p.timestamp;row.decodedBytes=(row.decodedBytes??0)+p.dataLength;row.encodedBytes=(row.encodedBytes??0)+p.encodedDataLength;
   }
   if(method==='Network.loadingFinished'){row.finishedAt=p.timestamp;row.totalEncodedBytes=p.encodedDataLength;active.delete(key);}
   if(method==='Network.loadingFailed'){row.failedAt=p.timestamp;row.failure=p.errorText;active.delete(key);}
  }
 }
 root.on('Target.receivedMessageFromTarget',p=>{
  const message=JSON.parse(p.message);
  if(message.id){const key=p.sessionId+':'+message.id,entry=rpc.get(key);if(entry){rpc.delete(key);clearTimeout(entry.timer);message.error?entry.reject(Error(message.error.message)):entry.resolve(message.result);}return;}
  if(message.method?.startsWith('Network.'))event('worker:'+sessions.get(p.sessionId),message.method,message.params);
 });
 async function workerCommand(sessionId,method){
  const id=++serial,key=sessionId+':'+id;
  const reply=new Promise((resolve,reject)=>{const timer=setTimeout(()=>{rpc.delete(key);reject(Error(method+' observation setup timed out'));},5000);rpc.set(key,{resolve,reject,timer});});
  try{await root.send('Target.sendMessageToTarget',{sessionId,message:JSON.stringify({id,method,params:{}})});return await reply;}
  catch(error){const entry=rpc.get(key);if(entry){clearTimeout(entry.timer);rpc.delete(key);entry.resolve({});}await reply.catch(()=>{});throw error;}
 }
 const attaching=new Map();
 async function attachWorker(info){
  if(info.type!=='service_worker'||attaching.has(info.targetId)||stopped)return;
  const promise=(async()=>{const {sessionId}=await root.send('Target.attachToTarget',{targetId:info.targetId,flatten:false});sessions.set(sessionId,info.targetId);await workerCommand(sessionId,'Network.enable');})();
  attaching.set(info.targetId,promise);await promise.catch(e=>errors.push({target:'service_worker',message:e.message}));
 }
 root.on('Target.targetCreated',p=>{void attachWorker(p.targetInfo);});
 await root.send('Target.setDiscoverTargets',{discover:true});
 for(const info of (await root.send('Target.getTargets')).targetInfos)await attachWorker(info);
 const pages=[];
 return{
  async page(page){const session=await context.newCDPSession(page),owner='page:'+pages.length;pages.push(session);
   for(const method of ['requestWillBeSent','requestWillBeSentExtraInfo','resourceChangedPriority','responseReceived','dataReceived','loadingFinished','loadingFailed'])session.on('Network.'+method,p=>event(owner,'Network.'+method,p));
   await session.send('Network.enable');},
  async workerReady(){await Promise.allSettled([...attaching.values()]);if(errors.length)throw Error(errors[0].message);},
  async stop(){if(stopped)return;stopped=true;
   const result={kind:'passive-page-and-service-worker-CDP-network',rows,maxActive,activeAtStop:active.size,workerTargets:sessions.size,errors};
   await writeFile(output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
   for(const session of pages)await session.detach().catch(()=>{});
   for(const sessionId of sessions.keys())await root.send('Target.detachFromTarget',{sessionId}).catch(()=>{});
   await root.detach().catch(()=>{});return{rows:rows.length,workerTargets:sessions.size,errors};},
 };
}
