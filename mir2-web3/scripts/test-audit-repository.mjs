#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  collectSnapshot, flattenPages, summarizeSnapshot, validateRepository, verifyDeletionUpdates,
} from "./audit-repository.mjs";

const MAIN = "1".repeat(40);
const OLD = "2".repeat(40);
const ZERO = "0".repeat(40);
const REPO = "Zombieliu/mir2";
const PUSH_URL = `https://github.com/${REPO}.git`;
const fixture = () => ({
  schemaVersion: 1, repository: REPO, defaultBranch: "main", observedAt: "2026-09-30T00:00:00Z",
  branches: [
    { name: "main", sha: MAIN, protected: true },
    { name: "codex/topic", sha: OLD, protected: false },
  ],
  pullRequests: [], tags: [], comparisons: [],
});
const comparison = (status, ahead, behind) => ({ branch: "codex/topic", baseSha: MAIN, headSha: OLD, status, ahead, behind });
const disposition = (snapshot) => summarizeSnapshot(snapshot).branches[1].disposition;
const plan = () => ({ repository: REPO, defaultBranch: "main", branches: [
  { name: "codex/old", sha: OLD, protected: false },
  { name: "fix/old", sha: MAIN, protected: false },
] });
const updates = () => `(delete) ${ZERO} refs/heads/codex/old ${OLD}\n(delete) ${ZERO} refs/heads/fix/old ${MAIN}\n`;

test("retains the default branch and any protected branch", () => {
  const snapshot = fixture();
  snapshot.branches[0].protected = false;
  snapshot.branches[1].protected = true;
  snapshot.comparisons = [comparison("behind", 0, 2)];
  assert.deepEqual(summarizeSnapshot(snapshot).branches.map((branch) => branch.disposition), ["keep-protected", "keep-protected"]);
});

test("retains PR heads and stacked PR bases", () => {
  for (const role of ["head", "base"]) {
    const snapshot = fixture();
    const pr = { number: 251, head: { repository: REPO, ref: "main", sha: MAIN }, base: { repository: REPO, ref: "main" } };
    pr[role].ref = "codex/topic";
    snapshot.pullRequests = [pr];
    snapshot.comparisons = [comparison("behind", 0, 2)];
    assert.equal(disposition(snapshot), "keep-pr-reference");
    assert.deepEqual(summarizeSnapshot(snapshot).branches[1][role === "head" ? "headOf" : "baseOf"], [251]);
  }
});

test("a fork head with the same name does not reference an origin branch", () => {
  const snapshot = fixture();
  snapshot.pullRequests = [{ number: 1, head: { repository: "Someone/mir2", ref: "codex/topic", sha: OLD }, base: { repository: REPO, ref: "main" } }];
  assert.equal(disposition(snapshot), "keep-unknown");
  assert.deepEqual(summarizeSnapshot(snapshot).branches[1].headOf, []);
});

test("never auto-deletes a contained branch or equates unique commits with discarded work", () => {
  for (const [status, ahead, behind, expected] of [
    ["identical", 0, 0, "review-contained-not-delete"],
    ["behind", 0, 8, "review-contained-not-delete"],
    ["ahead", 11, 0, "keep-unintegrated"],
    ["diverged", 5, 12, "keep-unintegrated"],
    ["unrelated", undefined, undefined, "archive-review-required"],
  ]) {
    const snapshot = fixture();
    snapshot.comparisons = [comparison(status, ahead, behind)];
    assert.equal(disposition(snapshot), expected);
  }
});

test("stale, duplicate or contradictory comparisons fail closed", () => {
  for (const rows of [
    [{ ...comparison("behind", 0, 1), headSha: MAIN }],
    [{ ...comparison("behind", 0, 1), baseSha: OLD }],
    [comparison("behind", 0, 1), comparison("behind", 0, 1)],
    [comparison("behind", 5, 0)], [comparison("behind", -1, 1)],
    [comparison("behind", 0, "1")], [comparison("unknown", 0, 1)],
  ]) {
    const snapshot = fixture();
    snapshot.comparisons = rows;
    assert.equal(disposition(snapshot), "keep-unknown");
  }
});

