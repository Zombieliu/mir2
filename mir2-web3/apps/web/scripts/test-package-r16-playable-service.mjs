import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, readdir, realpath, rename, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";
import { assertPreparedNextGateway, collectPlayableModules, packagePlayableService, packageRelativePath, parsePackageArguments, planPlayablePackage, playableLauncherFiles, R16_SUPPLEMENTAL_UI } from "./package-r16-playable-service.mjs";

const repo = fileURLToPath(new URL("../../../", import.meta.url));
const windows = process.platform === "win32";
const expectedGateway = "ws://127.0.0.1:19040/ws";
const preparedServer = "const nextConfig = " + JSON.stringify({ env: { NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: expectedGateway } }) + ";\n";
async function temporary(t) {
  const root = await mkdtemp(join(tmpdir(), "r16-package-test-"));
  t.after(async () => {
    const canonical = await realpath(root), parent = await realpath(tmpdir());
    assert.equal(dirname(canonical), parent); assert.ok(basename(canonical).startsWith("r16-package-test-"));
    await rm(canonical, { recursive: true });
  });
  return root;
}
async function inputs(t) {
  const root = await temporary(t), webPackageRoot = join(root, "prepared-next"), gatewayExe = join(root, "synthetic-gateway.exe");
  await mkdir(join(webPackageRoot, "apps/web/.next/server"), { recursive: true });
  await mkdir(join(webPackageRoot, "apps/web/public"));
  await writeFile(join(webPackageRoot, "apps/web/server.js"), "// Synthetic already-built entry; never executed by tests.\n" + preparedServer);
  await writeFile(join(webPackageRoot, "apps/web/.next/server/page.js"), "module.exports = {};\n");
  await writeFile(join(webPackageRoot, "apps/web/public/favicon.ico"), "synthetic-public-icon");
  await writeFile(join(webPackageRoot, "apps/web/public/empty-asset.txt"), "");
  await writeFile(gatewayExe, "MZsynthetic-gateway-never-executed");
  return { root, options: { webPackageRoot, serverRelative: "apps/web/server.js", gatewayExe, nodeExe: process.execPath,
    output: join(root, "portable space ' & 阿洛"), modelSettingsBundle: join(root, "nonexistent-machine-only-bundle") } };
}
const hash = bytes => createHash("sha256").update(bytes).digest("hex");

test("prepared Next Gateway reads a full static config without executing its surrounding server code", () => {
  const source = 'globalThis.__r16PackageGatewayCanary = "must-not-run";\n'
    + 'const nextConfig = ' + JSON.stringify({ env: { NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: expectedGateway },
      distDir: "./.next", experimental: { values: [null, true, false, -1, 0, "static"] } }) + ';\n'
    + 'throw new Error("server entry must never execute");';
  assert.equal(assertPreparedNextGateway(source), expectedGateway);
  assert.equal(globalThis.__r16PackageGatewayCanary, undefined);
  assert.equal(assertPreparedNextGateway(preparedServer), expectedGateway);
});

test("missing wrong dynamic or duplicate prepared Next Gateway config fails before output or executable inspection", async t => {
  const invalid = [
    "// nextConfig missing\n",
    'const nextConfig = {};',
    'const nextConfig = {env:{}};',
    'const nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:""}};',
    'const nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:7110/ws"}};',
    'const nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"wss://foreign.example/ws"}};',
    'const nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:process.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL}};',
    'const nextConfig = process.env.NEXT_CONFIG;',
    preparedServer + preparedServer,
    'const nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:19040/ws",NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:7110/ws"}};',
    'const nextConfig = {env:{},env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:19040/ws"}};',
    'let nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:19040/ws"}};',
    '{ const nextConfig = {env:{NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:19040/ws"}}; }',
    'const nextConfig = {env:{...process.env,NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:"ws://127.0.0.1:19040/ws"}};',
  ];
  const { root, options } = await inputs(t);
  for (const [index, source] of invalid.entries()) {
    await writeFile(join(options.webPackageRoot, options.serverRelative), source);
    const output = join(root, "rejected-package-" + index);
    // A nonexistent Node proves the build URL is checked before --version,
    // module/resource traversal, pinning, mkdir or any package copy.
    await assert.rejects(packagePlayableService({ ...options, output, nodeExe: join(root, "must-not-inspect-node.exe") }), /Prepared Next.*Gateway/, String(index));
    await assert.rejects(readdir(output), { code: "ENOENT" }, "no partial output directory for " + index);
  }
});

