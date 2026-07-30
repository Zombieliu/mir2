import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";

import {
  DeleteObjectsCommand,
  ListObjectsV2Command,
  S3Client,
} from "@aws-sdk/client-s3";

const PLAN_SCHEMA_VERSION = 1;
const MAX_DELETE_BATCH_SIZE = 1000;

export function normalizeKeepPrefixes(values) {
  const rawValues = Array.isArray(values) ? values : [values];
  const prefixes = [];
  const seen = new Set();

  for (const rawValue of rawValues) {
    if (rawValue === undefined || rawValue === null) continue;
    if (typeof rawValue !== "string") {
      throw new Error("--keepPrefix requires a string value.");
    }
    for (const candidate of String(rawValue).split(",")) {
      const prefix = candidate.trim().replace(/^\/+|\/+$/g, "");
      if (!prefix) {
        throw new Error("--keepPrefix values must not be empty.");
      }
      if (!seen.has(prefix)) {
        seen.add(prefix);
        prefixes.push(prefix);
      }
    }
  }

  if (prefixes.length === 0) {
    throw new Error("Provide at least one --keepPrefix value.");
  }

  return prefixes;
}

export function stablePlanJson(plan) {
  return JSON.stringify({
    schemaVersion: plan.schemaVersion,
    kind: plan.kind,
    bucket: plan.bucket,
    primaryKeepPrefix: plan.primaryKeepPrefix,
    keepPrefixes: plan.keepPrefixes,
    delete: {
      objectCount: plan.delete.objectCount,
      totalBytes: plan.delete.totalBytes,
      objects: plan.delete.objects,
    },
  });
}

export function calculatePlanSha256(plan) {
  return createHash("sha256").update(stablePlanJson(plan)).digest("hex");
}

export async function createCleanupPlan({
  client,
  bucket,
  keepPrefixes,
  ListCommand = ListObjectsV2Command,
}) {
  assertClient(client);
  const normalizedBucket = requireNonEmptyString(bucket, "bucket");
  const normalizedKeepPrefixes = normalizeKeepPrefixes(keepPrefixes);
  const objects = [];
  let continuationToken;
  let pageCount = 0;

  do {
    const input = {
      Bucket: normalizedBucket,
      ...(continuationToken ? { ContinuationToken: continuationToken } : {}),
    };
    const response = await client.send(new ListCommand(input));
    pageCount += 1;

    for (const object of response?.Contents ?? []) {
      if (typeof object?.Key !== "string" || object.Key.length === 0) continue;
      const size = normalizeObjectSize(object.Size);
      const etag = typeof object.ETag === "string" && object.ETag.length > 0
        ? object.ETag
        : null;
      objects.push({
        key: object.Key,
        size,
        ...(etag ? { etag } : {}),
      });
    }

    if (response?.IsTruncated === true) {
      const nextToken = response.NextContinuationToken;
      if (typeof nextToken !== "string" || nextToken.length === 0) {
        throw new Error("ListObjectsV2 returned IsTruncated without NextContinuationToken.");
      }
      if (nextToken === continuationToken) {
        throw new Error("ListObjectsV2 repeated the same continuation token.");
      }
      continuationToken = nextToken;
    } else {
      continuationToken = undefined;
    }
  } while (continuationToken);

  objects.sort(compareObjects);
  const keptObjects = [];
  const deleteObjects = [];

  for (const object of objects) {
    if (normalizedKeepPrefixes.some((prefix) => keyBelongsToPrefix(object.key, prefix))) {
      keptObjects.push(object);
    } else {
      deleteObjects.push(object);
    }
  }

  return {
    schemaVersion: PLAN_SCHEMA_VERSION,
    kind: "mir2-r2-bucket-cleanup-plan",
    bucket: normalizedBucket,
    primaryKeepPrefix: normalizedKeepPrefixes[0],
    keepPrefixes: normalizedKeepPrefixes,
    listed: summarizeObjects(objects, { pageCount }),
    kept: summarizeObjects(keptObjects),
    delete: {
      ...summarizeObjects(deleteObjects),
      objects: deleteObjects,
    },
  };
}

