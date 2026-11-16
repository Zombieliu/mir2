import {normalizeStaticResourceSources,staticResourceDescriptor} from './r16-static-waterfall.mjs';
const PRIORITIES=new Set(['VeryLow','Low','Medium','High','VeryHigh']);
const PROTOCOLS=new Set(['h2','h3','http/1.1','http/1.0']);
const TIMINGS=['proxyStart','proxyEnd','dnsStart','dnsEnd','connectStart','connectEnd','sslStart','sslEnd','sendStart','sendEnd','receiveHeadersStart','receiveHeadersEnd'];
const STATES=new Set(['HIT','MISS','STALE','DYNAMIC','BYPASS','REVALIDATED','EXPIRED','UPDATING']);

// Read only named public cache fields; never serialize raw headers or ray IDs.
export function staticCacheMetadata(headers={}) {
  if(!headers||typeof headers!=='object')return {};
  let names;try{names=Object.keys(headers);}catch{return {};}
  const out={};
  const read=name=>{try{const key=names.find(k=>k.toLowerCase()===name);return key===undefined?undefined:headers[key];}catch{return undefined;}};
  for(const name of ['age','content-length']){
    const value=read(name);if(typeof value==='string'&&/^\d{1,12}$/.test(value))out[name]=Number(value);
  }
  for(const name of ['cf-cache-status','x-vercel-cache']){
    const value=read(name);if(typeof value==='string'&&STATES.has(value.toUpperCase()))out[name]=value.toUpperCase();
  }
  const encoding=read('content-encoding');if(typeof encoding==='string'&&/^(?:br|gzip|deflate|identity|zstd)$/.test(encoding.toLowerCase()))out['content-encoding']=encoding.toLowerCase();
  const control=read('cache-control');
  if(typeof control==='string'&&control.length<=256){
    const directives=control.toLowerCase().split(',').map(s=>s.trim());
    if(directives.every(s=>/^(?:public|private|no-store|no-cache|immutable|must-revalidate|proxy-revalidate|no-transform|(?:s-maxage|max-age|stale-if-error|stale-while-revalidate)=\d{1,12})$/.test(s)))out['cache-control']=directives.join(', ');
  }
  const ray=read('cf-ray');if(typeof ray==='string'&&/^[a-f\d]{8,32}-[A-Z]{3,4}$/.test(ray))out.cfPop=ray.split('-')[1];
  return out;
}

