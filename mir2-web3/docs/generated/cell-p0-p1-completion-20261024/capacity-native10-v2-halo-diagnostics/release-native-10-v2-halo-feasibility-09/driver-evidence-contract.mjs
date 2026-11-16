import fs from 'node:fs/promises';
const final='72005514809dd480a015ee5414b14f60fe6264f10e35a7b825a6dc5e35e85c82',prefinal='bcc53ee440624ef6831a591fb693226b7aade5a48ecbd6e502debec07d7f463b';
export async function verifyDriverEvidenceBasis(frozen,assertPin){
 const full=frozen.fullRegression,delta=frozen.finalCompatibilityDelta;
 if(full?.passed!==151||full.failed!==0||full.exactSourceSha256!==prefinal||full.finalSourceFullRegressionPerformed!==false||delta?.sourceBeforeSha256!==prefinal||delta.sourceAfterSha256!==final||delta.productionChangedLines!==1)throw Error('Final driver evidence must preserve the pre-final151 and explicit final compatibility delta');
 for(const [suite,count]of[[delta.finalFocused,7],[delta.finalAdjacent,25]])if(suite?.passed!==count||suite.failed!==0||suite.skipped!==0||suite.cancelled!==0||suite.exactSourceSha256!==final)throw Error('Final timing driver focused/adjacent evidence differs');
 const candidates=[full.log,full.receipt,full.exactSourceManifest,delta.proof,delta.preFinalSourceManifest,...delta.preFinalRawFiles.map(p=>p.preimage),delta.finalFocused.log,delta.finalFocused.receipt,delta.finalFocused.exactSourceManifest,delta.finalAdjacent.log,delta.finalAdjacent.receipt,delta.finalAdjacent.exactSourceManifest,delta.independentReview,frozen.independentReview];
 const pins=[...new Map(candidates.map(p=>[p.file,p])).values()];for(const p of pins)await assertPin(p);
 for(const [entry,aggregate]of[[full.exactSourceManifest,prefinal],[delta.finalFocused.exactSourceManifest,final],[delta.finalAdjacent.exactSourceManifest,final]]){const source=JSON.parse(await fs.readFile(entry.file,'utf8'));if(source.sourceSha256!==aggregate||source.files?.length!==9)throw Error('Regression source manifest differs from its declared exact source');}
 return{pins,scope:'151 full tests on pre-final bcc53; final7200 verified by the explicit one-line compatibility delta and final7 focused plus25 adjacent. No full regression on final7200 is claimed.',fullTests:151,fullRegressionSourceSha256:prefinal,finalSourceFullRegressionPerformed:false,finalFocused:7,finalAdjacent:25};
}
