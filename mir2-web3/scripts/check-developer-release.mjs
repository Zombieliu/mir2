#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  readFileSync,
  statSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const PROJECT_ROOT = path.resolve(path.dirname(SCRIPT_PATH), "..");
const REPOSITORY_ROOT = path.resolve(PROJECT_ROOT, "..");
const WEB_ROOT = path.join(PROJECT_ROOT, "apps", "web");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const REQUIRED_RUST_TOOLCHAIN = "1.89.0";
const REQUIRED_NODE_VERSION = "22.18.0";
const REQUIRED_NODE_MAJOR = Number(REQUIRED_NODE_VERSION.split(".")[0]);
const REQUIRED_NPM_VERSION = "11.13.0";
const REQUIRED_GH_VERSION = "2.96.0";
const EXPECTED_FULL_ASSET_DESTINATION =
  "mir2-web3/apps/web/public/generated/crystal-packs/full";
const completedChecks = [];

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) {
    fail(message);
  }
}

function record(name, detail) {
  completedChecks.push({ name, detail });
  console.log(`[developer-release] ${name}: ${detail}`);
}

function projectPath(relativePath) {
  return path.resolve(PROJECT_ROOT, ...relativePath.split("/"));
}

function repositoryPath(relativePath) {
  return path.resolve(REPOSITORY_ROOT, ...relativePath.split("/"));
}

