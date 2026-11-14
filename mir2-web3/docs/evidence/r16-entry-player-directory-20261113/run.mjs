import { installPlayerProbe, verifyEarlyPlayer } from './entry-player-probe.mjs';
import assert from "node:assert/strict";
import {launchProfile,profileProcesses,fontCacheInventory} from "./profile.mjs";
import { verifySceneState } from "./verify-scene-state-body.mjs";
import { verifySave } from "file:///E:/r16-shared-ui-entry-20261111/verify-graduation.mjs";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync, readFileSync } from "node:fs";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer as createHttpServer, request as httpRequest } from "node:http";
import { createRequire } from "node:module";
import { createServer as createTcpServer } from "node:net";
import { basename, dirname, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const root = "C:/r16-first-playable-gap-20261113/game-full";
const mode = process.env.R16_QA_MODE ?? "compatibility";
assert(["compatibility", "webgl2"].includes(mode));
const browserLanguage = process.env.R16_QA_LANGUAGE ?? "zh-TW";
assert(["zh-TW", "en-US"].includes(browserLanguage));
const gamePath = mode === "compatibility" ? "/game" : "/game?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1&bevyBagUi=1";
const [runName, expectedRevision, expectedBuildReceiptSha, staticPath, expectedStaticSha] = process.argv.slice(2);
assert.match(runName ?? "", /^quest-(before|after)-r[1-9][0-9]*$/);
const baseline = runName.startsWith("quest-before-"), stalled = false, rawFirst = false;
const candidateParent = baseline ? "04420023eeec4025bb57be94611ad2d0ec6151d1" : "8e82f53fcd749e8c2f6a69cb9b911c3165de006a";
assert.match(expectedRevision ?? "", /^[a-z0-9][a-z0-9-]{10,79}$/);
assert.match(expectedBuildReceiptSha ?? "", /^[a-f0-9]{64}$/);
assert.match(staticPath ?? "", /^\/(?:[A-Za-z0-9._-]+\/)*[A-Za-z0-9._-]+\.json$/);
assert(!staticPath.startsWith("/api/") && !staticPath.split("/").includes(".."));
assert.match(expectedStaticSha ?? "", /^[a-f0-9]{64}$/);
const primary = true;
const invalidStatic = false;
const otherRegion = false;
const blockCovered = false;
const packedVisuals = false;
const invalidVisuals = false;
const lateObjects = false;
const maxMapBodyBytes = 16 * 1024 * 1024;

const buildRoot = "C:/r16-ui-asset-recovery-20261113/source";
const web = join(buildRoot, "mir2-web3/apps/web");
const floorManifest = JSON.parse(await readFile(join(web, "lib/generated/r16-entry-floor.json"), "utf8"));
const floorBytes = await readFile(join(web, "public", floorManifest.path.slice(1)));
assert.equal(floorBytes.length, floorManifest.bytes);
assert.equal(createHash("sha256").update(floorBytes).digest("hex"), floorManifest.sha256);
assert.equal(floorManifest.sceneSha256, expectedStaticSha);
const floorPaths = new Set(Object.keys(JSON.parse(floorBytes)));
const localeManifest = JSON.parse(await readFile(join(web, "lib/generated/r16-native-localization-delivery.json"), "utf8"));
const localeBytes = await readFile(join(web, "public", localeManifest.path.slice(1)));
assert.equal(localeBytes.length, localeManifest.bytes);
assert.equal(createHash("sha256").update(localeBytes).digest("hex"), localeManifest.sha256);
assert.equal(floorPaths.size, floorManifest.frameCount);
const visualsManifest = JSON.parse(await readFile(join(web, "lib/generated/r16-entry-visuals.json"), "utf8"));
const visualsByPath = new Map();
const actorKeys = new Set(), actorPaths = new Set(), monsterPaths = {};
const objectPaths = new Set();
const visualEntries = [
  ["player", visualsManifest.players[process.env.R16_QA_GENDER ?? "male"]], ["npcs", visualsManifest.npcs],
  ...visualsManifest.objectParts.map((part, index) => ["objects" + (index + 1), part]),
  ["monsterStanding", visualsManifest.monsterStanding], ["monsterWalking", visualsManifest.monsterWalking],
];
for (const [kind, entry] of visualEntries) {
  const bytes = await readFile(join(web,"public",entry.path.slice(1)));
  assert.equal(bytes.length,entry.bytes);
  assert.equal(createHash("sha256").update(bytes).digest("hex"),entry.sha256);
  visualsByPath.set(entry.path,{kind,bytes});
  const data = JSON.parse(bytes);
  if (kind.startsWith("objects")) Object.keys(data).forEach(path => objectPaths.add(path));
  else {
    const paths = Object.keys(data.images);
    paths.forEach(path => actorPaths.add(path));
    Object.keys(data.libraries).forEach(key => actorKeys.add(key));
    if (kind.startsWith("monster")) monsterPaths[kind] = paths;
  }
}
let releaseObjectBody, releaseWalkingBody;
const playerImages = new Map();
const playerPack = JSON.parse(visualsByPath.get(visualsManifest.players[process.env.R16_QA_GENDER ?? 'male'].path).bytes);
for(const [path,image] of Object.entries(playerPack.images))playerImages.set(path,Buffer.from(image.split(',')[1],'base64'));
const heldPlayerReads=[];let bodyDelayEndsAt=null;
function releasePlayerReads(){
 proof.earlyPlayer.releasedAt=at();
 for(const finish of heldPlayerReads.splice(0))finish();
}

const groundRequests = [];
const run = join(root, runName);
const [gatewayPort, proxyPort, tcpPort, nextPort] = [20630, 20631, 20632, 20633];
const gatewayOrigin = `http://127.0.0.1:${gatewayPort}`;
const origin = `http://127.0.0.1:${proxyPort}`;
const ws = `ws://127.0.0.1:${gatewayPort}/ws`;
const release = "20261108-r16-08ec86d5105c";
const expectedRenderer = "bevy-0c553a754308e5b8";
const assetRoot = `https://assets.mir2.obelisk.build/mir2/v/${release}`;

const prior = "C:/r16-public-scene-recovery-20261110/release-source/mir2-web3";
const className = process.env.R16_QA_CLASS;
assert(["Warrior", "Wizard", "Taoist"].includes(className));
const fixtures = JSON.parse(await readFile("E:/r16-shared-ui-entry-20261111/graduate-fixtures.json", "utf8"));
const selectedFixture = fixtures.find(row => row.className === className);
const fixture = "D:/mir2-builds/web-newcomer-30-20261107/realms/" + selectedFixture.realm;
const provisionRoot = "C:/r16-fishing-qa-20261110/provision-spares-r1";
const source = selectedFixture.source;
const sourceSha = selectedFixture.sourceSha256;
const original = join(fixture, "private-data/accounts.json");
const mapSha = "ed4783215ffa989658f79892c2dd6720753fb111e1102cb14da8e6182d88d7f6";
const clientAssets = "E:/mir2/Crystal/Build/Client/Debug";
const gatewayExe = "C:/r16-fishing-timing-20261110/mir2-gateway-fishing-wake.exe";
const gatewaySha = "f35261772f379b04759011a63a2f3c463a36e754ebbde71915e893c84d77d490";
const spawnTile = { x: 288, y: 616 };
const startTile = spawnTile;
const spawnCenter = { x: 296, y: 620 };
const expectedCenter = spawnCenter;
const schema = "2026-08-14-v7-door-interaction";
async function sha(file) {
  const hash = createHash("sha256");
  for await (const bytes of createReadStream(file)) hash.update(bytes);
  return hash.digest("hex");
}
const json = async file => JSON.parse(await readFile(file, "utf8"));
const sameTile = (a, b) => !!a && !!b && a.x === b.x && a.y === b.y;
const point = value => ({ x: value.x, y: value.y });
const at = () => Math.round(performance.timeOrigin + performance.now());
const mapName = value => String(value).split(/[\\/]/).at(-1).replace(/\.map$/i, "");

const lane = baseline ? "original-fallback" : "localized-fallback";
const catalogManifest=JSON.parse(await readFile(join(web,"lib/generated/client_core_runtime.json"),"utf8"));
const catalogLeaf=join(web,"public/client-core",catalogManifest.catalogTransport.version);
const catalogGlue=await readFile(join(catalogLeaf,"mir2_platform_web.js"));
const catalogWasm=await import("data:text/javascript;base64,"+catalogGlue.toString("base64"));
await catalogWasm.default({module_or_path:await readFile(join(catalogLeaf,"mir2_platform_web_bg.wasm"))});
const catalogSessions=[];let activeCatalog=null;
const proof = { kind: "r16-complete-prewarm-and-reopen", lane, className, mode, browserLanguage, gamePath,
  passed: false, renderer: expectedRenderer, assetRelease: release,
  ports: [gatewayPort, proxyPort, tcpPort, nextPort], isolatedFile: true,
  publicAuth: false, isolatedGateway: true, publicAssetReadsAllowed: true, nativeServiceWorker: true,
  staticPath, expectedStaticSha, expectedCenter, dynamicSpawnProxyStatus: null, stages: [], proxyRequests: [], browserRequests: [] };
proof.earlyPlayer={gender:process.env.R16_QA_GENDER ?? "male",bodyDelayMs:6000,releasedAt:null,reads:[]};
function stage(name) { proof.stages.push({ name, at: at() }); console.log(name); }
async function poll(read, test, label, timeout = 25000, interval = 30) {
  const deadline = performance.now() + timeout;
  do {
    const value = await read();
    if (test(value)) return value;
    await delay(interval);
  } while (performance.now() < deadline);
  throw Error(label);
}
const wait = (read, label, timeout) => poll(read, Boolean, label, timeout);
const stopped = child => !child || child.exitCode !== null || child.signalCode !== null;
function spawnOwned(file, args, cwd, env) {
  // Fixture credentials and world data stay in memory and the private File store.
  const child = spawn(file, args, { cwd, env, stdio: ["pipe", "ignore", "ignore"], windowsHide: true });
  child.on("error", error => { proof.childError = error.code ?? "SPAWN_FAILED"; });
  return child;
}
async function health(url, child) {
  return wait(() => !stopped(child) && fetch(url, { signal: AbortSignal.timeout(2500) })
    .then(response => { void response.body?.cancel().catch(() => {}); return response.ok; }).catch(() => false), "owned server health", 65000);
}

function classify(value) {
  const url = new URL(value, origin);
  if (url.origin !== origin) return null;
  const path = url.pathname;
  if (path === staticPath) return { path, scope: "static" };
  if (path === floorManifest.path) return { path, scope: "floor-pack" };
  if (path === localeManifest.path) return { path, scope: "gameplay-locale" };
  if (floorPaths.has(path)) return { path, scope: "covered-floor" };
  if (visualsByPath.has(path)) return { path, scope: "visual-pack", kind: visualsByPath.get(path).kind };
  if (objectPaths.has(path)) return { path, scope: "covered-object" };
  if (actorPaths.has(path)) return { path, scope: "covered-actor" };
  const library = path === "/api/original-ui-meta" ? url.searchParams.get("library")
    : path.match(/^\/original-ui\/(.+)\/meta\.json$/)?.[1];
  if (actorKeys.has(library)) return { path, scope: "covered-meta", library };
  if (path === "/api/asset-manifest" || path === "/api/scene/starter") return { path };
  if (path === "/api/scene/crystal") return { path, scope: "dynamic", query: Object.fromEntries(
    ["v", "assetRelease", "map", "x", "y", "width", "height"].map(key => [key, url.searchParams.get(key)])) };
  return null;
}

const exactScene = (entry, center) => entry?.scope === "dynamic"
  && entry.query.v === schema && entry.query.assetRelease === release && entry.query.map === "0"
  && entry.query.x === String(center.x) && entry.query.y === String(center.y)
  && entry.query.width === "56" && entry.query.height === "72";
const getBlueprint = value => value?.blueprint ?? value;
const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const inBounds = (bounds, tile) => tile.x >= bounds.minX && tile.x <= bounds.maxX
  && tile.y >= bounds.minY && tile.y <= bounds.maxY;

function summarizeBlueprint(value, center, tile) {
  const blueprint = getBlueprint(value);
  assert.deepEqual(blueprint.sceneView, { center, width: 48, height: 36 });
  const region = blueprint.originalMapRegion;
  assert.equal(mapName(region.mapFileName), "0");
  assert.deepEqual([region.cellWidth, region.cellHeight], [48, 32]);
  assert([region.mapWidth, region.mapHeight].every(Number.isSafeInteger));
  const playBounds = { minX: center.x - 24, maxX: center.x + 24, minY: center.y - 18, maxY: center.y + 18 };
  const regionBounds = { minX: playBounds.minX - 28, maxX: playBounds.maxX + 28,
    minY: playBounds.minY - 22, maxY: playBounds.maxY + 43 };
  assert.deepEqual(region.playBounds, playBounds);
  assert.deepEqual(region.regionBounds, regionBounds);
  assert(region.mapWidth > regionBounds.maxX && region.mapHeight > regionBounds.maxY && inBounds(playBounds, tile));
  assert(Array.isArray(region.cells) && region.cells.length > 1000);
  const uniqueCells = new Set();
  for (const cell of region.cells) {
    assert(Number.isSafeInteger(cell.x) && Number.isSafeInteger(cell.y) && inBounds(regionBounds, cell));
    uniqueCells.add(cell.x + "," + cell.y);
    for (const field of ["back", "middle", "front", "tileAnimation"]) {
      if (cell[field] != null) assert(Object.hasOwn(region.sprites, cell[field]));
    }
  }
  assert.equal(uniqueCells.size, region.cells.length);
  const sprites = Object.values(region.sprites);
  assert(sprites.length > 0);
  assert(sprites.every(sprite => sprite.frames.length > 0 &&
    sprite.frames.every(frame => typeof frame.path === "string" && frame.width > 0 && frame.height > 0)));
  return { sceneView: blueprint.sceneView, mapFileName: "0",
    mapWidth: region.mapWidth, mapHeight: region.mapHeight, cellWidth: region.cellWidth, cellHeight: region.cellHeight,
    playBounds, regionBounds, spriteCount: sprites.length, cellCount: region.cells.length,
    missingAssetCount: region.missingAssets?.length ?? 0, minimap: blueprint.miniMapIndex ?? null };
}

const controlledFontPins=JSON.parse(await readFile("C:/r16-hud-font-route-20261111/public-font-readback.json","utf8")).rows;
const controlledFontBodies=new Map();
for(const pin of controlledFontPins){const bytes=await readFile(join(web,"public/original-ui/fonts",pin.name));assert.equal(bytes.length,pin.bytes);assert.equal(createHash("sha256").update(bytes).digest("hex"),pin.sha256);controlledFontBodies.set(pin.name,{...pin,bytes});}
proof.controlledFontDelivery={kind:"fixed current-release font HTTP mirror; not a WAN font result",origin:"http://localhost:20631",reads:[],preferredFailures:[],failureInjection:true,scenario:"current503-plus-inherited404-then-complete-current-font-retry"};
const stalledScFont=await readFile(join(web,'public/original-ui/fonts/NotoSansCJKsc-Regular.otf'));
let retryFontHealthy=false;
const proxy = createHttpServer((request, response) => {
  const fontPath=new URL(request.url,'http://localhost').pathname;
  if(request.method==='GET'&&fontPath==='/mir2/v/older-release/original-ui/fonts/NotoSansCJKsc-Regular.otf'){
    retryFontHealthy=true;const row={path:fontPath,scope:'inherited-font-missing',requestedAt:at(),status:404};proof.proxyRequests.push(row);
    response.writeHead(404,{'access-control-allow-origin':'*','cache-control':'no-store'});response.end('inherited font absent');return;
  }
  if(request.method==='GET'&&fontPath.startsWith('/mir2/v/older-release/')){response.writeHead(404,{'access-control-allow-origin':'*'});response.end();return;}
  if(request.method==='GET'&&fontPath==='/mir2/v/20261108-r16-08ec86d5105c/original-ui/fonts/NotoSansCJKsc-Regular.otf'&&!retryFontHealthy){
    const row={path:fontPath,scope:'temporary-current-font-failure',requestedAt:at(),status:503};proof.proxyRequests.push(row);
    response.writeHead(503,{'access-control-allow-origin':'*','cache-control':'no-store','retry-after':'1'});response.end('temporary font transport failure');return;
  }
  if(request.method==='GET'&&fontPath.startsWith('/stalled-font/mir2/v/20261108-r16-08ec86d5105c/original-ui/fonts/')){
    const row={path:fontPath,requestedAt:at(),noHeaders:true};proof.controlledFontDelivery.preferredFailures.push(row);
    const timer=setTimeout(()=>{if(!response.destroyed){response.writeHead(503);response.end();}},30000);
    response.once('close',()=>{clearTimeout(timer);row.closedAt=at();});return;
  }
  const controlledFont=fontPath.match(/^\/mir2\/v\/20261108-r16-08ec86d5105c\/original-ui\/fonts\/([^/]+)$/);
  if(request.method==="GET"&&controlledFont){
    const pin=controlledFontBodies.get(controlledFont[1]);assert(pin);
    const fontDelayMs=pin.name==="NotoSansCJKsc-Regular.otf"?12000:50; const fontRead={path:fontPath,at:at(),bytes:pin.bytes.length,sha256:pin.sha256,controlledBodyDelayMs:fontDelayMs};proof.controlledFontDelivery.reads.push(fontRead);
    response.writeHead(200,{"access-control-allow-origin":"*","content-type":pin.name.endsWith(".otf")?"font/otf":"font/ttf","content-length":pin.bytes.length,"cache-control":"public, max-age=31536000"});response.flushHeaders(); const fontTimer=setTimeout(()=>{if(!response.destroyed)response.end(pin.bytes);},fontDelayMs);response.once("finish",()=>{fontRead.finishedAt=at();});response.once("close",()=>clearTimeout(fontTimer));return;
  }
  if(request.method==='GET'&&fontPath==='/original-ui/fonts/NotoSansCJKsc-Regular.otf'){
    proof.proxyRequests.push({path:fontPath,scope:'application-font-missing',requestedAt:at(),status:404});
    response.writeHead(404,{'cache-control':'no-store'});response.end('fixture app font absent');return;
  }
  if(request.method==='GET'&&fontPath.startsWith('/original-ui/fonts/')){
    const row={path:fontPath,scope:'app-font-read',requestedAt:at()};proof.proxyRequests.push(row);response.once('finish',()=>{row.finishedAt=at();row.status=response.statusCode;});
  }
  const fixtureHud = fontPath.match(/^\/mir2\/v\/20261108-r16-08ec86d5105c(\/original-ui\/Prguse\/([0-9]+)\.png)$/);
  if(request.method==='GET'&&fixtureHud){
    const body=readFileSync(join(web,'public',fixtureHud[1].slice(1)));
    response.writeHead(200,{'content-type':'image/png','content-length':body.length,'access-control-allow-origin':'*','cache-control':'public, max-age=31536000'});response.end(body);return;
  }
  const hostedHud = /^\/original-ui\/Prguse\/(1|4|6|8)\.png(?:\?|$)/.test(request.url);
  if(hostedHud){const row={scope:"forced-hud-header-stall",path:fontPath,requestedAt:at(),intentionallyNoResponse:true};proof.proxyRequests.push(row);response.once("close",()=>{row.closedAt=at();});return;}
  const actorPath=fontPath.replace('/mir2/v/20261108-r16-08ec86d5105c','');
  const actorMeta=/^\/original-ui\/(?:CArmour|CHair)\/00\/meta.json$/.test(actorPath);
  const png=playerImages.get(actorPath);
  if(request.method==='GET'&&(actorMeta||png)){
   const row={path:actorPath,kind:actorMeta?'remote-player-meta':'loose-player-frame',requestedAt:at()};proof.earlyPlayer.reads.push(row);
   const finish=()=>{if(response.destroyed)return;row.headersAt=at();row.status=png?200:503;if(png){row.bytes=png.length;row.sha256=digest(png);}response.writeHead(row.status,{'content-type':png?'image/png':'application/json','content-length':png?.length??0,'access-control-allow-origin':'*','cache-control':'no-store'});response.end(png);};
   response.once('finish',()=>{row.finishedAt=at();});response.once('close',()=>{row.closedAt=at();});
   if(actorMeta&&!proof.earlyPlayer.releasedAt)heldPlayerReads.push(finish);
   else if(png&&!proof.earlyPlayer.releasedAt){bodyDelayEndsAt??=at()+6000;const timer=setTimeout(finish,Math.max(0,bodyDelayEndsAt-at()));response.once('close',()=>clearTimeout(timer));}
   else finish();
   return;
  }
  const classified = classify(request.url);
  const event = classified ? { ...classified, requestedAt: at() } : null;
  if (event) proof.proxyRequests.push(event);
  if(request.method==='GET'&&event?.scope==='visual-pack'&&event.kind==='player'&&!proof.earlyPlayer.releasedAt){
   const payload=visualsByPath.get(event.path).bytes;
   const row={path:event.path,kind:'complete-player-pack',requestedAt:at()};proof.earlyPlayer.reads.push(row);
   heldPlayerReads.push(()=>{if(response.destroyed)return;Object.assign(row,{status:200,bytes:payload.length,sha256:digest(payload),headersAt:at()});response.writeHead(200,{'content-type':'application/json','content-length':payload.length,'cache-control':'no-store'});response.end(payload);});
   response.once('finish',()=>{row.finishedAt=at();});response.once('close',()=>{row.closedAt=at();});return;
  }
  if (request.method === "GET" && event?.scope === "visual-pack") {
    const payload = Buffer.from(visualsByPath.get(event.path).bytes);
    if (invalidVisuals) payload[64] ^= 1;
    Object.assign(event,{status:200,bytes:payload.length,sha256:digest(payload),headersAt:at()});
    response.writeHead(200,{"content-type":"application/json","content-length":payload.length,"cache-control":"no-store"});
    const interval = 0;
    if (!interval) {
      event.finishedAt = at();
      response.end(payload);
    } else {
      event.chunks = [];
      let timer;
      response.once("close", () => { clearTimeout(timer); event.closedAt = at(); });
      response.once("finish", () => { event.finishedAt = at(); });
      const count = event.kind === "player" ? 4 : 7;
      const size = Math.ceil(payload.length / count);
      function writePart(index) {
        if (response.destroyed) return;
        const chunk = payload.subarray(index * size, (index + 1) * size);
        event.chunks.push({ at: at(), bytes: chunk.length });
        if (index === count - 1) response.end(chunk);
        else {
          response.write(chunk);
          timer = setTimeout(() => writePart(index + 1), interval);
        }
      }
      writePart(0);
    }
    return;
  }
  if (request.method === "GET" && packedVisuals && ["covered-object","covered-actor","covered-meta"].includes(event?.scope)) {
    Object.assign(event,{status:503,blockedByHarness:true,headersAt:at()});
    response.writeHead(503,{"cache-control":"no-store"});
    response.end();
    return;
  }
  if (request.method === "GET" && ["floor-pack", "gameplay-locale"].includes(event?.scope)) {
    const floor = event.scope === "floor-pack";
    const payload = floor ? floorBytes : localeBytes;
    if (floor) groundRequests.push(event);
    Object.assign(event, { status: 200, sha256: digest(payload), bytes: payload.length, chunks: [] });
    const timer = setTimeout(() => {
      if (response.destroyed) return;
      event.headersAt = at();
      response.writeHead(200, { "Content-Type": "application/json", "Content-Length": payload.length, "Cache-Control": "no-store" });
      event.chunks.push({ at: at(), bytes: payload.length });
      response.end(payload);
    }, 0);
    response.once("close", () => { clearTimeout(timer); event.closedAt = at(); });
    response.once("finish", () => { event.finishedAt = at(); });
    return;
  }
  if (request.method === "GET" && event?.scope === "covered-floor" && blockCovered) {
    Object.assign(event, { blockedByHarness: true, status: 503, headersAt: at() });
    response.writeHead(503, { "Cache-Control": "no-store" });
    response.end();
    return;
  }
  if (primary && request.method === "GET" && exactScene(event, spawnCenter)) {
    Object.assign(event, { blockedByHarness: true, status: 503, headersAt: at() });
    response.writeHead(503, { "Content-Type": "text/plain", "Cache-Control": "no-store" });
    response.end("Exact spawn dynamic route deliberately disabled in this test.");
    return;
  }

  if (request.method === "GET" && event?.scope === "static" && proof.proxyRequests.filter(row => row.scope === "static").length <= 2) {
    Object.assign(event,{status:503,blockedByHarness:true,headersAt:at()});
    response.writeHead(503,{"Cache-Control":"no-store"});response.end();return;
  }
  const headersToOrigin = { ...request.headers, connection: "close" };
  if (event?.scope === "static" || event?.scope === "dynamic") headersToOrigin["accept-encoding"] = "identity";
  const upstream = httpRequest({ host: "127.0.0.1", port: nextPort, path: /^\/original-ui\/Prguse\/(1|4|6|8)\.png(?:\?|$)/.test(request.url) ? "/api/r2-proxy" + request.url : request.url,
    method: request.method, headers: headersToOrigin }, body => {
    const headers = { ...body.headers };
    delete headers.connection;
    if (event) Object.assign(event, { status: body.statusCode, originHeadersAt: at() });
    body.once("error", () => { if (!response.destroyed) response.destroy(); });
    if (body.statusCode !== 200 || !["static", "dynamic"].includes(event?.scope)) {
      if (event) event.headersAt = at();
      response.writeHead(body.statusCode, headers);
      body.pipe(response);
      return;
    }
    const received = [];
    let total = 0;
    body.on("data", chunk => {
      total += chunk.length;
      if (total > maxMapBodyBytes) {
        event.controlError = "map-body-too-large";
        response.destroy();
      } else received.push(chunk);
    });
    body.once("end", () => {
      if (response.destroyed) return;
      try {
        assert(!headers["content-encoding"] || headers["content-encoding"] === "identity");
        let bytes = Buffer.concat(received, total);
        event.originBodySha256 = digest(bytes);
        event.originBodyBytes = bytes.length;
        if (event.scope === "static") assert.equal(event.originBodySha256, expectedStaticSha);
        const decoded = JSON.parse(bytes.toString("utf8"));
        const center = event.scope === "static" ? spawnCenter : expectedCenter;
        const tile = event.scope === "static" ? spawnTile : startTile;
        if (event.scope === "static" || exactScene(event, expectedCenter)) {
          event.originGeometry = summarizeBlueprint(decoded, center, tile);
        }
        if (event.scope === "static" && invalidStatic) {
          // Preserve genuine map data while making its claimed coverage miss
          // the authoritative player tile only in this negative test.
          const region = getBlueprint(decoded).originalMapRegion;
          region.playBounds = { minX: 528, maxX: 576, minY: 424, maxY: 460 };
          region.regionBounds = { minX: 500, maxX: 604, minY: 402, maxY: 503 };
          assert(!inBounds(region.regionBounds, spawnTile));
          const serialized = Buffer.from(JSON.stringify(decoded), "utf8");
          assert(serialized.length <= bytes.length);
          bytes = Buffer.concat([serialized, Buffer.alloc(bytes.length - serialized.length, 32)]);
          event.injectedInvalidStaticGeometry = true;
          headers["cache-control"] = "no-store";
          delete headers.etag;
        }
        delete headers["transfer-encoding"];
        headers["content-length"] = String(bytes.length);
        event.deliveredSha256 = digest(bytes);
        event.deliveredBytes = bytes.length;
        const deliver = () => {
          if(response.destroyed)return;
          event.headersAt=at();response.writeHead(200,headers);response.end(bytes);event.finishedAt=at();
        };
        if(event.scope==="static"){
          event.controlledHeaderDelayMs=17000;
          const timer=setTimeout(deliver,17000);response.once("close",()=>{clearTimeout(timer);event.closedAt=at();});
        }else deliver();
      } catch (error) {
        event.controlError = "invalid-upstream-map: " + String(error.message).split("\n")[0].slice(0, 100);
        response.destroy();
      }
    });
  });
  upstream.on("error", () => {
    if (event) event.upstreamErrorAt = at();
    if (response.destroyed || response.writableEnded) return;
    if (!response.headersSent) response.writeHead(502);
    response.end();
  });
  response.once("finish", () => { if (event) event.finishedAt = at(); });
  response.once("close", () => {
    if (event) event.closedAt = at();
    upstream.destroy();
  });
  request.pipe(upstream);
});

let gateway, next, browser, page, identity, provision, fixtureIdentity;
let seq = 0;
const sent = [], frames = [];
const own = payload => payload?.entities?.find(row => String(row.objectId) === String(payload.playerObjectId));
const receipt = (packet, after = 0, predicate = () => true, timeout) => wait(
  () => frames.find(row => row.packet === packet && row.seq > after && predicate(row.payload)), `ordinary Gateway ${packet}`, timeout);
const location = payload => (payload.info ?? payload).location ?? (payload.info ?? payload).position ?? (payload.info ?? payload);
async function sceneState() {
  return page.evaluate(() => {
    const root = document.querySelector('main[data-client-screen="game"]');
    const stage = root?.querySelector(".client-stage-frame");
    const gate = window.__mir2SceneGate;
    const metrics = window.__mir2CacheMetrics ?? window.__mir2ActiveCacheMetrics;
    const wanted = new Set(["startGamePacket", "userInformationReady", "gameplayLocaleStart", "gameplayLocaleReady",
      "sceneBlueprintStart", "sceneBlueprintReady", "sceneSpritesReady", "sceneAssetsReady", "firstPlayableFrame"]);
    const marks = (metrics?.milestones ?? window.__mir2PendingCacheMilestones ?? []).filter(row => wanted.has(row.name));
    const first = marks.find(row => row.name === "firstPlayableFrame");
    return { visible: !!stage?.getClientRects().length, pending: stage?.classList.contains("scene-assets-pending") ?? null,
      hasPlayer: gate?.hasRenderPlayer === true, hasRegion: gate?.hasMapRegion === true,
      ready: gate?.screen === "game" && gate.sceneInteractionReady === true,
      serviceWorkerControlled: !!navigator.serviceWorker.controller,
      marks: marks.map(row => ({ name: row.name, atMs: row.atMs ?? null, epochMs: row.epochMs ?? null,
        sceneCache: row.detail?.sceneCache ?? null })),
      applied: window.__mir2Stage5?.state?.originalMapRegionSummary ?? null,
      first: first ? { owner: String(first.detail?.playerObjectId), x: first.detail?.player?.x,
        y: first.detail?.player?.y, map: String(first.detail?.mapFileName) } : null };
  });
}

async function requestOrdinaryLogout() {
  const gutter = await wait(() => page.evaluate(() => {
    const outer = document.querySelector("main[data-client-screen='game'] > .mir-stage");
    const frame = outer?.querySelector(".client-stage-frame");
    if (!outer || !frame) return null;
    const bounds = frame.getBoundingClientRect(), surround = outer.getBoundingClientRect();
    const x = Math.floor((surround.left + bounds.left) / 2), y = Math.floor((bounds.top + bounds.bottom) / 2);
    return x < bounds.left && document.elementFromPoint(x, y) === outer ? { x, y } : null;
  }), "existing player exit gutter is visible", 4000);
  await page.mouse.click(gutter.x, gutter.y, { button: "left", delay: 55 });
  const confirm = page.locator("dialog.r16-stage-exit-dialog[open] .r16-stage-exit-actions button").last();
  await confirm.click({ timeout: 4000 });
  return gutter;
}

await mkdir(run);
try {
  stage("pin-exact-build-and-copy-private-fixture");
  const buildReceiptFile = "C:/r16-first-playable-gap-20261113/build-receipt-r2.json";
  assert.equal(await sha(buildReceiptFile), expectedBuildReceiptSha);
  const buildReceipt = await json(buildReceiptFile);
  assert.equal(buildReceipt.parent, candidateParent);
  assert.equal(buildReceipt.candidate, expectedRevision);
  assert.equal(buildReceipt.version, release);
  assert.equal(buildReceipt.runtime, expectedRenderer);
  assert.deepEqual(buildReceipt.status, { code: 0, signal: null });
  assert.equal(await sha(resolve(buildRoot, "../source.tar")), buildReceipt.sourceArchiveSha256);
  assert.equal(await sha(join(web, "package-lock.json")), buildReceipt.packageLockSha256);
  assert.equal(buildReceipt.status.code, 0);
  assert.equal(buildReceipt.status.signal, null);
  assert.equal(await sha(resolve(buildRoot, "../source.tar")), buildReceipt.sourceArchiveSha256);
  for (const { filename, sha256 } of buildReceipt.changedFiles) {
    assert.match(filename, /^mir2-web3\/(?:apps\/|config\/|packages\/protocol\/|Cargo\.lock$)/);
    assert(!filename.split("/").includes(".."));
    assert.equal(await sha(join(buildRoot, filename)), sha256, filename);
  }
  const generated = Object.entries(buildReceipt.generatedInputs ?? {});
  if (basename(buildRoot).endsWith("-fast")) assert(generated.length > 0, "fast candidate must pin its reused asset bytes");
  for (const [filename, expected] of generated) {
    const relative = filename.replaceAll("\\", "/");
    assert.match(relative, /^(public|lib)\//);
    assert(!relative.split("/").includes(".."));
    assert.equal(await sha(join(web, relative)), expected, relative);
  }
  const compiled = await json(join(web, ".next-entry-player-meta-r2/required-server-files.json"));
  const compiledEnv = compiled.config.env;
  assert.equal(compiled.config.distDir, ".next-entry-player-meta-r2");
  assert.equal(compiledEnv.MIR2_BUILD_REVISION, expectedRevision);
  assert.equal(compiledEnv.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL, ws);
  assert.equal(compiledEnv.MIR2_PINNED_ASSET_VERSION, release);
  assert.equal(compiledEnv.MIR2_PINNED_ASSET_BASE_URL, assetRoot);
  assert.equal(JSON.parse(compiledEnv.MIR2_BEVY_RUNTIME_BUILD_MANIFEST).version, expectedRenderer);
  assert.equal((await readFile(join(web, ".next-entry-player-meta-r2/BUILD_ID"), "utf8")).trim(), buildReceipt.builtId);
  assert(buildReceipt.builtId);
  proof.build = { revision: expectedRevision, buildReceiptSha256: expectedBuildReceiptSha,
    sourceArchiveSha256: buildReceipt.sourceArchiveSha256, changedFiles: buildReceipt.changedFiles,
    verifiedReusedInputCount: generated.length, id: buildReceipt.builtId, assetRelease: release, renderer: expectedRenderer };
  const configFile = join(buildRoot, "mir2-web3/config/numeron-r16-web-assets.json");
  assert.equal(await sha(configFile), buildReceipt.buildConfigSha256);
  const production = await json(configFile);
  assert.equal(production.assetBaseUrl, assetRoot);
  assert.equal(production.version, release);

  const staticRelativePath = staticPath.startsWith("/_next/")
    ? join(".next-entry-player-meta-r2", staticPath.slice("/_next/".length)) : join("public", staticPath.slice(1));
  const staticFile = join(web, staticRelativePath);
  const staticManifest = await json(join(web, "lib/generated/r16-entry-scene.json"));
  assert.equal(staticManifest.assetReleaseId, release);
  assert.equal(staticManifest.schemaVersion, schema);
  assert.deepEqual(staticManifest.request, { mapFileName: "0", centerX: 296, centerY: 620, width: 56, height: 72 });
  assert.equal(staticManifest.path, staticPath);
  assert.equal(staticManifest.sha256, expectedStaticSha);
  assert(staticPath.includes(expectedStaticSha));
  const staticBytes = await readFile(staticFile);
  assert.equal(staticBytes.length, staticManifest.bytes);
  assert.equal(digest(staticBytes), expectedStaticSha);
  proof.exactStaticArtifact = summarizeBlueprint(JSON.parse(staticBytes.toString("utf8")), spawnCenter, spawnTile);
  assert.equal(proof.exactStaticArtifact.missingAssetCount, 0);

  identity = await json(selectedFixture.controller); fixtureIdentity = identity;
  provision = { fixtureSourceSha: sourceSha };
  assert.equal(await sha(source), sourceSha);
  assert.equal(await sha(original), sourceSha);
  assert.equal(await sha(join(clientAssets, "Map/0.map")), mapSha);
  assert.equal(await sha(gatewayExe), gatewaySha);
  const { playableBaseEnvironment, playableGatewayEnvironment, playablePaths, newPlayableState } =
    await import(`file:///${prior}/apps/web/scripts/r16-playable-service-config.mjs`);
  const { parseGatewayJson } = await import(`file:///${prior}/apps/ai-player/src/lossless-json.mjs`);
  const privateRoot = join(run, "private");
  const gatewayCwd = join(privateRoot, "gateway-cwd");
  const dataDir = join(privateRoot, "data");
  const store = playablePaths(dataDir);
  await mkdir(gatewayCwd, { recursive: true });
  await mkdir(store.recovery, { recursive: true });
  await copyFile(source, store.accounts);
  assert.equal(await sha(store.accounts), sourceSha);
  async function diskRole() {
    const db = await json(store.accounts);
    const key = Object.keys(db.accounts).find(value => value.toLowerCase() === identity.accountId.toLowerCase());
    const account = db.accounts[key];
    const character = account?.characters.find(row => row.name === identity.name);
    assert(character, "private fixture character is available");
    return { index: character.index, level: character.level, position: account.saves[String(character.index)].position };
  }

  stage("isolated-loopback-servers");
  for (const port of proof.ports) await new Promise((ok, fail) => {
    const check = createTcpServer(); check.once("error", fail);
    check.listen({ host: "127.0.0.1", port, exclusive: true }, () => check.close(ok));
  });
  const localRealm = newPlayableState();
  await writeFile(join(privateRoot, "realm.private.json"), JSON.stringify(localRealm), { flag: "wx" });
  const { MIR2_PASSKEY_AUTH_SECRET, MIR2_PASSKEY_ALLOWED_ORIGINS, ...gatewayEnv } =
    playableGatewayEnvironment(localRealm, dataDir);
  gateway = spawnOwned(gatewayExe, [], gatewayCwd, { ...gatewayEnv, CRYSTAL_CLIENT_ROOT: clientAssets,
    MIR2_GATEWAY_WEB_ADDR: `127.0.0.1:${gatewayPort}`, MIR2_GATEWAY_TCP_ADDR: `127.0.0.1:${tcpPort}`,
    MIR2_ALLOWED_WEB_ORIGINS: `${gatewayOrigin},${origin}`, MIR2_GATEWAY_RUNTIME_TICK_MS: "300",
    MIR2_GATEWAY_RUNTIME_TICK_INPUT_WAKE_MS: "75", MIR2_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS: "15000",
    MIR2_SPECTATOR_ENABLED: "false", MIR2_SPECTATOR_RECORDING_ENABLED: "false" });
  await health(`${gatewayOrigin}/health`, gateway);
  if (true) {
    stage("server-creates-a-normal-newcomer-on-the-isolated-file-store");
    const { GatewayClient } = await import("file:///" + prior + "/apps/ai-player/src/gateway-client.mjs");
    class IsolatedGatewaySocket extends WebSocket {
      constructor(url) {
        assert.equal(url, ws);
        super(url, { headers: { Origin: gatewayOrigin } });
      }
    }
    const controller = new GatewayClient({ url: ws, realmId: localRealm.world.realmId,
      WebSocketImpl: IsolatedGatewaySocket, timeoutMs: 15000 });
    try {
      await controller.connect();
      const oldRoles = await controller.login(identity.accountId, identity.password);
      assert.equal(oldRoles.length, 1);
      const name = "SpawnSmoke";
      const created = await controller.createCharacter(name, { gender: process.env.R16_QA_GENDER === "female" ? "Female" : "Male", characterClass: "Warrior" });
      identity = { ...identity, name };
      const persisted = await diskRole();
      assert(Number.isSafeInteger(persisted.index) && persisted.level === 1);
      const eventIndex = created.index ?? created.Index;
      if (eventIndex !== undefined) assert.equal(Number(eventIndex), persisted.index);
      const initialWorld = await controller.startGame(persisted.index);
      assert.equal(mapName(initialWorld.mapFileName), "0");
      assert(sameTile(initialWorld.player, startTile), "new role starts at the genuine Crystal spawn");
      assert(await controller.logout());
      proof.newcomer = { map: "0", position: point(initialWorld.player), level: persisted.level, ordinaryLogout: true };
    } finally {
      await controller.close();
    }
  }
  const spare = null;
  proof.spare = spare;
  const initial = await diskRole();
  assert.deepEqual(initial.position, startTile);
  next = spawnOwned(process.execPath, [join(web, "node_modules/next/dist/bin/next"), "start", "--hostname", "127.0.0.1", "--port", String(nextPort)],
    web, { ...playableBaseEnvironment(), NODE_ENV: "production", CRYSTAL_CLIENT_ROOT: clientAssets,
      MIR2_NEXT_STANDALONE: "0", MIR2_MARKETING_ORIGIN: origin, MIR2_NEXT_DIST_DIR: ".next-entry-player-meta-r2", NEXT_TELEMETRY_DISABLED: "1",
      MIR2_RUNTIME_ENV: "test", MIR2_PRODUCTION_ASSET_CONFIG: "numeron-r16-web-assets.json",
      MIR2_ORIGINAL_ASSET_MANIFEST_MODE: "remote-release", MIR2_ORIGINAL_ASSET_REMOTE_RELEASE: production.remoteReleaseUrl,
      MIR2_ASSET_VERSION: production.version, MIR2_ASSET_OBJECT_PREFIX: production.objectPrefix,
      NEXT_PUBLIC_MIR2_ASSET_BASE_URL: assetRoot, MIR2_R2_PROXY_BASE: assetRoot, MIR2_ASSET_BASE_URL: assetRoot,
      NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: ws, MIR2_GATEWAY_HTTP_URL: gatewayOrigin,
      NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: "newcomer-v2", MIR2_DEPLOY_REVISION: expectedRevision });
  await health(`http://127.0.0.1:${nextPort}/game`, next);
  await new Promise((ok, fail) => { proxy.once("error", fail); proxy.listen(proxyPort, "127.0.0.1", ok); });
  assert.equal((await fetch(`${origin}/version`).then(response => response.json())).revision, expectedRevision);

  const { chromium } = createRequire("F:/mir2-builds/web-r16-20261104/tools/package.json")("playwright");
  const profile=join(privateRoot,'chrome-profile');assert(!existsSync(profile),'the first profile must be new');
  let badSocket=false,wireDecodeFailures=0;
  async function openBrowser(){const owned=await launchProfile({chromium,profile,language:browserLanguage});browser=owned.handle;page=owned.page;return profileProcesses(profile);}
  let runtimeFaultActive = process.env.R16_CONTROLLED_RUNTIME_FAULT === "1";
  const pinnedRuntime = JSON.parse(await readFile(join(web,"lib/generated/bevy_runtime_version_numeron_r16.json"),"utf8"));
  const pinnedBodies = new Map();
  for (const row of pinnedRuntime.files) {
    const bytes = await readFile(join(web,row.path));
    assert.equal(createHash("sha256").update(bytes).digest("hex"),row.sha256);
    pinnedBodies.set(row.path.slice("public/".length),{...row,bytes});
  }
  proof.controlledRuntimeDelivery={kind:"Playwright fixed-byte response; not a WAN cold-download result",version:pinnedRuntime.version,deliveries:[]};
  let runtimeDelayUsed=false, runtimeTransientFailures=2;
  async function observePage(){  page.setDefaultTimeout(18000);
  const faultPins={};for(const path of ['/original-ui/fonts/NotoSansTC.ttf','/original-ui/Prguse/827.png','/original-ui/Prguse/961.png']){const bytes=await readFile(join(web,'public',path.slice(1)));faultPins[path]={bytes:bytes.length,sha256:digest(bytes)};}
  await installPlayerProbe(page);
  await page.route("**/bevy-runtime/v/**",async route=>{
    const path=new URL(route.request().url()).pathname;
    const match=path.match(/\/bevy-runtime\/v\/bevy-0c553a754308e5b8\/(pkg-(?:webgpu|webgl2|webgl2-shared)\/mir2_bevy_runtime(?:_bg.wasm|.js))$/);
    if(!match)return route.continue();
    const pin=pinnedBodies.get("bevy-runtime/"+match[1]);assert(pin);
    const failed=path.endsWith("_bg.wasm")&&(runtimeFaultActive||runtimeTransientFailures>0);
    if(failed&&runtimeTransientFailures>0)runtimeTransientFailures--;
    const delayMs=!failed&&!runtimeDelayUsed&&path.endsWith("_bg.wasm")?25000:0; if(delayMs)await delay(delayMs);
    proof.controlledRuntimeDelivery.deliveries.push({path,at:at(),status:failed?503:200,bytes:failed?0:pin.bytes.length,sha256:failed?null:pin.sha256,controlledHeaderDelayMs:delayMs});
    await route.fulfill({status:failed?503:200,contentType:path.endsWith(".wasm")?"application/wasm":"application/javascript",headers:{"access-control-allow-origin":"*","cache-control":"public, max-age=31536000"},body:failed?Buffer.from(""):pin.bytes});if(!failed&&path.endsWith("_bg.wasm"))runtimeDelayUsed=true;
  });
  await page.addInitScript(()=>{window.__r16InputTrace=[];const sample=()=>{const read=name=>{try{return JSON.parse(window.__mir2BevyRuntime?.[name]?.()??'null');}catch{return null;}};let movement=null;try{movement=JSON.parse(document.querySelector('[data-movement-input-status]')?.getAttribute('data-movement-input-status')??'null');}catch{}const q=read('getMir2QuestUiStatus'),h=read('getMir2HudUiStatus');return {active:{tag:document.activeElement?.tagName,id:document.activeElement?.id},movement,quest:q&&{generation:q.generation,revision:q.revision,open:q.questLogOpen,ready:q.ready,capturesPointer:q.capturesPointer},hud:h&&{generation:h.generation,revision:h.revision,ready:h.ready,navigation:h.navigation}};};for(const type of ['keydown','keyup'])window.addEventListener(type,event=>{if(!['q','Q','ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key))return;const row={type,key:event.key,repeat:event.repeat,trusted:event.isTrusted,at:Date.now(),target:{tag:event.target?.tagName,id:event.target?.id},before:sample()};window.__r16InputTrace.push(row);queueMicrotask(()=>{row.prevented=event.defaultPrevented;row.after=sample();});},true);});
  proof.runtimeMessages = [];
  proof.runtimeNetwork=[];
  const completeRuntimeRequests=new Map();
  page.on('request',request=>{
    const path=new URL(request.url()).pathname;
    if(!path.includes('/bevy-runtime/v/'+expectedRenderer+'/'))return;
    const match=path.match(/\/bevy-runtime\/v\/bevy-0c553a754308e5b8\/(pkg-(?:webgpu|webgl2|webgl2-shared)\/mir2_bevy_runtime(?:_bg.wasm|.js))$/);
    if(!match)return;
    const pin=pinnedBodies.get('bevy-runtime/'+match[1]);assert(pin);
    const row={path,requestedAt:at(),observer:'playwright-complete-body',expectedBytes:pin.bytes.length,expectedSha256:pin.sha256};
    completeRuntimeRequests.set(request,row);proof.runtimeNetwork.push(row);
  });
  page.on('response',response=>{const row=completeRuntimeRequests.get(response.request());if(row)Object.assign(row,{status:response.status(),headersAt:at(),serviceWorker:response.fromServiceWorker()});});
  page.on('requestfinished',async request=>{
    const row=completeRuntimeRequests.get(request);if(!row)return;
    try{const response=await request.response(),body=await response.body();Object.assign(row,{decodedBytes:body.length,bodySha256:digest(body),finishedAt:at()});}
    catch{row.observationFailure='RESPONSE_BODY_OBSERVATION_FAILED';}
  });
  page.on('requestfailed',request=>{const row=completeRuntimeRequests.get(request);if(row)Object.assign(row,{failedAt:at(),failure:request.failure()?.errorText});});
  const runtimeCdp=await page.context().newCDPSession(page);
  await runtimeCdp.send('Network.enable',{maxTotalBufferSize:192*1024*1024,maxResourceBufferSize:64*1024*1024});
  const runtimeReads=new Map();
  runtimeCdp.on('Network.requestWillBeSent',event=>{
    const url=new URL(event.request.url);
    if(!url.pathname.includes('/bevy-runtime/v/')&&!url.pathname.includes('/original-ui/fonts/'))return;
    const row={path:url.pathname,requestedAt:at(),dataEvents:0,decodedBytes:0,encodedDataBytes:0,maxDataGapMs:0};
    runtimeReads.set(event.requestId,row);proof.runtimeNetwork.push(row);
  });
  runtimeCdp.on('Network.responseReceived',event=>{
    const row=runtimeReads.get(event.requestId);if(!row)return;
    Object.assign(row,{headersAt:at(),status:event.response.status,protocol:event.response.protocol,serviceWorker:event.response.fromServiceWorker,fromDiskCache:event.response.fromDiskCache??false,cacheControl:event.response.headers["cache-control"]??event.response.headers["Cache-Control"]??null});
  });
  runtimeCdp.on('Network.dataReceived',event=>{
    const row=runtimeReads.get(event.requestId);if(!row)return;
    const now=at();row.firstDataAt??=now;
    row.maxDataGapMs=Math.max(row.maxDataGapMs,now-(row.lastDataAt??row.headersAt??row.requestedAt));
    row.lastDataAt=now;row.dataEvents++;row.decodedBytes+=event.dataLength;row.encodedDataBytes+=event.encodedDataLength;
  });
  runtimeCdp.on('Network.requestServedFromCache',event=>{const row=runtimeReads.get(event.requestId);if(row)row.cacheEvent=true;});
  runtimeCdp.on('Network.loadingFinished',async event=>{
    const row=runtimeReads.get(event.requestId);if(!row)return;
    Object.assign(row,{finishedAt:at(),totalEncodedBytes:event.encodedDataLength});
    const match=row.path.match(/\/bevy-runtime\/v\/bevy-0c553a754308e5b8\/(pkg-(?:webgpu|webgl2|webgl2-shared)\/mir2_bevy_runtime(?:_bg.wasm|.js))$/);
    if(!match)return;
    const pin=pinnedBodies.get('bevy-runtime/'+match[1]);assert(pin);
    try{const body=await runtimeCdp.send('Network.getResponseBody',{requestId:event.requestId});
      const bytes=Buffer.from(body.body,body.base64Encoded?'base64':'utf8');
      Object.assign(row,{completeBodyBytes:bytes.length,bodySha256:digest(bytes),bodyObservedAt:at(),expectedBytes:pin.bytes.length,expectedSha256:pin.sha256});
    }catch(error){row.bodyObservationFailure=String(error.message).replace(/https?:[^\s]+/g,'[url]').slice(0,220);}
  });
  runtimeCdp.on('Network.loadingFailed',event=>{
    const row=runtimeReads.get(event.requestId);if(row)Object.assign(row,{failedAt:at(),failure:event.errorText,canceled:event.canceled});
  });
  page.on("console", message => {
    const text = message.text();
    if (["warning", "error"].includes(message.type()) && /Bevy|wgpu|wasm|native|asset|font|HUD|Adapter|WebGL/i.test(text))
      proof.runtimeMessages.push({ type: message.type(), text: text.replace(/https?:\/\/[^\s)]+/g, "[url]").slice(0, 360), at: at() });
  });
  page.on("pageerror", error => proof.runtimeMessages.push({ type: "pageerror", text: String(error.message).replace(/https?:\/\/[^\s)]+/g, "[url]").slice(0, 360), at: at() }));
  const browserRequests = new Map();
  page.on("request", request => {
    const classified = classify(request.url());
    if (!classified) return;
    const entry = { ...classified, requestedAt: at() };
    browserRequests.set(request, entry); proof.browserRequests.push(entry);
  });
  page.on("response", response => {
    const entry = browserRequests.get(response.request());
    if (entry) Object.assign(entry, { status: response.status(), headersAt: at(), serviceWorker: response.fromServiceWorker() });
  });
  page.on("requestfinished", request => {
    const entry = browserRequests.get(request);
    if (entry) entry.bodyCompleteAt = at();
  });
  page.on("requestfailed", request => {
    const entry = browserRequests.get(request);
    const code = request.failure()?.errorText ?? "";
    if (entry) Object.assign(entry, { failureAt: at(), failure: /^net::ERR_[A-Z0-9_]+$/.test(code) ? code : "OTHER" });
  });
  badSocket = false;
  wireDecodeFailures = 0;
  activeCatalog={declared:false,binaryFrames:0,wireBytes:0,decodedBytes:0,packets:{}};catalogSessions.push(activeCatalog);
  page.on("websocket", socket => {
    if (socket.url() !== ws) { badSocket = true; return; }
    socket.on("framesent", ({ payload }) => {
      try {
        const row = parseGatewayJson(String(payload));
        if(row.type==="clientCapabilities"&&row.capabilities?.includes("serverCatalogGzipV1"))activeCatalog.declared=true;
        if (["startGame", "walk", "run", "logOut", "interact", "selectNpcDialog", "acceptQuest", "finishQuest", "shareQuest", "abandonQuest"].includes(row.type)) {
          sent.push({ type: row.type, direction: row.direction, target: row.target, seq: ++seq, at: at() });
        }
      } catch { wireDecodeFailures++; }
    });
    socket.on("framereceived", ({ payload }) => {
      try {
        if(Buffer.isBuffer(payload)){
          const texts=JSON.parse(catalogWasm.decode_gateway_catalog(payload));
          activeCatalog.binaryFrames++;activeCatalog.wireBytes+=payload.length;
          for(const text of texts){const row=JSON.parse(text);assert(["NewItemInfo","NewRecipeInfo","NewQuestInfo","GameShopInfo"].includes(row.packet));activeCatalog.decodedBytes+=Buffer.byteLength(text);activeCatalog.packets[row.packet]=(activeCatalog.packets[row.packet]??0)+1;}
          return;
        }

        const row = parseGatewayJson(String(payload));
        if (row.type === "worldSnapshot") {
          const { playerObjectId, mapFileName, entities, questLog } = row.payload;
          const relevantQuests = questLog ?? [];
          frames.push({ type: row.type, payload: { playerObjectId, mapFileName, entities, questLog: relevantQuests }, seq: ++seq, at: at() });
        } else if (["LoginSuccess", "StartGame", "UserLocation", "LogOutSuccess", "CompleteQuest"].includes(row.packet)) {
          frames.push({ ...row, seq: ++seq, at: at() });
        }
      } catch { wireDecodeFailures++; }
    });
  });

}
  const firstPids=await openBrowser();assert(firstPids.length>0,'owned Chrome process must be observed');
  await observePage();
  stage("ordinary-password-login-and-configured-service-worker");
  proof.navigationAt = at();
  await page.goto(origin + gamePath, { waitUntil: "domcontentloaded", timeout: 75000 });
  await page.locator(".login-input.account:enabled").fill(identity.accountId, { timeout: 65000 });
  await page.locator(".login-input.password").fill(identity.password);
  // The full TC stylesheet font starts during LoginSuccess, before Select.
  // Pin the real Worker's test mirror before that request, including any later
  // normal manifest reconfiguration. Never prefill a cache or replace font bytes.
  await page.waitForFunction(()=>navigator.serviceWorker.controller,null,{timeout:15000});
  const earlyWorker=page.context().serviceWorkers().find(w=>w.url().includes("mir2-asset-worker.js"));assert(earlyWorker);
  await poll(()=>earlyWorker.evaluate(()=>runtimeConfig.assetVersion),value=>value==='20261108-r16-08ec86d5105c','normal current-release Worker configuration',15000);
  await earlyWorker.evaluate((fault)=>{
    const mirror='http://localhost:20631/mir2/v/20261108-r16-08ec86d5105c';
    const bad='http://localhost:20631/stalled-font/mir2/v/20261108-r16-08ec86d5105c'; const apply=()=>{runtimeConfig.remoteAssetBaseUrls=[mirror,'http://localhost:20631/mir2/v/older-release'];};
    self.addEventListener('message',event=>{if(event.data?.type==='MIR2_ASSET_CACHE_CONFIG')apply();});apply();
  },process.env.R16_FONT_MIRROR_FAULT==='1');
  proof.controlledFontDelivery.configuredBeforeLogin=true;
  await page.locator(".login-button.ok button").click();
  const login = await receipt("LoginSuccess");
  assert.equal(login.payload.characters.find(row => row.name === identity.name)?.index, initial.index);
  await page.locator(".select-overlay").waitFor();
  const selectReadyAt = at();
  await page.locator(".select-character-slot-card").filter({ hasText: identity.name }).click();
  await page.waitForFunction(()=>navigator.serviceWorker.controller,null,{timeout:15000});
  const configuredWorker=page.context().serviceWorkers().find(w=>w.url().includes("mir2-asset-worker.js"));assert(configuredWorker);
  assert(await configuredWorker.evaluate(()=>runtimeConfig.remoteAssetBaseUrls.includes("http://localhost:20631/mir2/v/20261108-r16-08ec86d5105c")));
  const startClickAt = at();
  assert(!proof.browserRequests.some(row => row.kind?.startsWith("monster")), "no monster requests before selecting Start");
  await page.locator(".select-action.start button").click();
  proof.fastSelection = { loginAt: login.at, selectReadyAt, startClickAt };
  const start = await receipt("StartGame", login.seq, row => row.result === 4, 42000);
  const world = await wait(() => frames.find(row => row.type === "worldSnapshot" && row.seq > start.seq &&
    mapName(row.payload.mapFileName) === "0" && sameTile(own(row.payload), startTile)), "ordinary own map bootstrap", 23000);
  proof.startGame = { acknowledgedAt: start.at, bootstrapAt: world.at, position: startTile, map: "0" };

  stage("observe-default-shared-runtime-after-first-playable");
  const earlyScene=await poll(sceneState, row => row.visible && row.hasPlayer && row.hasRegion && row.ready && row.pending === false && Number.isFinite(row.marks.find(mark=>mark.name==="firstPlayableFrame")?.epochMs), "ordinary first playable", 60000);
  const firstPlayableAt=earlyScene.marks.find(row=>row.name==='firstPlayableFrame')?.epochMs;
  assert(Number.isFinite(firstPlayableAt)&&firstPlayableAt<=at()&&at()-firstPlayableAt<3000,'retain the real first-playable epoch');
  proof.firstPlayable={epochMs:firstPlayableAt,observedAt:at()};
  stage('two-early-physical-walks-before-native-HUD');
  const earlyRows=[];
  for(const [key,direction,to]of [['ArrowRight','Right',{x:289,y:616}],['ArrowLeft','Left',{x:288,y:616}]]){
    const nativeBefore=await page.evaluate(()=>{try{return JSON.parse(window.__mir2BevyRuntime?.getMir2HudUiStatus?.()??'null')?.ready===true;}catch{return false;}});
    assert.equal(nativeBefore,false,'the controlled cold binary must still be pending during the early input check');
    const before=seq,inputAt=at();await page.locator('.client-stage-frame').focus();
    let wire;
    await page.keyboard.down(key);
    try{wire=await wait(()=>sent.find(row=>row.seq>before&&row.type==='walk'&&row.direction===direction),'early physical walk emitted',1500);}
    finally{await page.keyboard.up(key);}
    const ack=await receipt('UserLocation',wire.seq,p=>sameTile(location(p),to),8000);
    assert(ack.at-wire.at<=8000,'same original first-walk ACK gate');
    const remaining=8000-(at()-inputAt);assert(remaining>0);
    const rendered=await poll(()=>page.evaluate(()=>{const el=document.querySelector('.client-stage-frame');return{x:Number(el?.dataset.viewportPlayerX),y:Number(el?.dataset.viewportPlayerY)};}),p=>sameTile(p,to),'early committed rendered owner position',remaining,20);
    const commands=sent.filter(row=>row.seq>before&&['walk','run'].includes(row.type));assert.equal(commands.length,1);assert.equal(commands[0].type,'walk');
    earlyRows.push({key,direction,to,nativeHudReadyBeforeInput:nativeBefore,inputAt,wireAt:wire.at,ackAt:ack.at,renderedAt:at(),ackMs:ack.at-wire.at,inputToRenderedMs:at()-inputAt,rendered});
  }
  proof.earlyWalks={passed:true,firstPlayableAt,rows:earlyRows,returnedToSpawn:true};
  proof.earlyPlayer.browser=await verifyEarlyPlayer(page,run,proof.earlyPlayer,firstPlayableAt,proof.startGame.acknowledgedAt);
  await page.locator('.client-stage-frame').screenshot({path:join(run,'actual-loose-player-first-playable.png')});
  releasePlayerReads();
  await page.waitForFunction(()=>{
   const first=(window.__mir2CacheMetrics?.milestones??window.__mir2PendingCacheMilestones??[]).find(row=>row.name==='firstPlayableFrame');
   const stack=document.querySelector('.entity-sprite-stack[data-object-id="'+CSS.escape(String(first?.detail?.playerObjectId))+'"]');
   const body=stack?.querySelector('img.entity-sprite-layer.body');return body?.currentSrc?.startsWith('data:image/png;base64,')&&body.complete&&body.naturalWidth>0;
  },null,{timeout:10000});
  proof.earlyPlayer.fullPackUpgrade={observedAt:at(),actualDecodedDataImage:true};


  await writeFile(join(run,'early-walks-before-native.json'),JSON.stringify(proof.earlyWalks,null,2)+'\n',{flag:'wx'});
  if(runtimeFaultActive){
    stage("controlled-first-binary-503-before-boot");
    await page.waitForFunction(()=>window.__mir2BevyRuntimeFailed===true,null,{timeout:15000});
    const flags=await page.evaluate(()=>({booted:window.__mir2BevyRuntimeBooted===true,attempted:window.__mir2BevyRuntimeBootAttempted===true,failed:window.__mir2BevyRuntimeFailed===true,retryable:window.__mir2BevyRuntimeRetryable===true}));
    assert(flags.failed&&flags.retryable&&!flags.booted&&!flags.attempted);
    assert(proof.controlledRuntimeDelivery.deliveries.some(row=>row.status===503));
    runtimeFaultActive=false;
    const retryBefore=seq;await requestOrdinaryLogout();
    const retryLogout=await wait(()=>sent.find(row=>row.seq>retryBefore&&row.type==="logOut"),"controlled-retry ordinary logout",7000);
    const retryAck=await receipt("LogOutSuccess",retryLogout.seq);await page.locator(".select-overlay").waitFor();
    await page.locator(".select-character-slot-card").filter({hasText:identity.name}).click();
    await page.locator(".select-action.start button").click();
    const retryStart=await receipt("StartGame",retryAck.seq,row=>row.result===4,42000);
    await poll(sceneState,row=>row.visible&&row.hasPlayer&&row.hasRegion&&row.ready&&row.pending===false,"controlled-retry first playable",60000);
    proof.controlledRecovery={kind:"same document, same default backend, ordinary logout/reentry",flagsBefore:flags,logoutAt:retryLogout.at,logoutAckAt:retryAck.at,reentryAt:retryStart.at,gateMs:120000};
  }
  try {
    proof.interaction = await verifySceneState({ page, run, sent, frames, stage, proxyRequests: proof.proxyRequests, runtimeNetwork: proof.runtimeNetwork, browserRequests: proof.browserRequests, runtimeRetry: Boolean(proof.controlledRecovery),firstPlayableAt });
  } catch (error) {
    if(error.code!=='NATIVE_READINESS_EXCEEDED_120_SECONDS'&&!String(error.message).includes('Timeout 120000ms exceeded'))throw error;
    proof.coldFailure={stage:proof.stages.at(-1)?.name,code:'NATIVE_READINESS_EXCEEDED_120_SECONDS'};
    stage('preserve-failed-cold-gate-and-observe-recovery');
    await page.waitForFunction(()=>window.__mir2BevyRuntimeBooted||window.__mir2BevyRuntimeFailed,null,{timeout:5000});
    const flags=await page.evaluate(()=>({booted:window.__mir2BevyRuntimeBooted===true,attempted:window.__mir2BevyRuntimeBootAttempted===true,failed:window.__mir2BevyRuntimeFailed===true,retryable:window.__mir2BevyRuntimeRetryable===true}));
    proof.recovery={flagsBefore:flags,passed:false,extraObservationBudgetMs:180000,kind:flags.booted?'observe-existing-app':'normal-logout-and-reenter'};
    if(!flags.booted){
      assert(flags.failed&&flags.retryable&&!flags.attempted,'only a download failure before boot may re-enter');
      stage('ordinary-logout-and-reenter-after-download-failure');
      const retryBefore=seq;await requestOrdinaryLogout();
      const retryLogout=await wait(()=>sent.find(row=>row.seq>retryBefore&&row.type==='logOut'),'retry logout send',7000);
      const retryAck=await receipt('LogOutSuccess',retryLogout.seq);await page.locator('.select-overlay').waitFor();
      await page.locator('.select-character-slot-card').filter({hasText:identity.name}).click();
      await page.locator('.select-action.start button').click();
      const retryStart=await receipt('StartGame',retryAck.seq,row=>row.result===4,42000);
      await poll(sceneState,row=>row.visible&&row.hasPlayer&&row.hasRegion&&row.ready&&row.pending===false,'reentry first playable',60000);
      proof.recovery.logout={sentAt:retryLogout.at,acknowledgedAt:retryAck.at};proof.recovery.startedAt=retryStart.at;
      await page.waitForFunction(()=>window.__mir2BevyRuntimeFailed!==true,null,{timeout:10000});
      const after=await page.evaluate(()=>({failed:window.__mir2BevyRuntimeFailed===true,retryable:window.__mir2BevyRuntimeRetryable===true,attempted:window.__mir2BevyRuntimeBootAttempted===true}));
      assert.equal(after.failed,false,'ordinary reentry clears a retryable failure without reloading');proof.recovery.flagsAfterReentry=after;
    }
    proof.interaction = await verifySceneState({ page, run, sent, frames, stage, proxyRequests: proof.proxyRequests, runtimeNetwork: proof.runtimeNetwork, browserRequests: proof.browserRequests, recovery: true });
    proof.recovery.passed=proof.interaction.passed;
  }

  stage('stop-owned-asset-worker-with-game-running');
  const lifecycleCdp=await page.context().newCDPSession(page),workerVersions=[];
  lifecycleCdp.on('ServiceWorker.workerVersionUpdated',event=>workerVersions.push(...event.versions.map(row=>({versionId:row.versionId,runningStatus:row.runningStatus,status:row.status}))));
  await lifecycleCdp.send('ServiceWorker.enable');
  const framePath='/original-ui/Prguse/1.png',expectedFrame=await sha(join(web,'public',framePath.slice(1)));
  const beforeReads=proof.proxyRequests.filter(row=>row.path===framePath).length;
  await lifecycleCdp.send('ServiceWorker.stopAllWorkers');
  await wait(()=>workerVersions.some(row=>row.runningStatus==='stopped'),'owned worker actually stopped',5000);
  const restored=await page.evaluate(async path=>{const response=await fetch(path);const bytes=await response.arrayBuffer();return{status:response.status,bytes:bytes.byteLength,sha256:[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('')};},framePath);
  assert.equal(restored.status,200);assert.equal(restored.sha256,expectedFrame);
  assert.equal(proof.proxyRequests.filter(row=>row.path===framePath).length,beforeReads,'a restarted Worker must read the existing HUD cache without a new original-origin request');
  const stoppedIndex=workerVersions.findIndex(row=>row.runningStatus==='stopped');
  await wait(()=>workerVersions.slice(stoppedIndex+1).some(row=>row.runningStatus==='running'),'owned worker actually restarted',5000);
  proof.workerRestart={passed:true,originalFrame:framePath,expectedSha256:expectedFrame,restored,originNetworkReads:0,lifecycle:workerVersions};
  await writeFile(join(run,'worker-restart.json'),JSON.stringify(proof.workerRestart,null,2)+'\n',{flag:'wx'});
  await page.locator('.client-stage-frame').screenshot({path:join(run,'native-hud-after-worker-restart.png')});
  await lifecycleCdp.detach();
  const finalTile = proof.interaction.finalTile;
  stage("ordinary-logout-and-private-file-readback");
  const before = seq;
  const gutter = await requestOrdinaryLogout();
  const logout = await wait(() => sent.find(row => row.seq > before && row.type === "logOut"), "ordinary logout send", 7000);
  const loggedOut = await receipt("LogOutSuccess", logout.seq);
  await page.locator(".select-overlay").waitFor();
  const saved = await poll(diskRole, row => sameTile(row.position, finalTile), "private File save matches accepted movement", 8000);
  proof.logout = { sentAt: logout.at, acknowledgedAt: loggedOut.at, savedPosition: saved.position, returnedToSelect: true, gutter };
  proof.firstSave = await verifySave({ source, saved: store.accounts, identity: fixtureIdentity, className });
  assert(!badSocket);assert.equal(wireDecodeFailures,0);
  proof.firstSession={passed:!proof.coldFailure&&proof.interaction.passed&&proof.firstSave.passed,coldFailure:proof.coldFailure??null,recovery:proof.recovery??null,
    interaction:proof.interaction,logout:proof.logout,firstSave:proof.firstSave,runtimeNetwork:proof.runtimeNetwork,browserRequests:proof.browserRequests,browserRequests:proof.browserRequests,
    startGame:proof.startGame,fastSelection:proof.fastSelection};
  stage('fully-close-owned-Chrome-before-reopening');
  await browser.close();assert.equal(browser.isConnected(),false);
  await poll(()=>profileProcesses(profile),rows=>rows.length===0,'all owned profile processes must exit',20000,200);
  proof.browserRestart={firstProcessCount:firstPids.length,allFirstProcessesExited:true,persistedProfileExists:existsSync(profile)};
  assert(proof.browserRestart.persistedProfileExists);
  sent.length=0;frames.length=0;seq=0;proof.browserRequests=[];
  const secondPids=await openBrowser();assert(secondPids.length>0);proof.browserRestart.secondProcessCount=secondPids.length;
  await observePage();const warmNavigationAt=at();
  stage('normal-login-after-complete-browser-restart');
  await page.goto(origin+gamePath,{waitUntil:'domcontentloaded',timeout:75000});
  await poll(async()=>({select:await page.locator('.select-overlay').isVisible().catch(()=>false),login:await page.locator('.login-input.account:enabled').isVisible().catch(()=>false)}),row=>row.select||row.login,'normal restored-login or password form',65000);
  let loginKind='restored-service-session';
  if(!await page.locator('.select-overlay').isVisible()){
    loginKind='normal-password';await page.locator('.login-input.account:enabled').fill(identity.accountId);await page.locator('.login-input.password').fill(identity.password);await page.locator('.login-button.ok button').click();
  }
  const warmLogin=await receipt('LoginSuccess');await page.locator('.select-overlay').waitFor();
  assert.equal(warmLogin.payload.characters.find(row=>row.name===identity.name)?.index,initial.index);
  await page.locator('.select-character-slot-card').filter({hasText:identity.name}).click();
  const preStartCache=await fontCacheInventory(page);assert(preStartCache.rows.every(row=>row.present),'all six fonts must exist before warm Start without manual prefill');
  const warmStartTile=point(saved.position);assert.deepEqual(warmStartTile,{x:289,y:616});
  const warmWorker=page.context().serviceWorkers().find(w=>w.url().includes("mir2-asset-worker.js"));assert(warmWorker);
  await warmWorker.evaluate(()=>{runtimeConfig.remoteAssetBaseUrls=["http://localhost:20631/mir2/v/20261108-r16-08ec86d5105c",...runtimeConfig.remoteAssetBaseUrls];});
  const warmStartClickAt=at();await page.locator('.select-action.start button').click();
  const warmStart=await receipt('StartGame',warmLogin.seq,row=>row.result===4,42000);
  const warmWorld=await wait(()=>frames.find(row=>row.type==='worldSnapshot'&&row.seq>warmStart.seq&&mapName(row.payload.mapFileName)==='0'&&sameTile(own(row.payload),warmStartTile)),'same saved role authoritative reentry',23000);
  await poll(sceneState,row=>row.visible&&row.hasPlayer&&row.hasRegion&&row.ready&&row.pending===false,'warm first playable',60000);
  const warmRun=join(run,'warm');await mkdir(warmRun);
  const warmInteraction=await verifySceneState({page,run:warmRun,sent,frames,stage,proxyRequests:proof.proxyRequests,runtimeNetwork:proof.runtimeNetwork,browserRequests:proof.browserRequests,warm:true,phaseStartAt:warmNavigationAt,startPosition:warmStartTile,targetPosition:{x:288,y:616},movementKey:'ArrowLeft'});
  stage('ordinary-second-logout-and-save');
  const warmBefore=seq;const warmGutter=await requestOrdinaryLogout();
  const warmLogout=await wait(()=>sent.find(row=>row.seq>warmBefore&&row.type==='logOut'),'warm ordinary logout send',7000);
  const warmLogoutAck=await receipt('LogOutSuccess',warmLogout.seq);await page.locator('.select-overlay').waitFor();
  const warmSaved=await poll(diskRole,row=>sameTile(row.position,warmInteraction.finalTile),'warm authoritative File save',8000);
  const secondSave=await verifySave({source,saved:store.accounts,identity:fixtureIdentity,className});
  assert(!badSocket);assert.equal(wireDecodeFailures,0);
  const runtimeTransfers=proof.runtimeNetwork.filter(row=>row.path.includes('/bevy-runtime/v/')).map(row=>({path:row.path,fromDiskCache:row.fromDiskCache??false,cacheEvent:row.cacheEvent??false,encodedBytes:row.totalEncodedBytes??null,decodedBytes:row.decodedBytes}));
  proof.warmSession={passed:warmInteraction.passed&&secondSave.passed,loginKind,navigationAt:warmNavigationAt,startClickAt:warmStartClickAt,startAcknowledgedAt:warmStart.at,
    authoritativeStart:{position:point(own(warmWorld.payload)),characterIndex:initial.index},preStartCache,interaction:warmInteraction,runtimeTransfers,runtimeNetwork:proof.runtimeNetwork,browserRequests:proof.browserRequests,
    logout:{sentAt:warmLogout.at,acknowledgedAt:warmLogoutAck.at,savedPosition:warmSaved.position,returnedToSelect:true,gutter:warmGutter},saved:secondSave.passed};
  assert.equal(catalogSessions.length,2);
  assert(catalogSessions.every(s=>s.declared&&s.binaryFrames>0&&s.wireBytes<s.decodedBytes&&["NewItemInfo","NewRecipeInfo","NewQuestInfo","GameShopInfo"].every(p=>s.packets[p]>0)));
  proof.catalogTransport={passed:true,sessions:catalogSessions};
  const binaryDeliveries=proof.controlledRuntimeDelivery.deliveries.filter(row=>row.path.endsWith("_bg.wasm"));
  assert.equal(runtimeTransientFailures,0);
  assert.deepEqual(binaryDeliveries.slice(0,3).map(row=>row.status),[503,503,200]);
  assert(binaryDeliveries[2].bytes>0&&binaryDeliveries[2].sha256);
  proof.transientRuntimeRecovery={passed:true,initialFailures:2,firstSuccessfulAttempt:3,
    originalHeaderDelayMs:25000,successfulBytes:binaryDeliveries[2].bytes,successfulSha256:binaryDeliveries[2].sha256,
    scope:"fixed-byte two503 fault recovery, not WAN performance; original8s walk/120s Native gates unchanged"};
  assert(proof.earlyPlayer.reads.some(row=>row.kind==='complete-player-pack'&&row.finishedAt&&row.finishedAt>=proof.earlyPlayer.releasedAt));
  proof.passed=proof.firstSession.passed&&proof.warmSession.passed;
} catch (error) {
  try { proof.prewarmFailureSnapshot = await page.evaluate(()=>window.__mir2CacheMetrics?.snapshot().prewarmRuns.map(({name,status,requested,ok,failed,failedUrls,requestFailures})=>({name,status,requested,ok,failed,failedUrls,requestFailures})));const w=page.context().serviceWorkers()[0];if(w)proof.staticFailureSnapshot=await w.evaluate(()=>({at:Date.now(),config:{version:runtimeConfig.version,remoteAssetBaseUrls:runtimeConfig.remoteAssetBaseUrls},inflight:[...staticAssetInflightFetches.keys()].map(u=>new URL(u).pathname),resources:performance.getEntriesByType('resource').filter(e=>/Prguse\/(1|4|6|8)\.png/.test(e.name)).map(e=>({path:new URL(e.name).pathname,start:e.startTime,duration:e.duration,responseStart:e.responseStart,responseEnd:e.responseEnd,bytes:e.decodedBodySize})),fontInflight:fontAssetInflightFetches.size})); } catch {}
  const forbidden = [identity?.accountId, identity?.password, identity?.name].filter(value => typeof value === "string" && value.length > 0);
  const message = forbidden.reduce((line, value) => line.replaceAll(value, "[filtered]"), String(error.message)).slice(0, 350);
  proof.failure = { stage: proof.stages.at(-1)?.name, message };
  if (page) {
    const state = await sceneState().catch(() => null);
    if (state) { if (state.first) delete state.first.owner; proof.failure.readiness = state; }
    proof.runtimeFinal=await page.evaluate(()=>{
      const runtime=window.__mir2BevyRuntime,debug=window.__mir2BevyRuntimeDebug;
      const read=fn=>{try{return JSON.parse(fn?.()??'null');}catch{return null;}};
      return{backend:debug?.selectedBackend,packageId:debug?.packageId,
        hud:read(runtime?.getMir2HudUiStatus?.bind(runtime)),quest:read(runtime?.getMir2QuestUiStatus?.bind(runtime)),
        failed:window.__mir2BevyRuntimeFailed===true,retryable:window.__mir2BevyRuntimeRetryable===true,attempted:window.__mir2BevyRuntimeBootAttempted===true,fonts:document.fonts.status};
    }).catch(()=>null);
    proof.fontWorkerFinal=[];
    for(const worker of page.context().serviceWorkers()){
      proof.fontWorkerFinal.push(await worker.evaluate(()=>({fontMirrorFirst:runtimeConfig.fontMirrorFirst,assetVersion:runtimeConfig.assetVersion,inflight:fontAssetInflightFetches.size,resources:performance.getEntriesByType('resource').filter(row=>row.name.includes('/original-ui/fonts/')).map(row=>({url:row.name,startMs:row.startTime,durationMs:row.duration,decodedBodySize:row.decodedBodySize}))})).catch(()=>null));
    }
  }
  console.log(JSON.stringify(proof.failure));
} finally {
  await browser?.close();
  if (proxy.listening) {
    proxy.closeAllConnections();
    await new Promise(done => proxy.close(done));
  }
  if (!stopped(next)) {
    next.kill();
    await wait(() => stopped(next), "owned Next stop", 8000).catch(() => {});
  }
  if (!stopped(gateway)) {
    gateway.stdin.end("shutdown\n");
    if (!await wait(() => stopped(gateway), "owned Gateway stop", 12000).catch(() => false)) gateway.kill();
    await wait(() => stopped(gateway), "owned Gateway exit", 5000).catch(() => {});
  }
  proof.closed = { browser: !browser?.isConnected(), proxy: !proxy.listening, next: stopped(next), gateway: stopped(gateway) };
  proof.fixtureUnchanged = await sha(source).catch(() => null) === sourceSha &&
    await sha(original).catch(() => null) === provision?.fixtureSourceSha;
  proof.passed = proof.passed && proof.fixtureUnchanged && Object.values(proof.closed).every(Boolean);
  if (!proof.passed) process.exitCode = 1;
  const output = join(run, "entry-visual-receipt.json");
  await writeFile(output, `${JSON.stringify(proof, null, 2)}\n`, { flag: "wx" });
  console.log(JSON.stringify({ passed: proof.passed, receipt: output, closed: proof.closed }));
}
