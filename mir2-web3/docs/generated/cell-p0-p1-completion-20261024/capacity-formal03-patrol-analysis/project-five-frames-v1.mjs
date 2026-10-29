import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024', output=path.join(base,'formal03-analysis');
const file=path.join(base,'formal-native-v2-ladder-03/recordings/0-498020.jsonl');
const fd=fs.openSync(file,'r'), stat=fs.fstatSync(fd); let bytesRead=0, headerProbes=0;
function read(offset,length){const buffer=Buffer.alloc(Math.min(length,stat.size-offset));const count=fs.readSync(fd,buffer,0,buffer.length,offset);bytesRead+=count;return buffer.subarray(0,count);}
function nextLine(offset){if(offset===0)return 0;let cursor=offset;while(cursor<stat.size){const chunk=read(cursor,65536),index=chunk.indexOf(10);if(index>=0)return cursor+index+1;cursor+=chunk.length;}return stat.size;}
function header(offset){if(offset>=stat.size)return null;const text=read(offset,4096).toString('utf8'),m=text.match(/"capturedAtMs":(\d+)/);if(!m)throw new Error('Missing expected public frame header');headerProbes++;return{offset,capturedAtMs:Number(m[1])};}
function before(at){let low=0,high=stat.size,best=header(0);for(let tries=0;low<high&&tries<40;tries++){const middle=Math.floor((low+high)/2),start=nextLine(middle),h=header(start);if(!h||h.capturedAtMs>at)high=middle;else{if(h.capturedAtMs>=best.capturedAtMs)best=h;low=start+1;}}
  let cursor=nextLine(best.offset+1);for(let tries=0;cursor<stat.size&&tries<10;tries++){const h=header(cursor);if(h.capturedAtMs>at)break;best=h;cursor=nextLine(cursor+1);}return best;}
function rawLine(offset){const parts=[];let cursor=offset,total=0;while(cursor<stat.size){const chunk=read(cursor,65536),index=chunk.indexOf(10),part=index<0?chunk:chunk.subarray(0,index);parts.push(part);total+=part.length;if(total>2200000)throw new Error('Selected frame exceeds bounded public frame allowance');if(index>=0)break;cursor+=chunk.length;}return Buffer.concat(parts);}
const requests=[{walkAtMs:1792873817398,targetAtMs:1792873817390},{walkAtMs:1792873826390,targetAtMs:1792873826380},{walkAtMs:1792873832410,targetAtMs:1792873832400},{walkAtMs:1792873853390,targetAtMs:1792873853380},{walkAtMs:1792873870655,targetAtMs:1792873870645}];
const bounds={minX:249,maxX:257,minY:668,maxY:676};
const frames=requests.map(request=>{const h=before(request.targetAtMs),raw=rawLine(h.offset),frame=JSON.parse(raw);return{...request,
  originalFrame:{byteOffset:h.offset,bytesExcludingNewline:raw.length,sha256:crypto.createHash('sha256').update(raw).digest('hex')},
  capturedBeforeTargetByMs:request.targetAtMs-frame.capturedAtMs,
  header:Object.fromEntries(Object.entries(frame).filter(([key])=>!['world','events'].includes(key))),
  entities:(frame.world?.entities??[]).filter(e=>e.x>=bounds.minX&&e.x<=bounds.maxX&&e.y>=bounds.minY&&e.y<=bounds.maxY)
    .map(e=>Object.fromEntries(Object.entries(e).filter(([key])=>['objectId','name','kind','x','y','dead','hp'].includes(key))))};});
fs.closeSync(fd);
const record={schemaVersion:1,projectedAt:new Date().toISOString(),file,originalFileBytes:stat.size,originalFileModifiedAtMs:stat.mtimeMs,bounds,
  scope:'Exactly five selected public frame lines are JSON parsed. Binary seek reads headers and line-boundary blocks only for other frames; no other entity payloads are inspected or copied. Selected raw-line SHA/offset bind projections; the complete recording is neither copied nor hashed.',
  equivalenceLimitation:'Spectator merged world can include multiple sources/stale positions. These public frames are not the actor00 send-time AOI or complete navigation obstacle set, and cannot alone prove which entity blocked a plan.',
  bytesReadForBoundedSeeks:bytesRead,headerProbes,selectedPayloadFramesParsed:frames.length,frames};
const out=path.join(output,'five-public-position-frames-v1.json');fs.writeFileSync(out,JSON.stringify(record,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({out,bytes:fs.statSync(out).size,headerProbes,bytesRead,frames}));
