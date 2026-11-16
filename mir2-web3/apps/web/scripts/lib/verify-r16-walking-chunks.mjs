import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import ts from 'typescript';
import {PNG} from 'pngjs';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
function literal(node){
 if(ts.isStringLiteralLike(node))return node.text;
 if(ts.isNumericLiteral(node))return Number(node.text);
 if(node.kind===ts.SyntaxKind.TrueKeyword)return true;
 if(node.kind===ts.SyntaxKind.FalseKeyword)return false;
 if(node.kind===ts.SyntaxKind.NullKeyword)return null;
 if(ts.isPrefixUnaryExpression(node)&&node.operator===ts.SyntaxKind.MinusToken&&ts.isNumericLiteral(node.operand))return -Number(node.operand.text);
 if(ts.isArrayLiteralExpression(node))return node.elements.map(literal);
 if(ts.isObjectLiteralExpression(node))return Object.fromEntries(node.properties.map(property=>{
  assert(ts.isPropertyAssignment(property)&&!ts.isComputedPropertyName(property.name),'not a JSON object');
  return[property.name.text,literal(property.initializer)];
 }));
 throw Error('not a JSON literal');
}

/** Inspect complete browser/build script bytes without evaluating the module. */
export function verifyWalkingPartScript(body,pin){
 assert(Buffer.isBuffer(body)&&body.length>0&&body.length<=1024*1024,'part script byte limit');
 const ast=ts.createSourceFile('walking-part.js',body.toString('utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),values=[];
 assert.equal(ast.parseDiagnostics.length,0,'part script parse failure');
 const accept=value=>{if(value&&typeof value==='object'&&!Array.isArray(value)&&value.libraries&&value.images)values.push(value);};
 const visit=node=>{
  if(ts.isCallExpression(node)&&node.expression.getText(ast)==='JSON.parse'&&node.arguments.length===1&&ts.isStringLiteralLike(node.arguments[0])){
   try{accept(JSON.parse(node.arguments[0].text));}catch{}
  }
  if(ts.isObjectLiteralExpression(node)&&node.properties.some(property=>property.name?.text==='libraries')){
   try{accept(literal(node));}catch{}
  }
  ts.forEachChild(node,visit);
 };visit(ast);
 assert.equal(values.length,1,'part payload must be unique');
 const payload=values[0],bytes=Buffer.from(JSON.stringify(payload)+'\n');
 assert.equal(bytes.length,pin.bytes,'part payload byte mismatch');assert.equal(hash(bytes),pin.sha256,'part payload digest mismatch');
 assert.deepEqual(Object.keys(payload).sort(),['images','libraries']);assert.deepEqual(Object.keys(payload.libraries),[pin.libraryKey]);
 assert.equal(Object.keys(payload.images).length,48,'complete part must contain 48 PNGs');
 return{payload,receipt:{passed:true,libraryKey:pin.libraryKey,scriptBytes:body.length,scriptSha256:hash(body),
  payloadBytes:bytes.length,payloadSha256:hash(bytes),imageCount:48}};
}

/** Require all five actual script bodies, the original aggregate pin and every decoded PNG. */
export function verifyWalkingChunkGroup(bodies,manifest){
 assert.equal(manifest.schemaVersion,1);assert.equal(manifest.parts.length,5);assert.equal(bodies.length,5);
 const byLibrary=new Map(bodies.map(entry=>[entry.libraryKey,entry.body]));assert.equal(byLibrary.size,5,'duplicate part body');
 const verified=manifest.parts.map(pin=>verifyWalkingPartScript(byLibrary.get(pin.libraryKey),pin));
 const payload={libraries:Object.fromEntries(verified.flatMap(value=>Object.entries(value.payload.libraries))),
  images:Object.fromEntries(verified.flatMap(value=>Object.entries(value.payload.images)))};
 const bytes=Buffer.from(JSON.stringify(payload)+'\n');
 assert.equal(bytes.length,manifest.sourceBytes);assert.equal(hash(bytes),manifest.sourceSha256,'whole walking pack digest mismatch');
 assert.equal(Object.keys(payload.images).length,240);const paths=new Set();
 for(const [libraryKey,library]of Object.entries(payload.libraries)){
  const walking=library.frameSet.actions.find(action=>action.actionName==='Walking');
  assert(walking&&walking.count===6&&Number.isSafeInteger(walking.start)&&walking.start>=0&&Number.isSafeInteger(walking.skip)&&walking.skip>=0);
  assert.equal(library.frames.length,48);
  const indices=Array.from({length:8},(_,direction)=>Array.from({length:6},(_,frame)=>walking.start+direction*(6+walking.skip)+frame)).flat();
  assert.deepEqual(library.frames.map(frame=>frame.index),indices);
  for(const frame of library.frames){
   assert.equal(frame.path,'/original-ui/'+libraryKey+'/'+frame.index+'.png');assert(!paths.has(frame.path));paths.add(frame.path);
   const image=payload.images[frame.path];assert.equal(typeof image,'string');assert(image.startsWith('data:image/png;base64,'));
   const encoded=image.slice('data:image/png;base64,'.length),png=Buffer.from(encoded,'base64');assert.equal(png.toString('base64'),encoded);
   const decoded=PNG.sync.read(png);assert.equal(decoded.width,frame.width);assert.equal(decoded.height,frame.height);
   assert(decoded.width>0&&decoded.height>0);
  }
 }
 assert.equal(paths.size,240);
 return{payload,receipt:{passed:true,parts:verified.map(value=>value.receipt),payloadBytes:bytes.length,
  payloadSha256:hash(bytes),pngs:240,libraries:5,directions:8,framesPerDirection:6,
  scope:'Complete AST/JSON extraction from observed scripts, exact aggregate digest and PNG decoding; no module execution or resource prefill.'}};
}
