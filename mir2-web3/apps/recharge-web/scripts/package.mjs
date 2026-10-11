import {readFile, mkdir, writeFile} from "node:fs/promises";
import {createHash} from "node:crypto";
import {fileURLToPath} from "node:url";
import path from "node:path";
const output=process.argv[2];
if(!output || !path.isAbsolute(output)) throw new Error("Provide an absolute new output directory.");
const names=["index.html","styles.css","mark.svg","app.js","core.js","locales.js"];
const root=fileURLToPath(new URL("../public/",import.meta.url));
const manifest={schemaVersion:1,application:"numeron-recharge-web",files:[]};
await mkdir(output,{recursive:true});
for(const name of names){
  const bytes=await readFile(path.join(root,name));
  const sha256=createHash("sha256").update(bytes).digest("hex");
  await writeFile(path.join(output,name),bytes,{flag:"wx"});
  manifest.files.push({name,bytes:bytes.length,sha256});
}
const bytes=Buffer.from(JSON.stringify(manifest,null,2)+"\n");
await writeFile(path.join(output,"release.json"),bytes,{flag:"wx"});
process.stdout.write(JSON.stringify({ok:true,files:names.length,bytes:manifest.files.reduce((sum,file)=>sum+file.bytes,0),releaseSha256:createHash("sha256").update(bytes).digest("hex")})+"\n");
