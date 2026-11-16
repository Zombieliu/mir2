/** Production R16 language smoke. Real UI only; no routes, runtime calls, or writes to game data. */
import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const codes = ["zh-CN", "zh-TW", "en", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"];
const hash = value => createHash("sha256").update(String(value)).digest("hex");
const stable = value => JSON.stringify(value, (_key, item) => typeof item === "bigint" ? String(item) : item);
const demand = (condition, name) => { if (!condition) { const error = new Error(name); error.safeName = name; throw error; } };
const safeName = value => ["Error", "TypeError", "SyntaxError", "TimeoutError", "AssertionError"].includes(value) ? value : "Error";
let currentPhase = "arguments", browser = null, page = null, options = null, state = null;
// The full forbidden set is Node-only. Browser selectors only receive IDs that
// this owner's ordinary public/private UI already displays, never service keys.
let privateValues = [], displayIdentityValues = [], fatal = null, lastView = null, firstView = null, packetSequence = 0;
let realmMatched = false, lastStartGameMatches = false, acceptedLoginMatches = false;
let logOutSuccessCount = 0, closingOwnedContext = false;
const successfulResourcePaths = new Set();
const proof = { schema: "r16-language-browser-smoke/1", startedAt: new Date().toISOString(), passed: false,
  scope: { realUi: true, originalOrdinaryOwner: true, credentialSource: "friendly/private/world/state.local.json",
    walletOpened: false, privateQrCaptured: false, gameDataEdited: false, lessonsSent: 0, economicCommands: 0,
    modelInstructionsSent: 0, fullNativeWasmTenLanguageClaim: false,
    nodeOnlyRootSecretValuesPassedToBrowserSelectors: false,
    browserMaskPolicy: "fixed-auth-fields-and-existing-display-identities-only" },
  phases: [], checks: [], languages: [], screenshots: [], errors: [],
  network: { failed: [], httpErrors: [], cancelled: [], abortedDuringClose: 0 }, console: { errorCount: 0, warningCount: 0, hashes: [] },
  commands: {}, companionReadCounts: { list: 0, view: 0 }, wireLanguages: [], unknownCommandCount: 0,
  runtimeWindows: [], hostErrors: [], packetStatuses: [], cleanup: { browserClosed: false, ordinaryLogout: false, chineseRestored: false } };
const phase = name => { currentPhase = name; proof.phases.push(name); process.stdout.write(stable({ phase: name, status: "running" }) + "\n"); };
const errorEntry = error => ({ phase: currentPhase, name: error?.safeName ?? safeName(error?.name), hash: hash(error?.stack ?? error?.message ?? error) });
const failAsync = name => { if (!fatal) { fatal = new Error(name); fatal.safeName = name; } };
const healthy = () => { if (fatal) throw fatal; };
const check = (name, detail = {}) => { healthy(); proof.checks.push({ name, passed: true, ...detail }); };
async function poll(read, predicate, name, milliseconds = 5000) {
  const until = Date.now() + milliseconds;
  while (Date.now() < until) { healthy(); const value = await read(); if (predicate(value)) return value; await sleep(100); }
  demand(false, name);
}
async function regular(path, maximum) {
  const info = await lstat(path);
  demand(info.isFile() && !info.isSymbolicLink() && info.size <= maximum, "LOCAL_SOURCE_FILE_INVALID");
  return readFile(path, "utf8");
}
function parseOptions(args) {
  const out = { origin: "http://127.0.0.1:19041", executable: "C:/Users/Administrator/AppData/Local/ms-playwright/chromium_headless_shell-1193/chrome-win/headless_shell.exe" };
  const names = { "--friendly-root": "friendlyRoot", "--repo-root": "repoRoot", "--output-dir": "outputDir", "--origin": "origin", "--browser-exe": "executable", "--continue-proof": "continueProof", "--start-phase": "startPhase", "--capture-login-gallery": "captureLoginGallery" };
  const seen = new Set();
  for (let i = 0; i < args.length; i++) {
    const key = names[args[i]], value = args[++i];
    demand(key && !seen.has(key) && typeof value === "string" && !value.startsWith("--"), "INVALID_SCRIPT_ARGUMENTS");
    seen.add(key); out[key] = value;
  }
  for (const key of ["friendlyRoot", "repoRoot", "outputDir", "executable"]) demand(isAbsolute(out[key] ?? ""), "EXPLICIT_ABSOLUTE_PATH_REQUIRED");
  for (const key of ["friendlyRoot", "repoRoot", "outputDir", "executable"]) out[key] = resolve(out[key]);
  if (out.continueProof) demand(isAbsolute(out.continueProof) && basename(out.continueProof) === "language-v10-browser-proof.json"
    && /^language-v10-browser(?:-[a-z0-9]+)*$/.test(basename(dirname(out.continueProof))), "CONTINUATION_MUST_BE_PUBLIC_LANGUAGE_PROOF");
  if (out.startPhase) demand(out.startPhase === "chinese" && out.continueProof, "CHINESE_FOCUS_REQUIRES_PRIOR_PUBLIC_EVIDENCE");
  if (out.captureLoginGallery) demand(["yes", "no"].includes(out.captureLoginGallery), "INVALID_LOGIN_GALLERY_OPTION");
  const url = new URL(out.origin);
  demand(url.origin === "http://127.0.0.1:19041" && url.pathname === "/" && !url.search && !url.hash && !url.username && !url.password, "FINAL_LOCAL_PRODUCTION_ORIGIN_REQUIRED");
  out.origin = url.origin;
  demand(relative(out.friendlyRoot, out.outputDir).startsWith(".."), "OUTPUT_MUST_BE_OUTSIDE_FRIENDLY_PACKAGE");
  return out;
}
async function localSources() {
  const privatePath = resolve(options.friendlyRoot, "private/world/state.local.json");
  const realFriendly = await realpath(options.friendlyRoot), realPrivate = await realpath(privatePath);
  demand(relative(realFriendly, realPrivate).replaceAll("\\", "/") === "private/world/state.local.json", "PRIVATE_STATE_PATH_CHANGED");
  state = JSON.parse(await regular(privatePath, 128 * 1024));
  demand(state?.schema === "mir2-r16-playable-web/1" && state.phase === "ready"
    && /^[A-Za-z0-9]{8,15}$/.test(state.owner?.accountId) && /^[A-Za-z0-9]{8,32}$/.test(state.owner?.password)
    && typeof state.owner?.characterName === "string" && Number.isSafeInteger(state.owner?.characterIndex)
    && state.owner.characterIndex >= 0 && ["archiveId", "residentId", "realmId", "worldEpoch"].every(key => typeof state.world?.[key] === "string" && state.world[key].length >= 8), "ORDINARY_OWNER_STATE_INVALID");
  privateValues = [state.owner.accountId, state.owner.password, state.owner.characterName,
    ...Object.values(state.world), ...Object.values(state.keys ?? {}), state.actor?.accountId, state.actor?.password, state.actor?.characterName].filter(value => typeof value === "string" && value.length >= 8);
  displayIdentityValues = [state.owner.characterName, state.actor?.characterName, state.world.archiveId, state.world.residentId];
  const catalogPath = resolve(options.repoRoot, "apps/web/lib/generated/r16-native-localization.json");
  const browserPath = resolve(options.repoRoot, "apps/web/lib/r16-browser-localization.ts");
  const [catalogRaw, browserRaw, keyRaw] = await Promise.all([
    regular(catalogPath, 20 * 1024 * 1024), regular(browserPath, 128 * 1024),
    regular(resolve(options.repoRoot, "apps/game-client/client-bevy/src/crystal_ui/keyboard_defaults.json"), 128 * 1024),
  ]);
  const catalog = JSON.parse(catalogRaw), ts = createRequire(resolve(options.repoRoot, "apps/web/package.json"))("typescript");
  demand(stable(catalog.languages.map(value => value.code)) === stable(codes), "NATIVE_CATALOG_ORDER_INVALID");
  const output = ts.transpileModule(browserRaw, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }, reportDiagnostics: true });
  demand(!(output.diagnostics ?? []).some(value => value.category === ts.DiagnosticCategory.Error), "BROWSER_DICTIONARY_TRANSPILE_FAILED");
  const module = { exports: {} };
  new Function("module", "exports", "require", output.outputText)(module, module.exports, () => { demand(false, "UNEXPECTED_BROWSER_DICTIONARY_RUNTIME_IMPORT"); });
  const browserText = module.exports.browserText, keys = module.exports.R16_BROWSER_TEXT_KEYS;
  demand(typeof browserText === "function" && Array.isArray(keys) && keys.length === 32, "BROWSER_DICTIONARY_CONTRACT_INVALID");
  for (const language of codes) for (const key of keys) demand(typeof browserText(language, key) === "string" && browserText(language, key).trim().length > 0, "BROWSER_DICTIONARY_TRANSLATION_MISSING");
  demand(browserText("unknown", keys[0]) === undefined && browserText("en", "unknown") === undefined, "BROWSER_DICTIONARY_UNKNOWN_FALLBACK_INVALID");
  const entries = new Map(catalog.entries.map(value => [value.key, value.values]));
  const native = (language, key) => { const text = entries.get(key)?.[codes.indexOf(language)]; demand(typeof text === "string" && text.trim(), "NATIVE_EXPECTATION_MISSING"); return text; };
  const keyboard = JSON.parse(keyRaw);
  const chord = name => {
    const binding = keyboard.find(value => value.function === name); demand(binding && binding.key !== "None", "CANONICAL_UI_KEY_MISSING");
    return [...["Control", "Alt", "Shift"].filter(value => binding[{ Control: "ctrl", Alt: "alt", Shift: "shift" }[value]] === 1), /^[A-Z]$/.test(binding.key) ? "Key" + binding.key : binding.key].join("+");
  };
  const { parseGatewayJson } = await import(pathToFileURL(resolve(options.repoRoot, "apps/ai-player/src/lossless-json.mjs")).href);
  proof.source = { nativeCatalogSha256: hash(catalogRaw), browserDictionarySha256: hash(browserRaw), browserDictionaryLanguages: codes.length, browserDictionaryEntries: keys.length };
  return { catalog, browserText, native, chord, parseGatewayJson };
}
const knownCommands = new Set(["login", "startGame", "logout", "logOut", "setLanguage", "queryBag", "queryEquipment", "queryBelt", "queryStorage", "queryFriends", "queryGuild", "queryMail", "heartbeat", "ping", "companionControl"]);
const forbiddenCommands = /^(?:newAccount|newCharacter|deleteCharacter|changePassword|passkeyLogin|walletLogin|walk|run|turn|moveTo|attack|rangeAttack|magic|useItem|equipItem|removeItem|moveItem|dropItem|pickUp|buyItem|sellItem|repairItem|tradeRequest|depositTradeItem|retrieveTradeItem|tradeGold|tradeConfirm|tradeCancel|groupInvite|groupReply|chat|stage5Command|qa\.|event\.)/i;
function observeCommand(command) {
  const type = typeof command?.type === "string" ? command.type : "unknown";
  if (forbiddenCommands.test(type)) { proof.scope.economicCommands += /Item|Gold|trade|pickUp/i.test(type) ? 1 : 0; failAsync("OUT_OF_SCOPE_UI_COMMAND_OBSERVED"); }
  if (knownCommands.has(type)) proof.commands[type] = (proof.commands[type] ?? 0) + 1; else proof.unknownCommandCount++;
  if (type === "login") acceptedLoginMatches = command.accountId === state.owner.accountId && command.password === state.owner.password;
  if (type === "startGame") lastStartGameMatches = command.characterIndex === state.owner.characterIndex;
  if (type === "setLanguage") {
    if (!["en", "zh-CN", "es", "pt-BR"].includes(command.language)) failAsync("UNSUPPORTED_RUNTIME_WIRE_LANGUAGE_OBSERVED");
    else if (!proof.wireLanguages.includes(command.language)) proof.wireLanguages.push(command.language);
  }
  if (type === "companionControl") {
    if (!["list", "view"].includes(command.operation)) failAsync("MUTATING_COMPANION_OPERATION_OBSERVED");
    else proof.companionReadCounts[command.operation]++;
  }
}
function observeEvent(event) {
  packetSequence++;
  if (event.type === "error") failAsync("GATEWAY_ERROR_OBSERVED");
  if (event.type === "realmInfo") {
    realmMatched = event.payload?.realmId === state.world.realmId && event.payload?.worldEpoch === state.world.worldEpoch;
    if (!realmMatched) failAsync("OWNER_WORLD_IDENTITY_MISMATCH");
  }
  if (event.type === "packet" && event.packet === "LogOutSuccess") logOutSuccessCount++;
  if (event.type === "packet" && ["Login", "LoginSuccess", "StartGame", "StartGameBanned", "StartGameDelay", "MapInformation", "UserInformation", "LogOutSuccess", "LogOutFailed"].includes(event.packet)) {
    const status = { phase: currentPhase, method: event.packet, sequence: packetSequence };
    if (Number.isSafeInteger(event.payload?.result)) status.status = event.payload.result;
    if (event.packet === "StartGameDelay" && Number.isSafeInteger(event.payload?.milliseconds)) status.delayMs = event.payload.milliseconds;
    proof.packetStatuses.push(status);
  }
  if (event.type === "companionResult") {
    const result = event.payload;
    if (!["list", "view"].includes(result?.operation)) { failAsync("UNEXPECTED_COMPANION_RESULT_OPERATION"); return; }
    if (result.status !== "ok") { failAsync("COMPANION_READ_NOT_OK"); return; }
    if (result.operation === "view") { lastView = result.result; if (!firstView) firstView = result.result; }
  }
}
function attachPage(target, sources, primary) {
  target.setDefaultTimeout(5000); target.setDefaultNavigationTimeout(15000);
  const locationHash = raw => { try { return hash(new URL(raw).pathname); } catch { return hash("invalid"); } };
  const method = request => ["GET", "POST", "HEAD", "OPTIONS", "PUT", "DELETE", "PATCH"].includes(request.method()) ? request.method() : "OTHER";
  target.on("pageerror", error => { proof.errors.push(errorEntry(error)); failAsync("BROWSER_PAGE_ERROR"); });
  target.on("console", message => {
    if (!["error", "warning"].includes(message.type())) return;
    proof.console[message.type() === "error" ? "errorCount" : "warningCount"]++;
    if (proof.console.hashes.length < 100) proof.console.hashes.push({ phase: currentPhase, name: message.type(), hash: hash(message.text()) });
  });
  target.on("response", response => {
    if (response.status() >= 200 && response.status() < 400) successfulResourcePaths.add(locationHash(response.url()));
    if (response.status() >= 400 && proof.network.httpErrors.length < 100) proof.network.httpErrors.push({ phase: currentPhase, method: method(response.request()), status: response.status(), pathHash: locationHash(response.url()) });
  });
  target.on("requestfailed", request => {
    if (closingOwnedContext && request.failure()?.errorText === "net::ERR_ABORTED") { proof.network.abortedDuringClose++; return; }
    if (request.failure()?.errorText === "net::ERR_ABORTED") {
      if (proof.network.cancelled.length < 100) proof.network.cancelled.push({ phase: currentPhase, method: method(request), status: "cancelled", pathHash: locationHash(request.url()) });
      return;
    }
    if (proof.network.failed.length < 100) proof.network.failed.push({ phase: currentPhase, method: method(request), status: "failed", pathHash: locationHash(request.url()), hash: hash(request.failure()?.errorText) });
  });
  target.on("websocket", socket => {
    let url; try { url = new URL(socket.url()); } catch { failAsync("INVALID_WEBSOCKET_URL"); return; }
    if (url.protocol !== "ws:" || url.hostname !== "127.0.0.1" || url.port !== "19040" || url.pathname !== "/ws" || url.search) return;
    if (!primary) return;
    socket.on("framesent", frame => { try { observeCommand(sources.parseGatewayJson(String(frame.payload))); } catch { failAsync("UNPARSEABLE_OUTBOUND_OBSERVATION"); } });
    socket.on("framereceived", frame => { try { observeEvent(sources.parseGatewayJson(String(frame.payload))); } catch { failAsync("UNPARSEABLE_INBOUND_OBSERVATION"); } });
  });
}
async function safeScreenshot(name, emptyInputs = false, allowFailure = false) {
  if (!allowFailure) healthy(); demand(await page.locator(".dubhe-wallet-qr-backdrop:visible").count() === 0, "ACTIVE_QR_SCREENSHOT_FORBIDDEN");
  if (emptyInputs) demand(await page.locator('.login-dialog input:not([type="range"])').evaluateAll(elements => elements.every(element => element.value === "")), "PUBLIC_AUTH_SCREENSHOT_REQUIRES_EMPTY_INPUTS");
  const masks = [page.locator('[data-login-auth-field],.login-input,textarea,.login-account-panel,.select-identity-panel,.select-character-slot-card .name,.select-character-name,[data-member-name]')];
  // Bounded UI copies only: keep policy/receipt/resource text readable while
  // masking the fixed archive row. Account/password selectors are fixed above.
  const displayedIdentity = new RegExp(displayIdentityValues.map(value => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|"));
  masks.push(page.locator(".r16-companion .copy,.entity-label,.entity-name,.actor-name,.hud-player-name").filter({ hasText: displayedIdentity }));
  const leaf = String(proof.screenshots.length + 1).padStart(2, "0") + "-" + name + ".png";
  const settings = { path: resolve(options.outputDir, leaf), mask: masks, maskColor: "#090909", timeout: 15000, animations: "disabled" };
  if (name.startsWith("alo-")) await page.locator(".r16-companion .panel").screenshot(settings);
  else await page.screenshot({ ...settings, fullPage: true });
  proof.screenshots.push(leaf);
}
async function chooseLanguage(language) {
  const toggle = page.locator(".r16-language-toggle:visible");
  if (await toggle.getAttribute("aria-expanded") !== "true") await toggle.click();
  await page.locator(`.r16-language-menu .language-selector-button[lang="${language}"]`).click();
  await poll(() => toggle.getAttribute("lang"), value => value === language, "LANGUAGE_SELECTION_NOT_APPLIED");
}
async function menuSnapshot(expectedLanguage, catalog) {
  const toggle = page.locator(".r16-language-toggle:visible");
  if (await toggle.getAttribute("aria-expanded") !== "true") await toggle.click();
  const snapshot = await page.locator(".language-selector.native-footer:visible").evaluate(element => {
    const menu = element.querySelector(".r16-language-menu"), toggle = element.querySelector(".r16-language-toggle");
    const rect = value => { const r = value.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
    const buttons = [...menu.querySelectorAll(".language-selector-button")].map(button => ({ code: button.lang, label: button.textContent.trim(), pressed: button.getAttribute("aria-pressed") === "true", rect: rect(button), fontFamily: getComputedStyle(button).fontFamily, fontSize: getComputedStyle(button).fontSize, fits: button.scrollWidth <= button.clientWidth + 1 }));
    return { root: rect(element), rootLogicalWidth: element.offsetWidth, menu: rect(menu), toggle: rect(toggle), viewport: { width: innerWidth, height: innerHeight }, buttons };
  });
  demand(stable(snapshot.buttons.map(row => row.code)) === stable(codes), "LANGUAGE_MENU_ORDER_MISMATCH");
  demand(stable(snapshot.buttons.map(row => row.label)) === stable(catalog.languages.map(row => row.nativeName)), "LANGUAGE_MENU_NATIVE_LABEL_MISMATCH");
  demand(snapshot.buttons.filter(row => row.pressed).length === 1 && snapshot.buttons.find(row => row.pressed).code === expectedLanguage, "LANGUAGE_MENU_ACTIVE_STATE_MISMATCH");
  demand(snapshot.rootLogicalWidth === 240 && snapshot.menu.y >= 0 && snapshot.menu.x >= 0
    && snapshot.menu.x + snapshot.menu.width <= snapshot.viewport.width + 1 && snapshot.menu.y + snapshot.menu.height <= snapshot.toggle.y + 1, "LANGUAGE_MENU_GEOMETRY_INVALID");
  for (let i = 0; i < codes.length; i++) {
    const row = snapshot.buttons[i];
    demand(row.fontFamily.includes(catalog.languages[i].fontFamily) && row.fontSize === "13px" && row.fits && row.rect.height >= 20, "LANGUAGE_MENU_FONT_OR_OVERFLOW_INVALID");
    if (i) demand(row.rect.y >= snapshot.buttons[i - 1].rect.y + snapshot.buttons[i - 1].rect.height - 1, "LANGUAGE_MENU_ROWS_OVERLAP");
  }
  return snapshot;
}
async function assertLoginLanguage(language, sources) {
  const native = key => sources.native(language, key), text = key => sources.browserText(language, key);
  demand((await page.locator(".login-dialog h1").textContent()).trim() === native("shell.login"), "LOGIN_TITLE_TRANSLATION_MISMATCH");
  demand((await page.locator('label[for="r16-login-account"]').textContent()).trim() === native("shell.account"), "ACCOUNT_CAPTION_TRANSLATION_MISMATCH");
  demand((await page.locator('label[for="r16-login-password"]').textContent()).trim() === native("shell.password"), "PASSWORD_CAPTION_TRANSLATION_MISMATCH");
  demand((await page.locator(".r16-wallet-entry").textContent()).replace(/\s*\[F8\]\s*$/, "").trim() === native("wallet.login.title"), "WALLET_TITLE_TRANSLATION_MISMATCH");
  demand(await page.locator(".r16-language-toggle").getAttribute("aria-label") === text("ui.languageSettings"), "LANGUAGE_ARIA_TRANSLATION_MISMATCH");
  const trigger = page.locator(".r16-auth-settings-trigger:visible");
  demand((await trigger.locator("span").textContent()).trim() === text("browser.settings") && (await trigger.locator("small").textContent()).trim() === text("browser.settingsDetail"), "BROWSER_SETTINGS_CAPTION_MISMATCH");
  if (await trigger.getAttribute("aria-expanded") !== "true") await trigger.click();
  const panel = page.locator(".r16-auth-browser-panel:visible"), audio = panel.locator(".audio-settings-controls");
  demand(await panel.getAttribute("aria-label") === text("browser.settings") && await audio.getAttribute("aria-label") === text("ui.audio"), "SETTINGS_OR_AUDIO_ARIA_MISMATCH");
  demand((await audio.locator(".audio-settings-title > span").textContent()).trim() === text("ui.audio"), "AUDIO_TITLE_TRANSLATION_MISMATCH");
  const mute = audio.locator(".audio-settings-mute");
  demand((await mute.textContent()).trim() === text(await mute.getAttribute("data-audio-enabled") === "true" ? "ui.muteAll" : "ui.unmuteAll"), "MUTE_CAPTION_TRANSLATION_MISMATCH");
  const sliderLabels = ["ui.masterVolume", "ui.musicVolume", "ui.effectsVolume"], sliders = audio.locator(".audio-settings-slider");
  demand(await sliders.count() === sliderLabels.length, "AUDIO_SLIDER_COUNT_MISMATCH");
  for (let i = 0; i < sliderLabels.length; i++) demand((await sliders.nth(i).locator("span").textContent()).trim() === text(sliderLabels[i]) && await sliders.nth(i).locator("input").getAttribute("aria-label") === text(sliderLabels[i]), "AUDIO_SLIDER_TRANSLATION_MISMATCH");
  const toggles = audio.locator(".audio-settings-toggle");
  demand(await toggles.count() === 2, "AUDIO_TOGGLE_COUNT_MISMATCH");
  for (let i = 0; i < 2; i++) demand((await toggles.nth(i).locator("span").textContent()).trim() === text(i ? "ui.effects" : "ui.music") && (await toggles.nth(i).locator("strong").textContent()).trim() === text(await toggles.nth(i).getAttribute("data-audio-enabled") === "true" ? "ui.on" : "ui.off"), "AUDIO_TOGGLE_TRANSLATION_MISMATCH");
  demand((await panel.locator(":scope > button").first().textContent()).trim() === text("browser.accountSecurity"), "ACCOUNT_SECURITY_TRANSLATION_MISMATCH");
  demand((await panel.locator(":scope > strong").textContent()).trim() === text("browser.signIn"), "OTHER_SIGNIN_TRANSLATION_MISMATCH");
  const signIn = panel.locator(".login-web3-actions button"), utilities = panel.locator(".login-utility-actions button");
  demand(stable((await signIn.allTextContents()).map(value => value.trim())) === stable([text("ui.passkeyLogin"), text("ui.walletLogin")]), "ALTERNATIVE_LOGIN_TRANSLATION_MISMATCH");
  demand(await utilities.count() === 2, "BROWSER_UTILITY_COUNT_MISMATCH");
  demand((await utilities.nth(0).textContent()).trim() === text("browser.accountInfo") && (await utilities.nth(1).textContent()).trim() === text("ui.quickEnter"), "BROWSER_UTILITY_TRANSLATION_MISMATCH");
  await safeScreenshot("settings-" + language, true);
  await trigger.click();
  proof.languages.push({ language, nativeLoginCaptions: true, walletEntryTitle: true, browserSettingsCaptions: true, dictionaryComplete: true });
}
async function loginUi(sources) {
  await page.locator(".login-input.account").fill(state.owner.accountId);
  await page.locator(".login-input.password").fill(state.owner.password);
  await page.locator(".login-button.ok button").click();
  await page.locator(".select-overlay").waitFor({ state: "visible" });
  demand(acceptedLoginMatches && realmMatched, "ORDINARY_LOGIN_OR_WORLD_NOT_CONFIRMED");
  const names = await page.locator(".select-character-slot-card .name").allTextContents();
  demand(names.filter(name => name === state.owner.characterName).length === 1, "ORIGINAL_OWNER_CHARACTER_NOT_UNIQUE");
  return names;
}
async function selectOwnerAndEnter() {
  const escaped = state.owner.characterName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const card = page.locator(".select-character-slot-card").filter({ hasText: new RegExp(escaped) });
  demand(await card.count() === 1 && (await card.locator(".name").textContent()).trim() === state.owner.characterName, "ORIGINAL_OWNER_CHARACTER_CARD_AMBIGUOUS");
  phase("ordinary-owner-card-selection"); await card.click(); lastStartGameMatches = false;
  phase("ordinary-owner-startgame-click");
  await page.locator(".select-action.start button").click();
  phase("ordinary-owner-startgame-screen-ack");
  const until = Date.now() + 45_000; let reportAt = 0, ready = false;
  proof.entryReadiness ??= [];
  while (Date.now() < until) {
    healthy(); const observed = await publicEntryReadiness();
    if (Date.now() >= reportAt || observed.screen === "game") {
      proof.entryReadiness.push({ phase: currentPhase, ...observed }); reportAt = Date.now() + 5000;
      process.stdout.write(stable({ phase: currentPhase, status: observed.screen === "game" ? "game-assets-pending" : "cold-entry-wait" }) + "\n");
    }
    if (observed.screen === "game" && observed.gameCanvasVisible && !observed.sceneAssetsPending) { ready = true; break; }
    await sleep(250);
  }
  demand(ready, "ORDINARY_OWNER_COLD_ENTRY_NOT_READY_AFTER_45_SECONDS");
  await poll(() => lastStartGameMatches, Boolean, "ORIGINAL_OWNER_STARTGAME_INDEX_MISMATCH");
}
async function publicEntryReadiness() {
  return page.locator("main[data-client-screen]").evaluate(element => {
    const frame = element.querySelector(".client-stage-frame"), canvas = element.querySelector("#mir2-web3-canvas");
    const visible = canvas && getComputedStyle(canvas).visibility !== "hidden" && getComputedStyle(canvas).display !== "none" && canvas.getBoundingClientRect().width > 0;
    const screen = element.getAttribute("data-client-screen");
    return { screen: ["login", "select", "game"].includes(screen) ? screen : "other", sceneAssetsPending: frame?.classList.contains("scene-assets-pending") === true,
      gameCanvasVisible: visible === true, documentVisible: document.visibilityState === "visible" };
  });
}
async function focusWorld() {
  const canvas = page.locator("#mir2-web3-canvas:visible");
  demand(await canvas.count() === 1, "WORLD_FOCUS_CANVAS_AMBIGUOUS");
  await canvas.focus();
}
async function bagStatus() {
  const raw = await page.locator("main[data-client-screen]").getAttribute("data-bag-input-status");
  if (!raw) return null;
  const value = JSON.parse(raw), status = value.status;
  return { active: value.active === true, ready: status?.ready === true, inputEnabled: status?.inputEnabled === true,
    ownerPresent: value.owner != null, hostError: value.error != null, nativeError: status?.error != null,
    revisionMatches: status ? status.appliedRevision === status.revision : false };
}
async function openReadOnlyWindows(language, sources) {
  await focusWorld(); await page.keyboard.press(sources.chord("Inventory"), { delay: 80 });
  const opened = await poll(bagStatus, value => value?.active && value.ready && value.inputEnabled && value.ownerPresent && !value.hostError && !value.nativeError, "NATIVE_BAG_NOT_READY_FOR_PRESENTATION_LOCALE");
  proof.runtimeWindows.push({ language, window: "bag", ...opened, fullTranslationClaim: false });
  await safeScreenshot("native-bag-" + language);
  await focusWorld(); await page.keyboard.press(sources.chord("Inventory"), { delay: 80 });
  await poll(bagStatus, value => value && !value.active && !value.hostError, "NATIVE_BAG_CLOSE_NOT_ACKNOWLEDGED");
  await focusWorld(); await page.keyboard.press(sources.chord("Group"), { delay: 80 });
  await page.locator('[data-r16-group-window="true"]').waitFor({ state: "visible" });
  proof.runtimeWindows.push({ language, window: "group", visible: true, fullTranslationClaim: false });
  await safeScreenshot("group-" + language);
  await focusWorld(); await page.keyboard.press(sources.chord("Group"), { delay: 80 });
  await page.locator('[data-r16-group-window="true"]').waitFor({ state: "hidden" });
}
const supplyReceipt = view => ({ status: view?.runtime?.supplyTask?.status, receiptKnown: view?.runtime?.supplyTask?.receiptKnown,
  chargedGold: view?.runtime?.supplyTask?.chargedGold, acquiredQuantity: view?.runtime?.supplyTask?.acquiredQuantity,
  returnedToAnchor: view?.runtime?.supplyTask?.returnedToAnchor });
const archiveRecords = view => stable({ feedback: view?.feedback, episodes: view?.episodes, growthRevision: view?.growthRevision, supplyReceipt: supplyReceipt(view) });
function aloAssertions(view) {
  const archive = view?.archive, party = view?.profile?.party, resources = view?.runtime?.resources, receipt = supplyReceipt(view);
  const result = { sameArchive: archive?.archiveId === state.world.archiveId, sameResident: archive?.residentId === state.world.residentId,
    sameRealm: archive?.realmId === state.world.realmId, sameWorld: archive?.worldEpoch === state.world.worldEpoch,
    originalActorCharacter: archive?.characterName === state.actor.characterName,
    policyVersion: view?.profile?.policyVersion, habitsEnabledCount: [party?.assistBeforeLoot, party?.lootOnlyAfterCombat, party?.askBeforeLeaving].filter(value => value === true).length,
    resourcesKnown: resources?.known === true && resources?.inventoryCurrent === true, gold: resources?.gold, hpPotionCount: resources?.hpPotionCount,
    supplyCompleted: receipt.status === "completed", supplyReceiptKnown: receipt.receiptKnown === true,
    supplyChargedGold: receipt.chargedGold, supplyAcquiredQuantity: receipt.acquiredQuantity, supplyReturned: receipt.returnedToAnchor === true,
    feedbackCount: Array.isArray(view?.feedback) ? view.feedback.length : 0, episodeCount: Array.isArray(view?.episodes) ? view.episodes.length : 0 };
  demand(result.sameArchive && result.sameResident && result.sameRealm && result.sameWorld && result.originalActorCharacter, "ALO_ORIGINAL_IDENTITY_MISMATCH");
  demand(result.policyVersion === 2 && result.habitsEnabledCount === 3, "ALO_ORIGINAL_POLICY_OR_HABITS_MISMATCH");
  demand(result.resourcesKnown && result.gold === 10 && result.hpPotionCount === 2, "ALO_ORIGINAL_RESOURCES_MISMATCH");
  demand(result.supplyCompleted && result.supplyReceiptKnown && result.supplyChargedGold === 40 && result.supplyAcquiredQuantity === 1, "ALO_ORIGINAL_SUPPLY_RECEIPT_MISMATCH");
  return result;
}
async function aloReadOnly(language) {
  phase("alo-entry-ready-" + language);
  let stableSince = 0;
  await poll(async () => {
    const entry = page.locator('[data-r16-companion-action="open"]');
    return await entry.count() === 1 && await entry.isVisible() && await entry.isEnabled();
  }, enabled => {
    if (!enabled) { stableSince = 0; return false; }
    if (!stableSince) stableSince = Date.now();
    return Date.now() - stableSince >= 1500;
  }, "ALO_READ_ONLY_ENTRY_NOT_STABLY_ENABLED", 15000);
  phase("alo-entry-real-click-" + language);
  const before = lastView; await page.locator('[data-r16-companion-action="open"]').click();
  phase("alo-dialog-open-ack-" + language);
  await page.locator('[data-r16-companion-open="true"]').waitFor({ state: "visible" });
  phase("alo-private-view-read-ack-" + language);
  const view = await poll(() => lastView, value => value && value !== before, "ALO_READ_ONLY_VIEW_NOT_RECEIVED");
  aloAssertions(view);
  phase("alo-policy-tab-real-click-" + language);
  await page.locator('[data-r16-companion-action="tab-0"]').click();
  if (language === "zh-CN") {
    phase("alo-chinese-policy-visible-copy");
    const body = await page.locator('.r16-companion .panel').textContent();
    demand(body.includes("培养 v2") && body.includes("组队先支援") && body.includes("战斗后再拾取") && body.includes("离队先询问"), "ALO_CHINESE_HABIT_VIEW_MISSING");
    phase("alo-chinese-policy-safe-screenshot");
    await safeScreenshot("alo-policy-zh-CN");
    phase("alo-chinese-supply-tab");
    await page.locator('[data-r16-companion-action="tab-4"]').click();
    await safeScreenshot("alo-original-supply-zh-CN");
    phase("alo-chinese-resources-tab");
    await page.locator('[data-r16-companion-action="tab-5"]').click();
    await safeScreenshot("alo-resources-zh-CN");
    await page.locator('[data-r16-companion-action="tab-0"]').click();
  }
  phase("alo-final-real-read-only-refresh-" + language);
  const refreshBefore = lastView;
  await page.locator('[data-r16-companion-action="refresh"]').click();
  const finalView = await poll(() => lastView, value => value && value !== refreshBefore, "ALO_FINAL_READ_ONLY_REFRESH_NOT_ACKNOWLEDGED");
  const result = aloAssertions(finalView);
  demand(archiveRecords(firstView) === archiveRecords(finalView), "ALO_ARCHIVE_OR_SUPPLY_CHANGED_DURING_READ_ONLY_SMOKE");
  const baselineCalls = firstView?.runtime?.modelCalls, currentCalls = finalView?.runtime?.modelCalls;
  const callsKnown = Number.isSafeInteger(baselineCalls) && Number.isSafeInteger(currentCalls);
  if (callsKnown) demand(currentCalls === baselineCalls, "ALO_MODEL_CALL_COUNT_CHANGED_DURING_READ_ONLY_SMOKE");
  proof.alo = { ...result, originalRecordsUnchanged: true, modelCallCounterKnown: callsKnown, modelCallCounterUnchanged: callsKnown ? currentCalls === baselineCalls : null };
  await page.locator('[data-r16-companion-action="close"]').click();
  await page.locator('[data-r16-companion-open="true"]').waitFor({ state: "hidden" });
}
async function logoutUi(sources) {
  const before = logOutSuccessCount; await focusWorld(); await page.keyboard.press(sources.chord("Logout"), { delay: 80 });
  await poll(() => logOutSuccessCount, value => value > before, "ORDINARY_LOGOUT_NOT_ACKNOWLEDGED");
  await page.locator(".login-overlay").waitFor({ state: "visible" });
}
async function clearLoginScreenshots() {
  phase("clear-input-public-language-screenshots");
  await page.locator(".login-input.account").fill(""); await page.locator(".login-input.password").fill("");
  await chooseLanguage("zh-CN");
  const toggle = page.locator(".r16-language-toggle"); await toggle.click();
  await safeScreenshot("clear-native-ten-language-menu", true); await toggle.click();
  for (const language of ["zh-CN", "hi", "ar"]) {
    await chooseLanguage(language); const trigger = page.locator(".r16-auth-settings-trigger");
    await trigger.click(); await safeScreenshot("clear-browser-settings-" + language, true); await trigger.click();
  }
  await chooseLanguage("zh-CN");
}
async function run() {
  options = parseOptions(process.argv.slice(2));
  await mkdir(options.outputDir, { recursive: false });
  phase("local-public-expectations-and-private-owner");
  const sources = await localSources();
  const executedSource = await regular(fileURLToPath(import.meta.url), 128 * 1024);
  const maskBody = executedSource.slice(executedSource.indexOf("async function safeScreenshot("), executedSource.indexOf("async function chooseLanguage("));
  const selectorBoundary = !maskBody.includes("privateValues") && !maskBody.includes("state.keys") && !maskBody.includes('page.locator("main *")')
    && executedSource.includes("displayIdentityValues = [state.owner.characterName, state.actor?.characterName, state.world.archiveId, state.world.residentId]");
  demand(selectorBoundary, "NODE_ONLY_FORBIDDEN_SET_MUST_NOT_ENTER_BROWSER_SELECTOR_CODE");
  proof.source.executedDriverSha256 = hash(executedSource);
  proof.scriptSafetyReview = { nodeOnlyForbiddenSetAbsentFromMaskCode: true, displayIdentityWhitelist: true, noMainDescendantWildcard: true,
    previousRunSafetyClaimInherited: false };
  let prior = null;
  if (options.continueProof) {
    const priorRaw = await regular(options.continueProof, 256 * 1024); prior = JSON.parse(priorRaw);
    const expected = ["ten-complete-browser-dictionaries", "fresh-browser-zh-CN-default-and-native-menu-order-font-geometry", "all-ten-login-wallet-title-and-browser-settings-ui", "russian-preference-persists-through-real-reload", "fresh-browser-zh-HK-defaults-to-zh-TW"];
    demand(prior.schema === proof.schema && prior.source?.nativeCatalogSha256 === proof.source.nativeCatalogSha256 && prior.source?.browserDictionarySha256 === proof.source.browserDictionarySha256
      && stable(prior.languages?.map(row => row.language)) === stable(codes) && expected.every(name => prior.checks?.some(row => row.name === name && row.passed === true)), "PUBLIC_CONTINUATION_EVIDENCE_INCOMPLETE_OR_DIFFERENT_BUILD");
    proof.previousEvidence = { directory: basename(dirname(options.continueProof)), proofSha256: hash(priorRaw), runPassed: prior.passed === true,
      successfulChecks: expected, screenshotCount: prior.screenshots?.length ?? 0 };
    proof.languages = prior.languages; proof.menu = prior.menu;
    for (const name of expected) check(name, { evidence: "previous-public-proof" });
    if (options.startPhase === "chinese") {
      const name = "russian-presentation-native-bag-group-and-companion-with-legal-wire-fallback";
      demand(prior.checks.some(row => row.name === name && row.passed === true) && prior.alo?.sameArchive === true && prior.alo?.originalRecordsUnchanged === true, "CHINESE_FOCUS_REQUIRES_RUSSIAN_NATIVE_AND_ORIGINAL_ARCHIVE_PROOF");
      proof.previousEvidence.successfulChecks.push(name); proof.runtimeWindows = prior.runtimeWindows;
      check(name, { evidence: "previous-public-proof" });
    }
  } else check("ten-complete-browser-dictionaries", { languages: 10, entriesPerLanguage: 32 });
  const { chromium } = require(resolve(here, "node_modules/playwright"));
  phase("fresh-zh-CN-login");
  browser = await chromium.launch({ executablePath: options.executable, headless: true, args: ["--no-sandbox", "--disable-dev-shm-usage"] });
  const context = await browser.newContext({ locale: "zh-CN", viewport: { width: 1280, height: 960 }, deviceScaleFactor: 1 });
  page = await context.newPage(); attachPage(page, sources, true);
  await page.goto(options.origin, { waitUntil: "domcontentloaded", timeout: 15000 });
  await page.locator('.r16-language-toggle[lang="zh-CN"]').waitFor({ state: "visible" });
  if (!prior) {
  const firstMenu = await menuSnapshot("zh-CN", sources.catalog); proof.menu = firstMenu;
  await safeScreenshot("native-ten-language-menu", true); await page.locator(".r16-language-toggle").click();
  check("fresh-browser-zh-CN-default-and-native-menu-order-font-geometry");
  for (const language of codes) {
    phase("login-and-browser-settings-" + language); await chooseLanguage(language); await assertLoginLanguage(language, sources);
  }
  check("all-ten-login-wallet-title-and-browser-settings-ui");
  phase("russian-preference-reload"); await chooseLanguage("ru");
  await page.reload({ waitUntil: "domcontentloaded", timeout: 15000 });
  await page.locator('.r16-language-toggle[lang="ru"]').waitFor({ state: "visible" });
  await safeScreenshot("russian-preference-after-reload", true); check("russian-preference-persists-through-real-reload");
  phase("fresh-zh-HK-traditional-default");
  const hk = await browser.newContext({ locale: "zh-HK", viewport: { width: 1280, height: 960 }, deviceScaleFactor: 1 });
  const hkPage = await hk.newPage(); attachPage(hkPage, sources, false);
  await hkPage.goto(options.origin, { waitUntil: "domcontentloaded", timeout: 15000 });
  await hkPage.locator('.r16-language-toggle[lang="zh-TW"]').waitFor({ state: "visible" });
  const mainPage = page; page = hkPage; await safeScreenshot("fresh-zh-HK-default-traditional", true); page = mainPage;
  phase("close-fresh-zh-HK-context"); closingOwnedContext = true; await hk.close(); closingOwnedContext = false;
  check("fresh-browser-zh-HK-defaults-to-zh-TW");
  }
  await chooseLanguage(options.startPhase === "chinese" ? "zh-CN" : "ru");
  phase("original-owner-ordinary-login-and-selection-languages");
  const names = await loginUi(sources);
  for (const language of options.startPhase === "chinese" ? ["zh-CN"] : ["ru", "zh-CN", "ru"]) {
    await chooseLanguage(language);
    demand(stable(await page.locator(".select-character-slot-card .name").allTextContents()) === stable(names), "CHARACTER_NAMES_CHANGED_WITH_PRESENTATION_LANGUAGE");
    demand((await page.locator(".select-scene > h1").textContent()).trim() === sources.native(language, "shell.character_select"), "CHARACTER_SELECT_TITLE_TRANSLATION_MISMATCH");
    await safeScreenshot("original-character-selection-" + language);
  }
  check("original-owner-login-and-opaque-character-names-retained");
  if (options.startPhase !== "chinese") {
  phase("russian-presentation-real-city-native-windows");
  await selectOwnerAndEnter(); await openReadOnlyWindows("ru", sources); await aloReadOnly("ru");
  demand(proof.wireLanguages.includes("en"), "RUSSIAN_PRESENTATION_LEGAL_WIRE_FALLBACK_NOT_OBSERVED");
  check("russian-presentation-native-bag-group-and-companion-with-legal-wire-fallback");
  phase("ordinary-logout-and-chinese-return"); await logoutUi(sources); await chooseLanguage("zh-CN");
  await loginUi(sources); await selectOwnerAndEnter();
  } else { phase("chinese-original-owner-cold-entry"); await selectOwnerAndEnter(); }
  phase("chinese-original-alo-read-only-records"); await aloReadOnly("zh-CN");
  check("chinese-original-alo-policy-habits-resources-and-supply-records");
  phase("final-ordinary-logout-chinese"); await logoutUi(sources);
  await chooseLanguage("zh-CN"); proof.cleanup.ordinaryLogout = true;
  proof.cleanup.chineseRestored = await page.locator(".r16-language-toggle").getAttribute("lang") === "zh-CN";
  demand(proof.cleanup.chineseRestored, "CHINESE_PREFERENCE_NOT_RESTORED");
  await safeScreenshot("final-chinese-logged-out");
  if (options.captureLoginGallery !== "no") await clearLoginScreenshots();
  check("ordinary-logout-and-chinese-preference-restored");
  const sessions = options.startPhase === "chinese" ? 1 : 2;
  demand(proof.scope.economicCommands === 0 && (proof.commands.newAccount ?? 0) === 0 && (proof.commands.login ?? 0) === sessions && (proof.commands.startGame ?? 0) === sessions && logOutSuccessCount === sessions, "UI_SCOPE_OR_ORDINARY_SESSION_COUNTS_MISMATCH");
  healthy();
  for (const entry of proof.network.cancelled) entry.successfulResponseObserved = successfulResourcePaths.has(entry.pathHash);
  demand(proof.errors.length === 0 && proof.console.errorCount === 0 && proof.network.httpErrors.length === 0 && proof.network.failed.length === 0, "BROWSER_ERROR_OR_FAILED_RESOURCE_OBSERVED");
  proof.passed = true; phase("complete");
}
try { await run(); }
catch (error) {
  proof.errors.push(errorEntry(error));
  if (page && !page.isClosed()) {
    try { proof.failureUi = { ...await publicEntryReadiness(), bag: await bagStatus() }; } catch (readError) { proof.errors.push(errorEntry(readError)); }
    try { proof.failureCompanionUi = await page.locator(".r16-companion").evaluate(element => ({ open: element.getAttribute("data-r16-companion-open") === "true",
      entryPresent: !!element.querySelector('[data-r16-companion-action="open"]'), entryDisabled: element.querySelector('[data-r16-companion-action="open"]')?.disabled ?? null,
      dialogPresent: !!element.querySelector('[role="dialog"]'), busy: element.querySelector('[role="dialog"]')?.getAttribute("aria-busy") === "true" })); } catch (readError) { proof.errors.push(errorEntry(readError)); }
    try { await safeScreenshot("failure-safe-masked", false, true); } catch (captureError) { proof.errors.push(errorEntry(captureError)); }
  }
  process.exitCode = 1;
} finally {
  if (browser) { closingOwnedContext = true; try { await browser.close(); proof.cleanup.browserClosed = true; } catch (error) { proof.errors.push(errorEntry(error)); proof.passed = false; process.exitCode = 1; } }
  proof.finishedAt = new Date().toISOString();
  if (proof.errors.length) proof.passed = false;
  if (options?.outputDir) {
    try {
      const output = JSON.stringify(proof, null, 2) + "\n";
      demand(!privateValues.some(value => output.includes(value)), "PRIVATE_VALUE_WOULD_ENTER_PUBLIC_PROOF");
      await writeFile(resolve(options.outputDir, "language-v10-browser-proof.json"), output, { flag: "wx" });
    } catch (error) { process.exitCode = 1; process.stdout.write(stable(errorEntry(error)) + "\n"); }
  }
  process.stdout.write(stable({ phase: currentPhase, status: proof.passed ? "passed" : "failed", checks: proof.checks.length, screenshots: proof.screenshots.length, errors: proof.errors.map(({ phase, name, hash }) => ({ phase, name, hash })) }) + "\n");
}
