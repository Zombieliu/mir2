import assert from "node:assert/strict";

import {
  redactCaptureSecrets,
  redactCommandArgs,
} from "./capture-secret-redaction.mjs";

assert.deepEqual(
  redactCaptureSecrets({ account: "tester", password: "secret", nested: { qaControlToken: "token" } }),
  { account: "tester", password: "[redacted]", nested: { qaControlToken: "[redacted]" } },
);

const frame = redactCaptureSecrets(
  JSON.stringify({ type: "login", accountId: "tester", password: "secret" }),
);
assert.deepEqual(JSON.parse(frame), {
  type: "login",
  accountId: "tester",
  password: "[redacted]",
});

assert.equal(
  redactCaptureSecrets("ws://localhost/?qaControlToken=secret&map=0"),
  "ws://localhost/?qaControlToken=[redacted]&map=0",
);
assert.deepEqual(
  redactCommandArgs(["--account", "tester", "--password", "secret", "--qaControlToken", "token"]),
  ["--account", "tester", "--password", "[redacted]", "--qaControlToken", "[redacted]"],
);
assert.deepEqual(
  redactCommandArgs([
    "--password=secret",
    "--baseUrl=ws://localhost/?qaControlToken=token&map=0",
  ]),
  [
    "--password=[redacted]",
    "--baseUrl=ws://localhost/?qaControlToken=[redacted]&map=0",
  ],
);

console.log("capture secret redaction tests passed");