test("CLI requires explicit absolute package inputs and a closed relative server entry", () => {
  const root = resolve(tmpdir(), "r16-cli"), args = ["--web-package-root", join(root, "next"), "--server-relative", "apps/web/server.js",
    "--gateway-exe", join(root, "gateway.exe"), "--node-exe", join(root, "node.exe"), "--output", join(root, "new")];
  assert.equal(parsePackageArguments(args).serverRelative, "apps/web/server.js");
  assert.equal(parsePackageArguments([...args, "--model-settings-bundle", join(root, "machine-only")]).modelSettingsBundle, join(root, "machine-only"));
  for (const bad of [[], args.slice(0, -2), [...args, "--output", join(root, "other")], [...args, "--unknown", root], [...args.slice(0, -1), "relative"]]) assert.throws(() => parsePackageArguments(bad));
  for (const entry of ["../server.js", "/server.js", "C:/server.js", "apps/../server.js", "apps\\web\\server.js", "apps//server.js", "apps/web/server.mjs", "apps/CON/server.js", "apps/web./server.js"]) assert.throws(() => packageRelativePath(entry, { server: true }));
});

test("the real runtime closure contains Actor/model/supply/life modules and excludes local fixtures or administration CLIs", async () => {
  const files = await collectPlayableModules();
  for (const path of ["apps/web/scripts/r16-next-network.mjs", "apps/ai-player/src/relay-http-transport.mjs",
    "apps/ai-player/src/resident-actor.mjs", "apps/ai-player/src/model/http-transport.mjs", "apps/ai-player/src/local-model-settings.mjs",
    "apps/ai-player/src/supply/supply-controller.mjs", "apps/ai-player/src/life/life-controller.mjs", "apps/ai-player/src/gifts/gift-controller.mjs", "apps/web/scripts/quest-agent/policy.mjs"]) assert.ok(files.includes(path), path);
  assert.ok(files.every(path => path.endsWith(".mjs") && !/tests|fixtures|\.data|local-playtest|(?:^|\/)(?:main|[^/]+-cli)\.mjs/.test(path)));
});

test("computed imports and modules outside the runtime allowlist cannot enter a package", async t => {
  const root = await temporary(t), scripts = join(root, "apps/web/scripts"); await mkdir(scripts, { recursive: true });
  await writeFile(join(scripts, "r16-playable-service.mjs"), "await import(process.env.PACKAGE_TARGET);\n");
  await assert.rejects(collectPlayableModules(root), /Computed runtime import/);
  await writeFile(join(scripts, "r16-playable-service.mjs"), "import './fixtures/secret.mjs';\n");
  await assert.rejects(collectPlayableModules(root), /Unexpected Actor runtime/);
});

test("the import closure ignores literal Node preload flags and import-looking strings without weakening computed-load rejection", async t => {
  const root = await temporary(t), scripts = join(root, "apps/web/scripts"); await mkdir(scripts, { recursive: true });
  await writeFile(join(scripts, "r16-playable-service.mjs"), 'const args = ["--import", "native-preload.mjs"]; const diagnostic = "import(./fixtures/private.mjs)";\n');
  await writeFile(join(scripts, "r16-next-network.mjs"), 'export const preload = true;\n');
  assert.deepEqual(await collectPlayableModules(root), ["apps/web/scripts/r16-next-network.mjs", "apps/web/scripts/r16-playable-service.mjs"]);
  await writeFile(join(scripts, "r16-playable-service.mjs"), 'const args = ["--import"]; await import(args[0]);\n');
  await assert.rejects(collectPlayableModules(root), /Computed runtime import/);
});

