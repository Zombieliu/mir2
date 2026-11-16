#!/usr/bin/env node
/** Packages an already-built local delivery. Never starts Gateway, Next or an Actor. */
import { spawnSync } from "node:child_process";
import { constants } from "node:fs";
import { copyFile, lstat, mkdir, open, readFile, readdir, realpath, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { basename, dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const projectRoot = fileURLToPath(new URL("../../../", import.meta.url));
const SERVICE = "apps/web/scripts/r16-playable-service.mjs";
const CONFIG = "apps/web/scripts/r16-playable-service-config.mjs";
const NEXT_NETWORK = "apps/web/scripts/r16-next-network.mjs";
const POLICY = "apps/web/scripts/quest-agent/policy.mjs";
const UNDICI = "apps/ai-player/node_modules/undici";
export const R16_SUPPLEMENTAL_UI = Object.freeze([
  ...[544, 983, 984, 985, 986, 987, 988, 1085, 1086, 1087, 1088].map(index => "original-ui/Prguse/" + index + ".png"),
  "original-ui/CArmour/00/18.png", "original-ui/CHair/00/18.png",
  "original-ui/Sound/52.wav",
]);
const slash = value => value.split(sep).join("/");
const fail = message => new Error(message);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const inside = (parent, child) => { const path = relative(parent, child); return path === "" || path !== ".." && !path.startsWith(".." + sep) && !isAbsolute(path); };

/** Read Next's build-time public endpoint as data, never by running server.js. */
export function assertPreparedNextGateway(source) {
  const reject = () => { throw fail("Prepared Next Gateway requires one static const nextConfig.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL = ws://127.0.0.1:19040/ws."); };
  const parsed = ts.createSourceFile("prepared-server.js", source, ts.ScriptTarget.ESNext, true, ts.ScriptKind.JS);
  if (parsed.parseDiagnostics.length) reject();
  const declarations = [];
  const bindsConfig = name => ts.isIdentifier(name) ? name.text === "nextConfig"
    : (ts.isObjectBindingPattern(name) || ts.isArrayBindingPattern(name))
      && name.elements.some(element => ts.isBindingElement(element) && bindsConfig(element.name));
  for (const statement of parsed.statements) {
    if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) if (bindsConfig(declaration.name))
        declarations.push({ declaration, constant: Boolean(statement.declarationList.flags & ts.NodeFlags.Const) });
    } else if ((ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) && statement.name?.text === "nextConfig") reject();
  }
  if (declarations.length !== 1 || !declarations[0].constant || !ts.isIdentifier(declarations[0].declaration.name)) reject();
  const json = node => {
    if (!node) return reject();
    if (ts.isStringLiteral(node)) return node.text;
    if (ts.isNumericLiteral(node) && Number.isFinite(Number(node.text))) return Number(node.text);
    if (node.kind === ts.SyntaxKind.TrueKeyword) return true;
    if (node.kind === ts.SyntaxKind.FalseKeyword) return false;
    if (node.kind === ts.SyntaxKind.NullKeyword) return null;
    if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.MinusToken
      && ts.isNumericLiteral(node.operand) && Number.isFinite(Number(node.operand.text))) return -Number(node.operand.text);
    if (ts.isArrayLiteralExpression(node)) return node.elements.map(json);
    if (ts.isObjectLiteralExpression(node)) {
      const value = Object.create(null), keys = new Set();
      for (const property of node.properties) {
        if (!ts.isPropertyAssignment(property) || !(ts.isIdentifier(property.name) || ts.isStringLiteral(property.name) || ts.isNumericLiteral(property.name))) reject();
        const name = property.name.text;
        if (keys.has(name)) reject();
        keys.add(name); value[name] = json(property.initializer);
      }
      return value;
    }
    return reject();
  };
  const config = json(declarations[0].declaration.initializer);
  if (config?.env?.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL !== "ws://127.0.0.1:19040/ws") reject();
  return config.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL;
}

export function packageRelativePath(value, { server = false } = {}) {
  if (typeof value !== "string" || !value || isAbsolute(value) || value.includes("\\")) throw fail("Use a slash-separated package-relative path.");
  const parts = value.split("/");
  if (parts.some(part => !part || part === "." || part === ".." || /[<>:"\\|?*\p{Cc}]/u.test(part)
    || /[. ]$/.test(part) || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part))) throw fail("Unsafe Windows package path.");
  if (server && parts.at(-1) !== "server.js") throw fail("The prepared standalone entry must be server.js.");
  return value;
}