function readJson(absolutePath, label) {
  try {
    return JSON.parse(readFileSync(absolutePath, "utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
}

function yamlTopLevelBlock(source, key, label) {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const start = lines.findIndex((line) => line === `${key}:`);
  assert(start >= 0, `${label} has no top-level ${key} block`);
  let end = start + 1;
  while (
    end < lines.length &&
    (lines[end].trim() === "" || lines[end].trimStart().startsWith("#") || /^\s/.test(lines[end]))
  ) {
    end += 1;
  }
  return lines.slice(start + 1, end);
}

function assertMainPushOnlyWorkflow(source, label) {
  const onBlock = yamlTopLevelBlock(source, "on", label);
  const events = onBlock
    .map((line) => /^  ([a-zA-Z0-9_-]+):(?:\s.*)?$/.exec(line)?.[1])
    .filter(Boolean);
  assertJsonEqual(events, ["push"], `${label} privileged triggers`);

  const pushStart = onBlock.findIndex((line) => line === "  push:");
  assert(pushStart >= 0, `${label} has no push trigger`);
  const pushBlock = onBlock.slice(pushStart + 1);
  const branchStart = pushBlock.findIndex((line) => line === "    branches:");
  assert(branchStart >= 0, `${label} push trigger has no branch allowlist`);
  const branches = [];
  for (let index = branchStart + 1; index < pushBlock.length; index += 1) {
    const line = pushBlock[index];
    if (/^\s{4}\S/.test(line)) break;
    const match = /^\s{6}-\s+(.+?)\s*$/.exec(line);
    if (match) branches.push(match[1]);
  }
  assertJsonEqual(branches, ["main"], `${label} privileged push branches`);
}

function assertPinnedWorkflowActions(source, label) {
  const actionRefs = [...source.matchAll(/^\s*uses:\s*([^\s#]+)(?:\s+#.*)?$/gm)].map(
    (match) => match[1],
  );
  assert(actionRefs.length > 0, `${label} has no external actions to inspect`);
  for (const actionRef of actionRefs) {
    assert(
      /@[a-f0-9]{40}$/.test(actionRef),
      `${label} action is not pinned to a full commit: ${actionRef}`,
    );
  }
}

function assertTopLevelPermissions(source, expected, label) {
  const permissions = {};
  for (const line of yamlTopLevelBlock(source, "permissions", label)) {
    if (line.trim() === "" || line.trimStart().startsWith("#")) continue;
    const match = /^  ([a-zA-Z0-9_-]+):\s*(read|write|none)\s*$/.exec(line);
    assert(match, `${label} has an invalid top-level permission line: ${line}`);
    permissions[match[1]] = match[2];
  }
  assertJsonEqual(permissions, expected, `${label} top-level permissions`);
}

function fileInfo(absolutePath, label) {
  let info;
  try {
    info = statSync(absolutePath);
  } catch {
    fail(`${label} is missing: ${absolutePath}`);
  }
  assert(info.isFile(), `${label} is not a regular file: ${absolutePath}`);
  assert(info.size > 0, `${label} is empty: ${absolutePath}`);
  return info;
}

function git(args, cwd = REPOSITORY_ROOT) {
  try {
    return execFileSync("git", args, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trimEnd();
  } catch (error) {
    const stderr = String(error.stderr ?? "").trim();
    fail(`git ${args.join(" ")} failed${stderr ? `: ${stderr}` : ""}`);
  }
}

function assertTracked(relativePath) {
  git(["ls-files", "--error-unmatch", "--", relativePath]);
}

function assertExternalized(relativePath) {
  assert(
    git(["ls-files", "--", relativePath]) === "",
    `externalized file must not be tracked: ${relativePath}`,
  );
  const ignoredPath = git(["check-ignore", "--no-index", "--", relativePath]);
  assert(
    ignoredPath.replaceAll("\\", "/") === relativePath.replaceAll("\\", "/"),
    `externalized file is not covered by .gitignore: ${relativePath}`,
  );
}

function sha256(absolutePath) {
  return createHash("sha256").update(readFileSync(absolutePath)).digest("hex");
}

function stableJson(value) {
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function assertJsonEqual(actual, expected, label) {
  assert(stableJson(actual) === stableJson(expected), `${label} is out of sync`);
}

function assertSafeRelativePath(relativePath, label) {
  assert(typeof relativePath === "string" && relativePath.length > 0, `${label} is empty`);
  assert(!path.isAbsolute(relativePath), `${label} must be relative: ${relativePath}`);
  const normalized = path.posix.normalize(relativePath.replaceAll("\\", "/"));
  assert(
    normalized !== ".." && !normalized.startsWith("../"),
    `${label} escapes its root: ${relativePath}`,
  );
  return normalized;
}

function assertPlainFileName(fileName, label) {
  assert(typeof fileName === "string" && fileName.length > 0, `${label} is empty`);
  assert(path.basename(fileName) === fileName, `${label} is not a plain file name: ${fileName}`);
  assert(!fileName.includes("/") && !fileName.includes("\\"), `${label} contains a separator`);
}

function readPngDimensions(absolutePath, label) {
  const bytes = readFileSync(absolutePath);
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert(bytes.length >= 24, `${label} is too small to be a PNG`);
  assert(bytes.subarray(0, 8).equals(signature), `${label} has an invalid PNG signature`);
  assert(bytes.toString("ascii", 12, 16) === "IHDR", `${label} has no PNG IHDR chunk`);
  const width = bytes.readUInt32BE(16);
  const height = bytes.readUInt32BE(20);
  assert(width > 0 && height > 0, `${label} has invalid dimensions ${width}x${height}`);
  return { width, height };
}

function assertWasmHeader(absolutePath, label) {
  const bytes = readFileSync(absolutePath);
  assert(bytes.length >= 8, `${label} is too small to be WebAssembly`);
  assert(
    bytes.subarray(0, 8).equals(Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00])),
    `${label} has an invalid WebAssembly header`,
  );
}

function checkRepositoryAndSubmoduleLock() {
  const root = git(["rev-parse", "--show-toplevel"]).replaceAll("\\", "/");
  assert(
    root.toLowerCase() === REPOSITORY_ROOT.replaceAll("\\", "/").toLowerCase(),
    `repository root mismatch: expected ${REPOSITORY_ROOT}, found ${root}`,
  );

  const treeEntry = git(["ls-tree", "HEAD", "Crystal"]);
  const match = /^160000 commit ([a-f0-9]{40})\tCrystal$/.exec(treeEntry);
  assert(match, `Crystal is not pinned as a Git submodule: ${treeEntry}`);

  const crystalRoot = repositoryPath("Crystal");
  const actualCommit = git(["rev-parse", "HEAD"], crystalRoot);
  assert(
    actualCommit === match[1],
    `Crystal submodule mismatch: expected ${match[1]}, found ${actualCommit}`,
  );

  const submoduleStatus = git(["submodule", "status", "--recursive"]);
  for (const line of submoduleStatus.split(/\r?\n/).filter(Boolean)) {
    assert(line.startsWith(" "), `submodule is not at its pinned clean commit: ${line}`);
  }

  const crystalChanges = git(["status", "--porcelain=v1", "--untracked-files=all"], crystalRoot);
  assert(!crystalChanges, `Crystal submodule has local changes:\n${crystalChanges}`);
  record("repository lock", `${git(["rev-parse", "HEAD"])} / Crystal ${actualCommit}`);
}

function checkDeveloperReleaseLock() {
  const relativePath = "config/developer-release.json";
  assertTracked(`mir2-web3/${relativePath}`);
  const release = readJson(projectPath(relativePath), "developer release lock");
  const assets = readJson(
    projectPath("config/developer-assets.json"),
    "developer asset manifest",
  );

  assert(release.schemaVersion === 1, "developer release schemaVersion must be 1");
  assert(release.kind === "mir2-developer-release", "developer release kind is invalid");
  assert(release.repository === "Zombieliu/mir2", "developer release repository is invalid");
  assert(release.projectDirectory === "mir2-web3", "developer release projectDirectory is invalid");

  const crystalEntry = git(["ls-tree", "HEAD", "Crystal"]);
  const crystalMatch = /^160000 commit ([a-f0-9]{40})\tCrystal$/.exec(crystalEntry);
  assert(crystalMatch, "Crystal gitlink is missing");
  assert(
    release.crystal?.commit === crystalMatch[1],
    "developer release Crystal commit differs from the gitlink",
  );

  assertJsonEqual(
    release.toolchains,
    {
      node: REQUIRED_NODE_VERSION,
      npm: REQUIRED_NPM_VERSION,
      rust: REQUIRED_RUST_TOOLCHAIN,
      githubCli: REQUIRED_GH_VERSION,
    },
    "developer release toolchains",
  );
  assert(
    release.container?.baseImage?.includes("@sha256:"),
    "developer base image must be pinned by digest",
  );
  assert(
    release.container?.publishedImage === "ghcr.io/zombieliu/mir2-developer",
    "developer published image name is invalid",
  );
  assert(
    release.container?.publishedDigest === null ||
      /^sha256:[a-f0-9]{64}$/.test(release.container.publishedDigest),
    "developer published image digest must be null or an OCI SHA-256 digest",
  );
  assert(
    release.container?.publishedRevision === null ||
      /^[a-f0-9]{40}$/.test(release.container.publishedRevision),
    "developer published image revision must be null or a full Git commit",
  );
  assert(
    (release.container.publishedDigest === null) ===
      (release.container.publishedRevision === null),
    "developer published image digest and revision must be set together",
  );
  assertJsonEqual(
    release.container?.platforms,
    ["linux/amd64", "linux/arm64"],
    "developer image platforms",
  );
  assert(
    release.assets?.manifest === "mir2-web3/config/developer-assets.json",
    "developer release asset manifest path is invalid",
  );
  assert(
    release.assets?.releaseTag === assets.releaseTag &&
      release.assets?.contentHash === assets.contentHash,
    "developer release asset lock differs from developer-assets.json",
  );
  assert(
    release.assets?.remoteBaseUrl === null ||
      /^https:\/\/[^/]+\/.+/.test(release.assets.remoteBaseUrl),
    "developer remote asset URL must be null or an HTTPS version path",
  );

  const nvmVersion = readFileSync(projectPath(".nvmrc"), "utf8").trim();
  assert(nvmVersion === REQUIRED_NODE_VERSION, ".nvmrc differs from developer release");
  const rustToolchain = readFileSync(projectPath("rust-toolchain.toml"), "utf8");
  assert(
    rustToolchain.includes(`channel = "${REQUIRED_RUST_TOOLCHAIN}"`),
    "rust-toolchain.toml differs from developer release",
  );
  assert(
    rustToolchain.includes('"wasm32-unknown-unknown"'),
    "rust-toolchain.toml must install wasm32-unknown-unknown",
  );

  const runtimeToolchain = readFileSync(
    projectPath("apps/game-client/runtime/rust-toolchain.toml"),
    "utf8",
  );
  const runtimeRustVersion = runtimeToolchain.match(/^channel\s*=\s*"([^"]+)"$/m)?.[1];
  assert(runtimeRustVersion, "Bevy runtime Rust toolchain lock is missing");
  const runtimeCargoLock = readFileSync(
    projectPath("apps/game-client/runtime/Cargo.lock"),
    "utf8",
  );
  const runtimeWasmBindgenVersion = runtimeCargoLock.match(
    /\[\[package\]\]\s*\r?\nname = "wasm-bindgen"\s*\r?\nversion = "([^"]+)"/,
  )?.[1];
  assert(runtimeWasmBindgenVersion, "Bevy runtime wasm-bindgen lock is missing");

  const dockerfile = readFileSync(projectPath("infra/developer.Dockerfile"), "utf8");
  for (const needle of [
    release.container.baseImage,
    `ARG RUST_VERSION=${REQUIRED_RUST_TOOLCHAIN}`,
    `ARG BEVY_RUNTIME_RUST_VERSION=${runtimeRustVersion}`,
    `ARG WASM_BINDGEN_VERSION=${runtimeWasmBindgenVersion}`,
    `ARG NPM_VERSION=${REQUIRED_NPM_VERSION}`,
    `ARG GH_VERSION=${REQUIRED_GH_VERSION}`,
    "ARG MIR2_DEVELOPER_IMAGE_REVISION=unknown",
    'org.opencontainers.image.revision="${MIR2_DEVELOPER_IMAGE_REVISION}"',
  ]) {
    assert(dockerfile.includes(needle), `developer.Dockerfile is missing lock: ${needle}`);
  }

  const entrypointRelativePath = "infra/developer-entrypoint.sh";
  assertTracked(`mir2-web3/${entrypointRelativePath}`);
  const entrypoint = readFileSync(projectPath(entrypointRelativePath), "utf8");
  for (const needle of [
    "/run/secrets/mir2_save_recovery_mac_key",
    '[[ "${save_recovery_mac_key}" =~ ^[0-9a-f]{64}$ ]]',
    'export MIR2_SAVE_RECOVERY_MAC_KEY="${save_recovery_mac_key}"',
    'exec gosu node "$@"',
  ]) {
    assert(entrypoint.includes(needle), `${entrypointRelativePath} is missing ${needle}`);
  }
  assert(
    entrypoint.indexOf("/run/secrets/mir2_save_recovery_mac_key") <
      entrypoint.indexOf('exec gosu node "$@"'),
    `${entrypointRelativePath} must import the mounted secret before dropping privileges`,
  );

  const compose = readFileSync(projectPath("infra/compose.developer.yml"), "utf8");
  for (const needle of [
    `BEVY_RUNTIME_RUST_VERSION: "${runtimeRustVersion}"`,
    `WASM_BINDGEN_VERSION: "${runtimeWasmBindgenVersion}"`,
  ]) {
    assert(compose.includes(needle), `developer Compose is missing runtime lock: ${needle}`);
  }
  for (const service of ["workspace:", "asset-fetch:", "gateway:", "web:"]) {
    assert(compose.includes(service), `developer Compose is missing ${service}`);
  }
  assert(
    compose.includes(
      "MIR2_DEVELOPER_IMAGE_REVISION: ${MIR2_DEVELOPER_IMAGE_REVISION:-unknown}",
    ),
    "developer Compose must pass the source revision into local image builds",
  );
  const workspaceSection = compose
    .split("\n  asset-fetch:", 1)[0]
    .split("\n  workspace:", 2)[1];
  assert(workspaceSection, "developer Compose workspace service could not be inspected");
  assert(
    workspaceSection.includes("../..:/workspace"),
    "developer workspace must mount the complete repository",
  );
  assert(
    !workspaceSection.includes("developer-gh-config:/home/node/.config/gh"),
    "default developer workspace must not mount GitHub authorization",
  );
  assert(
    !compose.includes("developer-gh-config:/home/node/.config/gh"),
    "developer Compose must not persist GitHub authorization inside project containers",
  );
  assert(
    !compose.includes("GH_TOKEN"),
    "developer Compose must not declare a persistent GitHub token environment",
  );
  for (const needle of [
    "source: mir2_save_recovery_mac_key",
    "target: mir2_save_recovery_mac_key",
    "file: ../.mir2-data/local-secrets/save-recovery-mac-key.hex",
  ]) {
    assert(compose.includes(needle), `developer Compose is missing secret lock: ${needle}`);
  }
  assert(
    !compose.includes("\n      MIR2_SAVE_RECOVERY_MAC_KEY:"),
    "developer Compose must not inject the save-recovery key through container configuration",
  );

  const fetcherRelativePath = "infra/developer-asset-fetch.sh";
  assertTracked(`mir2-web3/${fetcherRelativePath}`);
  const fetcher = readFileSync(projectPath(fetcherRelativePath), "utf8");
  for (const needle of [
    "Zombieliu/mir2",
    "gh release download",
    "sha256sum",
    "GitHub token was not supplied on standard input",
  ]) {
    assert(fetcher.includes(needle), `${fetcherRelativePath} is missing ${needle}`);
  }

  const bashLauncher = readFileSync(projectPath("scripts/dev.sh"), "utf8");
  const powerShellLauncher = readFileSync(projectPath("scripts/dev.ps1"), "utf8");
  const developerCompose = readFileSync(projectPath("infra/compose.developer.yml"), "utf8");
  assert(
    developerCompose.includes("node ./scripts/fetch-prebuilt-bevy-runtime.mjs &&"),
    "developer Web startup must fetch the pinned externalized Bevy runtime",
  );
  for (const [label, launcher, needles] of [
    [
      "scripts/dev.sh",
      bashLauncher,
      [
        "repos/Zombieliu/mir2/git/ref/tags/",
        'published_image}" != "ghcr.io/zombieliu/mir2-developer"',
        "DOCKER_CONFIG",
        "compose run --rm --no-deps -T asset-fetch",
        "compose run --rm --no-deps \\",
        '--user "$(id -u):$(id -g)"',
        "Pinned Bevy runtime is unavailable; rebuilding it from current source.",
        "MIR2_USE_PREBUILT_BEVY_RUNTIME=0 node apps/web/scripts/build-bevy-runtime.mjs release",
        "node apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs && node scripts/check-developer-release.mjs",
      ],
    ],
    [
      "scripts/dev.ps1",
      powerShellLauncher,
      [
        "repos/Zombieliu/mir2/git/ref/tags/",
        '$script:PublishedImage -ne "ghcr.io/zombieliu/mir2-developer"',
        "DOCKER_CONFIG",
        '"run", "--rm", "--no-deps", "-T", "asset-fetch"',
        '"--user", "node"',
        "Pinned Bevy runtime is unavailable; rebuilding it from current source.",
        "MIR2_USE_PREBUILT_BEVY_RUNTIME=0 node apps/web/scripts/build-bevy-runtime.mjs release",
        "node apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs && node scripts/check-developer-release.mjs",
      ],
    ],
  ]) {
    for (const needle of needles) {
      assert(launcher.includes(needle), `${label} is missing secure asset lock: ${needle}`);
    }
    assert(
      !launcher.includes("-e GH_TOKEN"),
      `${label} must not expose GH_TOKEN through container configuration`,
    );
  }

  const workflowChecks = [
    [
      ".github/workflows/developer-image.yml",
      ["linux/amd64:x86_64", "linux/arm64:aarch64", "developer-image-${GITHUB_SHA}"],
    ],
    [
      ".github/workflows/developer-environment.yml",
      [
        "macos-15-intel",
        "developer-environment-starter-${ACCEPTED_REVISION}",
        "Verify current-source Bevy runtime bundle",
        "npm run test:bevy-runtime-budget",
        "Restore tracked Bevy runtime release lock after source validation",
        "Verify host-private developer secret handoff",
      ],
    ],
    [
      ".github/workflows/developer-handoff.yml",
      [
        "Verify active Bevy runtime bundle",
        "Verify pinned prebuilt Bevy runtime manifest remains exact",
        "Restore current-source Bevy runtime manifest after fallback validation",
      ],
    ],
    [
      ".github/workflows/developer-full-assets.yml",
      [
        "mir2-full-assets",
        "--full-assets",
        "developer-environment-full-${ACCEPTED_REVISION}",
      ],
    ],
  ];
  for (const [relativeWorkflowPath, needles] of workflowChecks) {
    assertTracked(relativeWorkflowPath);
    const workflow = readFileSync(repositoryPath(relativeWorkflowPath), "utf8");
    for (const needle of needles) {
      assert(workflow.includes(needle), `${relativeWorkflowPath} is missing ${needle}`);
    }
    assert(
      !workflow.includes("git push --force") && !workflow.includes("git tag --force"),
      `${relativeWorkflowPath} must not overwrite acceptance evidence`,
    );
  }
  const imageWorkflow = readFileSync(
    repositoryPath(".github/workflows/developer-image.yml"),
    "utf8",
  );
  const fullAssetWorkflow = readFileSync(
    repositoryPath(".github/workflows/developer-full-assets.yml"),
    "utf8",
  );
  assertMainPushOnlyWorkflow(
    imageWorkflow,
    ".github/workflows/developer-image.yml",
  );
  assertMainPushOnlyWorkflow(
    fullAssetWorkflow,
    ".github/workflows/developer-full-assets.yml",
  );
  assertPinnedWorkflowActions(imageWorkflow, ".github/workflows/developer-image.yml");
  assertPinnedWorkflowActions(
    fullAssetWorkflow,
    ".github/workflows/developer-full-assets.yml",
  );
  assertTopLevelPermissions(
    imageWorkflow,
    {
      contents: "read",
      packages: "write",
      attestations: "write",
      "id-token": "write",
    },
    ".github/workflows/developer-image.yml",
  );
  assertTopLevelPermissions(
    fullAssetWorkflow,
    {
      contents: "read",
      packages: "read",
    },
    ".github/workflows/developer-full-assets.yml",
  );
  for (const needle of [
    'auth_home="$(mktemp -d "$RUNNER_TEMP/mir2-gh-home.XXXXXXXX")"',
    'rm -rf -- "$auth_home"',
    "trap cleanup EXIT",
    'credential_file="$auth_home/git-credentials"',
    "umask 077",
    "printf 'https://x-access-token:%s@github.com\\n' \"$GH_TOKEN\"",
    "GIT_CONFIG_COUNT=3",
    "GIT_CONFIG_KEY_0=credential.helper",
    "GIT_CONFIG_VALUE_0=",
    "GIT_CONFIG_KEY_1=credential.helper",
    'GIT_CONFIG_VALUE_1="store --file=$credential_file"',
    "DockerRootDir",
    'builder prune \\\n            --force \\\n            --filter "until=168h"',
    "::warning::Unable to prune stale dedicated-runner build cache.",
  ]) {
    assert(
      fullAssetWorkflow.includes(needle),
      `.github/workflows/developer-full-assets.yml is missing isolation: ${needle}`,
    );
  }
  for (const forbidden of [
    "gh auth setup-git",
    "git config --global",
    "git push --force",
    "git tag --force",
    "workflow_dispatch",
    "pull_request_target",
    "repository_dispatch",
  ]) {
    assert(
      !fullAssetWorkflow.includes(forbidden),
      `.github/workflows/developer-full-assets.yml contains forbidden privilege path: ${forbidden}`,
    );
  }

  record(
    "developer release",
    `Node ${REQUIRED_NODE_VERSION} / npm ${REQUIRED_NPM_VERSION} / ` +
      `Rust ${REQUIRED_RUST_TOOLCHAIN} / gh ${REQUIRED_GH_VERSION}`,
  );
}

function checkPackageLock(appRelativePath) {
  const packageRelativePath = `${appRelativePath}/package.json`;
  const lockRelativePath = `${appRelativePath}/package-lock.json`;
  assertTracked(`mir2-web3/${packageRelativePath}`);
  assertTracked(`mir2-web3/${lockRelativePath}`);

  const packageJson = readJson(projectPath(packageRelativePath), packageRelativePath);
  const packageLock = readJson(projectPath(lockRelativePath), lockRelativePath);
  const lockRoot = packageLock.packages?.[""];

  assert(packageLock.lockfileVersion === 3, `${lockRelativePath} must use lockfileVersion 3`);
  assert(lockRoot && typeof lockRoot === "object", `${lockRelativePath} has no root package record`);
  for (const key of ["name", "version", "dependencies", "devDependencies", "engines"]) {
    assertJsonEqual(lockRoot[key], packageJson[key], `${lockRelativePath} root ${key}`);
  }
  assert(
    String(packageJson.engines?.node ?? "").includes(String(REQUIRED_NODE_MAJOR)),
    `${packageRelativePath} must require Node ${REQUIRED_NODE_MAJOR}`,
  );
  assert(
    Object.keys(packageLock.packages).length > 1,
    `${lockRelativePath} contains no resolved dependency records`,
  );
  record("npm lock", `${packageJson.name} / lockfileVersion ${packageLock.lockfileVersion}`);
}

function checkLanguageAndScriptPins() {
  const cargoLockRelativePath = "mir2-web3/Cargo.lock";
  assertTracked(cargoLockRelativePath);
  const cargoLock = readFileSync(repositoryPath(cargoLockRelativePath), "utf8");
  assert(
    /^\s*version\s*=\s*4\s*$/m.test(cargoLock),
    "Cargo.lock must use Cargo lockfile version 4",
  );
  const packageCount = (cargoLock.match(/^\[\[package\]\]$/gm) ?? []).length;
  assert(packageCount > 20, `Cargo.lock has an implausible package count: ${packageCount}`);

  const webPackage = readJson(projectPath("apps/web/package.json"), "Player Web package.json");
  assert(
    webPackage.packageManager === `npm@${REQUIRED_NPM_VERSION}`,
    `Player Web must pin npm@${REQUIRED_NPM_VERSION}`,
  );

  const requiredScriptPins = [
    ["scripts/bootstrap-developer.ps1", ["Node.js 22", "1.89.0"]],
    ["scripts/start-developer.ps1", ["cargo +1.89.0", "MIR2_USE_PREBUILT_BEVY_RUNTIME"]],
    ["scripts/verify-developer-setup.ps1", ["cargo +1.89.0", "entities-starter"]],
    ["scripts/install-developer-assets.ps1", ["mir2-developer-asset-bundle", "contentHash"]],
  ];
  for (const [relativePath, needles] of requiredScriptPins) {
    assertTracked(`mir2-web3/${relativePath}`);
    const source = readFileSync(projectPath(relativePath), "utf8");
    for (const needle of needles) {
      assert(source.includes(needle), `${relativePath} is missing required pin/reference: ${needle}`);
    }
  }

  record(
    "toolchain lock",
    `Node ${REQUIRED_NODE_MAJOR} / npm ${REQUIRED_NPM_VERSION} / Rust ${REQUIRED_RUST_TOOLCHAIN}`,
  );
  record("Cargo lock", `${packageCount} packages / format 4`);
}

function checkDeveloperAssetManifest() {
  const relativePath = "config/developer-assets.json";
  assertTracked(`mir2-web3/${relativePath}`);
  const manifest = readJson(projectPath(relativePath), "developer asset manifest");

  assert(manifest.schemaVersion === 1, "developer asset schemaVersion must be 1");
  assert(
    manifest.kind === "mir2-developer-asset-bundle",
    `unexpected developer asset kind: ${manifest.kind}`,
  );
  assert(
    /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(manifest.repository),
    "developer asset repository is invalid",
  );
  assert(SHA256_PATTERN.test(manifest.contentHash), "developer asset contentHash is invalid");
  assert(
    SHA256_PATTERN.test(manifest.sourceContentHash),
    "developer asset sourceContentHash is invalid",
  );
  assert(
    manifest.releaseTag === `developer-assets-${manifest.contentHash.slice(0, 12)}`,
    "developer asset releaseTag does not match contentHash",
  );
  assert(
    manifest.destination === EXPECTED_FULL_ASSET_DESTINATION,
    `developer asset destination must be ${EXPECTED_FULL_ASSET_DESTINATION}`,
  );
  assertSafeRelativePath(manifest.destination, "developer asset destination");

  const archive = manifest.archive;
  assert(archive && typeof archive === "object", "developer asset archive metadata is missing");
  assertPlainFileName(archive.name, "developer asset archive name");
  assert(archive.format === "ustar", `unsupported developer asset format: ${archive.format}`);
  assert(Number.isSafeInteger(archive.size) && archive.size > 0, "archive size is invalid");
  assert(SHA256_PATTERN.test(archive.sha256), "archive SHA-256 is invalid");
  assert(
    archive.name === `mir2-crystal-full-pack-${manifest.contentHash.slice(0, 12)}.tar`,
    "archive name does not match contentHash",
  );

  const parts = manifest.parts;
  assert(Array.isArray(parts) && parts.length > 0, "developer asset manifest has no parts");
  assert(manifest.summary?.partCount === parts.length, "developer asset partCount is stale");

  const names = new Set();
  let totalSize = 0;
  for (const [index, part] of parts.entries()) {
    assertPlainFileName(part.name, `developer asset part ${index + 1} name`);
    assert(!names.has(part.name), `duplicate developer asset part: ${part.name}`);
    names.add(part.name);
    assert(
      part.name === `${archive.name}.part${String(index + 1).padStart(3, "0")}`,
      `developer asset part ${index + 1} is out of sequence`,
    );
    assert(Number.isSafeInteger(part.size) && part.size > 0, `${part.name} size is invalid`);
    assert(SHA256_PATTERN.test(part.sha256), `${part.name} SHA-256 is invalid`);
    totalSize += part.size;
  }
  assert(totalSize === archive.size, "developer asset part sizes do not equal archive size");

  record(
    "full asset lock",
    `${manifest.releaseTag} / ${parts.length} public-release parts / ${formatBytes(totalSize)}`,
  );
}

function checkBevyRuntimeLock() {
  const manifestRelativePath = "apps/web/lib/generated/bevy_runtime_version.json";
  assertTracked(`mir2-web3/${manifestRelativePath}`);
  assertTracked("mir2-web3/apps/web/scripts/fetch-prebuilt-bevy-runtime.mjs");
  assertTracked("mir2-web3/config/production-web-assets.json");
  const manifest = readJson(projectPath(manifestRelativePath), "Bevy runtime version manifest");
  assert(/^bevy-[a-f0-9]{16}$/.test(manifest.version), "Bevy runtime version is invalid");
  assert(
    Array.isArray(manifest.files) && manifest.files.length === 4,
    "Bevy runtime file set is incomplete",
  );

  const combined = createHash("sha256");
  const records = new Map();
  for (const record of manifest.files) {
    const normalized = assertSafeRelativePath(record.path, "Bevy runtime path");
    assert(
      normalized.startsWith("public/bevy-runtime/"),
      `Bevy runtime path is outside public/bevy-runtime: ${record.path}`,
    );
    assert(SHA256_PATTERN.test(record.sha256), `invalid Bevy runtime SHA-256: ${record.path}`);
    const absolutePath = path.resolve(WEB_ROOT, ...normalized.split("/"));
    fileInfo(absolutePath, `Bevy runtime ${record.path}`);
    assertExternalized(`mir2-web3/apps/web/${normalized}`);
    const actualHash = sha256(absolutePath);
    assert(actualHash === record.sha256, `Bevy runtime hash mismatch: ${record.path}`);
    if (record.path.endsWith(".wasm")) {
      assertWasmHeader(absolutePath, record.path);
    }
    combined.update(record.path);
    combined.update("\0");
    combined.update(record.sha256);
    combined.update("\0");
    records.set(record.path, record.sha256);
  }

  const expectedVersion = `bevy-${combined.digest("hex").slice(0, 16)}`;
  assert(manifest.version === expectedVersion, `Bevy runtime version should be ${expectedVersion}`);
  assert(
    records.has("public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js") &&
      records.has("public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm") &&
      records.has("public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js") &&
      records.has("public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime_bg.wasm"),
    "WebGPU/WebGL2 runtime variants are incomplete",
  );
  record("Bevy runtime lock", `${manifest.version} / ${manifest.files.length} hashed files`);
}

function checkStarterResources() {
  const manifestRelativePath =
    "apps/web/public/generated/crystal-packs/entities-starter/manifest.json";
  assertTracked(`mir2-web3/${manifestRelativePath}`);
  const manifest = readJson(projectPath(manifestRelativePath), "Starter entity manifest");
  assert(manifest.schemaVersion === 1, "Starter entity schemaVersion must be 1");
  assert(manifest.kind === "mir2-crystal-entity-pack", "Starter entity kind is invalid");
  assert(manifest.id === "entities-starter", "Starter entity id is invalid");
  assert(SHA256_PATTERN.test(manifest.contentHash), "Starter entity contentHash is invalid");

  const libraries = Object.values(manifest.libraries ?? {});
  const pages = manifest.pages;
  assert(libraries.length > 0, "Starter entity manifest has no libraries");
  assert(Array.isArray(pages) && pages.length > 0, "Starter entity manifest has no pages");

  const pageKeys = new Set();
  let networkBytes = 0;
  for (const [index, page] of pages.entries()) {
    assert(SHA256_PATTERN.test(page.sha256), `Starter page ${index + 1} SHA-256 is invalid`);
    assert(page.key === `sha256:${page.sha256}`, `Starter page ${index + 1} key is invalid`);
    assert(
      typeof page.imageUrl === "string" &&
        page.imageUrl.startsWith("/generated/crystal-packs/entities-starter/pages/"),
      `Starter page ${index + 1} URL is outside the Starter pack`,
    );
    const publicRelativePath = assertSafeRelativePath(
      page.imageUrl.slice(1),
      `Starter page ${index + 1} URL`,
    );
    const absolutePath = path.resolve(PUBLIC_ROOT, ...publicRelativePath.split("/"));
    const info = fileInfo(absolutePath, `Starter page ${index + 1}`);
    assertTracked(`mir2-web3/apps/web/public/${publicRelativePath}`);
    assert(info.size === page.networkBytes, `Starter page ${index + 1} networkBytes is stale`);
    assert(sha256(absolutePath) === page.sha256, `Starter page ${index + 1} hash mismatch`);
    const dimensions = readPngDimensions(absolutePath, `Starter page ${index + 1}`);
    assert(
      dimensions.width === page.width && dimensions.height === page.height,
      `Starter page ${index + 1} dimensions are stale`,
    );
    pageKeys.add(page.key);
    networkBytes += info.size;
  }

  let frameCount = 0;
  let actionCount = 0;
  for (const library of libraries) {
    actionCount += library.frameSet?.actions?.length ?? 0;
    for (const frame of (library.frames ?? []).filter(Boolean)) {
      frameCount += 1;
      assert(pageKeys.has(frame.pageKey), `Starter frame references unknown page ${frame.pageKey}`);
      if (frame.maskPageKey) {
        assert(
          pageKeys.has(frame.maskPageKey),
          `Starter frame references unknown mask page ${frame.maskPageKey}`,
        );
      }
    }
  }

  assert(manifest.summary?.libraryCount === libraries.length, "Starter libraryCount is stale");
  assert(manifest.summary?.pageCount === pages.length, "Starter pageCount is stale");
  assert(manifest.summary?.frameCount === frameCount, "Starter frameCount is stale");
  assert(manifest.summary?.actionCount === actionCount, "Starter actionCount is stale");
  assert(manifest.summary?.networkBytes === networkBytes, "Starter networkBytes is stale");

  const trackedPngs = [
    "apps/web/public/original-ui/NPC/03/0.png",
    "apps/web/public/original-map/WemadeMir2/Objects/2136.png",
  ];
  for (const relativePath of trackedPngs) {
    assertTracked(`mir2-web3/${relativePath}`);
    const absolutePath = projectPath(relativePath);
    fileInfo(absolutePath, relativePath);
    readPngDimensions(absolutePath, relativePath);
  }

  record(
    "Starter resources",
    `${libraries.length} libraries / ${frameCount} frames / ${pages.length} atlas page`,
  );
}

function formatBytes(bytes) {
  const gibibytes = bytes / 1024 ** 3;
  return `${gibibytes.toFixed(2)} GiB`;
}

function main() {
  checkRepositoryAndSubmoduleLock();
  checkDeveloperReleaseLock();
  checkPackageLock("apps/web");
  checkPackageLock("apps/admin-web");
  checkLanguageAndScriptPins();
  checkDeveloperAssetManifest();
  checkBevyRuntimeLock();
  checkStarterResources();
  console.log(
    `[developer-release] PASS: ${completedChecks.length} read-only checks completed; ` +
      "no private full asset pack or Crystal native build was required.",
  );
}

try {
  main();
} catch (error) {
  console.error(`[developer-release] FAIL: ${error.message}`);
  process.exitCode = 1;
}