test("reports tag groups and name/target mismatch without rewriting tags", () => {
  const snapshot = fixture();
  snapshot.tags = [
    { name: `developer-environment-starter-${MAIN}`, sha: MAIN },
    { name: `developer-image-${MAIN}`, sha: OLD },
    { name: "archive/legacy-branches/test", sha: OLD },
    { name: "developer-assets-f71b89aa3850", sha: MAIN },
    { name: "repository-evidence-052c4c27", sha: MAIN },
  ];
  const original = structuredClone(snapshot);
  const report = summarizeSnapshot(snapshot);
  assert.equal(report.witnessNameMismatches.length, 1);
  assert.deepEqual(report.witnessNameMismatches[0], { name: `developer-image-${MAIN}`, namedSha: MAIN, targetSha: OLD });
  assert.equal(report.tagGroups.archive, 1);
  assert.equal(report.counts.tags, 5);
  assert.deepEqual(snapshot, original);
});

test("rejects incomplete inventory, duplicate refs and missing protection metadata", () => {
  for (const mutate of [
    (snapshot) => delete snapshot.tags,
    (snapshot) => delete snapshot.pullRequests,
    (snapshot) => snapshot.branches.shift(),
    (snapshot) => snapshot.branches.push(snapshot.branches[0]),
    (snapshot) => delete snapshot.branches[1].protected,
    (snapshot) => snapshot.branches[1].sha = "short-sha",
    (snapshot) => snapshot.observedAt = "not-a-date",
  ]) {
    const snapshot = fixture();
    mutate(snapshot);
    assert.throws(() => summarizeSnapshot(snapshot));
  }
});

test("repository arguments cannot become options, URL paths or shell input", () => {
  assert.equal(validateRepository(REPO), REPO);
  for (const input of ["--repo", "a/b/c", "../mir2", "a/b?token=secret", "a/b;command", "a/b\n", null]) {
    assert.throws(() => validateRepository(input));
  }
});

test("pagination keeps every page and rejects non-array pages", () => {
  assert.deepEqual(flattenPages([[1, 2], [3], []]), [1, 2, 3]);
  assert.throws(() => flattenPages([{ message: "API error" }]));
});

test("collector uses pinned SHAs, paginates all lists and filters huge comparison payloads", () => {
  const calls = [];
  const snapshot = collectSnapshot(REPO, (endpoint, options = {}) => {
    calls.push({ endpoint, options });
    if (endpoint === `repos/${REPO}`) return { full_name: REPO, default_branch: "main" };
    if (endpoint.includes("/branches?")) return [[{ name: "main", commit: { sha: MAIN }, protected: true }], [{ name: "codex/topic", commit: { sha: OLD }, protected: false }]];
    if (endpoint.includes("/pulls?")) return [[]];
    if (endpoint.includes("/tags?")) return [[{ name: "archive/old", commit: { sha: OLD } }]];
    assert.ok(endpoint.includes(`${MAIN}...${OLD}`));
    assert.equal(options.jq, "{status, ahead: .ahead_by, behind: .behind_by}");
    return { status: "ahead", ahead: 11, behind: 0 };
  }, () => "2026-09-30T00:00:00Z");
  assert.equal(snapshot.branches.length, 2);
  assert.equal(snapshot.tags[0].sha, OLD);
  assert.equal(disposition(snapshot), "keep-unintegrated");
  assert.equal(calls.filter((call) => call.options.paginate).length, 3);
});

