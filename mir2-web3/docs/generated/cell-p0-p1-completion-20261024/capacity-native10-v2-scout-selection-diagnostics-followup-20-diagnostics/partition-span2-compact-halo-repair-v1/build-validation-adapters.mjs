import fs from 'node:fs';
import { ROOT as PRIOR, pin } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname,receipts=[];
const localWriter=`const ROOT=import.meta.dirname;\nconst writeNew=(file,value)=>{if(path.dirname(path.resolve(file))!==path.resolve(ROOT))throw new Error('Output must be an immediate child of dedicated repairdirectory');fs.writeFileSync(file,JSON.stringify(value,null,2)+'\\n',{flag:'wx'});return pin(file);};`;
for(const [from,to,extra]of[
  ['validate-candidate.mjs','validate-repaired.mjs',[['candidate-100-v1.json','candidate-100-repaired-v4.json'],['validation-v1.json','validation-repaired-v4.json']]],
  ['prefix-scout-cost-nearest-final-v3.mjs','prefix-scout-cost-repaired-v4.mjs',[['candidate-100-nearest-ordered-final-v3.json','candidate-100-repaired-v4.json'],['prefix-scout-cost-nearest-final-v3.json','prefix-scout-cost-repaired-v4.json']]],
  ['conditional-level-supply-nearest-final-v3.mjs','conditional-level-supply-repaired-v4.mjs',[['candidate-100-nearest-ordered-final-v3.json','candidate-100-repaired-v4.json'],['validation-nearest-ordered-final-v3.json','validation-repaired-v4.json'],['conditional-level-supply-nearest-final-v3.json','conditional-level-supply-repaired-v4.json'],['closed124canonical-slot','closed observed canonical-slot'],['recalculates124deduplicated','recalculates the deduplicated']]]
]){
  const original=fs.readFileSync(PRIOR+'/'+from,'utf8'),importLine=original.split('\n').find(x=>x.includes("from './static-lib.mjs'"));
  if(!importLine)throw new Error('Missing unique shared import');
  let adaptedImport=importLine.replace('ROOT, ','').replace('writeNew, ','').replace(', writeNew }',' }').replace("from './static-lib.mjs';","from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';");
  if(/\bROOT\b|\bwriteNew\b/.test(adaptedImport))throw new Error('Imported writer/root not fully relocated');
  const needsPath=!original.includes("from 'node:path'"),replacement=(needsPath?"import path from 'node:path';\n":'')+adaptedImport+'\n'+localWriter;
  const changes=[[importLine,replacement],...extra];let text=original;
  for(const[a,b]of changes){if(text.split(a).length!==2)throw new Error('Ambiguous adapter replacement '+a);text=text.replace(a,b);}
  let inverse=text;for(const[a,b]of[...changes].reverse()){if(inverse.split(b).length!==2)throw new Error('Ambiguous inverse');inverse=inverse.replace(b,a);}
  if(inverse!==original)throw new Error('Algorithm body changed');
  const file=ROOT+'/'+to;fs.writeFileSync(file,text,{flag:'wx'});
  receipts.push({source:pin(PRIOR+'/'+from),adapter:pin(file),changes,inverseRawExact:true,algorithmBodyUnchanged:true,
    writerRelocation:'Same JSONLF/wx/absolute-parent output guard andpin semantics, dedicated new repairdirectory instead of sealedprior root.'});
}
const out=ROOT+'/validation-adapter-provenance-v2.json';fs.writeFileSync(out,JSON.stringify({schemaVersion:1,scope:'Exact external path/input/output/writer relocation only; original core validator/navigation/geometry algorithms unchanged',receipts,helper:pin(import.meta.filename)},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({out:pin(out),adapters:receipts.map(r=>r.adapter)}));
