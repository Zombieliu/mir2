import assert from "node:assert/strict";
import test from "node:test";

import {
  calculatePlanSha256,
  createCleanupPlan,
  executeBucketCleanup,
} from "./cleanup-r2-bucket.mjs";

class FakeListCommand {
  constructor(input) {
    this.input = input;
  }
}

class FakeDeleteCommand {
  constructor(input) {
    this.input = input;
  }
}

function createFixtureClient({
  pages = [],
  deleteResponses = [],
} = {}) {
  const listInputs = [];
  const deleteInputs = [];
  let listIndex = 0;
  let deleteIndex = 0;

  return {
    listInputs,
    deleteInputs,
    async send(command) {
      if (command instanceof FakeListCommand) {
        listInputs.push(command.input);
        const page = pages[listIndex];
        listIndex += 1;
        if (!page) throw new Error(`Unexpected list request ${listIndex}.`);
        return structuredClone(page);
      }
      if (command instanceof FakeDeleteCommand) {
        deleteInputs.push(command.input);
        const response = deleteResponses[deleteIndex] ?? {};
        deleteIndex += 1;
        return structuredClone(response);
      }
      throw new Error(`Unexpected command: ${command?.constructor?.name}`);
    },
  };
}

function object(Key, Size = 1, ETag = undefined) {
  return {
    Key,
    Size,
    ...(ETag ? { ETag } : {}),
  };
}

test("paginates ListObjectsV2 and keeps only whitelisted prefixes", async () => {
  const client = createFixtureClient({
    pages: [
      {
        IsTruncated: true,
        NextContinuationToken: "page-2",
        Contents: [
          object("trash/z.bin", 9, '"z"'),
          object("mir2/v/current/b.bin", 2, '"b"'),
        ],
      },
      {
        IsTruncated: false,
        Contents: [
          object("shared/a.bin", 3),
          object("old/a.bin", 7),
          object("mir2/v/current/a.bin", 1, '"a"'),
          object("mir2/v/current-old/leak.bin", 4),
        ],
      },
    ],
  });

  const plan = await createCleanupPlan({
    client,
    bucket: "fixture-bucket",
    keepPrefixes: ["mir2/v/current", "shared"],
    ListCommand: FakeListCommand,
  });

  assert.deepEqual(client.listInputs, [
    { Bucket: "fixture-bucket" },
    { Bucket: "fixture-bucket", ContinuationToken: "page-2" },
  ]);
  assert.deepEqual(plan.keepPrefixes, ["mir2/v/current", "shared"]);
  assert.equal(plan.primaryKeepPrefix, "mir2/v/current");
  assert.deepEqual(plan.listed, {
    objectCount: 6,
    totalBytes: 26,
    pageCount: 2,
  });
  assert.deepEqual(plan.kept, {
    objectCount: 3,
    totalBytes: 6,
  });
  assert.deepEqual(plan.delete.objects, [
    { key: "mir2/v/current-old/leak.bin", size: 4 },
    { key: "old/a.bin", size: 7 },
    { key: "trash/z.bin", size: 9, etag: '"z"' },
  ]);

  const reorderedClient = createFixtureClient({
    pages: [{
      IsTruncated: false,
      Contents: [
        object("mir2/v/current/a.bin", 1, '"a"'),
        object("old/a.bin", 7),
        object("shared/a.bin", 3),
        object("mir2/v/current/b.bin", 2, '"b"'),
        object("mir2/v/current-old/leak.bin", 4),
        object("trash/z.bin", 9, '"z"'),
      ],
    }],
  });
  const reorderedPlan = await createCleanupPlan({
    client: reorderedClient,
    bucket: "fixture-bucket",
    keepPrefixes: ["mir2/v/current", "shared"],
    ListCommand: FakeListCommand,
  });
  reorderedPlan.listed.pageCount = plan.listed.pageCount;
  assert.equal(calculatePlanSha256(reorderedPlan), calculatePlanSha256(plan));
});

test("defaults to dry-run and never sends DeleteObjects", async () => {
  const client = createFixtureClient({
    pages: [{
      IsTruncated: false,
      Contents: [
        object("keep/a.bin"),
        object("remove/a.bin"),
      ],
    }],
  });

  const result = await executeBucketCleanup({
    client,
    bucket: "fixture-bucket",
    keepPrefixes: ["keep"],
    ListCommand: FakeListCommand,
    DeleteCommand: FakeDeleteCommand,
  });

  assert.equal(result.mode, "dry-run");
  assert.match(result.planSha256, /^[a-f0-9]{64}$/);
  assert.equal(result.plan.delete.objectCount, 1);
  assert.deepEqual(client.deleteInputs, []);
});