export async function validateProductionManifest({
  productionManifestUrl,
  primaryKeepPrefix,
  fetchImpl = globalThis.fetch,
}) {
  if (!productionManifestUrl) return null;
  if (typeof fetchImpl !== "function") {
    throw new Error("A fetch implementation is required for production manifest validation.");
  }

  const response = await fetchImpl(productionManifestUrl, {
    headers: { Accept: "application/json" },
  });
  if (!response?.ok) {
    throw new Error(
      `Production manifest request failed with HTTP ${response?.status ?? "unknown"}.`,
    );
  }

  const payload = await response.json();
  const objectPrefix = payload?.remoteAssets?.objectPrefix ?? payload?.objectPrefix;
  if (typeof objectPrefix !== "string" || objectPrefix.length === 0) {
    throw new Error("Production manifest does not contain remoteAssets.objectPrefix or objectPrefix.");
  }
  if (objectPrefix !== primaryKeepPrefix) {
    throw new Error(
      `Production manifest objectPrefix mismatch: expected ${JSON.stringify(primaryKeepPrefix)}, ` +
        `received ${JSON.stringify(objectPrefix)}.`,
    );
  }

  return {
    url: String(productionManifestUrl),
    objectPrefix,
  };
}

export async function deletePlannedObjects({
  client,
  bucket,
  objects,
  DeleteCommand = DeleteObjectsCommand,
  batchSize = MAX_DELETE_BATCH_SIZE,
}) {
  assertClient(client);
  const normalizedBucket = requireNonEmptyString(bucket, "bucket");
  if (!Number.isInteger(batchSize) || batchSize < 1 || batchSize > MAX_DELETE_BATCH_SIZE) {
    throw new Error(`DeleteObjects batchSize must be between 1 and ${MAX_DELETE_BATCH_SIZE}.`);
  }

  let deletedObjectCount = 0;
  let deleteBatchCount = 0;

  for (let offset = 0; offset < objects.length; offset += batchSize) {
    const batch = objects.slice(offset, offset + batchSize);
    const response = await client.send(
      new DeleteCommand({
        Bucket: normalizedBucket,
        Delete: {
          Objects: batch.map((object) => ({ Key: object.key })),
          Quiet: true,
        },
      }),
    );
    deleteBatchCount += 1;

    if (Array.isArray(response?.Errors) && response.Errors.length > 0) {
      const failures = response.Errors.map((error) => ({
        key: error?.Key ?? null,
        code: error?.Code ?? null,
        message: error?.Message ?? null,
      }));
      throw new Error(`DeleteObjects returned errors: ${JSON.stringify(failures)}`);
    }

    deletedObjectCount += batch.length;
  }

  return {
    deleteBatchCount,
    deletedObjectCount,
  };
}

export async function executeBucketCleanup({
  client,
  bucket,
  keepPrefixes,
  apply = false,
  confirmBucket,
  confirmKeepPrefix,
  planSha256,
  productionManifestUrl,
  fetchImpl = globalThis.fetch,
  ListCommand = ListObjectsV2Command,
  DeleteCommand = DeleteObjectsCommand,
}) {
  const plan = await createCleanupPlan({
    client,
    bucket,
    keepPrefixes,
    ListCommand,
  });
  const actualPlanSha256 = calculatePlanSha256(plan);

  if (apply !== true) {
    return {
      ok: true,
      mode: "dry-run",
      planSha256: actualPlanSha256,
      plan,
      deletion: {
        deleteBatchCount: 0,
        deletedObjectCount: 0,
      },
    };
  }

  if (confirmBucket !== plan.bucket) {
    throw new Error("--confirmBucket must exactly match the target bucket.");
  }
  if (confirmKeepPrefix !== plan.primaryKeepPrefix) {
    throw new Error("--confirmKeepPrefix must exactly match the primary keep prefix.");
  }
  if (planSha256 !== actualPlanSha256) {
    throw new Error(
      `--planSha256 does not match the recomputed plan. Recomputed sha256: ${actualPlanSha256}`,
    );
  }
  if (!productionManifestUrl) {
    throw new Error("--productionManifestUrl is required when --apply true.");
  }

  const productionManifest = await validateProductionManifest({
    productionManifestUrl,
    primaryKeepPrefix: plan.primaryKeepPrefix,
    fetchImpl,
  });
  const deletion = await deletePlannedObjects({
    client,
    bucket: plan.bucket,
    objects: plan.delete.objects,
    DeleteCommand,
  });

  return {
    ok: true,
    mode: "apply",
    planSha256: actualPlanSha256,
    plan,
    productionManifest,
    deletion,
  };
}