export function parsePackageArguments(args) {
  const names = { "--web-package-root": "webPackageRoot", "--server-relative": "serverRelative", "--gateway-exe": "gatewayExe",
    "--node-exe": "nodeExe", "--output": "output", "--model-settings-bundle": "modelSettingsBundle" };
  const result = {}, seen = new Set();
  for (let index = 0; index < args.length; index++) {
    const option = args[index], value = args[++index], name = names[option];
    if (!name || seen.has(option) || typeof value !== "string" || !value || value.startsWith("--")) throw fail("Unknown, duplicate or incomplete package option.");
    seen.add(option);
    if (name === "serverRelative") result[name] = packageRelativePath(value, { server: true });
    else { if (!isAbsolute(value)) throw fail("Package inputs and output require explicit absolute paths."); result[name] = resolve(value); }
  }
  if (["webPackageRoot", "serverRelative", "gatewayExe", "nodeExe", "output"].some(key => !result[key])) throw fail("Missing a required package input.");
  return result;
}

function sensitive(path) {
  return path.split("/").some(part => ["private", ".data", ".git", ".npmrc", ".pypirc"].includes(part.toLowerCase())
    || /^\.env(?:\.|$)/i.test(part) || /\.local(?:\.|$)/i.test(part) || /\.(?:pem|key|p12|pfx|sqlite(?:-.*)?|db|log)$/i.test(part));
}
async function regularFile(path, { nonempty = true } = {}) {
  const stat = await lstat(path);
  if (!stat.isFile() || stat.isSymbolicLink() || nonempty && stat.size === 0) throw fail("A regular package input is required.");
  return stat;
}
async function pin(path) { await regularFile(path, { nonempty: false }); const bytes = await readFile(path); return { bytes: bytes.length, sha256: hash(bytes) }; }
async function mapPackageFiles(values, callback) {
  const results = new Array(values.length); let cursor = 0, failed = false;
  const settled = await Promise.allSettled(Array.from({ length: Math.min(8, values.length) }, async () => {
    while (!failed && cursor < values.length) {
      const index = cursor++;
      try { results[index] = await callback(values[index]); }
      catch (cause) { failed = true; throw cause; }
    }
  }));
  for (const result of settled) if (result.status === "rejected") throw result.reason;
  return results;
}
async function directory(path) {
  const stat = await lstat(path);
  if (!stat.isDirectory() || stat.isSymbolicLink()) throw fail("A regular package directory is required.");
}
async function filesIn(root, prefix = "") {
  await directory(resolve(root, prefix));
  const files = [];
  for (const entry of (await readdir(resolve(root, prefix), { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) {
    const path = packageRelativePath(prefix ? prefix + "/" + entry.name : entry.name);
    if (sensitive(path)) throw fail("Private material is not a distributable package input: " + path);
    const stat = await lstat(resolve(root, path));
    if (stat.isSymbolicLink()) throw fail("Materialize prepared package links before delivery: " + path);
    if (stat.isDirectory()) files.push(...await filesIn(root, path));
    else if (stat.isFile()) files.push(path);
    else throw fail("Unsupported package filesystem entry.");
  }
  return files;
}

/** Explicit Next preload plus literal import closure. Unknown/computed imports fail closed. */
export async function collectPlayableModules(root = projectRoot) {
  const pending = [NEXT_NETWORK, SERVICE], found = new Set();
  while (pending.length) {
    const path = pending.pop();
    if (found.has(path)) continue;
    if (![SERVICE, CONFIG, POLICY, NEXT_NETWORK].includes(path) && !(path.startsWith("apps/ai-player/src/") && path.endsWith(".mjs") && !sensitive(path)
      && !/(?:^|\/)(?:tests?|fixtures?|[^/]*-cli)\b/i.test(path))) throw fail("Unexpected Actor runtime module: " + path);
    packageRelativePath(path); await regularFile(resolve(root, path)); found.add(path);
    const source = await readFile(resolve(root, path), "utf8");
    // Parse actual imports: a Node '--import' argument or quoted diagnostic is
    // ordinary data, while every computed module load remains forbidden.
    const parsed = ts.createSourceFile(path, source, ts.ScriptTarget.ESNext, true, ts.ScriptKind.JS);
    if (parsed.parseDiagnostics.length) throw fail("Runtime module must parse as JavaScript.");
    const imports = new Set();
    const inspect = node => {
      if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
        if (!ts.isStringLiteral(node.moduleSpecifier)) throw fail("Computed runtime import needs an explicit package contract.");
        imports.add(node.moduleSpecifier.text);
      } else if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
        if (node.arguments.length !== 1 || !ts.isStringLiteral(node.arguments[0])) throw fail("Computed runtime import needs an explicit package contract.");
        imports.add(node.arguments[0].text);
      }
      ts.forEachChild(node, inspect);
    };
    inspect(parsed);
    for (const specifier of imports) {
      if (specifier.startsWith("node:") || specifier === "undici") continue;
      if (!specifier.startsWith(".")) throw fail("Unexpected Actor dependency: " + specifier);
      const target = resolve(dirname(resolve(root, path)), specifier);
      if (!inside(root, target)) throw fail("Runtime dependency escapes the source tree.");
      pending.push(slash(relative(root, target)));
    }
  }
  return [...found].sort();
}

function minimalEnvironment() {
  const allow = new Set(["SYSTEMROOT", "WINDIR", "SYSTEMDRIVE", "COMSPEC", "PATHEXT", "TEMP", "TMP", "USERPROFILE", "HOMEDRIVE", "HOMEPATH"]);
  return Object.fromEntries(Object.entries(process.env).filter(([name]) => allow.has(name.toUpperCase())));
}
async function windowsExecutable(path) {
  await regularFile(path); const file = await open(path, "r");
  try { const bytes = Buffer.alloc(2); await file.read(bytes, 0, 2, 0); if (bytes.toString("ascii") !== "MZ") throw fail("Windows executable input required."); }
  finally { await file.close(); }
}

export async function planPlayablePackage(options, { root = projectRoot } = {}) {
  root = await realpath(root);
  const webRoot = await realpath(options.webPackageRoot), output = resolve(options.output), outputParent = dirname(output);
  await directory(options.webPackageRoot); await directory(outputParent);
  if ((await realpath(outputParent)).toLowerCase() !== outputParent.toLowerCase()) throw fail("Output parent must be a real directory, without links.");
  for (const input of [root, webRoot, options.gatewayExe, options.nodeExe]) {
    if (inside(input, output) || inside(output, input)) throw fail("Output must be separate from every source input.");
  }
  try { await lstat(output); throw fail("Output already exists; old delivery/private data is never overwritten."); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
  const serverRelative = packageRelativePath(options.serverRelative, { server: true });
  const serverPath = resolve(webRoot, serverRelative);
  await regularFile(serverPath);
  const serverSource = await readFile(serverPath);
  assertPreparedNextGateway(serverSource.toString("utf8"));
  await windowsExecutable(options.gatewayExe); await windowsExecutable(options.nodeExe);
  const version = spawnSync(options.nodeExe, ["--version"], { cwd: dirname(options.nodeExe), env: minimalEnvironment(), shell: false, windowsHide: true, timeout: 10_000, encoding: "utf8" });
  if (version.error || version.status !== 0 || version.stdout.trim() !== "v22.18.0") throw fail("The Windows delivery requires Node 22.18.0.");
  const modules = await collectPlayableModules(root);
  const actorRoot = resolve(root, UNDICI), undici = JSON.parse(await readFile(resolve(actorRoot, "package.json"), "utf8"));
  if (undici.name !== "undici" || undici.version !== "7.30.1" || undici.main !== "index.js" || Object.keys(undici.dependencies ?? {}).length || Object.keys(undici.optionalDependencies ?? {}).length) throw fail("Pinned standalone Undici 7.30.1 required.");
  await directory(actorRoot);
  const dependencyFiles = ["index.js", "package.json", "LICENSE", ...(await filesIn(resolve(actorRoot, "lib"))).filter(path => /\.(?:js|json|wasm)$/.test(path) || /(?:^|\/)LICENSE$/.test(path)).map(path => "lib/" + path)];
  const files = [
    ...modules.map(path => ({ source: resolve(root, path), path, kind: "runtime-module" })),
    ...["crystal-map-pack/0.map.gz", "crystal-map-pack/0141.map.gz", "crystal_respawn_manifest.json"].map(path => ({ source: resolve(root, "apps/web/lib/generated", path), path: "apps/web/lib/generated/" + path, kind: "actor-navigation" })),
    ...dependencyFiles.map(path => ({ source: resolve(actorRoot, path), path: UNDICI + "/" + path, kind: "undici-7.30.1" })),
    ...(await filesIn(webRoot)).map(path => ({ source: resolve(webRoot, path), path: "web/" + path, kind: "prepared-next" })),
    { source: resolve(options.gatewayExe), path: "bin/mir2-gateway.exe", kind: "gateway" },
    { source: resolve(options.nodeExe), path: "bin/node.exe", kind: "node-22.18.0" },
  ];
  // The older thin selector predates R16's quest markers and mail lamp. Keep
  // these real UI dependencies local; two current body/hair frames also cover
  // the compatibility preloader while the primary atlas already contains them.
  // Sword swings use the real Crystal 52.wav in local combat.
  for (const publicPath of R16_SUPPLEMENTAL_UI) {
    const source = resolve(root, "apps/web/public", publicPath);
    const path = "web/" + slash(dirname(serverRelative)) + "/public/" + publicPath;
    const existing = files.find(entry => entry.path === path);
    if (existing) {
      if (JSON.stringify(await pin(existing.source)) !== JSON.stringify(await pin(source))) throw fail("R16 UI supplement differs from its source.");
    } else files.push({ source, path, kind: "r16-ui-supplement" });
  }
  const seen = new Set();
  for (const file of files) {
    packageRelativePath(file.path);
    if (seen.has(file.path.toLowerCase())) throw fail("Case-colliding package files.");
    seen.add(file.path.toLowerCase());
  }
  await mapPackageFiles(files, async file => { file.pin = await pin(file.source); });
  const pinnedServer = files.find(file => file.source === serverPath || process.platform === "win32" && file.source.toLowerCase() === serverPath.toLowerCase());
  if (!pinnedServer || pinnedServer.pin.sha256 !== hash(serverSource)) throw fail("Prepared Next Gateway configuration changed during package planning.");
  return { root, output, serverRelative: "web/" + serverRelative, modelSettingsBundle: options.modelSettingsBundle ?? null, modules, files };
}

const launcher = String.raw`param(
  [ValidateSet('Start', 'Stop', 'Status')][string]$Command = 'Start',
  [switch]$UseLocalModel
)
$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot
$Node = Join-Path $Root 'bin/node.exe'
$Service = Join-Path $Root 'apps/web/scripts/r16-playable-service.mjs'
$Data = Join-Path $Root 'private/world'
$LauncherData = Join-Path $Root 'private/launcher'
$Config = Get-Content -LiteralPath (Join-Path $Root 'launcher-config.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$Server = Join-Path $Root $Config.serverRelative
$WebRoot = [System.IO.Path]::GetDirectoryName($Server)
$Origin = 'http://127.0.0.1:19041'
function Invoke-PrivateControl([string]$Action) {
  $Process = New-Object System.Diagnostics.Process
  try {
    $Process.StartInfo.FileName = $Node
    $Process.StartInfo.Arguments = (@($Service, '--data-dir', $Data, '--command', $Action) | ForEach-Object { Quote-WindowsArgument $_ }) -join ' '
    $Process.StartInfo.UseShellExecute = $false
    $Process.StartInfo.CreateNoWindow = $true
    $Process.StartInfo.RedirectStandardOutput = $true
    $Process.StartInfo.RedirectStandardError = $true
    $Process.StartInfo.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $Process.StartInfo.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    [void]$Process.Start()
    $ReadOutput = $Process.StandardOutput.ReadToEndAsync()
    $ReadError = $Process.StandardError.ReadToEndAsync()
    $Timeout = 70000
    if ($Action -eq 'status') { $Timeout = 8000 }
    if (-not $Process.WaitForExit($Timeout)) { $Process.Kill(); return $null }
    if ($Process.ExitCode -ne 0) { return $null }
    $Raw = $ReadOutput.Result
    $Status = ($Raw -join [Environment]::NewLine) | ConvertFrom-Json
    if ($Status.type -ne 'r16PlayableStatus' -or $Status.origin -ne $Origin -or $Status.dataDir -ine $Data) { return $null }
    return $Status
  } catch { return $null }
  finally { $Process.Dispose() }
}
function Quote-WindowsArgument([string]$Value) {
  return '"' + [regex]::Replace([regex]::Replace($Value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1') + '"'
}
function Open-PlayBrowser { Start-Process -FilePath $Origin }
$Lock = $null
try {
  if ($Command -eq 'Status') {
    $Status = Invoke-PrivateControl 'status'
    if ($null -eq $Status) { Write-Host 'No responding private service. Files are preserved.'; exit 3 }
    $Status | ConvertTo-Json -Compress; exit 0
  }
  if ($Command -eq 'Stop') {
    $Status = Invoke-PrivateControl 'stop'
    if ($null -eq $Status) { Write-Host 'Private control unavailable. No process was terminated; files are preserved.'; exit 3 }
    $Status | ConvertTo-Json -Compress
    if ($Status.phase -ne 'stopped') { exit 1 }; exit 0
  }
  $Status = Invoke-PrivateControl 'status'
  if ($null -ne $Status -and $Status.phase -eq 'running') { Open-PlayBrowser; exit 0 }
  if ($null -ne $Status -and $Status.phase -notin @('starting', 'initializing')) { throw 'The existing service is stopping. Use Status before starting again.' }
  New-Item -ItemType Directory -Path $LauncherData -Force | Out-Null
  try { $Lock = [System.IO.File]::Open((Join-Path $LauncherData 'start.lock'), [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None) }
  catch { throw 'Another Start is waiting for this same delivery. Use Status; no second service was started.' }
  $Status = Invoke-PrivateControl 'status'
  $Child = $null
  if ($null -eq $Status) {
    $Arguments = @($Service, '--gateway-exe', (Join-Path $Root 'bin/mir2-gateway.exe'), '--server-entry', $Server, '--data-dir', $Data, '--web-root', $WebRoot, '--node-exe', $Node, '--with-alo')
    if ($UseLocalModel) {
      if (-not [System.IO.Path]::IsPathRooted($Config.modelSettingsBundle)) { throw 'Set an absolute machine-local model bundle reference.' }
      $Arguments += @('--model-settings-bundle', $Config.modelSettingsBundle)
    }
    $Quoted = @($Arguments | ForEach-Object { Quote-WindowsArgument $_ })
    $Child = Start-Process -FilePath $Node -ArgumentList $Quoted -WorkingDirectory $Root -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $LauncherData 'service.stdout.log') -RedirectStandardError (Join-Path $LauncherData 'service.stderr.log')
  }
  $Deadline = [DateTime]::UtcNow.AddSeconds(300)
  while ([DateTime]::UtcNow -lt $Deadline) {
    $Status = Invoke-PrivateControl 'status'
    if ($null -ne $Status -and $Status.phase -eq 'running') { Open-PlayBrowser; exit 0 }
    if ($null -ne $Status -and $Status.phase -notin @('starting', 'initializing')) { throw 'Startup did not reach running. Use Status and inspect private/launcher logs.' }
    if ($null -ne $Child -and $Child.HasExited) { throw 'Startup stopped. Inspect private/launcher logs; existing state was preserved.' }
    Start-Sleep -Milliseconds 500
  }
  throw 'Startup is still unconfirmed. Use Status or Stop; no process was forcibly terminated.'
} catch { Write-Host $_.Exception.Message; exit 1 }
finally { if ($null -ne $Lock) { $Lock.Dispose() } }
`;

export function playableLauncherFiles(plan) {
  const cmd = (command, extra = "") => "@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File \"%~dp0Launcher.ps1\" -Command " + command + extra
    + "\r\nset \"r16Exit=%errorlevel%\"\r\nif not \"%r16Exit%\"==\"0\" pause\r\nexit /b %r16Exit%\r\n";
  return {
    "Launcher.ps1": launcher.replaceAll("\n", "\r\n"),
    "Start.cmd": cmd("Start"), "Start-With-Local-Model.cmd": cmd("Start", " -UseLocalModel"), "Stop.cmd": cmd("Stop"), "Status.cmd": cmd("Status"),
    "Play.url": "[InternetShortcut]\r\nURL=http://127.0.0.1:19041/\r\n",
    "launcher-config.json": JSON.stringify({ schema: "mir2-r16-launcher/1", serverRelative: plan.serverRelative, modelSettingsBundle: plan.modelSettingsBundle }, null, 2) + "\n",
    ".gitignore": "/private/\n",
    "README.txt": ["Numeron · Windows R16 本地 Web 试玩", "",
      "1. 双击 Start.cmd，启动后浏览器会打开 http://127.0.0.1:19041/ 。默认阿洛使用规则。",
      "2. 首次启动后打开 private/world/试玩账号.txt，用其中账号、密码和角色登录。这名角色已绑定阿洛。",
      "3. 保留即时 RPG 操作；进城后从右侧「伙伴 Companion」邀请阿洛组队、教学及查看记忆。",
      "4. 退出时用 Stop.cmd，保存阿洛后再停止本包服务。下次 Start 沿用同一角色、身份和记忆。",
      "5. 有原本机模型设置时，先 Stop，再双击 Start-With-Local-Model.cmd。模型凭证不在此包或浏览器中。",
      "", "Status.cmd 查看运行状态；Play.url 打开已启动的游戏。此包已含 Node、Gateway、运行代码和本地素材，无需 npm install。",
      "本机模型设置目录由 launcher-config.json 引用，移动到另一台机器后需在那里配置模型。",
      "", "private/ 是本机账户、密钥、SQLite和日志；重启及更新时保留。不要分享、上传或递归压缩已试玩目录。",
      "分发清单为 package-manifest.json 的 files[]；打包分享时只取该清单和清单文件，排除 private/。",
      "", "本交付是完整本地资源试玩包；360 MiB 轻量发行包资格仍未达成，未发布到线上。", ""].join("\r\n"),
  };
}

export async function packagePlayableService(options, configuration) {
  const plan = await planPlayablePackage(options, configuration);
  await mkdir(plan.output); // Exclusive fresh destination, never update a played package.
  const files = await mapPackageFiles(plan.files, async entry => {
    const destination = resolve(plan.output, entry.path); await mkdir(dirname(destination), { recursive: true });
    await copyFile(entry.source, destination, constants.COPYFILE_EXCL);
    if (JSON.stringify(await pin(destination)) !== JSON.stringify(entry.pin) || JSON.stringify(await pin(entry.source)) !== JSON.stringify(entry.pin)) throw fail("Package source/output changed during copy.");
    return { path: entry.path, kind: entry.kind, ...entry.pin };
  });
  for (const [path, text] of Object.entries(playableLauncherFiles(plan))) {
    const bytes = Buffer.from(text, "utf8"); await writeFile(resolve(plan.output, path), bytes, { flag: "wx" });
    files.push({ path, kind: "launcher", bytes: bytes.length, sha256: hash(bytes) });
  }
  await mkdir(resolve(plan.output, "private"));
  const manifest = { schema: "mir2-r16-playable-package/1", createdAt: new Date().toISOString(), nodeVersion: "22.18.0", undiciVersion: "7.30.1",
    serverRelative: plan.serverRelative, serviceRelative: SERVICE, gatewayRelative: "bin/mir2-gateway.exe", nodeRelative: "bin/node.exe",
    privateRelative: "private/world", privateExcludedFromDistribution: true, archivePolicy: "Only files[] and package-manifest.json; never private/.",
    defaults: { withAlo: true, modelProvider: "rules", gatewayPort: 19040, webPort: 19041 },
    modelSettings: { copied: false, optionalMachineBundle: plan.modelSettingsBundle },
    files: files.sort((a, b) => a.path.localeCompare(b.path)) };
  await writeFile(resolve(plan.output, "package-manifest.json"), JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
  return manifest;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  Promise.resolve().then(() => packagePlayableService(parsePackageArguments(process.argv.slice(2))))
    .then(manifest => process.stdout.write(JSON.stringify({ type: "r16PlayablePackaged", files: manifest.files.length, bytes: manifest.files.reduce((sum, file) => sum + file.bytes, 0), privateDataCreated: false }) + "\n"))
    .catch(error => { process.stderr.write("R16 packaging refused: " + error.message + "\n"); process.exitCode = 1; });
}
