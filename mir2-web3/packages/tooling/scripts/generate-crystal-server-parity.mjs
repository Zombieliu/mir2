import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const crystalServerRoot = resolve(repoRoot, "..", "Crystal", "Server");
const outputDir = resolve(repoRoot, "docs", "generated");
const outputPath = resolve(outputDir, "crystal-server-parity.json");

const moduleRoots = [
  "MirEnvir",
  "MirObjects",
  "MirDatabase",
  "MirNetwork",
  "Helpers",
  "Utils",
];

function walkFiles(root) {
  const entries = readdirSync(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(fullPath));
      continue;
    }
    if (entry.isFile() && fullPath.endsWith(".cs")) {
      files.push(fullPath);
    }
  }
  return files;
}

function summarizeFile(fullPath) {
  const relative = fullPath.slice(crystalServerRoot.length + 1).replaceAll(sep, "/");
  const content = readFileSync(fullPath, "utf8");
  const lineCount = content.split(/\r?\n/).length;
  const classMatches = [...content.matchAll(/\b(class|struct|interface)\s+([A-Za-z0-9_]+)/g)];
  const methodMatches = [...content.matchAll(/\b(public|protected|private|internal)\s+(override\s+|virtual\s+|static\s+)?[A-Za-z0-9_<>,\[\]\?]+\s+([A-Za-z0-9_]+)\s*\(/g)];

  return {
    relativePath: relative,
    lineCount,
    classes: classMatches.map((match) => match[2]),
    methodCount: methodMatches.length,
  };
}

const modules = moduleRoots.map((moduleName) => {
  const fullRoot = resolve(crystalServerRoot, moduleName);
  const files = walkFiles(fullRoot).map(summarizeFile);
  return {
    module: moduleName,
    fileCount: files.length,
    totalLines: files.reduce((sum, file) => sum + file.lineCount, 0),
    sampleClasses: files.flatMap((file) => file.classes).slice(0, 24),
    files,
  };
});

const summary = {
  generatedAt: new Date().toISOString(),
  crystalServerRoot,
  moduleCount: modules.length,
  totalFiles: modules.reduce((sum, module) => sum + module.fileCount, 0),
  totalLines: modules.reduce((sum, module) => sum + module.totalLines, 0),
  modules,
};

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, JSON.stringify(summary, null, 2));

console.log(`wrote ${outputPath}`);
