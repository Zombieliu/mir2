#!/usr/bin/env node
// One bounded request per isolated subprocess; stdout is a single JSON result.
import { dispatch } from './driver.mjs';
const MAX=1048576;
const timeout=setTimeout(()=>{process.stdout.write(JSON.stringify({ok:false,error:'usdcDriverTimeout'})+'\n');process.exit(1);},19000);
try{
  let length=0,chunks=[];
  for await(const chunk of process.stdin){length+=chunk.length;if(length>MAX)throw Error('usdcRequestTooLarge');chunks.push(chunk);}
  const request=JSON.parse(Buffer.concat(chunks).toString('utf8'));
  const result=await dispatch(request);
  const output=JSON.stringify({ok:true,result},(_,v)=>typeof v==='bigint'?v.toString():v);
  if(Buffer.byteLength(output)>MAX)throw Error('usdcResponseTooLarge');process.stdout.write(output+'\n');
}catch(e){
  // Never serialize RPC URLs, transaction contents, environment or key material.
  const message=/^usdc[A-Za-z]{1,96}$/.test(e?.message??'')?e.message:'usdcDriverUnavailable';
  process.stdout.write(JSON.stringify({ok:false,error:message})+'\n');process.exitCode=1;
}finally{clearTimeout(timeout);}
