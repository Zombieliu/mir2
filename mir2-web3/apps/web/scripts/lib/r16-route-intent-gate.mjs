/** QA-only loopback gate. All server messages and all released payloads stay unchanged. */
export async function installRouteIntentGate(page, url) {
 const endpoint=new URL(url);
 if(endpoint.protocol!=='ws:'||!['127.0.0.1','localhost'].includes(endpoint.hostname)||endpoint.pathname!=='/ws'||endpoint.search||endpoint.hash||endpoint.username||endpoint.password)throw Error('loopback-only intent gate');
 const events=[];let active=null,armed=null,sequence=0,connections=0,closed=false;
 const record=(type,detail={})=>{if(events.length<128)events.push({type,at:Date.now(),...detail});};
 function release(reason='explicit'){
  const hold=armed;if(!hold?.held)return false;armed=null;clearTimeout(hold.timer);
  for(const message of hold.messages)hold.server.send(message);
  record('released',{id:hold.id,reason,count:hold.messages.length,heldMs:Date.now()-hold.at});hold.messages.length=0;return true;
 }
 await page.routeWebSocket(url,route=>{
  const server=route.connectToServer(),connection=++connections;active={route,server,connection};record('connected',{connection});
  // Server-to-browser forwarding remains Playwright's unmodified real-server default.
  route.onMessage(message=>{
   const hold=armed;
   if(hold?.held&&hold.connection===connection){
    if(hold.messages.length>=32){release('overflow');server.send(message);return;}
    hold.messages.push(message);return;
   }
   let move;try{if(typeof message==='string'){const value=JSON.parse(message);if(value.type==='walk'&&value.direction===hold?.direction)move=value;}}catch{}
   if(hold&&!hold.held&&move){
    clearTimeout(hold.armTimer);hold.held=true;hold.connection=connection;hold.server=server;hold.messages=[message];hold.at=Date.now();
    hold.timer=setTimeout(()=>release('deadline'),hold.maxMs);record('held',{id:hold.id,connection,direction:move.direction,bytes:Buffer.byteLength(message)});hold.resolve({id:hold.id,at:hold.at});return;
   }
   server.send(message);
  });
 });
 return {events,
  arm(direction,maxMs=2500){
   if(closed||armed||!active||!['Up','Right','Down','Left','UpRight','DownRight','DownLeft','UpLeft'].includes(direction)||!Number.isInteger(maxMs)||maxMs<1||maxMs>2500)throw Error('invalid intent gate arm');
   let resolve,reject;const held=new Promise((yes,no)=>{resolve=yes;reject=no;});void held.catch(()=>{});armed={id:++sequence,direction,maxMs,resolve,held:false};
   const pending=armed;pending.armTimer=setTimeout(()=>{if(armed===pending&&!pending.held){armed=null;record('arm-expired',{id:pending.id});reject(Error('No matching real walk before intent gate arm deadline'));}},2500);
   record('armed',{id:sequence,direction,maxMs});return{held};
  },release,
  close(){closed=true;if(armed)clearTimeout(armed.armTimer);if(armed?.held){clearTimeout(armed.timer);armed.messages.length=0;record('cancelled',{id:armed.id});}armed=null;record('closed');},
  status(){return{held:armed?.held===true,connections,closed};}
 };
}
