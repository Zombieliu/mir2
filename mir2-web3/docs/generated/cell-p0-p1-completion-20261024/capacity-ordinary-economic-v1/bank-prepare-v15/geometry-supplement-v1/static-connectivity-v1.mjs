import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {pathToFileURL,fileURLToPath} from 'node:url';
import path from 'node:path';
const root=path.dirname(fileURLToPath(import.meta.url));
const hash=b=>createHash('sha256').update(b).digest('hex');
const pin=async file=>{const b=await readFile(file);return{file,bytes:b.length,sha256:hash(b)};};
const inputFile=path.join(root,'geometry-result-v1.json'),inputPin=await pin(inputFile);
if(inputPin.bytes!==206696||inputPin.sha256!=='76788189f25be3227e5ee09f9136736dfbcb3444782d07d4e45419801fd3eea7')throw new Error('geometry input differs');
const input=JSON.parse(await readFile(inputFile,'utf8'));
const source=input.sourcePins.find(p=>p.file.replaceAll('\\','/').endsWith('/quest-agent/protocol-navigation.mjs'));
const current=await pin(source.file);if(current.bytes!==source.bytes||current.sha256!==source.sha256)throw new Error('navigation source differs');
const nav=await import(pathToFileURL(source.file));
const map=await nav.loadProtocolCollisionMap('0');
const mapPin=await pin(map.sourcePath),expected=input.sourcePins.find(p=>p.file===map.sourcePath);
if(mapPin.bytes!==expected.bytes||mapPin.sha256!==expected.sha256)throw new Error('collision data differs');
const distance=(p,q)=>Math.max(Math.abs(p.x-q.x),Math.abs(p.y-q.y));
function boundary(target,radius){const out=[];for(let y=target.y-radius-1;y<=target.y+radius+1;y++)for(let x=target.x-radius-1;x<=target.x+radius+1;x++)if(x>=0&&y>=0&&distance({x,y},target)===radius+1)out.push({x,y});return out;}
const cases=[];
for(const row of input.cases.filter(c=>c.actualResult==='missing')){
  const staticMode=row.projections.find(p=>p.label==='static-collision-and-authored-transfers');
  const pathAt=radius=>nav.planProtocolNavigation({map,start:row.start,target:row.target,desiredDistance:1,dynamicObstacles:[...row.transferCells,...boundary(row.target,radius)]});
  const global=staticMode.unconstrained,globalMax=global.maxTargetDistance;
  if(!global.found||!Number.isSafeInteger(globalMax)||globalMax>128)throw new Error('unexpected bounded static global route');
  const radiusResults=[];let first=null;
  for(let radius=16;radius<=globalMax;radius++){
    const route=pathAt(radius);radiusResults.push({radius,found:route!==null,steps:route?.steps.length??null});
    if(route){first={radius,steps:route.steps.length,path:route.path,maxTargetDistance:Math.max(...route.path.map(p=>distance(p,row.target)))};break;}
  }
  if(!first||first.radius<=16)throw new Error('static AOI counterexample changed');
  cases.push({objectId:row.objectId,start:row.start,target:row.target,range:1,staticGlobalSteps:global.steps,staticGlobalMaxTargetDistance:globalMax,
    originalActualLastDistance:row.actualLastDistance,checkedRadii:radiusResults,minimumStaticRouteRadius:first.radius,firstFeasibleStaticRoute:first,
    interpretation:'With the pinned closed-door static map and all authored transfers blocked, no range1 route exists inside target AOI16. A larger-radius static route exists; it is not authorized movement and not live safety evidence.'});
}
const out=path.join(root,'static-route-proof-v1.json');
const result={schemaVersion:1,createdAt:new Date().toISOString(),offlineOnly:true,commandsSent:0,productionSourceModified:false,
  inputPin,sourcePin:current,mapPin,scriptPin:await pin(fileURLToPath(import.meta.url)),cases,
  conclusion:{aoi16BoundaryAloneIsNotASolution:true,allFourStaticTargetNeighborhoodsDisconnectedAt16:true,
    candidateQueryNeedsStaticLocalFeasibilityRatherThanRefundedRetries:true,
    candidateQueryMayRejectOnlyAndMustNotGrantHealthOrCombatReadiness:true,
    dynamicAndHistoricalPlannerEqualityStillRequiredForClaimsBeyondStaticImpossibility:true},
  limitations:['No earlier complete dynamic planning view exists for these four attempts.',
    'Static impossibility applies to the same closed-door collision model used by the ordinary navigator; no unobserved live door overrides are invented.',
    'Larger-radius routes are explanatory geometry only. Following them would violate the present selected-target visibility guard and is not proposed.']};
await writeFile(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output:await pin(out),script:result.scriptPin,summary:cases.map(c=>({id:c.objectId,minimumStaticRadius:c.minimumStaticRouteRadius,globalSteps:c.staticGlobalSteps,firstFeasibleSteps:c.firstFeasibleStaticRoute.steps}))},null,2));
