import assert from "node:assert/strict";
import { after, test } from "node:test";

import { GET } from "../app/version/route.ts";

const originalRevision = process.env.MIR2_DEPLOY_REVISION;
const originalUnrelatedSecret = process.env.MIR2_UNRELATED_SECRET;

after(() => {
  if (originalRevision === undefined) {
    delete process.env.MIR2_DEPLOY_REVISION;
  } else {
    process.env.MIR2_DEPLOY_REVISION = originalRevision;
  }

  if (originalUnrelatedSecret === undefined) {
    delete process.env.MIR2_UNRELATED_SECRET;
  } else {
    process.env.MIR2_UNRELATED_SECRET = originalUnrelatedSecret;
  }
});

async function assertVersionResponse(expectedRevision) {
  const response = GET();

  assert.equal(response.status, 200);
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.deepEqual(await response.json(), { revision: expectedRevision });
}

test("returns only the configured deployment revision", async () => {
  process.env.MIR2_DEPLOY_REVISION = " 0123456789abcdef ";
  process.env.MIR2_UNRELATED_SECRET = "must-not-leak";

  await assertVersionResponse("0123456789abcdef");
});

test("returns an explicit local marker when the revision is unset", async () => {
  delete process.env.MIR2_DEPLOY_REVISION;
  await assertVersionResponse("local/unset");
});

test("treats a blank revision as unset", async () => {
  process.env.MIR2_DEPLOY_REVISION = "   ";
  await assertVersionResponse("local/unset");
});
