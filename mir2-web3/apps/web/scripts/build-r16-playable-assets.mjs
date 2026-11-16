import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { collectR16PlayableAssets, materializeR16PlayableAssetFiles } from "./lib/r16-playable-assets.mjs";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  const name = process.argv[i], value = process.argv[i + 1];
  if (!["--output", "--publicRoot", "--soundRoot", "--uiScope"].includes(name) || !value || args[name]
    || (name === "--uiScope" ? !["r16", "common"].includes(value) : !path.isAbsolute(value)))
    throw new Error("Use --output <new absolute JSON> [--publicRoot <absolute public root>] [--uiScope r16|common] [--soundRoot <absolute raw Sound root>].");
  args[name] = value;
}
if (!args["--output"]) throw new Error("An explicit external --output path is required.");
const output = path.resolve(args["--output"]), relative = path.relative(path.resolve(webRoot, "../.."), output);
if (!(relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) || output === path.parse(output).root)
  throw new Error("Write the immutable manifest outside the repository.");
const publicRoot = args["--publicRoot"] ?? path.join(webRoot, "public");
const sourceOptions = { soundRoot: args["--soundRoot"] ?? "" };
const { manifest, diagnostics } = await collectR16PlayableAssets({ webRoot, publicRoot, uiScope: args["--uiScope"] ?? "r16", ...sourceOptions });
const bytes = Buffer.from(JSON.stringify(manifest, null, 2) + "\n");
await fs.mkdir(path.dirname(output), { recursive: true });
await fs.lstat(output).then(() => { throw new Error("Manifest output already exists."); }, error => { if (error.code !== "ENOENT") throw error; });
const materialized = await materializeR16PlayableAssetFiles(publicRoot, `${output}.public`, manifest, sourceOptions);
await fs.writeFile(output, bytes, { flag: "wx" });
console.log(JSON.stringify({ output, manifestSha256: createHash("sha256").update(bytes).digest("hex"),
  assetRoot: `${output}.public`, files: manifest.files.length, bytes: manifest.totalBytes, materialized,
  categories: Object.fromEntries(Object.entries(manifest.requiredPaths).map(([key, values]) => [key, values.length])), diagnostics }, null, 2));
