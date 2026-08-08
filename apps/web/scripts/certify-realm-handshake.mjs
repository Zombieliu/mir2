import fs from "node:fs/promises";
import path from "node:path";

const wsUrl = process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:17310/ws";
const outputPath = path.resolve(
  process.cwd(),
  "..",
  "..",
  process.env.MIR2_REALM_HANDSHAKE_OUT ??
    "docs/generated/player-qa/platinum-176/latest-realm-handshake.json",
);
const expectedRealmId = process.env.MIR2_EXPECT_REALM_ID ?? null;
const expectedProfileId = process.env.MIR2_EXPECT_PROFILE_ID ?? "platinum_176";
const expectedProfileVersion = numberFromEnv("MIR2_EXPECT_PROFILE_VERSION", 6);
const expectedAcceptanceLevel = numberFromEnv("MIR2_EXPECT_ACCEPTANCE_LEVEL", 50);
const timeoutMs = numberFromEnv("MIR2_REALM_HANDSHAKE_TIMEOUT_MS", 5_000);

if (typeof WebSocket !== "function") {
  throw new Error("This certificate requires a Node runtime with the WebSocket global.");
}

const realmInfo = await receiveRealmInfo();
const tiers = realmInfo.ratePolicy?.monsterExperienceTiers ?? [];
const assertions = {
  schemaMatches: realmInfo.schema === "mir2-realm-handshake/1",
  realmMatches: expectedRealmId === null || realmInfo.realmId === expectedRealmId,
  profileMatches: realmInfo.profileId === expectedProfileId,
  profileVersionMatches: realmInfo.profileVersion === expectedProfileVersion,
  acceptanceLevelMatches: realmInfo.acceptanceLevel === expectedAcceptanceLevel,
  ratePolicyNamed:
    typeof realmInfo.ratePolicy?.label === "string" &&
    realmInfo.ratePolicy.label.trim().length > 0,
  bundleHashPresent:
    typeof realmInfo.bundleHash === "string" &&
    /^[0-9a-f]{64}$/.test(realmInfo.bundleHash),
  sourceVersionPresent:
    Number.isInteger(realmInfo.sourceData?.crystalDatabaseVersion) &&
    Number.isInteger(realmInfo.sourceData?.crystalDatabaseCustomVersion),
  experienceTiersContiguous:
    tiers.length > 0 &&
    tiers[0]?.minLevel === 1 &&
    tiers.at(-1)?.maxLevel === expectedAcceptanceLevel &&
    tiers.every(
      (tier, index) =>
        Number.isInteger(tier.minLevel) &&
        Number.isInteger(tier.maxLevel) &&
        Number.isFinite(tier.multiplier) &&
        tier.minLevel <= tier.maxLevel &&
        tier.multiplier > 0 &&
        (index === 0 || tiers[index - 1].maxLevel + 1 === tier.minLevel),
    ),
  economyRatesPositive:
    Number.isFinite(realmInfo.ratePolicy?.goldMultiplier) &&
    realmInfo.ratePolicy.goldMultiplier > 0 &&
    Number.isFinite(realmInfo.ratePolicy?.dropMultiplier) &&
    realmInfo.ratePolicy.dropMultiplier > 0,
};
const report = {
  schema: "mir2-realm-handshake-certificate/1",
  capturedAt: new Date().toISOString(),
  wsUrl,
  expected: {
    realmId: expectedRealmId,
    profileId: expectedProfileId,
    profileVersion: expectedProfileVersion,
    acceptanceLevel: expectedAcceptanceLevel,
  },
  realmInfo,
  assertions,
  passed: Object.values(assertions).every(Boolean),
};

await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));

if (!report.passed) process.exitCode = 1;

function receiveRealmInfo() {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(wsUrl);
    const timeout = setTimeout(() => {
      socket.close();
      reject(new Error(`Timed out after ${timeoutMs}ms waiting for realmInfo from ${wsUrl}`));
    }, timeoutMs);

    socket.addEventListener("message", (event) => {
      let message;
      try {
        message = JSON.parse(String(event.data));
      } catch {
        return;
      }
      if (message?.type !== "realmInfo") return;
      clearTimeout(timeout);
      socket.close();
      resolve(message.payload);
    });
    socket.addEventListener("error", () => {
      clearTimeout(timeout);
      reject(new Error(`WebSocket error while connecting to ${wsUrl}`));
    });
  });
}

function numberFromEnv(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw.trim() === "") return fallback;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) throw new Error(`${name} must be a finite number`);
  return parsed;
}