test("rejects every apply confirmation mismatch before deletion", async () => {
  const makeClient = () => createFixtureClient({
    pages: [{
      IsTruncated: false,
      Contents: [object("keep/a.bin"), object("remove/a.bin")],
    }],
  });
  const planningClient = makeClient();
  const plan = await createCleanupPlan({
    client: planningClient,
    bucket: "fixture-bucket",
    keepPrefixes: ["keep"],
    ListCommand: FakeListCommand,
  });
  const sha = calculatePlanSha256(plan);

  for (const [overrides, expected] of [
    [{ confirmBucket: "wrong" }, /confirmBucket/],
    [{ confirmKeepPrefix: "wrong" }, /confirmKeepPrefix/],
    [{ planSha256: "0".repeat(64) }, /planSha256/],
  ]) {
    const client = makeClient();
    await assert.rejects(
      executeBucketCleanup({
        client,
        bucket: "fixture-bucket",
        keepPrefixes: ["keep"],
        apply: true,
        confirmBucket: "fixture-bucket",
        confirmKeepPrefix: "keep",
        planSha256: sha,
        ListCommand: FakeListCommand,
        DeleteCommand: FakeDeleteCommand,
        ...overrides,
      }),
      expected,
    );
    assert.deepEqual(client.deleteInputs, []);
  }

  const clientWithoutProductionManifest = makeClient();
  await assert.rejects(
    executeBucketCleanup({
      client: clientWithoutProductionManifest,
      bucket: "fixture-bucket",
      keepPrefixes: ["keep"],
      apply: true,
      confirmBucket: "fixture-bucket",
      confirmKeepPrefix: "keep",
      planSha256: sha,
      ListCommand: FakeListCommand,
      DeleteCommand: FakeDeleteCommand,
    }),
    /productionManifestUrl/,
  );
  assert.deepEqual(clientWithoutProductionManifest.deleteInputs, []);
});

test("validates the production objectPrefix before applying", async () => {
  const client = createFixtureClient({
    pages: [{
      IsTruncated: false,
      Contents: [object("keep/a.bin"), object("remove/a.bin")],
    }],
  });
  const planningClient = createFixtureClient({
    pages: [{
      IsTruncated: false,
      Contents: [object("keep/a.bin"), object("remove/a.bin")],
    }],
  });
  const plan = await createCleanupPlan({
    client: planningClient,
    bucket: "fixture-bucket",
    keepPrefixes: ["keep"],
    ListCommand: FakeListCommand,
  });

  await assert.rejects(
    executeBucketCleanup({
      client,
      bucket: "fixture-bucket",
      keepPrefixes: ["keep"],
      apply: true,
      confirmBucket: "fixture-bucket",
      confirmKeepPrefix: "keep",
      planSha256: calculatePlanSha256(plan),
      productionManifestUrl: "https://example.test/api/asset-manifest",
      fetchImpl: async () => ({
        ok: true,
        async json() {
          return { remoteAssets: { objectPrefix: "other" } };
        },
      }),
      ListCommand: FakeListCommand,
      DeleteCommand: FakeDeleteCommand,
    }),
    /objectPrefix mismatch/,
  );
  assert.deepEqual(client.deleteInputs, []);
});

test("applies DeleteObjects in batches of at most 1000", async () => {
  const candidates = Array.from(
    { length: 2001 },
    (_, index) => object(`remove/${String(index).padStart(4, "0")}.bin`),
  );
  const pages = [{
    IsTruncated: false,
    Contents: [object("keep/release.json"), ...candidates],
  }];
  const planningClient = createFixtureClient({ pages });
  const plan = await createCleanupPlan({
    client: planningClient,
    bucket: "fixture-bucket",
    keepPrefixes: ["keep"],
    ListCommand: FakeListCommand,
  });
  const client = createFixtureClient({ pages });

  const result = await executeBucketCleanup({
    client,
    bucket: "fixture-bucket",
    keepPrefixes: ["keep"],
    apply: true,
    confirmBucket: "fixture-bucket",
    confirmKeepPrefix: "keep",
    planSha256: calculatePlanSha256(plan),
    productionManifestUrl: "https://example.test/api/asset-manifest",
    fetchImpl: async () => ({
      ok: true,
      async json() {
        return { remoteAssets: { objectPrefix: "keep" } };
      },
    }),
    ListCommand: FakeListCommand,
    DeleteCommand: FakeDeleteCommand,
  });

  assert.equal(result.mode, "apply");
  assert.deepEqual(result.deletion, {
    deleteBatchCount: 3,
    deletedObjectCount: 2001,
  });
  assert.deepEqual(
    client.deleteInputs.map((input) => input.Delete.Objects.length),
    [1000, 1000, 1],
  );
  assert.ok(client.deleteInputs.every((input) => input.Bucket === "fixture-bucket"));
  assert.ok(client.deleteInputs.every((input) => input.Delete.Quiet === true));
});

test("fails when DeleteObjects returns any Errors", async () => {
  const pages = [{
    IsTruncated: false,
    Contents: [object("keep/a.bin"), object("remove/a.bin")],
  }];
  const planningClient = createFixtureClient({ pages });
  const plan = await createCleanupPlan({
    client: planningClient,
    bucket: "fixture-bucket",
    keepPrefixes: ["keep"],
    ListCommand: FakeListCommand,
  });
  const client = createFixtureClient({
    pages,
    deleteResponses: [{
      Errors: [{
        Key: "remove/a.bin",
        Code: "AccessDenied",
        Message: "fixture failure",
      }],
    }],
  });

  await assert.rejects(
    executeBucketCleanup({
      client,
      bucket: "fixture-bucket",
      keepPrefixes: ["keep"],
      apply: true,
      confirmBucket: "fixture-bucket",
      confirmKeepPrefix: "keep",
      planSha256: calculatePlanSha256(plan),
      productionManifestUrl: "https://example.test/api/asset-manifest",
      fetchImpl: async () => ({
        ok: true,
        async json() {
          return { remoteAssets: { objectPrefix: "keep" } };
        },
      }),
      ListCommand: FakeListCommand,
      DeleteCommand: FakeDeleteCommand,
    }),
    /DeleteObjects returned errors.*AccessDenied/,
  );
  assert.equal(client.deleteInputs.length, 1);
});
