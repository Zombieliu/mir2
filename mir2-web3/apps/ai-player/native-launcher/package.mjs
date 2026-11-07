import { constants } from "node:fs";
import { copyFile, mkdir, readdir, realpath, writeFile, readFile, lstat } from "node:fs/promises";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { appRoot, repoRoot, sha256, R16_GAMEPLAY_BASELINE } from "../scripts/local-gateway.mjs";
import { assembleNativeFixture, verifyR16ResourceInput } from "../scripts/native-resource-input.mjs";

async function exclusiveCopy(source, destination) {
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination, constants.COPYFILE_EXCL);
}

export async function packageLocalPlaytest({ directory, nativeExecutable, gatewayExecutable, launcherExecutable, settingsExecutable, onProgress = () => {} }) {
  const branch = execFileSync("git", ["branch", "--show-current"], { cwd: repoRoot, encoding: "utf8" }).trim();
  const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).trim();
  if (branch !== "codex/windows-player-journey") throw new Error("Build this package from the canonical branch.");
  execFileSync("git", ["merge-base", "--is-ancestor", "e9117b6691e3f7ec238ff1c1eb77b699b0646c2f", "HEAD"], { cwd: repoRoot });
  if (process.versions.node !== "22.18.0") throw new Error("Package the verified Node.js 22.18.0 runtime.");
  const requested = resolve(directory);
  const root = resolve(await realpath(dirname(requested)), basename(requested));
  const difference = relative(repoRoot, root);
  const outsideSource = isAbsolute(difference) || difference === ".." || difference.startsWith("..\\") || difference.startsWith("../");
  if (!outsideSource) throw new Error("Keep package output outside its source tree.");
  const resources = await verifyR16ResourceInput({ onProgress });
  await mkdir(root); // Fresh output only, never overwrite an installation.
  const game = await assembleNativeFixture({ directory: resolve(root, "game"), nativeExecutable, resources });
  await writeFile(resolve(root, "game/locale-seed.json"), JSON.stringify({ schema: 1, locale: "zh-CN" }) + "\n", { flag: "wx" });
  await exclusiveCopy(gatewayExecutable, resolve(root, "gateway/mir2-gateway.exe"));
  await exclusiveCopy(launcherExecutable, resolve(root, "Numeron-Alo.exe"));
  if (settingsExecutable) await exclusiveCopy(settingsExecutable, resolve(root, "Numeron-Alo-Settings.exe"));
  await exclusiveCopy(process.execPath, resolve(root, "runtime/node.exe"));
  const payload = [];
  for (const item of await readdir(resources.root, { withFileTypes: true })) {
    if (item.isFile() && /^(?:OFL-.*|NotoSansTC-OFL)\.txt$/.test(item.name)) {
      const destination = resolve(root, "licenses", item.name);
      await exclusiveCopy(resolve(resources.root, item.name), destination); payload.push(destination);
    }
  }
  async function copySource(source, destination) {
    for (const item of await readdir(source, { withFileTypes: true })) {
      if ([".data", "node_modules", "target", ".git"].includes(item.name) || item.name.endsWith(".local.json")) continue;
      if (item.isSymbolicLink()) throw new Error("Unexpected link in the source package.");
      const from = resolve(source, item.name), to = resolve(destination, item.name);
      if (item.isDirectory()) { await copySource(from, to); }
      else if (item.isFile()) { await exclusiveCopy(from, to); payload.push(to); }
    }
  }
  await copySource(appRoot, resolve(root, "apps/ai-player"));
  // Only the one locked production dependency is shipped. No npm install or
  // lifecycle script runs on the player's machine, and no dev tree is copied.
  const lock = JSON.parse(await readFile(resolve(appRoot, "package-lock.json"), "utf8"));
  const undiciPath = resolve(appRoot, "node_modules/undici");
  const installed = JSON.parse(await readFile(resolve(undiciPath, "package.json"), "utf8"));
  if (lock.packages?.["node_modules/undici"]?.version !== "7.30.1" || installed.version !== "7.30.1" ||
    Object.keys(installed.dependencies ?? {}).length || (await lstat(undiciPath)).isSymbolicLink()) throw new Error("Prepare the exact locked production model transport before packaging.");
  await copySource(undiciPath, resolve(root, "apps/ai-player/node_modules/undici"));
  for (const path of ["docs/ALO-R16-MIGRATION-20261103.zh-CN.md", "docs/COMPANION-CONTROL-CONTRACT.md", "docs/ALO-OPENAI-20261103.zh-CN.md",
    "docs/NPC-ROW-ORDER-20261103.zh-CN.md", "docs/ALO-CULTIVATION-SUPPLY-20261103.zh-CN.md", "docs/ALO-AUTONOMOUS-LIFE-20261103.zh-CN.md"]) {
    await exclusiveCopy(resolve(repoRoot, path), resolve(root, path)); payload.push(resolve(root, path));
  }
  await copySource(resolve(repoRoot, "docs/evidence/alo-r16-20261103"), resolve(root, "docs/evidence/alo-r16-20261103"));
  if (settingsExecutable) await copySource(resolve(repoRoot, "docs/evidence/alo-openai-20261103"), resolve(root, "docs/evidence/alo-openai-20261103"));
  if (settingsExecutable) await copySource(resolve(repoRoot, "docs/evidence/alo-openai-live-20261103"), resolve(root, "docs/evidence/alo-openai-live-20261103"));
  await copySource(resolve(repoRoot, "docs/evidence/npc-row-order-20261103"), resolve(root, "docs/evidence/npc-row-order-20261103"));
  await copySource(resolve(repoRoot, "docs/evidence/alo-cultivation-supply-20261103"), resolve(root, "docs/evidence/alo-cultivation-supply-20261103"));
  await copySource(resolve(repoRoot, "docs/evidence/alo-autonomous-life-20261103"), resolve(root, "docs/evidence/alo-autonomous-life-20261103"));
  // The navigation policy reads only published collision data. Ship its exact
  // code and two-map client catalog, never personal ECS or server admin access.
  for (const path of ["apps/web/scripts/quest-agent/policy.mjs", "apps/web/lib/generated/crystal-map-pack/0.map.gz",
    "apps/web/lib/generated/crystal-map-pack/0141.map.gz", "apps/web/lib/generated/crystal_respawn_manifest.json"]) {
    await exclusiveCopy(resolve(repoRoot, path), resolve(root, path)); payload.push(resolve(root, path));
  }
  for (const path of ["Numeron-Alo.exe", "gateway/mir2-gateway.exe", "game/mir2-platform-windows.exe", "game/locale-seed.json", "runtime/node.exe"]) payload.push(resolve(root, path));
  if (settingsExecutable) payload.push(resolve(root, "Numeron-Alo-Settings.exe"));
  const readme = `Numeron 阿洛 · R16 本地试用\n\n双击 Numeron-Alo.exe。首次初始化后自动通过普通登录进入独立本地世界。\n打开“伙伴”邀请阿洛组队，出城冒险；在培养页教它“组队先支援、战后拾取”。\n关闭游戏后正常等待后台保存退出；再次双击仍使用同一角色、世界和记忆。\n“记忆管理”可撤销、删除，导出不需要购买服务。\n\n这是基于 R16 的本地开发试用构建，内容配置 ${R16_GAMEPLAY_BASELINE.contentProfile}。\n不是公网 R16 更新，没有接入公共测试账号、真实付款或大模型。\n包含独立 Node 22.18.0 与网关，运行日志在 apps/ai-player/.data/local-playtest/launcher.log。\n账号和签名密钥首次运行时在 .data 内生成，只用于这份本地世界；不要公开分享运行后的 .data。\n\n本机素材通过 game/mir2-assets 链接到已验证的原 R16 完整资源，原安装不改写。\n不要移动或删除该 R16 资源目录。此目录不是可在其他电脑直接解压使用的完整安装包。\n详细资源路径、哈希与源码基线见 BUILD.json / game/resource-input.json。\n\n直接运行 game/mir2-platform-windows.exe 只打开客户端；请用 Numeron-Alo.exe 启动完整试用。\n`;
  const modelInstructions = settingsExecutable ? "\n模型试用：先打开 Numeron-Alo-Settings.exe，在本机填写 OpenAI API key，默认 gpt-6-luna。保存后重启本地试用生效。\n自然语言教学：伙伴 → 培养体验 → 用自己的话教阿洛。输入反馈，生成提案，核对旧值→新值，再点击确认。确认前不生效；取消不会改变习惯。\n私人委托：伙伴 → 私人委托，发送‘在附近打两只怪，再回来找我，途中别捡东西。’。需要阿洛与你组队，怪物须在当前可见范围与六格边界内。\n新委托与生成教学提案共享一小时最多20次模型调用；确认教学不再次调用，无自动付费重试。走位、挥刀、吃药由普通游戏执行器完成。模型计划与实际参战击杀分别显示。\nkey存当前Windows用户凭证库，不在记忆/导出/游戏回包里。重启后保留经历和已确认习惯，旧任务暂停，需重新委托。\n" : "";
  const supplyInstructions = "\n独立补给：伙伴 → 独立补给。确认本次最多50游戏金币、1瓶小红药、300秒；阿洛需要先有足够游戏金币。它按普通规则走到比奇药商购买，再回到授权时主人位置旁。\n授权只限当前比奇范围。可撤销或召回；未知订单只核对原回执，不重新购买。重启保留记忆和账本，旧补给不自动恢复花费。模型调用和游戏金币分别计量。\n";
  const lifeInstructions = "\n自主生活：伙伴 → 自主生活 → 选择20或30分钟 → 查看并确认生活授权。阿洛沿比奇南侧固定路线行动，危险、缺药、钱不足或背包满会回家休息或等主人。先通过原交易窗口给它自己的金币或药；授权不产生金币。\n本次生活累计最多50游戏金币/一瓶药，重启不重置预算、不延长时间，恢复原生活需再次明确确认。已配置模型时，每次生活最多4个阶段调用，和教学/委托共享每小时20次上限。原角色、已确认习惯和共同经历继续保留。\n";
  await writeFile(resolve(root, "开始试玩.txt"), readme.replace("、真实付款或大模型", "或真实付款") + modelInstructions + supplyInstructions + lifeInstructions, { flag: "wx" });
  payload.push(resolve(root, "开始试玩.txt"), resolve(root, "game/resource-input.json"));
  const files = await Promise.all(payload.sort().map(async path => ({ path: path.slice(root.length + 1).replaceAll("\\", "/"), sha256: await sha256(path) })));
  const manifest = { schema: "mir2-alo-r16-local-package/1", createdAt: new Date().toISOString(), sourceBranch: branch, sourceBaseRevision: head,
    includesR16Ancestor: "e9117b6691e3f7ec238ff1c1eb77b699b0646c2f", uncommittedIntegration: true,
    mode: "local-development-playtest", publicDeployment: false, modelEnabled: false, modelAvailable: Boolean(settingsExecutable), defaultModel: settingsExecutable ? "gpt-6-luna" : null, paymentEnabled: false,
    gameplayBaseline: R16_GAMEPLAY_BASELINE, resourceInput: game.provenance.resourceInput,
    nativeSha256: await sha256(game.exe), gatewaySha256: await sha256(resolve(root, "gateway/mir2-gateway.exe")),
    launcherSha256: await sha256(resolve(root, "Numeron-Alo.exe")), nodeVersion: process.versions.node,
    runtimeDependencies: { undici: "7.30.1", integrity: lock.packages["node_modules/undici"].integrity },
    privateStateIncluded: false, resourceMode: "verified-R16-input-directory-junction", files };
  await writeFile(resolve(root, "BUILD.json"), JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
  return { root, launcher: resolve(root, "Numeron-Alo.exe"), native: game.exe, manifest };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2), options = {};
  const flags = { "--out": "directory", "--native-exe": "nativeExecutable", "--gateway-exe": "gatewayExecutable", "--launcher-exe": "launcherExecutable", "--settings-exe": "settingsExecutable" };
  for (let index = 0; index < args.length; index += 2) {
    const key = flags[args[index]];
    if (!key || !args[index + 1] || options[key]) throw new Error("Provide exact local package inputs.");
    options[key] = resolve(args[index + 1]);
  }
  if (!["directory", "nativeExecutable", "gatewayExecutable", "launcherExecutable"].every(key => options[key])) throw new Error("Provide --out, --native-exe, --gateway-exe and --launcher-exe.");
  packageLocalPlaytest(options).then(value => console.log(JSON.stringify({ root: value.root, launcher: value.launcher, native: value.native,
    nativeSha256: value.manifest.nativeSha256, gatewaySha256: value.manifest.gatewaySha256, launcherSha256: value.manifest.launcherSha256,
    sourceBranch: value.manifest.sourceBranch, sourceBaseRevision: value.manifest.sourceBaseRevision, verifiedResources: value.manifest.resourceInput.verifiedResourceCount }))).catch(() => {
    console.error("Local package creation stopped. Existing installs and private data were not overwritten."); process.exitCode = 1;
  });
}