export function parseCliArgs(argv) {
  const values = new Map();

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      throw new Error(`Unexpected positional argument: ${argument}`);
    }

    const equalsIndex = argument.indexOf("=");
    let name;
    let value;
    if (equalsIndex >= 0) {
      name = argument.slice(2, equalsIndex);
      value = argument.slice(equalsIndex + 1);
    } else {
      name = argument.slice(2);
      const next = argv[index + 1];
      if (next !== undefined && !next.startsWith("--")) {
        value = next;
        index += 1;
      } else {
        value = true;
      }
    }

    if (!name) throw new Error("CLI option names must not be empty.");
    const current = values.get(name) ?? [];
    current.push(value);
    values.set(name, current);
  }

  return {
    all(name) {
      return values.get(name) ?? [];
    },
    last(name) {
      const entries = values.get(name) ?? [];
      return entries.at(-1);
    },
  };
}

export function createR2Client({ args, env = process.env }) {
  const endpoint = optionalString(
    args.last("endpoint") ?? env.MIR2_R2_S3_ENDPOINT,
  );
  const accountId = optionalString(
    args.last("accountId") ?? env.CLOUDFLARE_ACCOUNT_ID,
  );
  const resolvedEndpoint = endpoint ||
    (accountId ? `https://${accountId}.r2.cloudflarestorage.com` : "");
  if (!resolvedEndpoint) {
    throw new Error("Set MIR2_R2_S3_ENDPOINT, CLOUDFLARE_ACCOUNT_ID, or pass --endpoint.");
  }

  const accessKeyId = optionalString(env.MIR2_R2_ACCESS_KEY_ID);
  const secretAccessKey = optionalString(env.MIR2_R2_SECRET_ACCESS_KEY);
  const sessionToken = optionalString(env.MIR2_R2_SESSION_TOKEN);
  if (!accessKeyId || !secretAccessKey) {
    throw new Error("Set MIR2_R2_ACCESS_KEY_ID and MIR2_R2_SECRET_ACCESS_KEY before accessing R2.");
  }

  return new S3Client({
    region: "auto",
    endpoint: resolvedEndpoint,
    forcePathStyle: true,
    credentials: {
      accessKeyId,
      secretAccessKey,
      ...(sessionToken ? { sessionToken } : {}),
    },
  });
}

export async function runCli({
  argv = process.argv.slice(2),
  env = process.env,
  clientFactory = createR2Client,
  fetchImpl = globalThis.fetch,
  stdout = process.stdout,
} = {}) {
  const args = parseCliArgs(argv);
  const bucket = args.last("bucket") ?? env.MIR2_R2_BUCKET;
  const keepPrefixes = normalizeKeepPrefixes(args.all("keepPrefix"));
  const applyValue = args.last("apply");
  if (applyValue !== undefined && applyValue !== "true" && applyValue !== "false") {
    throw new Error("--apply must be explicitly set to true or false.");
  }

  const client = clientFactory({ args, env });
  try {
    const result = await executeBucketCleanup({
      client,
      bucket,
      keepPrefixes,
      apply: applyValue === "true",
      confirmBucket: args.last("confirmBucket"),
      confirmKeepPrefix: args.last("confirmKeepPrefix"),
      planSha256: args.last("planSha256"),
      productionManifestUrl: args.last("productionManifestUrl"),
      fetchImpl,
    });
    stdout.write(`${JSON.stringify(cliOutput(result), null, 2)}\n`);
    return result;
  } finally {
    client.destroy?.();
  }
}

function cliOutput(result) {
  if (result.mode !== "apply") return result;
  const { objects: _objects, ...deleteSummary } = result.plan.delete;
  return {
    ...result,
    plan: {
      ...result.plan,
      delete: deleteSummary,
    },
  };
}

function summarizeObjects(objects, extra = {}) {
  return {
    objectCount: objects.length,
    totalBytes: objects.reduce((sum, object) => sum + object.size, 0),
    ...extra,
  };
}

function compareObjects(left, right) {
  if (left.key < right.key) return -1;
  if (left.key > right.key) return 1;
  if (left.size !== right.size) return left.size - right.size;
  return String(left.etag ?? "").localeCompare(String(right.etag ?? ""), "en");
}

function keyBelongsToPrefix(key, prefix) {
  return key === prefix || key.startsWith(`${prefix}/`);
}

function normalizeObjectSize(value) {
  const size = Number(value ?? 0);
  if (!Number.isSafeInteger(size) || size < 0) {
    throw new Error(`ListObjectsV2 returned an invalid object size: ${String(value)}`);
  }
  return size;
}

function assertClient(client) {
  if (!client || typeof client.send !== "function") {
    throw new Error("An S3-compatible client with send(command) is required.");
  }
}

function requireNonEmptyString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`A non-empty ${label} is required.`);
  }
  return value;
}

function optionalString(value) {
  if (value === undefined || value === null) return "";
  return String(value).trim();
}

const isDirectExecution =
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectExecution) {
  runCli().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