test("an existing output containing old state is never changed", { skip: !windows }, async t => {
  const { options } = await inputs(t); await mkdir(options.output); await writeFile(join(options.output, "state.local.json"), "old-state-preserved");
  await assert.rejects(planPlayablePackage(options), /already exists/);
  assert.equal(await readFile(join(options.output, "state.local.json"), "utf8"), "old-state-preserved");
});

test("source/output overlap and escaped server locations fail before creating an artifact", { skip: !windows }, async t => {
  const { options } = await inputs(t);
  await assert.rejects(planPlayablePackage({ ...options, output: join(options.webPackageRoot, "nested") }), /separate/);
  await assert.rejects(planPlayablePackage({ ...options, output: join(repo, "never-created-package") }), /separate/);
  await assert.rejects(planPlayablePackage({ ...options, serverRelative: "../server.js" }), /package/);
  await assert.rejects(lstatAbsent(options.output));
});
async function lstatAbsent(path) { await readFile(join(path, "package-manifest.json")); }

test("private build inputs are refused rather than silently copied into the prepared Web tree", { skip: !windows }, async t => {
  const { options } = await inputs(t), secret = join(options.webPackageRoot, "apps/web/.env.production");
  await writeFile(secret, "OPENAI_API_KEY=synthetic-key-never-copied\n");
  await assert.rejects(planPlayablePackage(options), /Private material/);
  assert.equal(await readFile(secret, "utf8"), "OPENAI_API_KEY=synthetic-key-never-copied\n");
  await assert.rejects(lstatAbsent(options.output));
});

test("prepared Next links are refused without traversing their targets", { skip: !windows }, async t => {
  const { root, options } = await inputs(t), outside = join(root, "do-not-copy");
  await mkdir(outside); await writeFile(join(outside, "state.local.json"), "synthetic-private-state");
  await symlink(outside, join(options.webPackageRoot, "linked-outside"), "junction");
  await assert.rejects(planPlayablePackage(options), /links/);
  assert.equal(await readFile(join(outside, "state.local.json"), "utf8"), "synthetic-private-state");
});

