#!/usr/bin/env node

// Read-only GitHub inventory. Nothing here creates, updates, or deletes refs.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SHA = /^[a-f0-9]{40}$/;
const REPOSITORY = /^[A-Za-z0-9][A-Za-z0-9_.-]*\/[A-Za-z0-9][A-Za-z0-9_.-]*$/;

function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}

export function validateRepository(repository) {
  requireValue(typeof repository === "string" && REPOSITORY.test(repository), "Expected owner/repository");
  return repository;
}

function validRef(name) {
  return typeof name === "string" && name.length > 0 && name !== "@" &&
    !/[\x00-\x20\x7f~^:?*\[\\]/.test(name) && !name.includes("..") && !name.includes("@{") &&
    name.split("/").every((part) => part && !part.startsWith(".") && !part.endsWith(".") && !part.endsWith(".lock"));
}

export function flattenPages(pages) {
  requireValue(Array.isArray(pages) && pages.every(Array.isArray), "Incomplete paginated inventory");
  return pages.flat();
}

function githubGet(endpoint, { paginate = false, jq } = {}) {
  const args = ["api", "--method", "GET", endpoint];
  if (paginate) args.push("--paginate", "--slurp");
  if (jq) args.push("--jq", jq);
  return JSON.parse(execFileSync("gh", args, {
    encoding: "utf8", maxBuffer: 16 * 1024 * 1024,
    timeout: 30_000,
    stdio: ["ignore", "pipe", "pipe"],
  }));
}

export function collectSnapshot(repository, get = githubGet, now = () => new Date().toISOString()) {
  validateRepository(repository);
  const endpoint = `repos/${repository}`;
  const repo = get(endpoint);
  requireValue(repo.full_name?.toLowerCase() === repository.toLowerCase(), "Repository identity changed");
  const branches = flattenPages(get(`${endpoint}/branches?per_page=100`, { paginate: true }))
    .map((branch) => ({ name: branch.name, sha: branch.commit.sha, protected: branch.protected }));
  const pullRequests = flattenPages(get(`${endpoint}/pulls?state=open&per_page=100`, { paginate: true }))
    .map((pr) => ({
      number: pr.number, draft: pr.draft, url: pr.html_url,
      head: { repository: pr.head.repo?.full_name ?? null, ref: pr.head.ref, sha: pr.head.sha },
      base: { repository: pr.base.repo.full_name, ref: pr.base.ref },
    }));
  const tags = flattenPages(get(`${endpoint}/tags?per_page=100`, { paginate: true }))
    .map((tag) => ({ name: tag.name, sha: tag.commit.sha }));
  const snapshot = {
    schemaVersion: 1, repository: repo.full_name, defaultBranch: repo.default_branch,
    observedAt: now(), branches, pullRequests, tags, comparisons: [],
  };
  validateSnapshot(snapshot);
  const baseSha = branches.find((branch) => branch.name === snapshot.defaultBranch).sha;
  for (const branch of branches.filter((branch) => branch.name !== snapshot.defaultBranch)) {
    const comparison = { branch: branch.name, baseSha, headSha: branch.sha };
    try {
      const result = get(`${endpoint}/compare/${baseSha}...${branch.sha}?per_page=1`, {
        jq: "{status, ahead: .ahead_by, behind: .behind_by}",
      });
      snapshot.comparisons.push({ ...comparison, ...result });
    } catch (error) {
      const detail = `${error.message ?? ""}\n${error.stderr?.toString() ?? ""}`;
      // Generic 404, rate limits, permissions and transport failures are NOT ancestry evidence.
      snapshot.comparisons.push({
        ...comparison, status: /No common ancestor between/.test(detail) ? "unrelated" : "unknown",
      });
    }
  }
  return snapshot;
}

export function validateSnapshot(snapshot) {
  requireValue(snapshot?.schemaVersion === 1, "Unsupported inventory schema");
  validateRepository(snapshot.repository);
  requireValue(validRef(snapshot.defaultBranch), "Missing default branch");
  requireValue(typeof snapshot.observedAt === "string" && !Number.isNaN(Date.parse(snapshot.observedAt)), "Missing observation time");
  for (const field of ["branches", "pullRequests", "tags", "comparisons"]) {
    requireValue(Array.isArray(snapshot[field]), `Missing ${field} inventory`);
  }
  for (const field of ["branches", "tags"]) {
    const names = new Set();
    for (const ref of snapshot[field]) {
      requireValue(validRef(ref.name) && SHA.test(ref.sha) && !names.has(ref.name), `Invalid or duplicate ${field} ref`);
      names.add(ref.name);
      if (field === "branches") requireValue(typeof ref.protected === "boolean", "Missing branch protection state");
    }
  }
  requireValue(snapshot.branches.some((branch) => branch.name === snapshot.defaultBranch), "Default branch is absent");
  const numbers = new Set();
  for (const pr of snapshot.pullRequests) {
    requireValue(Number.isSafeInteger(pr.number) && pr.number > 0 && !numbers.has(pr.number), "Invalid or duplicate PR");
    numbers.add(pr.number);
    requireValue(validRef(pr.head?.ref) && validRef(pr.base?.ref) && SHA.test(pr.head?.sha), "Invalid PR refs");
    validateRepository(pr.base.repository);
    if (pr.head.repository !== null) validateRepository(pr.head.repository);
  }
}

export function summarizeSnapshot(snapshot) {
  validateSnapshot(snapshot);
  const sameRepo = (repository) => repository?.toLowerCase() === snapshot.repository.toLowerCase();
  const baseSha = snapshot.branches.find((branch) => branch.name === snapshot.defaultBranch).sha;
  const branches = snapshot.branches.map((branch) => {
    const headOf = snapshot.pullRequests.filter((pr) => sameRepo(pr.head.repository) && pr.head.ref === branch.name).map((pr) => pr.number);
    const baseOf = snapshot.pullRequests.filter((pr) => sameRepo(pr.base.repository) && pr.base.ref === branch.name).map((pr) => pr.number);
    const comparisons = snapshot.comparisons.filter((entry) => entry.branch === branch.name);
    const found = comparisons.length === 1 ? comparisons[0] : null;
    const comparison = found?.baseSha === baseSha && found?.headSha === branch.sha ? found : null;
    const validCounts = comparison && Number.isSafeInteger(comparison.ahead) && comparison.ahead >= 0 &&
      Number.isSafeInteger(comparison.behind) && comparison.behind >= 0 &&
      ({ identical: comparison.ahead === 0 && comparison.behind === 0,
        ahead: comparison.ahead > 0 && comparison.behind === 0,
        behind: comparison.ahead === 0 && comparison.behind > 0,
        diverged: comparison.ahead > 0 && comparison.behind > 0 })[comparison.status];
    let disposition = "keep-unknown";
    if (branch.name === snapshot.defaultBranch || branch.protected) disposition = "keep-protected";
    else if (headOf.length || baseOf.length) disposition = "keep-pr-reference";
    else if (comparison?.status === "unrelated") disposition = "archive-review-required";
    else if (validCounts) disposition = comparison.ahead === 0 ? "review-contained-not-delete" : "keep-unintegrated";
    return {
      ...branch, headOf, baseOf, disposition,
      comparison: branch.name === snapshot.defaultBranch ? { status: "default-branch" } :
        validCounts ? { status: comparison.status, ahead: comparison.ahead, behind: comparison.behind } :
        { status: comparison?.status === "unrelated" ? "unrelated" : "unknown" },
    };
  });
  const tagGroups = {};
  const witnessNameMismatches = [];
  for (const tag of snapshot.tags) {
    const group = tag.name.startsWith("archive/") ? "archive" :
      tag.name.startsWith("developer-environment-starter-") ? "developer-environment-starter" :
      tag.name.startsWith("developer-image-") ? "developer-image" :
      tag.name.startsWith("developer-assets-") ? "developer-assets" :
      tag.name.startsWith("repository-evidence-") ? "repository-evidence" : "other";
    tagGroups[group] = (tagGroups[group] ?? 0) + 1;
    const match = /^(?:developer-environment-starter|developer-full-assets|developer-image)-([a-f0-9]{40})$/.exec(tag.name);
    if (match && match[1] !== tag.sha) witnessNameMismatches.push({ name: tag.name, namedSha: match[1], targetSha: tag.sha });
  }
  return {
    counts: { branches: branches.length, openPullRequests: snapshot.pullRequests.length, tags: snapshot.tags.length },
    branches, tagGroups, witnessNameMismatches,
    limitations: [
      "Read-only report, never deletion authorization. Recheck live SHAs and PR head/base references before maintenance.",
      "Deployment, rollback and local worktree references are not covered. Even contained branches need human review.",
      "Unique commits may be squash/cherry-pick equivalents; counts alone do not establish discarded or merged work.",
      "Tag name/target matching does not prove CI, current image validity, game completion or device acceptance.",
    ],
  };
}

// Optional pre-push guard: validates advertised old SHAs for an explicitly prepared plan.
// It does not perform the push and is not a substitute for verifying remote archive tags.
export function verifyDeletionUpdates(plan, updates, remoteUrl) {
  validateRepository(plan?.repository);
  requireValue(remoteUrl === `https://github.com/${plan.repository}.git`, "Unexpected push destination");
  requireValue(validRef(plan.defaultBranch) && Array.isArray(plan.branches) && plan.branches.length > 0, "Missing deletion plan");
  const expected = new Map();
  for (const branch of plan.branches) {
    requireValue(validRef(branch.name) && branch.name !== plan.defaultBranch && branch.protected === false &&
      SHA.test(branch.sha) && branch.sha !== "0".repeat(40) &&
      !expected.has(`refs/heads/${branch.name}`), "Unsafe or duplicate deletion target");
    expected.set(`refs/heads/${branch.name}`, branch.sha);
  }
  const seen = new Set();
  for (const line of updates.trim().split(/\r?\n/)) {
    const fields = line.split(/\s+/);
    requireValue(fields.length === 4, "Malformed push update");
    const [localRef, localSha, remoteRef, remoteSha] = fields;
    requireValue(localRef === "(delete)" && localSha === "0".repeat(40), "Only branch deletion is permitted");
    requireValue(expected.has(remoteRef) && !seen.has(remoteRef), "Unexpected or duplicate ref update");
    requireValue(remoteSha === expected.get(remoteRef), "Remote branch moved; stop and re-audit");
    seen.add(remoteRef);
  }
  requireValue(seen.size === expected.size, "Incomplete deletion push");
  return true;
}

export async function main(args) {
  if (args.length === 1 && args[0] === "--help") {
    process.stdout.write("Read-only: node audit-repository.mjs [--repo owner/name | --snapshot report.json]\n" +
      "Validation only: --guard-deletions plan.json --remote-url https://github.com/owner/name.git\n");
    return;
  }
  if (args.length === 4 && args[0] === "--guard-deletions" && args[2] === "--remote-url") {
    verifyDeletionUpdates(JSON.parse(readFileSync(args[1], "utf8")), readFileSync(0, "utf8"), args[3]);
    process.stdout.write("Exact branch deletion updates verified.\n");
    return;
  }
  requireValue(args.length === 0 || (args.length === 2 && ["--repo", "--snapshot"].includes(args[0])), "Use --help for supported arguments");
  const loaded = args[0] === "--snapshot" ? JSON.parse(readFileSync(args[1], "utf8")) : null;
  const snapshot = args[0] === "--snapshot" ? (loaded?.snapshot ?? loaded) : collectSnapshot(args[1] ?? "Zombieliu/mir2");
  process.stdout.write(`${JSON.stringify({ snapshot, audit: summarizeSnapshot(snapshot) }, null, 2)}\n`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch(() => {
    // Keep tokens, transport stderr and arbitrary snapshot contents out of diagnostics.
    process.stderr.write("Repository audit failed: incomplete/invalid inventory, unavailable GitHub access, or rejected deletion plan. Use --help.\n");
    process.exitCode = 1;
  });
}
