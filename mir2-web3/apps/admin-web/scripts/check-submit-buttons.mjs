import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");
const sourceRoots = ["app", "components"].map((directory) =>
  path.join(root, directory)
);
const implementation = path.join(root, "components", "submit-button.tsx");
const allowedClientForm = path.join(
  root,
  "components",
  "service-trace-console.tsx"
);

const files = (
  await Promise.all(sourceRoots.map((directory) => collectTsx(directory)))
).flat();
const failures = [];

for (const file of files) {
  const sourceText = await readFile(file, "utf8");
  const source = ts.createSourceFile(
    file,
    sourceText,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX
  );

  visit(source);

  function visit(node) {
    if (ts.isJsxElement(node) && node.openingElement.tagName.getText(source) === "button") {
      const attributes = node.openingElement.attributes.properties;
      const type = stringAttribute(attributes, "type", source);
      if (type === "submit" && file !== implementation) {
        const hasBusy = hasAttribute(attributes, "aria-busy");
        const hasDisabled = hasAttribute(attributes, "disabled");
        if (file !== allowedClientForm || !hasBusy || !hasDisabled) {
          const line =
            source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
          failures.push(
            `${path.relative(root, file)}:${line} must use SubmitButton or expose a guarded client loading state`
          );
        }
      }
    }
    ts.forEachChild(node, visit);
  }
}

assert.deepEqual(failures, [], failures.join("\n"));
console.log(`checked ${files.length} admin TSX files: submit actions are guarded`);

async function collectTsx(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        return collectTsx(file);
      }
      return entry.isFile() && entry.name.endsWith(".tsx") ? [file] : [];
    })
  );
  return nested.flat();
}

function hasAttribute(attributes, name) {
  return attributes.some(
    (attribute) =>
      ts.isJsxAttribute(attribute) && attribute.name.getText() === name
  );
}

function stringAttribute(attributes, name, source) {
  const attribute = attributes.find(
    (candidate) =>
      ts.isJsxAttribute(candidate) && candidate.name.getText(source) === name
  );
  return attribute && ts.isJsxAttribute(attribute) &&
    attribute.initializer &&
    ts.isStringLiteral(attribute.initializer)
    ? attribute.initializer.text
    : undefined;
}