/** Context-owned page CDP metadata only. Does not route, pause, cache-disable or read bodies. */
export async function startStaticTransportProbe({context,elapsed,sourceBases,maxRows=4096,maxPages=4}) {
  if(!context?.on||!context?.off||!context?.pages||!context?.newCDPSession||typeof elapsed!=='function'
    ||!Number.isSafeInteger(maxRows)||maxRows<1||maxRows>8192||!Number.isSafeInteger(maxPages)||maxPages<1||maxPages>8)throw Error('invalid transport probe');
  const sources=normalizeStaticResourceSources(sourceBases),rows=[],sessions=new Set(),setups=new Set(),creations=new Set(),seen=new WeakSet(),errors=[];
  let stopped=false,dropped=0,pages=0,skippedPages=0,cleanupComplete=false;
  const now=()=>{const n=elapsed();if(!Number.isFinite(n)||n<0)throw Error('invalid transport clock');return Math.round(n);};
  const startedAtMs=now();
  const error=(page,phase)=>{if(!stopped&&errors.length<16)errors.push({page,phase,code:'CDP_METADATA_UNAVAILABLE'});};
  const bounded=async(promise,ms)=>{let timer;try{return await Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('probe deadline')),ms);})]);}finally{clearTimeout(timer);}};
  const attach=async page=>{
    if(stopped||seen.has(page))return;seen.add(page);
    if(pages>=maxPages){skippedPages++;return;}const ordinal=++pages;let session,abandoned=false;
    try{
      const creation=context.newCDPSession(page);creations.add(creation);
      void creation.then(late=>{if(abandoned||stopped)void late.detach().catch(()=>{});},()=>{}).finally(()=>creations.delete(creation));
      session=await bounded(creation,3000);
      if(stopped){await bounded(session.detach(),2000).catch(()=>{});return;}
      sessions.add(session);const active=new Map();
      session.on('Network.requestWillBeSent',event=>{
        if(stopped)return;
        const before=active.get(event.requestId);if(before){before.outcome='redirected';before.finishedAtMs=now();active.delete(event.requestId);}
        if(event.request?.method!=='GET')return;
        const descriptor=staticResourceDescriptor(event.request.url,sources);if(!descriptor)return;
        if(rows.length>=maxRows){dropped++;return;}
        const row={ordinal:rows.length,page:ordinal,...descriptor,source:'page-cdp',requestAtMs:now(),outcome:'pending'};
        if(PRIORITIES.has(event.request.initialPriority))row.initialPriority=event.request.initialPriority;
        if(before)row.redirectedFrom=before.ordinal;
        rows.push(row);active.set(event.requestId,row);
      });
      session.on('Network.resourceChangedPriority',event=>{if(stopped)return;const row=active.get(event.requestId);if(row&&PRIORITIES.has(event.newPriority))row.finalPriority=event.newPriority;});
      session.on('Network.responseReceived',event=>{
        if(stopped)return;const row=active.get(event.requestId);if(!row)return;
        const response=event.response;row.headersAtMs=now();row.status=response.status;row.fromServiceWorker=response.fromServiceWorker===true;row.fromDiskCache=response.fromDiskCache===true;
        if(PROTOCOLS.has(response.protocol))row.protocol=response.protocol;
        row.cache=staticCacheMetadata(response.headers);
        if(response.timing)row.networkTimingMs=Object.fromEntries(TIMINGS.filter(k=>Number.isFinite(response.timing[k])&&response.timing[k]>=-1&&response.timing[k]<=600000).map(k=>[k,response.timing[k]]));
      });
      const finish=(event,failed)=>{
        if(stopped)return;const row=active.get(event.requestId);if(!row)return;active.delete(event.requestId);
        row.outcome=failed?'failed':'finished';row.finishedAtMs=now();
        if(failed)row.failure=/^net::ERR_[A-Z0-9_]{1,64}$/.test(event.errorText??'')?event.errorText:'other';
        else if(Number.isSafeInteger(event.encodedDataLength)&&event.encodedDataLength>=0)row.browserReportedEncodedBytes=event.encodedDataLength;
      };
      session.on('Network.loadingFinished',event=>finish(event,false));session.on('Network.loadingFailed',event=>finish(event,true));
      await bounded(session.send('Network.enable'),3000);
    }catch{abandoned=true;error(ordinal,'setup');if(session){sessions.delete(session);await bounded(session.detach(),2000).catch(()=>{});}}
  };
  const onPage=page=>{const work=attach(page).finally(()=>setups.delete(work));setups.add(work);};
  context.on('page',onPage);for(const page of context.pages())onPage(page);
  await Promise.allSettled([...setups]);
  const snapshot=()=>({schemaVersion:1,kind:'r16-passive-static-transport',startedAtMs,stopped,cleanupComplete,observedPages:pages,skippedPages,capturedRows:rows.length,maxRows,droppedRows:dropped,truncated:dropped>0,observerErrors:structuredClone(errors),
    scope:'Allowlisted GET assets, page CDP only. Safe cache fields and priorities; no request headers, cookies, raw response headers, payloads or routing. Worker upstream priorities are not observed. Rows overlap the Playwright waterfall; do not add byte totals.',rows:structuredClone(rows)});
  return{snapshot,async stop(){
    if(!stopped){stopped=true;context.off('page',onPage);await Promise.allSettled([...setups]);
      const results=await Promise.allSettled([...sessions].map(session=>bounded(session.detach(),2000)));cleanupComplete=creations.size===0&&results.every(r=>r.status==='fulfilled');sessions.clear();}
    return snapshot();
  }};
}
