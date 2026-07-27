import assert from "node:assert/strict";
import { after, test } from "node:test";

import { GET } from "../app/version/route.ts";

const revisionVariables = [
  "MIR2_DEPLOY_REVISION",
  "VERCEL_GIT_COMMIT_SHA",
  "MIR2_BUILD_REVISION",
];
const originalRevisions = Object.fromEntries(
  revisionVariables.map((name) => [name, process.env[name]]),
);
const originalUnrelatedSecret = process.env.MIR2_UNRELATED_SECRET;

after(() => {
  for (const name of revisionVariables) {
    if (originalRevisions[name] === undefined) {
      delete process.env[name];
    } else {
      process.env[name] = originalRevisions[name];
    }
  }

  if (originalUnrelatedSecret === undefined) {
    delete process.env.MIR2_UNRELATED_SECRET;
  } else {
    process.env.MIR2_UNRELATED_SECRET = originalUnrelatedSecret;
  }
});

function clearRevisionVariables() {
  for (const name of revisionVariables) {
    delete process.env[name];
  }
}

async function assertVersionResponse(expectedRevision) {
  const response = GET();

  assert.equal(response.status, 200);
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.deepEqual(await response.json(), { revision: expectedRevision });
}

test("returns only the configured deployment revision", async () => {
  clearRevisionVariables();
  process.env.MIR2_DEPLOY_REVISION = " 0123456789abcdef ";
  process.env.VERCEL_GIT_COMMIT_SHA = "vercel-fallback";
  process.env.MIR2_BUILD_REVISION = "build-fallback";
  process.env.MIR2_UNRELATED_SECRET = "must-not-leak";

  await assertVersionResponse("0123456789abcdef");
});

test("uses the Vercel runtime revision when no explicit revision exists", async () => {
  clearRevisionVariables();
  process.env.VERCEL_GIT_COMMIT_SHA = " vercel-runtime-sha ";
  process.env.MIR2_BUILD_REVISION = "build-fallback";

  await assertVersionResponse("vercel-runtime-sha");
});

test("uses the build-captured revision when runtime variables are unavailable", async () => {
  clearRevisionVariables();
  process.env.MIR2_BUILD_REVISION = " build-time-sha ";

  await assertVersionResponse("build-time-sha");
});

test("Next config captures the Vercel revision at build time", async () => {
  clearRevisionVariables();
  process.env.VERCEL_GIT_COMMIT_SHA = "vercel-build-sha";

  const { default: nextConfig } = await import(
    `../next.config.ts?version-test=${Date.now()}`
  );
  assert.equal(nextConfig.env?.MIR2_BUILD_REVISION, "vercel-build-sha");
});

test("returns an explicit local marker when the revision is unset", async () => {
  clearRevisionVariables();
  await assertVersionResponse("local/unset");
});

test("treats a blank revision as unset", async () => {
  clearRevisionVariables();
  process.env.MIR2_DEPLOY_REVISION = "   ";
  process.env.VERCEL_GIT_COMMIT_SHA = " ";
  process.env.MIR2_BUILD_REVISION = "\t";
  await assertVersionResponse("local/unset");
});
