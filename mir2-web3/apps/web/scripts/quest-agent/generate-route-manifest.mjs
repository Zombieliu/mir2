#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { buildAuthoritativeClassQuestRoute } from "./route-manifest.mjs";
import { annotateNewcomerRoute, loadNewcomerGuidance, validateNewcomerGuidance } from "./newcomer-guidance.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDirectory, "../../../..");
const maxLevel = Number(process.env.MIR2_QUEST_AGENT_MAX_LEVEL ?? 50);
const guidanceProfile = process.env.MIR2_QUEST_AGENT_GUIDANCE;
if (guidanceProfile && guidanceProfile !== "newcomer-v1") {
  throw new Error("Unsupported guidance profile");
}
const guidance = guidanceProfile ? await loadNewcomerGuidance() : null;
if (guidance) {
  if (!Number.isInteger(maxLevel) || maxLevel < 1 || maxLevel > guidance.maxLevel) {
    throw new Error(`Guidance requires MIR2_QUEST_AGENT_MAX_LEVEL between 1 and ${guidance.maxLevel}`);
  }
  const routes = await Promise.all(["Warrior", "Wizard", "Taoist"].map((className) =>
    buildAuthoritativeClassQuestRoute({ className, maxLevel: guidance.maxLevel })));
  validateNewcomerGuidance(guidance, routes);
}
const classNames = process.env.MIR2_QUEST_AGENT_CLASS
  ? [process.env.MIR2_QUEST_AGENT_CLASS]
  : String(process.env.MIR2_QUEST_AGENT_CLASSES ?? "Warrior,Wizard,Taoist")
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean);
if (process.env.MIR2_QUEST_AGENT_ROUTE_OUTPUT && classNames.length !== 1) {
  throw new Error("MIR2_QUEST_AGENT_ROUTE_OUTPUT requires exactly one requested class");
}
if (guidance && classNames.some((name) => !["Warrior", "Wizard", "Taoist"].includes(name))) {
  throw new Error("newcomer-v1 supports Warrior, Wizard and Taoist only");
}

const summaries = [];
for (const className of classNames) {
  const outputPath = process.env.MIR2_QUEST_AGENT_ROUTE_OUTPUT
    ? path.resolve(process.cwd(), process.env.MIR2_QUEST_AGENT_ROUTE_OUTPUT)
    : path.join(
        projectRoot,
        "docs/generated/quest-agent",
        `${className.toLowerCase()}-1-${maxLevel}${guidance ? "-newcomer-v1" : ""}.json`,
      );
  const sourceRoute = await buildAuthoritativeClassQuestRoute({ className, maxLevel });
  const route = guidance ? annotateNewcomerRoute(sourceRoute, guidance, guidance.journey) : sourceRoute;
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
