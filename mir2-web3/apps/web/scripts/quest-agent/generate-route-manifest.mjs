#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { buildAuthoritativeClassQuestRoute } from "./route-manifest.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDirectory, "../../../..");
const maxLevel = Number(process.env.MIR2_QUEST_AGENT_MAX_LEVEL ?? 50);
const classNames = process.env.MIR2_QUEST_AGENT_CLASS
  ? [process.env.MIR2_QUEST_AGENT_CLASS]
  : String(process.env.MIR2_QUEST_AGENT_CLASSES ?? "Warrior,Wizard,Taoist")
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean);
if (process.env.MIR2_QUEST_AGENT_ROUTE_OUTPUT && classNames.length !== 1) {
  throw new Error("MIR2_QUEST_AGENT_ROUTE_OUTPUT requires exactly one requested class");
}

const summaries = [];
for (const className of classNames) {
  const outputPath = process.env.MIR2_QUEST_AGENT_ROUTE_OUTPUT
    ? path.resolve(process.cwd(), process.env.MIR2_QUEST_AGENT_ROUTE_OUTPUT)
    : path.join(
        projectRoot,
        "docs/generated/quest-agent",
        `${className.toLowerCase()}-1-${maxLevel}.json`,
      );
  const route = await buildAuthoritativeClassQuestRoute({ className, maxLevel });
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(route, null, 2)}\n`, "utf8");
  summaries.push({
    outputPath,
    className: route.className,
    maxLevel: route.maxLevel,
    routeQuestCount: route.routeQuestCount,
    segments: route.segments.map(({ label, questCount, blockedQuestIds }) => ({
      label,
      questCount,
      blockedQuestCount: blockedQuestIds.length,
    })),
    capabilityMatrix: route.capabilityMatrix,
    blockerMatrix: route.blockerMatrix,
  });
}

console.log(JSON.stringify(summaries.length === 1 ? summaries[0] : summaries, null, 2));
