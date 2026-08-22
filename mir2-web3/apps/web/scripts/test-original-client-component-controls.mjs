import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const componentRoot = new URL("../app/components/", import.meta.url);

async function source(name) {
  return readFile(new URL(name, componentRoot), "utf8");
}

test("bounded original-client controls do not leave visible empty click handlers", async () => {
  const files = await Promise.all([
    source("original-client-overlays.tsx"),
    source("original-client-map-panels.tsx"),
    source("original-client-game-shop.tsx"),
    source("original-client-dialogs.tsx"),
  ]);
  const combined = files.join("\n");

  assert.doesNotMatch(combined, /onClick=\{\(\)\s*=>\s*undefined\}/);
  assert.match(files[0], /disabled\?: boolean/);
  assert.match(files[0], /disabled=\{disabled\}/);
  assert.match(files[0], /onClick=\{onToggleQuestLog\} active=\{showQuestLog\}/);
  assert.doesNotMatch(files[0], /onOpenCharacterTab\("stats2"\)/);
  assert.match(files[1], /value=\{search\}/);
  assert.match(files[1], /searchRef\.current\?\.focus\(\)/);
  assert.match(files[2], /setPage\(\(current\) => Math\.max\(0, current - 1\)\)/);
  assert.match(files[2], /setPage\(\(current\) => Math\.min\(pageCount - 1, current \+ 1\)\)/);
  assert.equal((files[2].match(/disabled=\{currentPage <= 0\}/g) ?? []).length, 2);
  assert.equal((files[2].match(/disabled=\{currentPage >= pageCount - 1\}/g) ?? []).length, 2);
  assert.match(files[3], /\$\{currentPage \+ 1\} \/ \$\{pageCount\}/);
  assert.match(files[3], /onSelect=\{\(\) => setSelectedIndex\(index\)\}/);
});

test("unsupported actions use disabled semantics instead of pretending to reach a backend", async () => {
  const [mapPanels, shop, dialogs] = await Promise.all([
    source("original-client-map-panels.tsx"),
    source("original-client-game-shop.tsx"),
    source("original-client-dialogs.tsx"),
  ]);

  assert.match(mapPanels, /positionBar\} label=.* disabled \/>/);
  assert.match(mapPanels, /upButton\} label=.* disabled \/>/);
  assert.match(mapPanels, /downButton\} label=.* disabled \/>/);
  assert.match(mapPanels, /teleportButton\} label=.* disabled active \/>/);
  assert.match(shop, /positionBar\} label=.* disabled \/>/);
  assert.match(dialogs, /readButton\}\s*label=.*\s*disabled/);
  assert.match(dialogs, /blockListButton\} label=.* disabled \/>/);
  assert.match(dialogs, /bugReportButton\} label=.* disabled \/>/);
});