test("generic 404, permissions, rate limits and EOF are not no-common-ancestor evidence", () => {
  for (const [message, expected] of [
    ["HTTP 404: No common ancestor between base and head.", "archive-review-required"],
    ["HTTP 404: Not Found", "keep-unknown"], ["HTTP 403: forbidden", "keep-unknown"],
    ["secondary rate limit", "keep-unknown"], ["early EOF", "keep-unknown"],
  ]) {
    const snapshot = collectSnapshot(REPO, (endpoint) => {
      if (endpoint === `repos/${REPO}`) return { full_name: REPO, default_branch: "main" };
      if (endpoint.includes("/branches?")) return [fixture().branches.map((branch) => ({ ...branch, commit: { sha: branch.sha } }))];
      if (endpoint.includes("/pulls?") || endpoint.includes("/tags?")) return [[]];
      throw new Error(message);
    }, () => "2026-09-30T00:00:00Z");
    assert.equal(disposition(snapshot), expected);
  }
});

test("deletion guard permits only the complete exact-SHA plan", () => {
  assert.equal(verifyDeletionUpdates(plan(), updates(), PUSH_URL), true);
});

test("deletion guard rejects moved branches, creations, tags, main and partial pushes", () => {
  for (const input of [
    updates().replace(OLD, MAIN),
    updates().replace(ZERO, OLD),
    updates().replace("(delete)", "refs/heads/codex/new"),
    updates().replace("refs/heads/codex/old", "refs/tags/codex/old"),
    updates().replace("refs/heads/codex/old", "refs/heads/main"),
    updates().split("\n")[0], "", `${updates()}${updates()}`, "malformed",
  ]) {
    assert.throws(() => verifyDeletionUpdates(plan(), input, PUSH_URL));
  }
  assert.throws(() => verifyDeletionUpdates(plan(), updates(), "https://github.com/Someone/mir2.git"));
});

test("deletion plans reject protected, default, duplicate or invalid refs", () => {
  for (const mutate of [
    (value) => value.branches[0].protected = true,
    (value) => delete value.branches[0].protected,
    (value) => value.branches[0].name = "main",
    (value) => value.branches[0].name = "codex/../old",
    (value) => value.branches[0].name = "codex/foo.lock",
    (value) => value.branches[0].name = "codex//old",
    (value) => value.branches.push(value.branches[0]),
    (value) => value.branches = [],
  ]) {
    const value = plan();
    mutate(value);
    assert.throws(() => verifyDeletionUpdates(value, updates(), PUSH_URL));
  }
  const value = plan();
  value.branches[0].sha = ZERO;
  assert.throws(() => verifyDeletionUpdates(value, updates().replace(OLD, ZERO), PUSH_URL));
});

test("CLI replays a captured report offline and rejects an invalid snapshot", async (context) => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-repository-audit-test-"));
  context.after(() => fs.rm(directory, { recursive: true, force: true }));
  const snapshotPath = path.join(directory, "report with spaces.json");
  const script = fileURLToPath(new URL("./audit-repository.mjs", import.meta.url));
  await fs.writeFile(snapshotPath, JSON.stringify({ snapshot: fixture() }));
  const report = JSON.parse(execFileSync(process.execPath, [script, "--snapshot", snapshotPath], { encoding: "utf8" }));
  assert.deepEqual(report.audit.counts, { branches: 2, openPullRequests: 0, tags: 0 });
  assert.equal(report.audit.branches[0].comparison.status, "default-branch");
  await fs.writeFile(snapshotPath, "null");
  assert.throws(() => execFileSync(process.execPath, [script, "--snapshot", snapshotPath], { stdio: "pipe" }), (error) => error.status === 1);
});

test("CLI help is offline and unexpected arguments fail without exposing contents", () => {
  const script = fileURLToPath(new URL("./audit-repository.mjs", import.meta.url));
  assert.match(execFileSync(process.execPath, [script, "--help"], { encoding: "utf8" }), /Read-only/);
  assert.throws(() => execFileSync(process.execPath, [script, "--delete", "secret-marker"], { stdio: "pipe" }), (error) => {
    assert.equal(error.status, 1);
    assert.doesNotMatch(error.stderr.toString(), /secret-marker/);
    return true;
  });
});
