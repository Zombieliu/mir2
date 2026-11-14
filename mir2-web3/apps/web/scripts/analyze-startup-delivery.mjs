import {readFile,writeFile} from "node:fs/promises";
import {createHash} from "node:crypto";
import {analyzeStartupDelivery} from "./lib/startup-delivery-analysis.mjs";

const [receiptPath, probePath, serverPath, outputPath, ...extra] = process.argv.slice(2);
if (!receiptPath || !probePath || !serverPath || !outputPath || extra.length) {
  throw new Error("Usage: analyze-startup-delivery.mjs receipt.json startup-probe.json server-readback.json|- output.json");
}
const paths = [receiptPath, probePath, ...(serverPath === "-" ? [] : [serverPath])];
const raw = await Promise.all(paths.map(path => readFile(path)));
const result = analyzeStartupDelivery(...raw.map(bytes => JSON.parse(bytes.toString("utf8"))));
result.inputs = raw.map((bytes, index) => ({name: ["receipt", "startupProbe", "serverReadback"][index],
  bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex")}));
result.producerSha256 = createHash("sha256").update(await readFile(new URL(import.meta.url))).digest("hex");
result.analyzerSha256 = createHash("sha256").update(await readFile(new URL("./lib/startup-delivery-analysis.mjs", import.meta.url))).digest("hex");
await writeFile(outputPath, JSON.stringify(result, null, 2) + "\n", {flag: "wx"});
console.log(JSON.stringify({written: true, durationMs: result.movement.durationMs,
  aggregateDelta: result.aggregateAdmission.countDelta, aggregateUs: result.aggregateAdmission.sumUsDelta,
  callbackMaxGapMs: result.browser.maxObservedCallbackGapMs, rootCauseProven: false}));