test("a moved package imports the complete real runtime and reads navigation locally without starting any service", { skip: !windows, timeout: 90_000 }, async t => {
  const { root, options } = await inputs(t), manifest = await packagePlayableService(options);
  assert.equal(manifest.privateRelative, "private/world"); assert.equal(manifest.privateExcludedFromDistribution, true);
  assert.equal(manifest.defaults.withAlo, true); assert.equal(manifest.defaults.modelProvider, "rules");
  assert.equal(manifest.nodeVersion, "22.18.0"); assert.equal(manifest.undiciVersion, "7.30.1"); assert.equal(manifest.modelSettings.copied, false);
  assert.deepEqual(await readdir(join(options.output, "private")), []);
  assert.ok(manifest.files.every(file => !file.path.startsWith("private/") && !/\.local\.|fixtures|tests\//.test(file.path)));
  assert.ok(!manifest.files.some(file => file.path.startsWith("apps/ai-player/node_modules/undici/docs/") || file.path.startsWith("apps/ai-player/node_modules/undici/scripts/")));
  for (const asset of R16_SUPPLEMENTAL_UI) assert.equal(manifest.files.filter(file => file.path === "web/apps/web/public/" + asset).length, 1, asset);
  for (const entry of manifest.files) {
    const bytes = await readFile(join(options.output, entry.path)); assert.equal(bytes.length, entry.bytes, entry.path); assert.equal(hash(bytes), entry.sha256, entry.path);
  }
  assert.equal(await readFile(join(options.output, "web/apps/web/public/favicon.ico"), "utf8"), "synthetic-public-icon");
  const packaged = await readFile(join(options.output, "package-manifest.json"), "utf8"); assert.ok(!packaged.includes(repo));
  const moved = join(root, "moved delivery ' & 阿洛"); await rename(options.output, moved);
  const urls = ["apps/web/scripts/r16-playable-service.mjs", "apps/web/scripts/r16-next-network.mjs", "apps/ai-player/src/relay-http-transport.mjs",
    "apps/ai-player/src/resident-actor.mjs", "apps/ai-player/src/local-model-settings.mjs",
    "apps/ai-player/src/model/http-transport.mjs", "apps/ai-player/src/skills/navigation.mjs"].map(path => pathToFileURL(join(moved, path)).href);
  const code = "const nativeFetch = globalThis.fetch; const modules = await Promise.all(" + JSON.stringify(urls) + ".map(url => import(url))); if (globalThis.fetch !== nativeFetch) throw new Error('ordinary import must be inert'); const nav = modules.at(-1); for (const map of ['0', '0141']) { const grid = await nav.loadClientCollision(map); if (grid.mapFileName !== map || grid.width < 1 || grid.blocked.size < 1) throw new Error('navigation closure'); } console.log('offline-import-and-navigation-ok');";
  const environment = Object.fromEntries(Object.entries(process.env).filter(([name]) => ["SYSTEMROOT", "WINDIR", "TEMP", "TMP"].includes(name.toUpperCase())));
  const loaded = spawnSync(join(moved, "bin/node.exe"), ["--input-type=module", "--eval", code], { cwd: root, env: environment, shell: false, windowsHide: true, timeout: 30_000, encoding: "utf8" });
  assert.equal(loaded.status, 0, loaded.stderr); assert.equal(loaded.stdout.trim(), "offline-import-and-navigation-ok");
  const preloaded = spawnSync(join(moved, "bin/node.exe"), ["--import", pathToFileURL(join(moved, "apps/web/scripts/r16-next-network.mjs")).href,
    "--input-type=module", "--eval", "const r = await fetch('data:text/plain,native-pass-through'); if (await r.text() !== 'native-pass-through') throw new Error('preload replaced unrelated transport'); console.log('relay-preload-ok');"],
    { cwd: root, env: environment, shell: false, windowsHide: true, timeout: 10_000, encoding: "utf8" });
  assert.equal(preloaded.status, 0, preloaded.stderr); assert.equal(preloaded.stdout.trim(), "relay-preload-ok");
  assert.deepEqual(await readdir(join(moved, "private")), [], "inert imports never create accounts, SQLite, logs or keys");
  const parseCode = "$tokens = $null; $errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile($args[0], [ref]$tokens, [ref]$errors); if ($errors.Count) { $errors | ForEach-Object { Write-Output $_.Message }; exit 1 }";
  const parserScript = join(root, "parse-only.ps1"); await writeFile(parserScript, parseCode);
  const parsed = spawnSync("powershell.exe", ["-NoProfile", "-File", parserScript, join(moved, "Launcher.ps1")], { env: environment, shell: false, windowsHide: true, timeout: 10_000, encoding: "utf8" });
  assert.equal(parsed.status, 0, parsed.stdout + parsed.stderr);

  // Synthetic control replies exercise the real PowerShell launcher without
  // starting any Gateway/Next/Actor or opening a browser.
  const serviceStub = "import fs from 'node:fs'; const flag = name => process.argv[process.argv.indexOf(name) + 1]; const action = flag('--command'); if (!['status', 'stop'].includes(action)) throw new Error('unexpected startup'); fs.appendFileSync(new URL('../../../control-probe.txt', import.meta.url), action + '\\n'); console.log(JSON.stringify({type:'r16PlayableStatus', phase:action === 'stop' ? 'stopped' : 'running', origin:'http://127.0.0.1:19041', dataDir:flag('--data-dir')}));";
  await writeFile(join(moved, "apps/web/scripts/r16-playable-service.mjs"), serviceStub);
  const launcherFile = join(moved, "Launcher.ps1"), launcherSource = await readFile(launcherFile, "utf8");
  await writeFile(launcherFile, launcherSource.replace("function Open-PlayBrowser { Start-Process -FilePath $Origin }", "function Open-PlayBrowser { Write-Output 'browser-would-open' }"));
  for (const command of ["Start", "Start", "Status", "Stop"]) {
    const controlled = spawnSync("powershell.exe", ["-NoProfile", "-File", launcherFile, "-Command", command], { env: environment, shell: false, windowsHide: true, timeout: 10_000, encoding: "utf8" });
    assert.equal(controlled.status, 0, controlled.stdout + controlled.stderr);
    if (command === "Start") assert.match(controlled.stdout, /browser-would-open/);
  }
  assert.equal(await readFile(join(moved, "control-probe.txt"), "utf8"), "status\nstatus\nstatus\nstop\n");
  assert.deepEqual(await readdir(join(moved, "private")), [], "already-running Start never launches a second service or writes runtime state");
});

test("hidden Windows argument quoting round-trips spaces, Unicode, empty values, quotes and trailing backslashes", { skip: !windows }, async t => {
  const root = await temporary(t), files = playableLauncherFiles({ serverRelative: "web/server.js", modelSettingsBundle: "F:/optional" });
  const quoteFunction = files["Launcher.ps1"].match(/function Quote-WindowsArgument\([^]*?\r\n\}/)?.[0]; assert.ok(quoteFunction);
  const expected = ["plain", "space ' & 阿洛", "", "trailing\\", 'embedded"quote', 'slashes\\\\"quote\\'];
  await writeFile(join(root, "values.json"), JSON.stringify(expected));
  const script = quoteFunction + "\r\n$Values = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'values.json') -Raw -Encoding UTF8 | ConvertFrom-Json\r\n"
    + "$Arguments = @('--eval', 'process.stdout.write(JSON.stringify(process.argv.slice(1)))', '--') + $Values\r\n"
    + "$Quoted = @($Arguments | ForEach-Object { Quote-WindowsArgument $_ })\r\n"
    + "$Child = Start-Process -FilePath $args[0] -ArgumentList $Quoted -WorkingDirectory $PSScriptRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $PSScriptRoot 'args.json') -RedirectStandardError (Join-Path $PSScriptRoot 'stderr.txt')\r\nexit $Child.ExitCode\r\n";
  const scriptPath = join(root, "quote-only.ps1"); await writeFile(scriptPath, script);
  const checked = spawnSync("powershell.exe", ["-NoProfile", "-File", scriptPath, process.execPath], { shell: false, windowsHide: true, timeout: 10_000, encoding: "utf8" });
  assert.equal(checked.status, 0, checked.stdout + checked.stderr);
  assert.deepEqual(JSON.parse(await readFile(join(root, "args.json"), "utf8")), expected);
});

test("launchers default to Alo/rules, hide the owned service, and stop only through existing authenticated control", () => {
  const files = playableLauncherFiles({ serverRelative: "web/apps/web/server.js", modelSettingsBundle: "F:/machine-only" });
  assert.match(files["Launcher.ps1"], /Invoke-PrivateControl 'status'[\s\S]*Start-Process -FilePath \$Node/);
  assert.match(files["Launcher.ps1"], /-WindowStyle Hidden/); assert.match(files["Launcher.ps1"], /'--with-alo'/);
  assert.match(files["Launcher.ps1"], /if \(\$UseLocalModel\)[\s\S]*'--model-settings-bundle'/);
  assert.match(files["Launcher.ps1"], /Invoke-PrivateControl 'stop'/);
  assert.ok(!/Stop-Process|taskkill|Remove-Item|Get-Credential|ReadAllText.*state|OPENAI_API_KEY|GetFolderPath.*Desktop/i.test(files["Launcher.ps1"]));
  assert.ok(!files["Start.cmd"].includes("UseLocalModel")); assert.ok(files["Start-With-Local-Model.cmd"].includes("-UseLocalModel"));
  assert.equal(files["Play.url"], "[InternetShortcut]\r\nURL=http://127.0.0.1:19041/\r\n");
});
