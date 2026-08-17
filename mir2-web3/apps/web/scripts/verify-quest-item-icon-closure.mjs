#!/usr/bin/env node

import { assertQuestItemIconClosure } from "./asset-pipeline/quest-item-icon-closure.mjs";

const report = assertQuestItemIconClosure();
console.log(JSON.stringify({
  ok: true,
  questItemCount: report.questItemCount,
  uniqueImageCount: report.uniqueImageCount,
}, null, 2));
