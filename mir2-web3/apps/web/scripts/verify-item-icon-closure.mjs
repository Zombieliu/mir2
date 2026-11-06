#!/usr/bin/env node

import { assertItemIconClosure } from "./asset-pipeline/item-icon-closure.mjs";

const report = await assertItemIconClosure();
console.log(JSON.stringify({
  ok: true,
  itemCount: report.itemCount,
  catalogueImageCount: report.catalogueImageCount,
  stackImageCount: report.stackImageCount,
  uniqueImageCount: report.uniqueImageCount,
  sourceLibrary: report.sourceLibrary,
  itemCatalogue: report.itemCatalogue,
}, null, 2));
