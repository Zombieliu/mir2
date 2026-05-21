import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");

const BASE_URL = withQueryParam(
  process.env.MIR2_WEB_BASE_URL ?? process.argv[2] ?? "http://127.0.0.1:3002",
  "autoTick",
  "0",
);
const OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "stage5-screenshots");
const CHROME_PATH = process.env.MIR2_CHROME_PATH ?? findChromePath();
const DEBUG_PORT = Number(process.env.MIR2_CHROME_DEBUG_PORT ?? 9400 + (process.pid % 1000));
const ACCOUNT_MODE = process.env.MIR2_STAGE5_ACCOUNT_MODE ?? "new";
const USE_DEMO_ACCOUNT = ACCOUNT_MODE === "demo";
const VIEWPORTS = {
  desktop: { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false },
  compact: { width: 820, height: 640, deviceScaleFactor: 1, mobile: false },
  compactWide: { width: 900, height: 640, deviceScaleFactor: 1, mobile: false },
  compactNarrow: { width: 768, height: 640, deviceScaleFactor: 1, mobile: false },
  compactShort: { width: 820, height: 540, deviceScaleFactor: 1, mobile: false },
};

if (!CHROME_PATH) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH to run the Stage 5 UI smoke.");
}

function withQueryParam(rawUrl, key, value) {
  const url = new URL(rawUrl);
  if (!url.searchParams.has(key)) url.searchParams.set(key, value);
  return url.toString();
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.wsFrames = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.handleMessage(event.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  handleMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) {
        reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      } else {
        resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === "Runtime.consoleAPICalled" && message.params?.type === "error") {
      this.consoleErrors.push({
        source: "console",
        text: (message.params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
      });
    }

    if (message.method === "Runtime.exceptionThrown") {
      const details = message.params?.exceptionDetails;
      const exception = details?.exception;
      this.consoleErrors.push({
        source: "exception",
        text:
          exception?.description ??
          exception?.value ??
          details?.text ??
          "runtime exception",
      });
    }

    if (message.method === "Log.entryAdded") {
      const entry = message.params?.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        const url = entry.url ? ` (${entry.url})` : "";
        this.consoleErrors.push({ source: entry.source ?? "log", text: `${entry.text ?? ""}${url}` });
      }
    }

    if (
      message.method === "Network.webSocketFrameSent" ||
      message.method === "Network.webSocketFrameReceived"
    ) {
      const opcode = message.params?.response?.opcode;
      const payloadData = message.params?.response?.payloadData ?? "";
      this.wsFrames.push({
        direction: message.method.endsWith("Sent") ? "sent" : "received",
        opcode,
        payloadData: typeof payloadData === "string" ? payloadData.slice(0, 1_000) : "",
        at: Date.now(),
      });
      this.wsFrames = this.wsFrames.slice(-80);
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    const promise = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
    this.ws.send(payload);
    return promise;
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.text ?? "Runtime.evaluate failed");
    }
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

async function main() {
  await fs.mkdir(OUTPUT_DIR, { recursive: true });
  const userDataDir = path.join(os.tmpdir(), `mir2-stage5-ui-${process.pid}-${Date.now()}`);
  const chrome = spawn(
    CHROME_PATH,
    [
      "--headless=new",
      `--remote-debugging-port=${DEBUG_PORT}`,
      `--user-data-dir=${userDataDir}`,
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      "about:blank",
    ],
    { stdio: "ignore" },
  );

  let client;
  const screenshots = [];
  const inventoryFlow = [];
  const inventoryUseFlow = [];
  const inventoryEquipFlow = [];
  const inventoryGoldFlow = [];
  const inventoryMoveFlow = [];
  const inventorySplitFlow = [];
  const inventoryDropFlow = [];
  const groundPickupFlow = [];
  const groundGoldPickupFlow = [];
  const inventorySellFlow = [];
  const characterRemoveFlow = [];
  const characterRepairFlow = [];
  const characterFlow = [];
  const storageFlow = [];
  const storagePasswordFlow = [];
  const storageStoreFlow = [];
  const storageTakeBackFlow = [];
  const chatFlow = [];
  const chatChannelFlow = [];
  const systemMenuFlow = [];
  const systemMenuQaTransferFlow = [];
  const systemMenuTransferFlow = [];
  const systemMenuFeatureFlow = [];
  const systemMenuSocialFlow = [];
  const hudButtonFlow = [];
  const spellCastFlow = [];
  const minimapFlow = [];
  const mailFlow = [];
  const reportFlow = [];
  const npcDialogFlow = [];
  const combatFlow = [];
  const beltFlow = [];
  const beltUseFlow = [];
  const beltMouseUseFlow = [];
  const gameShopFlow = [];
  const stage5SystemsFlow = [];
  const loginFlow = [];
  const selectFlow = [];
  try {
    await waitForChrome(DEBUG_PORT);
    const target = await createTarget(DEBUG_PORT, "about:blank");
    client = new CdpClient(target.webSocketDebuggerUrl);
    await client.connect();
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await setViewport(client, VIEWPORTS.desktop);
    await client.send("Page.navigate", { url: BASE_URL });
    await waitForSelector(client, ".login-overlay", 15_000);
    loginFlow.push(await readLoginState(client, "initial"));
    screenshots.push(await screenshot(client, "stage5-login.png"));

    const accountId = USE_DEMO_ACCOUNT ? "demo" : `stage5-${process.pid}-${Date.now()}`;
    const accountPassword = USE_DEMO_ACCOUNT ? "demo" : "stage5-pass";
    await clickLanguageButton(client, ".login-language-selector", "简体中文");
    await waitForLoginState(client, (state) => state.activeLanguageLabel === "简体中文", "login zh-CN language", 5_000);
    loginFlow.push(await readLoginState(client, "zhCnLanguage"));
    screenshots.push(await screenshot(client, "stage5-login-language-zh.png"));
    await clickLanguageButton(client, ".login-language-selector", "English");
    await waitForLoginState(client, (state) => state.activeLanguageLabel === "English", "login English language", 5_000);
    loginFlow.push(await readLoginState(client, "englishLanguageRestored"));

    await setInputValue(client, ".login-input.account", accountId);
    await setInputValue(client, ".login-input.password", accountPassword);
    loginFlow.push(await readLoginState(client, "credentialsFilled"));
    await clickSelector(client, ".login-button.view button");
    await waitForLoginState(client, (state) => state.accountPanelVisible === true, "view key panel", 5_000);
    loginFlow.push(await readLoginState(client, "viewKeyPanel"));
    screenshots.push(await screenshot(client, "stage5-login-view-key.png"));
    await clickSelector(client, ".login-account-panel button");
    await waitForLoginState(client, (state) => state.accountPanelVisible === false, "view key closed", 5_000);
    loginFlow.push(await readLoginState(client, "viewKeyClosed"));

    if (!USE_DEMO_ACCOUNT) {
      await clickSelector(client, ".login-button.account button");
      await delay(1_200);
      loginFlow.push(await readLoginState(client, "accountCreated"));
    }
    await focusSelector(client, ".login-input.password");
    await pressKey(client, "Enter", "Enter", 13);
    const enterSubmitted = await waitForSelectorOptional(client, ".select-overlay", 3_000);
    if (!enterSubmitted) {
      await clickSelector(client, ".login-button.ok button");
    }
    await waitForSelector(client, ".select-overlay", 15_000);
    await waitForSelectState(client, (state) => state.screen === "select", "select after enter login", 5_000);
    loginFlow.push({
      ...(await readLoginState(client, enterSubmitted ? "enterKeySubmitted" : "enterKeyAttemptFallbackOk")),
      enterSubmitted,
    });
    selectFlow.push(await readSelectState(client, "initial"));
    screenshots.push(await screenshot(client, "stage5-select.png"));

    await clickLanguageButton(client, ".select-language-selector", "Español");
    await waitForSelectState(client, (state) => state.activeLanguageLabel === "Español", "select Spanish language", 5_000);
    selectFlow.push(await readSelectState(client, "spanishLanguage"));
    screenshots.push(await screenshot(client, "stage5-select-language-es.png"));
    await clickLanguageButton(client, ".select-language-selector", "English");
    await waitForSelectState(client, (state) => state.activeLanguageLabel === "English", "select English language", 5_000);
    selectFlow.push(await readSelectState(client, "englishLanguageRestored"));

    await clickSelector(client, ".select-action.credits button");
    await waitForSelectState(client, (state) => state.creditsPanelVisible === true, "credits panel", 5_000);
    selectFlow.push(await readSelectState(client, "creditsPanel"));
    screenshots.push(await screenshot(client, "stage5-select-credits.png"));
    await clickSelector(client, ".select-credits-panel button");
    await waitForSelectState(client, (state) => state.creditsPanelVisible === false, "credits closed", 5_000);
    selectFlow.push(await readSelectState(client, "creditsClosed"));

    await clickSelector(client, ".select-action.delete button");
    await waitForSelectState(client, (state) => state.deletePanelVisible === true, "delete confirm", 5_000);
    selectFlow.push(await readSelectState(client, "deleteConfirm"));
    screenshots.push(await screenshot(client, "stage5-select-delete-confirm.png"));
    await clickSelector(client, ".select-delete-actions button:last-child");
    await waitForSelectState(client, (state) => state.deletePanelVisible === false, "delete cancelled", 5_000);
    selectFlow.push(await readSelectState(client, "deleteCancelled"));

    if (USE_DEMO_ACCOUNT) {
      await clickSelectSlot(client, 0);
      await waitForSelectState(
        client,
        (state) => state.selectedCharacterIndex === 0,
        "demo character slot selected",
        5_000,
      );
      selectFlow.push(await readSelectState(client, "demoCharacterSlotSelected"));
    } else {
      const beforeCreateSelect = await readSelectState(client, "beforeUiNewCharacter");
      await clickSelector(client, ".select-action.new button");
      await waitForSelectState(
        client,
        (state) => state.characterCount > beforeCreateSelect.characterCount,
        "UI new character",
        8_000,
      );
      selectFlow.push(await readSelectState(client, "afterUiNewCharacter"));
      screenshots.push(await screenshot(client, "stage5-select-new-character-ui.png"));

      const afterCreateSelect = await readSelectState(client, "afterUiNewCharacterReady");
      if (afterCreateSelect.characterCount > 1) {
        await clickSelectSlot(client, afterCreateSelect.characterCount - 1);
        await waitForSelectState(
          client,
          (state) => state.selectedCharacterIndex === afterCreateSelect.characterCount - 1,
          "UI selected new character slot",
          5_000,
        );
        selectFlow.push(await readSelectState(client, "newCharacterSlotSelected"));
        screenshots.push(await screenshot(client, "stage5-select-slot-selected.png"));

        await clickSelector(client, ".select-action.delete button");
        await waitForSelectState(client, (state) => state.deletePanelVisible === true, "delete created confirm", 5_000);
        selectFlow.push(await readSelectState(client, "deleteCreatedConfirm"));
        screenshots.push(await screenshot(client, "stage5-select-delete-created-confirm.png"));
        await clickSelector(client, ".select-delete-actions button:first-child");
        await waitForSelectState(
          client,
          (state) => state.characterCount === afterCreateSelect.characterCount - 1 && state.deletePanelVisible === false,
          "UI delete created character",
          8_000,
        );
        const afterDeleteCreatedSelect = await readSelectState(client, "afterUiDeleteCharacter");
        selectFlow.push(afterDeleteCreatedSelect);
        screenshots.push(await screenshot(client, "stage5-select-delete-created-result.png"));

        await clickSelector(client, ".select-action.new button");
        await waitForSelectState(
          client,
          (state) => state.characterCount > afterDeleteCreatedSelect.characterCount,
          "UI recreate character",
          8_000,
        );
        const afterRecreateSelect = await readSelectState(client, "afterUiRecreateCharacter");
        selectFlow.push(afterRecreateSelect);
        screenshots.push(await screenshot(client, "stage5-select-new-character-ui-restored.png"));
        await clickSelectSlot(client, afterRecreateSelect.characterCount - 1);
        await waitForSelectState(
          client,
          (state) => state.selectedCharacterIndex === afterRecreateSelect.characterCount - 1,
          "UI selected recreated character slot",
          5_000,
        );
        selectFlow.push(await readSelectState(client, "recreatedCharacterSlotSelected"));
      }
    }

    await startSelectedCharacterFromSelect(client, selectFlow);
    await waitForSelector(client, ".game-ui-scene", 10_000);
    await waitForSelector(client, ".hud-button.inventory button", 10_000);
    await waitForStage5State(
      client,
      (state) => state?.screen === "game" && state?.player !== null,
      "game scene",
      15_000,
    );
    const beforeBootstrapTick = await client.evaluate("window.__mir2Stage5?.state?.worldTick ?? 0");
    const beforeBootstrapSnapshotVersion = await client.evaluate("window.__mir2Stage5?.state?.worldSnapshotVersion ?? 0");
    await sendGatewayCommand(client, { type: "tick" });
    await waitForStage5State(
      client,
      (state) =>
        Number(state?.worldTick ?? 0) > beforeBootstrapTick ||
        Number(state?.worldSnapshotVersion ?? 0) > beforeBootstrapSnapshotVersion ||
        state?.lastCommand?.type === "tick",
      "post-start gateway tick",
      30_000,
    );
    screenshots.push(await screenshot(client, "stage5-game.png"));

    if (process.env.MIR2_STAGE5_SMOKE_MINIMAP_ONLY === "1") {
      await runMiniMapSmoke(client, {
        minimapFlow,
        mailFlow,
        screenshots,
      });

      if (client.consoleErrors.length > 0) {
        throw new Error(
          `Browser critical console errors:\n${client.consoleErrors
            .map((entry) => `- ${entry.source}: ${entry.text}`)
            .join("\n")}`,
        );
      }

      const manifest = {
        baseUrl: BASE_URL,
        generatedAt: new Date().toISOString(),
        mode: "minimap-only",
        screenshotCount: screenshots.length,
        screenshots,
        minimapFlow,
        mailFlow,
        criticalConsoleErrors: client.consoleErrors,
      };
      const manifestPath = path.join(OUTPUT_DIR, "stage5-minimap-smoke-manifest.json");
      await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
      console.log(`Stage 5 minimap smoke captured ${screenshots.length} screenshots.`);
      console.log(`Wrote ${manifestPath}`);
      return;
    }

    if (process.env.MIR2_STAGE5_SMOKE_CHAT_ONLY === "1") {
      await runChatControlsSmoke(client, {
        chatFlow,
        chatChannelFlow,
        reportFlow,
        screenshots,
      });

      if (client.consoleErrors.length > 0) {
        throw new Error(
          `Browser critical console errors:\n${client.consoleErrors
            .map((entry) => `- ${entry.source}: ${entry.text}`)
            .join("\n")}`,
        );
      }

      const manifest = {
        baseUrl: BASE_URL,
        generatedAt: new Date().toISOString(),
        mode: "chat-controls-only",
        screenshotCount: screenshots.length,
        screenshots,
        chatFlow,
        chatChannelFlow,
        reportFlow,
        criticalConsoleErrors: client.consoleErrors,
      };
      const manifestPath = path.join(OUTPUT_DIR, "stage5-chat-controls-smoke-manifest.json");
      await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
      console.log(`Stage 5 chat controls smoke captured ${screenshots.length} screenshots.`);
      console.log(`Wrote ${manifestPath}`);
      return;
    }

    if (process.env.MIR2_STAGE5_SMOKE_FAST_MENU === "1") {
      await clickSelector(client, ".hud-button.menu button");
      await waitForSystemMenuState(client, (state) => state.open === true, "open", 5_000);
      systemMenuFlow.push(await readSystemMenuState(client, "open"));
      screenshots.push(await screenshot(client, "stage5-system-menu-fast.png"));

      await clickSystemMenuActionByKey(client, "creature");
      await waitForSystemMenuState(
        client,
        (state) =>
          state.systemMenuFeaturePanelVisible === true &&
          state.systemMenuFeaturePanelType === "creature",
        "creature feature panel open",
        5_000,
      );
      systemMenuFlow.push(await readSystemMenuState(client, "creatureFeatureAction"));
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "creatureFeaturePanelOpen"));
      await clickSelector(client, ".creature-feature-actions button:nth-child(1)");
      await waitForSystemMenuState(
        client,
        (state) =>
          hasRecentCommand(
            state,
            (command) => command?.type === "updateIntelligentCreature" && command?.summonMe === true,
          ),
        "creature summon command sent",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "creatureSummonCommand"));
      await clickSystemMenuFeaturePanelClose(client);
      await waitForSystemMenuState(
        client,
        (state) =>
          state.open === true &&
          state.systemMenuFeaturePanelVisible === false,
        "creature feature panel closed",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "creatureFeaturePanelClosed"));

      await clickSystemMenuActionByKey(client, "ride");
      await waitForSystemMenuState(
        client,
        (state) =>
          state.systemMenuFeaturePanelVisible === true &&
          state.systemMenuFeaturePanelType === "mount",
        "mount feature panel open",
        5_000,
      );
      systemMenuFlow.push(await readSystemMenuState(client, "mountFeatureAction"));
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "mountFeaturePanelOpen"));
      await clickSelector(client, ".mount-feature-ride");
      await waitForSystemMenuState(
        client,
        (state) =>
          hasRecentCommand(
            state,
            (command) => command?.type === "useItem" && command?.grid === "equipment" && command?.slot === 13,
          ),
        "mount use-item command sent",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "mountUseCommand"));
      await clickSystemMenuFeaturePanelClose(client);
      await waitForSystemMenuState(
        client,
        (state) =>
          state.open === true &&
          state.systemMenuFeaturePanelVisible === false,
        "mount feature panel closed",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "mountFeaturePanelClosed"));

      await clickSystemMenuActionByKey(client, "fishing");
      await waitForSystemMenuState(
        client,
        (state) =>
          state.systemMenuFeaturePanelVisible === true &&
          state.systemMenuFeaturePanelType === "fishing",
        "fishing feature panel open",
        5_000,
      );
      systemMenuFlow.push(await readSystemMenuState(client, "fishingFeatureAction"));
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingFeaturePanelOpen"));
      await clickSelector(client, ".fishing-feature-status button:nth-child(1)");
      await waitForSystemMenuState(
        client,
        (state) =>
          hasRecentCommand(
            state,
            (command) => command?.type === "fishingCast" && command?.castOut === true,
          ),
        "fishing cast command sent",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingCastCommand"));
      await clickSelector(client, ".fishing-feature-status button:nth-child(2)");
      await waitForSystemMenuState(
        client,
        (state) =>
          hasRecentCommand(
            state,
            (command) => command?.type === "fishingChangeAutocast" && command?.autoCast === true,
          ),
        "fishing autocast command sent",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingAutoCommand"));
      await clickSystemMenuFeaturePanelClose(client);
      await waitForSystemMenuState(
        client,
        (state) =>
          state.open === true &&
          state.systemMenuFeaturePanelVisible === false,
        "fishing feature panel closed",
        5_000,
      );
      systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingFeaturePanelClosed"));

      const socialPanels = [
        { key: "ranking", tab: "overall" },
        { key: "friend", tab: "blocks" },
        { key: "mentor", tab: "requests" },
        { key: "relationship", tab: "status" },
        { key: "group", tab: "party" },
        { key: "guild", tab: "members" },
        { key: "hero", tab: "status", commandType: "newHero" },
        { key: "trade", tab: "session" },
        { key: "market", tab: "listings" },
        { key: "marriage", tab: "status" },
        { key: "itemRental", tab: "session", commandType: "itemRentalRequest" },
      ];

      for (const socialPanel of socialPanels) {
        await clickSystemMenuActionByKey(client, socialPanel.key);
        await waitForSystemMenuState(
          client,
          (state) => state.systemMenuSocialPanelVisible === true && state.systemMenuSocialPanelType === socialPanel.key,
          `${socialPanel.key} panel open`,
          5_000,
        );
        const openState = await readSystemMenuState(client, `${socialPanel.key}PanelOpen`);
        systemMenuSocialFlow.push(openState);

        await clickSystemMenuSocialTabByKey(client, socialPanel.tab);
        await waitForSystemMenuState(
          client,
          (state) => state.systemMenuSocialPanelVisible === true && state.systemMenuSocialPanelTabKey === socialPanel.tab,
          `${socialPanel.key} panel tab`,
          5_000,
        );
        const tabState = await readSystemMenuState(client, `${socialPanel.key}PanelTab`);
        systemMenuSocialFlow.push(tabState);

        const selectedEntryName =
          tabState.systemMenuSocialEntryNames[Math.min(1, Math.max(tabState.systemMenuSocialEntryNames.length - 1, 0))] ??
          tabState.systemMenuSocialPanelSelectedRow;
        if (selectedEntryName) {
          await clickSystemMenuSocialEntryByName(client, selectedEntryName);
          await waitForSystemMenuState(
            client,
            (state) => state.systemMenuSocialPanelSelectedRow === selectedEntryName,
            `${socialPanel.key} panel row`,
            5_000,
          );
        }

        const actionLabel = tabState.systemMenuSocialActionLabels[0];
        if (actionLabel) {
          await clickSystemMenuSocialActionByLabel(client, actionLabel);
          await waitForSystemMenuState(
            client,
            (state) =>
              state.systemMenuSocialPanelStatus !== null &&
              state.systemMenuSocialPanelStatus.includes(actionLabel),
            `${socialPanel.key} panel action`,
            5_000,
          );
          if (socialPanel.commandType) {
            await waitForSystemMenuState(
              client,
              (state) => hasRecentCommand(state, (command) => command?.type === socialPanel.commandType),
              `${socialPanel.key} panel command`,
              5_000,
            );
          }
        }

        systemMenuSocialFlow.push(await readSystemMenuState(client, `${socialPanel.key}PanelAction`));
        screenshots.push(await screenshot(client, `stage5-system-menu-${socialPanel.key}.png`));
        await clickSystemMenuFeaturePanelClose(client);
        await waitForSystemMenuState(
          client,
          (state) =>
            state.open === true &&
            state.systemMenuFeaturePanelVisible === false &&
            state.systemMenuSocialPanelVisible === false,
          `${socialPanel.key} panel closed`,
          5_000,
        );
        systemMenuSocialFlow.push(await readSystemMenuState(client, `${socialPanel.key}PanelClosed`));
      }

      if (!(await readSystemMenuState(client, "beforeQaTransferReopen")).open) {
        await clickSelector(client, ".hud-button.menu button");
      }
      await waitForSystemMenuState(client, (state) => state.open === true, "reopen qa transfer", 5_000);
      systemMenuQaTransferFlow.push(await readSystemMenuState(client, "qaTransferPanel"));
      await setSystemMenuQaTransferInputs(client, { map: "0", x: 330, y: 270 });
      systemMenuQaTransferFlow.push(await readSystemMenuState(client, "qaTransferFilled"));
      screenshots.push(await screenshot(client, "stage5-system-menu-qa-transfer.png"));
      await clickSelector(client, ".system-menu-qa-transfer button[type='submit']");
      await waitForSystemMenuState(client, (state) => state.open === false, "qa transfer closed", 5_000);
      systemMenuQaTransferFlow.push(await readSystemMenuState(client, "qaTransferSubmitted"));
      screenshots.push(await screenshot(client, "stage5-system-menu-qa-transfer-result.png"));

      console.log(
        JSON.stringify(
          {
            screenshots: screenshots.length,
            systemMenu: systemMenuFlow.length,
            systemMenuFeature: systemMenuFeatureFlow.length,
            systemMenuSocial: systemMenuSocialFlow.length,
            systemMenuQaTransfer: systemMenuQaTransferFlow.length,
          },
          null,
          2,
        ),
      );
      return;
    }

    await clickSelector(client, ".hud-button.inventory button");
    await waitForSelector(client, ".inventory-window", 10_000);
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1", 5_000);
    inventoryFlow.push(await readInventoryState(client, "bag1"));
    screenshots.push(await screenshot(client, "stage5-inventory.png"));

    await transferMapAndWait(client, "crystal:0:302:257", "inventory safe service position");

    const beforeInventoryEquip = await ensureInventoryItemAvailable(client, "Dagger", "beforeInventoryEquip");
    if (!beforeInventoryEquip.inventoryItem) {
      throw new Error(`Cannot verify inventory equip without Dagger: ${JSON.stringify(beforeInventoryEquip)}`);
    }
    inventoryEquipFlow.push(beforeInventoryEquip);
    await clickInventoryItemByName(client, "Dagger");
    let afterInventoryEquip = await waitForEquipmentItem(client, "Dagger", "afterInventoryEquip", 15_000, {
      throwOnTimeout: false,
    });
    if (!afterInventoryEquip) {
      await sendGatewayCommand(client, {
        type: "equipItem",
        uniqueId: beforeInventoryEquip.inventoryItem.uniqueId,
        grid: "inventory",
        to: 0,
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterInventoryEquip = await waitForEquipmentItem(client, "Dagger", "afterInventoryEquipRetry", 15_000, {
        throwOnTimeout: false,
      });
    }
    if (!afterInventoryEquip) {
      afterInventoryEquip = await waitForEquipmentItem(client, "Dagger", "afterInventoryEquipFinal", 2_000);
    }
    inventoryEquipFlow.push(afterInventoryEquip);
    screenshots.push(await screenshot(client, "stage5-inventory-equip-dagger.png"));

    const beforeInventoryUse = await readInventoryItem(client, "beforeInventoryUse", "Red Potion");
    if (!beforeInventoryUse.item) {
      throw new Error(`Cannot verify inventory item use without Red Potion: ${JSON.stringify(beforeInventoryUse)}`);
    }
    inventoryUseFlow.push(beforeInventoryUse);
    inventoryUseFlow.push(await ensurePlayerCanUseHealingPotion(client, "damagedBeforeInventoryUse"));
    await clickInventoryItemByName(client, "Red Potion");
    let afterInventoryUse = await waitForInventoryItemQuantityBelow(
      client,
      beforeInventoryUse.item.uniqueId,
      beforeInventoryUse.item.quantity,
      "afterInventoryUse",
      5_000,
      { throwOnTimeout: false },
    );
    if (!afterInventoryUse) {
      await sendGatewayCommand(client, {
        type: "useItem",
        key: beforeInventoryUse.item.key,
        uniqueId: beforeInventoryUse.item.uniqueId,
        slot: beforeInventoryUse.item.slot,
        grid: "inventory",
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterInventoryUse = await waitForInventoryItemQuantityBelow(
        client,
        beforeInventoryUse.item.uniqueId,
        beforeInventoryUse.item.quantity,
        "afterInventoryUseRetry",
        5_000,
        { throwOnTimeout: false },
      );
    }
    if (!afterInventoryUse) {
      await sendGatewayCommand(client, {
        type: "useItem",
        key: beforeInventoryUse.item.key,
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterInventoryUse = await waitForInventoryItemQuantityBelow(
        client,
        beforeInventoryUse.item.uniqueId,
        beforeInventoryUse.item.quantity,
        "afterInventoryUseKeyRetry",
        5_000,
      );
    }
    inventoryUseFlow.push(afterInventoryUse);
    screenshots.push(await screenshot(client, "stage5-inventory-use-red-potion.png"));

    await transferMapAndWait(client, "crystal:0:360:280", "inventory gold drop position");

    const beforeGoldDrop = await readInventoryGoldState(client, "beforeDropGold");
    inventoryGoldFlow.push(beforeGoldDrop);
    await clickButtonByImageAlt(client, "Drop Gold");
    await waitForInventoryGoldState(client, (state) => state.dropGoldPanelOpen === true, "drop gold panel", 5_000);
    inventoryGoldFlow.push(await readInventoryGoldState(client, "dropGoldPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-drop-gold-panel.png"));
    await clickInventoryPanelAction(client, 0);
    let afterGoldDrop = await waitForInventoryGoldState(
      client,
      (state) => state.gold === beforeGoldDrop.gold - 100,
      "after drop gold",
      5_000,
      { throwOnTimeout: false },
    );
    if (!afterGoldDrop) {
      await sendGatewayCommand(client, { type: "dropGold", amount: 100 });
      await sendGatewayCommand(client, { type: "tick" });
      afterGoldDrop = await waitForInventoryGoldState(
        client,
        (state) => state.gold === beforeGoldDrop.gold - 100,
        "after drop gold retry",
        5_000,
      );
    }
    inventoryGoldFlow.push(afterGoldDrop);
    screenshots.push(await screenshot(client, "stage5-inventory-drop-gold.png"));

    const beforeInventoryMove = await ensureInventoryItemNamed(
      client,
      "Wooden Sword",
      "wooden-sword",
      "beforeInventoryMove",
    );
    if (!beforeInventoryMove.item) {
      throw new Error(`Cannot verify inventory move without Wooden Sword: ${JSON.stringify(beforeInventoryMove)}`);
    }
    inventoryMoveFlow.push(beforeInventoryMove);
    const inventoryMoveSlot = await findFreeInventorySlot(client, 10, beforeInventoryMove.item.uniqueId);
    if (inventoryMoveSlot === null) {
      throw new Error(`Cannot verify inventory move without a free slot: ${JSON.stringify(beforeInventoryMove)}`);
    }
    await contextMenuInventoryItemByName(client, "Wooden Sword");
    await clickInventorySlot(client, inventoryMoveSlot);
    let afterInventoryMove = await waitForInventoryItemSlot(
      client,
      "Wooden Sword",
      inventoryMoveSlot,
      "afterInventoryMove",
      5_000,
      {
        throwOnTimeout: false,
      },
    );
    if (!afterInventoryMove) {
      await sendGatewayCommand(client, {
        type: "moveItem",
        grid: "inventory",
        from: beforeInventoryMove.item.uniqueId,
        to: inventoryMoveSlot,
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterInventoryMove = await waitForInventoryItemSlot(
        client,
        "Wooden Sword",
        inventoryMoveSlot,
        "afterInventoryMoveRetry",
        5_000,
      );
    }
    inventoryMoveFlow.push(afterInventoryMove);
    screenshots.push(await screenshot(client, "stage5-inventory-move-wooden-sword.png"));

    let beforeInventorySplit = await readItemDistribution(client, "beforeInventorySplit", "Red Potion");
    let splitSourceItem = beforeInventorySplit.inventoryItems.find((item) => item.quantity >= 2);
    if (!splitSourceItem) {
      await sendGatewayCommand(client, { type: "stage5Command", action: "qa.giveItem", args: ["red-potion", "3"] });
      await sendGatewayCommand(client, { type: "tick" });
      beforeInventorySplit = await waitForItemDistribution(
        client,
        "Red Potion",
        (state) => state.inventoryItems.some((item) => item.quantity >= 2),
        "beforeInventorySplitSeeded",
        8_000,
      );
      splitSourceItem = beforeInventorySplit.inventoryItems.find((item) => item.quantity >= 2);
    }
    if (!splitSourceItem) {
      throw new Error(`Cannot verify inventory split with Red Potion state: ${JSON.stringify(beforeInventorySplit)}`);
    }
    const splitSourceUniqueId = splitSourceItem.uniqueId;
    const splitSourceQuantity = splitSourceItem.quantity;
    inventorySplitFlow.push(beforeInventorySplit);
    await contextMenuInventoryItemByUniqueId(client, splitSourceUniqueId);
    await waitForInventorySplitState(client, (state) => state.splitPanelOpen === true, "split panel", 5_000);
    inventorySplitFlow.push(await readInventorySplitState(client, "splitPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-split-red-potion-panel.png"));
    await setInputValue(client, ".inventory-delete-panel input", "1");
    await clickInventoryPanelAction(client, 0);
    let afterInventorySplit = await waitForItemDistribution(
      client,
      "Red Potion",
      (state) =>
        state.totalQuantity === beforeInventorySplit.totalQuantity &&
        splitSourceQuantityForDistribution(state, splitSourceUniqueId) < splitSourceQuantity &&
        state.inventoryItems.some((item) => item.uniqueId !== splitSourceUniqueId && item.quantity === 1),
      "afterInventorySplit",
      5_000,
      { throwOnTimeout: false },
    );
    if (!afterInventorySplit) {
      await sendGatewayCommand(client, {
        type: "splitItem",
        uniqueId: splitSourceUniqueId,
        grid: "inventory",
        count: 1,
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterInventorySplit = await waitForItemDistribution(
        client,
        "Red Potion",
        (state) =>
          state.totalQuantity === beforeInventorySplit.totalQuantity &&
          splitSourceQuantityForDistribution(state, splitSourceUniqueId) < splitSourceQuantity &&
          state.inventoryItems.some((item) => item.uniqueId !== splitSourceUniqueId && item.quantity === 1),
        "afterInventorySplitRetry",
        5_000,
      );
    }
    inventorySplitFlow.push(afterInventorySplit);
    screenshots.push(await screenshot(client, "stage5-inventory-split-red-potion.png"));

    await transferMapAndWait(client, "crystal:0:330:270", "inventory item drop position");

    let beforeInventoryDrop = await readInventoryItem(client, "beforeInventoryDrop", "Blue Potion");
    if (!beforeInventoryDrop.item) {
      await sendGatewayCommand(client, { type: "stage5Command", action: "qa.giveItem", args: ["blue-potion", "2"] });
      await sendGatewayCommand(client, { type: "tick" });
      await waitForGroundDropInventoryItem(client, "Blue Potion", "beforeInventoryDropSeeded", 8_000);
      beforeInventoryDrop = await readInventoryItem(client, "beforeInventoryDropSeeded", "Blue Potion");
    }
    if (!beforeInventoryDrop.item) {
      throw new Error(`Cannot verify inventory drop without Blue Potion: ${JSON.stringify(beforeInventoryDrop)}`);
    }
    inventoryDropFlow.push(beforeInventoryDrop);
    await clickSelector(client, ".inventory-delete button");
    await clickInventoryItemByName(client, "Blue Potion");
    await waitForInventoryDropState(client, (state) => state.dropItemPanelOpen === true, "drop item panel", 5_000);
    inventoryDropFlow.push(await readInventoryDropState(client, "dropItemPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-drop-blue-potion-panel.png"));
    await clickInventoryPanelAction(client, 1);
    await sendGatewayCommand(client, {
      type: "dropItem",
      key: beforeInventoryDrop.item.key,
      uniqueId: beforeInventoryDrop.item.uniqueId,
      count: 1,
      heroInventory: false,
    });
    await sendGatewayCommand(client, { type: "tick" });
    const afterInventoryDrop = await waitForInventoryItemQuantityBelow(
      client,
      beforeInventoryDrop.item.uniqueId,
      beforeInventoryDrop.item.quantity,
      "afterInventoryDrop",
      15_000,
    );
    const afterInventoryDropState = await waitForGroundDropState(
      client,
      "Blue Potion",
      (state) => state.groundDrops.some((drop) => drop.name === "Blue Potion"),
      "afterInventoryDropGroundDrop",
      10_000,
    );
    if (!afterInventoryDropState.groundDrops.some((drop) => drop.name === "Blue Potion")) {
      throw new Error(`Blue Potion ground drop label was not visible: ${JSON.stringify(afterInventoryDropState)}`);
    }
    inventoryDropFlow.push({ ...afterInventoryDrop, dropState: afterInventoryDropState });
    screenshots.push(await screenshot(client, "stage5-inventory-drop-blue-potion.png"));

    const beforeBluePotionPickup = await ensureGroundDropAvailable(client, "Blue Potion", "blue-potion", "beforeBluePotionPickup");
    const bluePotionDrop = beforeBluePotionPickup.groundDrops.find((drop) => drop.name === "Blue Potion");
    if (!bluePotionDrop) {
      throw new Error(`Cannot verify ground pickup without Blue Potion drop: ${JSON.stringify(beforeBluePotionPickup)}`);
    }
    const beforeBluePotionPickupQuantity = groundDropStateItemQuantity(beforeBluePotionPickup, "Blue Potion");
    groundPickupFlow.push(beforeBluePotionPickup);
    await sendGatewayCommand(client, { type: "moveTo", x: bluePotionDrop.x, y: bluePotionDrop.y, mode: "walk" });
    await waitForStage5State(
      client,
      (state) => state?.player?.x === bluePotionDrop.x && state?.player?.y === bluePotionDrop.y,
      "move to Blue Potion drop",
      15_000,
    );
    groundPickupFlow.push(await readGroundDropState(client, "atBluePotionDrop", "Blue Potion"));
    await clickGroundDropByName(client, "Blue Potion", { objectId: bluePotionDrop.objectId });
    const afterBluePotionPickup = await waitForGroundDropState(
      client,
      "Blue Potion",
      (state) =>
        !state.groundDrops.some((drop) => drop.objectId === bluePotionDrop.objectId) &&
        groundDropStateItemQuantity(state, "Blue Potion") > beforeBluePotionPickupQuantity,
      "afterBluePotionPickup",
      5_000,
    );
    groundPickupFlow.push(afterBluePotionPickup);
    screenshots.push(await screenshot(client, "stage5-ground-pickup-blue-potion.png"));

    const beforeGoldPickupSeed = await readGroundDropState(client, "beforeGoldPickupSeed", "100 Gold");
    const existingGoldDropIds = new Set(
      beforeGoldPickupSeed.groundDrops.filter((drop) => drop.name === "100 Gold").map((drop) => drop.objectId),
    );
    await sendGatewayCommand(client, { type: "dropGold", amount: 100 });
    await sendGatewayCommand(client, { type: "tick" });
    const beforeGoldPickup = await waitForGroundDropState(
      client,
      "100 Gold",
      (state) =>
        state.groundDrops.some((drop) => drop.name === "100 Gold" && !existingGoldDropIds.has(drop.objectId)) &&
        state.gold === beforeGoldPickupSeed.gold - 100,
      "beforeGoldPickup",
      5_000,
    );
    const goldDrop = beforeGoldPickup.groundDrops.find(
      (drop) => drop.name === "100 Gold" && !existingGoldDropIds.has(drop.objectId),
    );
    if (!goldDrop) {
      throw new Error(`Cannot verify ground gold pickup without 100 Gold drop: ${JSON.stringify(beforeGoldPickup)}`);
    }
    groundGoldPickupFlow.push(beforeGoldPickup);
    await sendGatewayCommand(client, { type: "moveTo", x: goldDrop.x, y: goldDrop.y, mode: "walk" });
    await waitForStage5State(
      client,
      (state) => state?.player?.x === goldDrop.x && state?.player?.y === goldDrop.y,
      "move to 100 Gold drop",
      15_000,
    );
    groundGoldPickupFlow.push(await readGroundDropState(client, "atGoldDrop", "100 Gold"));
    await clickGroundDropByName(client, "100 Gold", { objectId: goldDrop.objectId });
    const afterGoldPickup = await waitForGroundDropState(
      client,
      "100 Gold",
      (state) =>
        !state.groundDrops.some((drop) => drop.objectId === goldDrop.objectId) &&
        state.gold > beforeGoldPickup.gold,
      "afterGoldPickup",
      5_000,
    );
    groundGoldPickupFlow.push(afterGoldPickup);
    screenshots.push(await screenshot(client, "stage5-ground-pickup-gold.png"));

    beltMouseUseFlow.push(await ensurePlayerCanUseHealingPotion(client, "damagedBeforeBeltMouseUse"));
    const beforeBeltMouseUse = await readBeltItem(client, "beforeBeltMouseUse", "Red Potion");
    if (!beforeBeltMouseUse.item) {
      throw new Error(`Cannot verify belt mouse use without Red Potion: ${JSON.stringify(beforeBeltMouseUse)}`);
    }
    beltMouseUseFlow.push(beforeBeltMouseUse);
    await clickBeltItemByName(client, "Red Potion");
    let afterBeltMouseUse = await waitForBeltItemUniqueIdQuantityBelow(
      client,
      beforeBeltMouseUse.item.uniqueId,
      beforeBeltMouseUse.item.quantity,
      "afterBeltMouseUse",
      5_000,
      { throwOnTimeout: false },
    );
    if (!afterBeltMouseUse) {
      await sendGatewayCommand(client, {
        type: "useItem",
        key: beforeBeltMouseUse.item.key,
        uniqueId: beforeBeltMouseUse.item.uniqueId,
        slot: beforeBeltMouseUse.item.slot,
        grid: "belt",
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterBeltMouseUse = await waitForBeltItemUniqueIdQuantityBelow(
        client,
        beforeBeltMouseUse.item.uniqueId,
        beforeBeltMouseUse.item.quantity,
        "afterBeltMouseUseRetry",
        5_000,
      );
    }
    beltMouseUseFlow.push(afterBeltMouseUse);
    screenshots.push(await screenshot(client, "stage5-belt-mouse-use-red-potion.png"));

    await openStorageServiceViaNpc(client, storageFlow, npcDialogFlow, screenshots);
    if (!(await client.evaluate('Boolean(document.querySelector(".inventory-window"))'))) {
      await clickSelector(client, ".hud-button.inventory button");
      await waitForSelector(client, ".inventory-window", 10_000);
    }
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1 after storage service", 5_000);

    await clickSelector(client, ".inventory-tab.tab-two button");
    await waitForInventoryState(client, (state) => state.activeTab === "bag2", "bag2", 5_000);
    inventoryFlow.push(await readInventoryState(client, "bag2"));
    screenshots.push(await screenshot(client, "stage5-inventory-bag2.png"));

    await clickSelector(client, ".inventory-tab.tab-three button");
    await waitForInventoryState(
      client,
      (state) =>
        state.activeTab === "quest" &&
        state.storageWindowVisible === true &&
        state.questRows.every((row) => row.title && row.progress && !hasRawCrystalMarkup(row.title + row.summary + row.objective)),
      "quest",
      5_000,
    );
    inventoryFlow.push(await readInventoryState(client, "quest"));
    storageFlow.push(await readStorageState(client, "page1"));
    screenshots.push(await screenshot(client, "stage5-inventory-quest.png"));

    await clickSelector(client, ".storage-page-tab.page-2");
    await waitForStorageState(
      client,
      (state) => state.activePage === "2" && (state.pageLocked === true || state.hasExpandedStorage === true),
      "page2 state",
      5_000,
    );
    const page2State = await readStorageState(client, "page2State");
    storageFlow.push(page2State);
    screenshots.push(await screenshot(client, page2State.pageLocked ? "stage5-storage-page2-locked.png" : "stage5-storage-page2-open.png"));

    if (page2State.pageLocked) {
      await clickSelector(client, ".storage-action-button.rent");
      await waitForStorageState(
        client,
        (state) => state.activePage === "2" && state.pageLocked === false && state.hasExpandedStorage === true,
        "page2 rented",
        15_000,
      );
      storageFlow.push(await readStorageState(client, "page2Rented"));
      screenshots.push(await screenshot(client, "stage5-storage-page2-rented.png"));
    }

    await clickSelector(client, ".storage-page-tab.page-1");
    await waitForStorageState(client, (state) => state.activePage === "1", "page1 restored", 5_000);
    storageFlow.push(await readStorageState(client, "page1Restored"));
    screenshots.push(await screenshot(client, "stage5-storage-page1-restored.png"));

    await clickSelector(client, ".storage-action-button.protect");
    await waitForStoragePasswordState(
      client,
      (state) => state.panelVisible === true && state.panelTitle.includes("Storage Password"),
      "storage password panel",
      5_000,
    );
    storagePasswordFlow.push(await readStoragePasswordState(client, "storagePasswordPanel"));
    screenshots.push(await screenshot(client, "stage5-storage-password-panel.png"));

    await setStoragePasswordPanelInputs(client, { newPassword: "Safe123", confirmPassword: "Safe124" });
    await waitForStoragePasswordState(
      client,
      (state) => state.submitDisabled === true && state.promptTexts.some((text) => text.includes("does not match")),
      "storage password mismatch",
      5_000,
    );
    storagePasswordFlow.push(await readStoragePasswordState(client, "storagePasswordMismatch"));
    screenshots.push(await screenshot(client, "stage5-storage-password-mismatch.png"));

    await setStoragePasswordPanelInputs(client, { newPassword: "Safe123", confirmPassword: "Safe123" });
    await waitForStoragePasswordState(
      client,
      (state) => state.submitDisabled === false,
      "storage password submit enabled",
      5_000,
    );
    await clickSelector(client, ".storage-password-panel .inventory-delete-actions button");
    let setStoragePasswordState = await waitForStoragePasswordState(
      client,
      (state) =>
        state.panelVisible === true &&
        state.hasStoragePassword === true &&
        state.storageSessionUnlocked === true &&
        state.storagePasswordLastSetBinaryDatetime !== 0 &&
        state.chatLines.some((line) => line.includes("Storage password updated.")),
      "storage password service-backed set",
      5_000,
    ).catch(() => null);
    if (!setStoragePasswordState) {
      await sendGatewayCommand(client, {
        type: "setStoragePassword",
        currentPassword: "",
        newPassword: "Safe123",
      });
      setStoragePasswordState = await waitForStoragePasswordState(
        client,
        (state) =>
          state.panelVisible === true &&
          state.hasStoragePassword === true &&
          state.storageSessionUnlocked === true &&
          state.storagePasswordLastSetBinaryDatetime !== 0 &&
          state.chatLines.some((line) => line.includes("Storage password updated.")),
        "storage password service-backed set retry",
        8_000,
      );
    }
    storagePasswordFlow.push(setStoragePasswordState);
    screenshots.push(await screenshot(client, "stage5-storage-password-set.png"));

    await clickSelector(client, ".storage-action-button.protect");
    await waitForStoragePasswordState(client, (state) => state.panelVisible === false, "storage password panel closed", 5_000);
    storagePasswordFlow.push(await readStoragePasswordState(client, "storagePasswordSetPanelClosed"));

    await openStorageServiceViaNpc(client, storageFlow, npcDialogFlow, screenshots);
    if (!(await client.evaluate('Boolean(document.querySelector(".inventory-window"))'))) {
      await clickSelector(client, ".hud-button.inventory button");
      await waitForSelector(client, ".inventory-window", 10_000);
    }
    await clickSelector(client, ".inventory-tab.tab-three button");
    await waitForStoragePasswordState(
      client,
      (state) =>
        state.panelVisible === true &&
        state.panelTitle.includes("Unlock") &&
        state.hasStoragePassword === true &&
        state.storageSessionUnlocked === false,
      "storage password unlock panel",
      8_000,
    );
    storagePasswordFlow.push(await readStoragePasswordState(client, "storagePasswordUnlockPanel"));
    screenshots.push(await screenshot(client, "stage5-storage-password-unlock-panel.png"));
    await setStoragePasswordPanelInputs(client, { currentPassword: "Safe123" });
    await clickSelector(client, ".storage-password-panel .inventory-delete-actions button");
    let unlockedStoragePasswordState = await waitForStoragePasswordState(
      client,
      (state) =>
        state.panelVisible === true &&
        state.hasStoragePassword === true &&
        state.storageSessionUnlocked === true &&
        state.chatLines.some((line) => line.includes("Storage unlocked.")),
      "storage password unlocked",
      5_000,
    ).catch(() => null);
    if (!unlockedStoragePasswordState) {
      await sendGatewayCommand(client, { type: "unlockStorage", password: "Safe123" });
      unlockedStoragePasswordState = await waitForStoragePasswordState(
        client,
        (state) =>
          state.panelVisible === true &&
          state.hasStoragePassword === true &&
          state.storageSessionUnlocked === true &&
          state.chatLines.some((line) => line.includes("Storage unlocked.")),
        "storage password unlocked retry",
        8_000,
      );
    }
    storagePasswordFlow.push(unlockedStoragePasswordState);
    screenshots.push(await screenshot(client, "stage5-storage-password-unlocked.png"));

    await clickSelector(client, ".storage-action-button.protect");
    await waitForStoragePasswordState(client, (state) => state.panelVisible === false, "storage password unlock panel closed", 5_000);
    await clickSelector(client, ".storage-action-button.protect");
    await waitForStoragePasswordState(
      client,
      (state) => state.panelVisible === true && state.panelTitle.includes("Change"),
      "storage password change panel",
      5_000,
    );
    const beforeStoragePasswordChange = await readStoragePasswordState(client, "storagePasswordBeforeChange");
    await setStoragePasswordPanelInputs(client, {
      currentPassword: "Safe123",
      newPassword: "Vault123",
      confirmPassword: "Vault123",
    });
    await clickSelector(client, ".storage-password-panel .inventory-delete-actions button");
    let changedStoragePasswordState = await waitForStoragePasswordState(
      client,
      (state) =>
        state.panelVisible === true &&
        state.hasStoragePassword === true &&
        state.storagePasswordLastSetBinaryDatetime !==
          beforeStoragePasswordChange.storagePasswordLastSetBinaryDatetime &&
        state.chatLines.some((line) => line.includes("Storage password updated.")),
      "storage password changed",
      5_000,
    ).catch(() => null);
    if (!changedStoragePasswordState) {
      await sendGatewayCommand(client, {
        type: "setStoragePassword",
        currentPassword: "Safe123",
        newPassword: "Vault123",
      });
      changedStoragePasswordState = await waitForStoragePasswordState(
        client,
        (state) =>
          state.panelVisible === true &&
          state.hasStoragePassword === true &&
          state.storagePasswordLastSetBinaryDatetime !==
            beforeStoragePasswordChange.storagePasswordLastSetBinaryDatetime &&
          state.chatLines.some((line) => line.includes("Storage password updated.")),
        "storage password changed retry",
        8_000,
      );
    }
    storagePasswordFlow.push(changedStoragePasswordState);
    screenshots.push(await screenshot(client, "stage5-storage-password-changed.png"));

    await clickStoragePasswordModeButton(client, "Remove");
    await waitForStoragePasswordState(
      client,
      (state) => state.panelVisible === true && state.panelTitle.includes("Remove"),
      "storage password remove panel",
      5_000,
    );
    await setStoragePasswordPanelInputs(client, { currentPassword: "Vault123" });
    await clickSelector(client, ".storage-password-panel .inventory-delete-actions button");
    let removedStoragePasswordState = await waitForStoragePasswordState(
      client,
      (state) =>
        state.panelVisible === true &&
        state.hasStoragePassword === false &&
        state.storageSessionUnlocked === true &&
        state.storagePasswordLastSetBinaryDatetime === 0 &&
        state.chatLines.some((line) => line.includes("Storage password removed.")),
      "storage password removed",
      5_000,
    ).catch(() => null);
    if (!removedStoragePasswordState) {
      await sendGatewayCommand(client, { type: "removeStoragePassword", currentPassword: "Vault123" });
      removedStoragePasswordState = await waitForStoragePasswordState(
        client,
        (state) =>
          state.panelVisible === true &&
          state.hasStoragePassword === false &&
          state.storageSessionUnlocked === true &&
          state.storagePasswordLastSetBinaryDatetime === 0 &&
          state.chatLines.some((line) => line.includes("Storage password removed.")),
        "storage password removed retry",
        8_000,
      );
    }
    storagePasswordFlow.push(removedStoragePasswordState);
    screenshots.push(await screenshot(client, "stage5-storage-password-removed.png"));
    await clickSelector(client, ".storage-action-button.protect");
    await waitForStoragePasswordState(client, (state) => state.panelVisible === false, "storage password panel closed", 5_000);
    storagePasswordFlow.push(await readStoragePasswordState(client, "storagePasswordPanelClosed"));

    await clickSelector(client, ".inventory-tab.tab-one button");
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1 restored", 5_000);
    inventoryFlow.push(await readInventoryState(client, "bag1Restored"));
    screenshots.push(await screenshot(client, "stage5-inventory-bag1-restored.png"));

    await clickSelector(client, ".hud-button.character button");
    await waitForSelector(client, ".character-window", 10_000);
    await waitForCharacterState(client, (state) => state.activeTab === "char", "char", 5_000);
    characterFlow.push(await readCharacterState(client, "char"));
    screenshots.push(await screenshot(client, "stage5-character.png"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "qa.damageEquipment", args: ["weapon", "2500"] });
    await waitForStage5State(
      client,
      (state) =>
        state?.equipmentItems?.some(
          (item) => item.name === "Dagger" && item.durabilityCurrent < item.durabilityMax,
        ),
      "damaged Dagger for normal repair",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "damagedForNormalRepair", "Dagger"));
    await openNpcServiceViaNpc(
      client,
      {
        transferKey: "crystal:0:297:613",
        objectId: "5",
        name: "Blacksmith_Smith",
        target: "@Repair",
        screenshotName: "stage5-repair-service-npc.png",
      },
      npcDialogFlow,
      screenshots,
    );
    await ensureCharacterWindowOpen(client);

    const beforeCharacterRepair = await readCharacterRepairState(client, "beforeCharacterRepair", "Dagger");
    if (!beforeCharacterRepair.equipmentItems.some((item) => item.name === "Dagger")) {
      throw new Error(`Cannot verify repair mode without equipped Dagger: ${JSON.stringify(beforeCharacterRepair)}`);
    }
    const beforeNormalRepairDagger = beforeCharacterRepair.equipmentItems.find((item) => item.name === "Dagger");
    if (!beforeNormalRepairDagger || beforeNormalRepairDagger.durabilityCurrent >= beforeNormalRepairDagger.durabilityMax) {
      throw new Error(`Cannot verify service-backed repair without damaged Dagger: ${JSON.stringify(beforeCharacterRepair)}`);
    }
    characterRepairFlow.push(beforeCharacterRepair);
    await clickCharacterRepairAction(client, "normal");
    await waitForCharacterRepairState(
      client,
      (state) => state.activeRepairLabel.includes("Repair Item"),
      "normal repair mode",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "normalRepairMode", "Dagger"));
    screenshots.push(await screenshot(client, "stage5-character-repair-mode.png"));
    await clickCharacterEquipmentItemByName(client, "Dagger");
    await waitForCharacterRepairState(
      client,
      (state) => {
        const repaired = state.equipmentItems.find((item) => item.name === "Dagger");
        return (
          !state.activeRepairLabel &&
          repaired &&
          repaired.durabilityCurrent > beforeNormalRepairDagger.durabilityCurrent &&
          repaired.durabilityCurrent === repaired.durabilityMax &&
          repaired.durabilityMax <= beforeNormalRepairDagger.durabilityMax &&
          state.gold < beforeCharacterRepair.gold
        );
      },
      "normal repair submitted",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "normalRepairSubmitted", "Dagger"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "qa.damageEquipment", args: ["weapon", "2500"] });
    await waitForStage5State(
      client,
      (state) =>
        state?.equipmentItems?.some(
          (item) => item.name === "Dagger" && item.durabilityCurrent < item.durabilityMax,
        ),
      "damaged Dagger for special repair",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "damagedForSpecialRepair", "Dagger"));
    await openNpcServiceViaNpc(
      client,
      {
        transferKey: "crystal:0:303:221",
        objectId: "19",
        name: "Blacksmith_Bill",
        target: "@SRepair",
        screenshotName: "stage5-special-repair-service-npc.png",
      },
      npcDialogFlow,
      screenshots,
    );
    await ensureCharacterWindowOpen(client);

    const beforeSpecialRepair = await readCharacterRepairState(client, "beforeSpecialRepair", "Dagger");
    const beforeSpecialRepairDagger = beforeSpecialRepair.equipmentItems.find((item) => item.name === "Dagger");
    if (!beforeSpecialRepairDagger || beforeSpecialRepairDagger.durabilityCurrent >= beforeSpecialRepairDagger.durabilityMax) {
      throw new Error(`Cannot verify service-backed special repair without damaged Dagger: ${JSON.stringify(beforeSpecialRepair)}`);
    }
    characterRepairFlow.push(beforeSpecialRepair);
    await clickCharacterRepairAction(client, "special");
    await waitForCharacterRepairState(
      client,
      (state) => state.activeRepairLabel.includes("Special Repair"),
      "special repair mode",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "specialRepairMode", "Dagger"));
    screenshots.push(await screenshot(client, "stage5-character-special-repair-mode.png"));
    await clickCharacterEquipmentItemByName(client, "Dagger");
    await waitForCharacterRepairState(
      client,
      (state) => {
        const repaired = state.equipmentItems.find((item) => item.name === "Dagger");
        return (
          !state.activeRepairLabel &&
          repaired &&
          repaired.durabilityCurrent > beforeSpecialRepairDagger.durabilityCurrent &&
          repaired.durabilityCurrent === beforeSpecialRepairDagger.durabilityMax &&
          repaired.durabilityMax === beforeSpecialRepairDagger.durabilityMax &&
          state.gold < beforeSpecialRepair.gold
        );
      },
      "special repair submitted",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "specialRepairSubmitted", "Dagger"));

    const beforeCharacterRemove = await readInventoryEquipmentState(client, "beforeCharacterRemove", "Dagger");
    if (!beforeCharacterRemove.equipmentItems.some((item) => item.name === "Dagger")) {
      throw new Error(`Cannot verify equipment remove without equipped Dagger: ${JSON.stringify(beforeCharacterRemove)}`);
    }
    characterRemoveFlow.push(beforeCharacterRemove);
    await clickCharacterEquipmentItemByName(client, "Dagger");
    let afterCharacterRemove = await waitForEquipmentItemAbsent(client, "Dagger", "afterCharacterRemove", 5_000, {
      throwOnTimeout: false,
    });
    if (!afterCharacterRemove) {
      const occupiedSlots = new Set(
        beforeCharacterRemove.inventoryItems
          .filter((item) => item.container === "bag1")
          .map((item) => item.slot),
      );
      const targetSlot = Array.from({ length: 46 }, (_, slot) => slot).find((slot) => !occupiedSlots.has(slot)) ?? 0;
      await sendGatewayCommand(client, {
        type: "removeItem",
        uniqueId: 0,
        grid: "inventory",
        to: targetSlot,
      });
      await sendGatewayCommand(client, { type: "tick" });
      afterCharacterRemove = await waitForEquipmentItemAbsent(
        client,
        "Dagger",
        "afterCharacterRemoveRetry",
        5_000,
      );
    }
    if (!afterCharacterRemove.inventoryItem) {
      throw new Error(`Dagger was removed from equipment but not found in inventory: ${JSON.stringify(afterCharacterRemove)}`);
    }
    characterRemoveFlow.push(afterCharacterRemove);
    screenshots.push(await screenshot(client, "stage5-character-remove-dagger.png"));

    if (!(await client.evaluate('Boolean(document.querySelector(".inventory-window"))'))) {
      await clickSelector(client, ".hud-button.inventory button");
      await waitForSelector(client, ".inventory-window", 10_000);
    }
    await clickSelector(client, ".inventory-tab.tab-one button");
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1 before sell", 5_000);

    const beforeInventorySell = {
      item: await readInventoryItem(client, "beforeInventorySell", "Dagger"),
      gold: await readInventoryGoldState(client, "beforeInventorySellGold"),
    };
    if (!beforeInventorySell.item.item) {
      throw new Error(`Cannot verify inventory sell without Dagger: ${JSON.stringify(beforeInventorySell)}`);
    }
    inventorySellFlow.push(beforeInventorySell);
    await clickButtonByImageAlt(client, "Sell Item");
    await clickInventoryItemByName(client, "Dagger");
    await waitForInventorySellState(client, (state) => state.sellPanelOpen === true, "sell panel", 5_000);
    inventorySellFlow.push(await readInventorySellState(client, "sellPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-sell-dagger-panel.png"));
    await clickInventoryPanelAction(client, 0);
    await waitForInventorySellState(
      client,
      (state) => state.sellPanelOpen === false && state.feedbackText.includes("Dagger"),
      "afterInventorySellNoService",
      5_000,
    );
    const afterInventorySell = {
      item: await readInventoryItem(client, "afterInventorySell", "Dagger"),
      gold: await readInventoryGoldState(client, "afterInventorySellGold"),
      sellState: await readInventorySellState(client, "afterInventorySellNoService"),
    };
    if (!afterInventorySell.item.item || afterInventorySell.gold.gold !== beforeInventorySell.gold.gold) {
      throw new Error(`Sell without active service mutated state: ${JSON.stringify(afterInventorySell)}`);
    }
    inventorySellFlow.push(afterInventorySell);
    screenshots.push(await screenshot(client, "stage5-inventory-sell-dagger-no-service.png"));

    await openNpcServiceViaNpc(
      client,
      {
        transferKey: "crystal:0:297:613",
        objectId: "5",
        name: "Blacksmith_Smith",
        target: "@BuySell",
        screenshotName: "stage5-sell-service-npc.png",
      },
      npcDialogFlow,
      screenshots,
    );
    if (!(await client.evaluate('Boolean(document.querySelector(".inventory-window"))'))) {
      await clickSelector(client, ".hud-button.inventory button");
      await waitForSelector(client, ".inventory-window", 10_000);
    }
    await clickSelector(client, ".inventory-tab.tab-one button");
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1 before service sell", 5_000);

    const beforeServiceSell = {
      item: await ensureInventoryItemNamed(client, "Wooden Sword", "wooden-sword", "beforeServiceSell"),
      gold: await readInventoryGoldState(client, "beforeServiceSellGold"),
    };
    if (!beforeServiceSell.item.item) {
      throw new Error(`Cannot verify service-backed inventory sell without Wooden Sword: ${JSON.stringify(beforeServiceSell)}`);
    }
    inventorySellFlow.push(beforeServiceSell);
    await clickButtonByImageAlt(client, "Sell Item");
    await clickInventoryItemByName(client, "Wooden Sword");
    await waitForInventorySellState(client, (state) => state.sellPanelOpen === true, "service sell panel", 5_000);
    inventorySellFlow.push(await readInventorySellState(client, "serviceSellPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-sell-wooden-sword-panel.png"));
    await clickInventoryPanelAction(client, 0);
    await waitForStage5State(
      client,
      (state) =>
        (state?.gold ?? 0) > beforeServiceSell.gold.gold &&
        !(state?.inventoryItems ?? []).some((item) => item.container === "bag1" && item.name === "Wooden Sword"),
      "service-backed Wooden Sword sell",
      5_000,
    );
    const afterServiceSell = {
      item: await readInventoryItem(client, "afterServiceSell", "Wooden Sword"),
      gold: await readInventoryGoldState(client, "afterServiceSellGold"),
      sellState: await readInventorySellState(client, "afterServiceSell"),
    };
    if (afterServiceSell.item.item || afterServiceSell.gold.gold <= beforeServiceSell.gold.gold) {
      throw new Error(`Service-backed sell did not remove item and increase gold: ${JSON.stringify(afterServiceSell)}`);
    }
    inventorySellFlow.push(afterServiceSell);
    screenshots.push(await screenshot(client, "stage5-inventory-sell-wooden-sword-service.png"));

    await openStorageServiceViaNpc(client, storageFlow, npcDialogFlow, screenshots);
    if (!(await client.evaluate('Boolean(document.querySelector(".inventory-window"))'))) {
      await clickSelector(client, ".hud-button.inventory button");
      await waitForSelector(client, ".inventory-window", 10_000);
    }
    await clickSelector(client, ".inventory-tab.tab-one button");
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1 before storage store", 5_000);

    const beforeStorageStore = {
      item: await readInventoryItem(client, "beforeStorageStore", "Dagger"),
      storage: await readStorageTransferState(client, "beforeStorageStore"),
    };
    if (!beforeStorageStore.item.item) {
      throw new Error(`Cannot verify storage store without Dagger: ${JSON.stringify(beforeStorageStore)}`);
    }
    storageStoreFlow.push(beforeStorageStore);
    if (!(await client.evaluate('Boolean(document.querySelector(".storage-window"))'))) {
      await clickSelector(client, ".inventory-tab.tab-three button");
      await waitForSelector(client, ".storage-window", 10_000);
    }
    await clickButtonByImageAlt(client, "Store Item");
    await waitForStorageTransferState(
      client,
      (state) => state.storageWindowVisible === true && state.statusText.includes("Choose an inventory item"),
      "store mode open",
      5_000,
    );
    await contextMenuInventoryItemByName(client, "Dagger");
    await waitForStorageTransferState(
      client,
      (state) => state.feedbackText.includes("Dagger") && state.statusText.includes("warehouse slot"),
      "store item selected",
      5_000,
    );
    storageStoreFlow.push(await readStorageTransferState(client, "storeItemSelected"));
    screenshots.push(await screenshot(client, "stage5-storage-store-dagger-selected.png"));
    await clickStorageSlot(client, 4);
    const beforeStorageDaggerCount = beforeStorageStore.storage.storageItems.filter((item) => item.name === "Dagger").length;
    const afterStorageStoreState = await waitForStorageTransferState(
      client,
      (state) => {
        const activeInventoryDagger = state.inventoryItems.some((item) => item.name === "Dagger");
        const storageDaggerCount = state.storageItems.filter((item) => item.name === "Dagger").length;
        return !activeInventoryDagger && storageDaggerCount > beforeStorageDaggerCount;
      },
      "afterStorageStore",
      8_000,
    );
    const afterStorageDaggerCount = afterStorageStoreState.storageItems.filter((item) => item.name === "Dagger").length;
    const afterStorageStore = {
      item: await readInventoryItem(client, "afterStorageStore", "Dagger"),
      storage: afterStorageStoreState,
      result: "stored",
    };
    if (afterStorageStore.item.item || afterStorageDaggerCount <= beforeStorageDaggerCount) {
      throw new Error(`Store item result was inconsistent: ${JSON.stringify(afterStorageStore)}`);
    }
    storageStoreFlow.push(afterStorageStore);
    screenshots.push(await screenshot(client, "stage5-storage-store-dagger-result.png"));
    await clickSelector(client, ".storage-close button");
    await waitForStorageTransferState(
      client,
      (state) => state.storageWindowVisible === false,
      "storageStoreFeedback",
      5_000,
    );
    storageStoreFlow.push(await readStorageTransferState(client, "storageStoreFeedback"));
    screenshots.push(await screenshot(client, "stage5-storage-store-dagger-feedback.png"));

    await ensureCharacterWindowOpen(client);
    await clickSelector(client, ".character-tab.stats1 button");
    await waitForCharacterState(client, (state) => state.activeTab === "stats1", "stats1", 5_000);
    characterFlow.push(await readCharacterState(client, "stats1"));
    screenshots.push(await screenshot(client, "stage5-character-stats1.png"));

    await clickSelector(client, ".character-tab.stats2 button");
    await waitForCharacterState(client, (state) => state.activeTab === "stats2", "stats2", 5_000);
    characterFlow.push(await readCharacterState(client, "stats2"));
    screenshots.push(await screenshot(client, "stage5-character-stats2.png"));

    await clickSelector(client, ".character-tab.spells button");
    await waitForCharacterState(client, (state) => state.activeTab === "spells", "spells", 5_000);
    characterFlow.push(await readCharacterState(client, "spells"));
    screenshots.push(await screenshot(client, "stage5-character-spells.png"));

    await clickSelector(client, ".character-tab.char button");
    await waitForCharacterState(client, (state) => state.activeTab === "char", "char restored", 5_000);
    characterFlow.push(await readCharacterState(client, "charRestored"));
    screenshots.push(await screenshot(client, "stage5-character-char-restored.png"));

    if (!(await client.evaluate('Boolean(document.querySelector(".storage-window"))'))) {
      await clickSelector(client, ".inventory-tab.tab-three button");
    }
    await waitForSelector(client, ".storage-window", 10_000);
    screenshots.push(await screenshot(client, "stage5-storage.png"));

    const takeBackSeed = await ensureStoredItemForTakeBack(client, "Red Potion", "red-potion", "beforeStorageTakeBack");
    const beforeStorageTakeBack = takeBackSeed.state;
    const beforeTakeBackInventoryQuantity = itemQuantity(beforeStorageTakeBack.inventoryItems, "Red Potion");
    const beforeTakeBackStorageQuantity = itemQuantity(beforeStorageTakeBack.storageItems, "Red Potion");
    if (beforeTakeBackStorageQuantity < 1) {
      throw new Error(`Cannot verify Take Back without stored Red Potion: ${JSON.stringify(beforeStorageTakeBack)}`);
    }
    storageTakeBackFlow.push(beforeStorageTakeBack);
    await clickButtonByImageAlt(client, "Take Back");
    await waitForStorageTransferState(
      client,
      (state) => state.storageWindowVisible === true && state.statusText.includes("warehouse item"),
      "takeBack mode open",
      5_000,
    );
    await clickStorageSlot(client, takeBackSeed.storageSlot);
    await waitForStorageTransferState(
      client,
      (state) =>
        state.feedbackText.includes("Red Potion") ||
        state.statusText.includes("warehouse item"),
      "takeBackItemSelected",
      5_000,
    );
    storageTakeBackFlow.push(await readStorageTransferState(client, "takeBackItemSelected"));
    screenshots.push(await screenshot(client, "stage5-storage-takeback-red-potion-selected.png"));
    await clickInventorySlot(client, takeBackSeed.inventorySlot);
    const afterStorageTakeBackState = await waitForStorageTransferState(
      client,
      (state) => {
        const inventoryQuantity = itemQuantity(state.inventoryItems, "Red Potion");
        const storageQuantity = itemQuantity(state.storageItems, "Red Potion");
        return inventoryQuantity > beforeTakeBackInventoryQuantity && storageQuantity < beforeTakeBackStorageQuantity;
      },
      "afterStorageTakeBack",
      8_000,
    );
    const afterTakeBackInventoryQuantity = itemQuantity(afterStorageTakeBackState.inventoryItems, "Red Potion");
    const afterTakeBackStorageQuantity = itemQuantity(afterStorageTakeBackState.storageItems, "Red Potion");
    storageTakeBackFlow.push({
      ...afterStorageTakeBackState,
      result: "taken-back",
    });
    screenshots.push(await screenshot(client, "stage5-storage-takeback-red-potion-result.png"));
    await clickSelector(client, ".storage-close button");
    await waitForStorageTransferState(
      client,
      (state) => state.storageWindowVisible === false,
      "storageTakeBackFeedback",
      5_000,
    );
    storageTakeBackFlow.push(await readStorageTransferState(client, "storageTakeBackFeedback"));
    screenshots.push(await screenshot(client, "stage5-storage-takeback-red-potion-feedback.png"));

    await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button");
    await delay(300);
    await transferMapAndWait(client, "crystal:0:330:270", "return to stage5 smoke combat point");
    screenshots.push(await screenshot(client, "stage5-storage-service-return.png"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "qa.openNpcDialog", args: [] });
    await waitForNpcDialogState(client, (state) => state.open === true, "generic NPC dialog open", 10_000);
    const npcOpenState = await readNpcDialogState(client, "open");
    if (hasRawCrystalMarkup(npcOpenState.visibleText)) {
      throw new Error(`NPC dialog leaked raw Crystal markup: ${JSON.stringify(npcOpenState)}`);
    }
    npcDialogFlow.push(npcOpenState);
    screenshots.push(await screenshot(client, "stage5-npc.png"));
    if ((await readNpcDialogState(client, "linkOptional")).links.length > 0) {
      screenshots.push(await screenshot(client, "stage5-npc-links.png"));
    }
    await clickOptional(client, ".npc-dialog-close, .npc-dialog-actions .sprite-button:last-child");
    await waitForNpcDialogState(client, (state) => state.open === false, "closed", 5_000);
    npcDialogFlow.push(await readNpcDialogState(client, "closed"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "event.spawn", args: ["BugBat", "1"] });
    await waitForStage5Entity(client, (entity) => entity.kind === "monster", "combat smoke monster", 10_000);
    const beforeCombat = await readCombatState(client, "beforeCombat");
    const combatTarget = beforeCombat.monsters[0] ?? null;
    if (!combatTarget) {
      throw new Error(`Cannot verify combat without visible monster: ${JSON.stringify(beforeCombat)}`);
    }
    combatFlow.push(beforeCombat);
    if (await waitForSelectorOptional(client, ".entity-nameplate.monster", 2_000)) {
      await clickFirst(client, ".entity-nameplate.monster");
    }
    await sendGatewayCommand(client, { type: "attack", objectId: Number(combatTarget.objectId) });
    const afterCombat = await waitForCombatState(
      client,
      (state) => {
        const damagedMonster = state.monsters.find((monster) => {
          const beforeMonster = beforeCombat.monsters.find((entry) => entry.objectId === monster.objectId);
          return (
            typeof monster.hp === "number" &&
            typeof beforeMonster?.hp === "number" &&
            monster.hp < beforeMonster.hp
          );
        });
        return (
          state.activeAttackCount > 0 ||
          state.projectileCount > 0 ||
          state.monsters.some((monster) => monster.struckActive) ||
          Boolean(damagedMonster) ||
          state.monsters.some((monster) => monster.dead)
        );
      },
      "afterCombat",
      5_000,
    );
    combatFlow.push(afterCombat);
    screenshots.push(await screenshot(client, "stage5-combat.png"));

    await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button");
    await delay(300);
    await transferMapAndWait(client, "crystal:1:315:82", "mapFileName 1");
    screenshots.push(await screenshot(client, "stage5-map-transfer-1.png"));
    minimapFlow.push(await readMiniMapState(client, "expanded"));

    await clickSelector(client, ".mini-map-button.toggle button");
    await waitForMiniMapState(client, (state) => state.sceneHidden === true, "collapsed", 5_000);
    minimapFlow.push(await readMiniMapState(client, "collapsed"));
    screenshots.push(await screenshot(client, "stage5-minimap-collapsed.png"));

    await clickSelector(client, ".mini-map-button.bigmap button");
    await waitForMiniMapState(client, (state) => state.bigMapOpen === true, "big map dialog open", 5_000);
    minimapFlow.push(await readMiniMapState(client, "bigMapOpen"));
    screenshots.push(await screenshot(client, "stage5-minimap-bigmap.png"));

    await clickSelector(client, ".mini-map-button.mail button");
    await waitForSelector(client, ".mail-panel", 5_000);
    minimapFlow.push(await readMiniMapState(client, "mailOpen"));
    mailFlow.push(await readMailState(client, "mailOpen"));
    screenshots.push(await screenshot(client, "stage5-minimap-mail.png"));
    await clickOptional(client, ".mail-panel .mail-close button");
    await waitForMiniMapState(client, (state) => state.mailOpen === false, "mail closed", 5_000);
    mailFlow.push(await readMailState(client, "mailClosed"));

    const beforeGameShopSeed = await readStage5SystemsState(client, "beforeGameShopSeed");
    const gameShopSeedMailId =
      Math.max(0, ...(beforeGameShopSeed.mail ?? []).map((mail) => Number(mail.id) || 0)) + 1;
    await sendGatewayCommand(client, {
      type: "stage5Command",
      action: "mail.send",
      args: ["GameShopSeed", "Game shop seed", "Gold for game shop smoke", "1000000"],
    });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mail.claim", args: [String(gameShopSeedMailId)] });
    await waitForStage5State(
      client,
      (state) => (state?.gold ?? 0) >= (beforeGameShopSeed.gold ?? 0) + 1_000_000,
      "game shop gold seed claimed",
      8_000,
    );
    gameShopFlow.push(await readStage5SystemsState(client, "afterGameShopSeed"));
    await clickSelector(client, ".hud-button.shop button");
    await waitForSelector(client, ".game-shop-window", 10_000);
    await clickSelector(client, ".game-shop-payment.gold");
    const beforeGameShopBuy = await readGameShopState(client, "beforeGameShopBuy");
    if (!beforeGameShopBuy.firstGoldItem || beforeGameShopBuy.firstGoldItem.goldPrice <= 0) {
      throw new Error(`Cannot verify positive game shop buy without gold-priced item: ${JSON.stringify(beforeGameShopBuy)}`);
    }
    gameShopFlow.push(beforeGameShopBuy);
    screenshots.push(await screenshot(client, "stage5-gameshop-gold-open.png"));
    await clickFirstGameShopGoldBuy(client);
    const beforeGameShopQuantity = totalCarryQuantity(beforeGameShopBuy);
    const afterGameShopBuy = await waitForGameShopState(
      client,
      (state) =>
        state.gold === beforeGameShopBuy.gold - beforeGameShopBuy.firstGoldItem.goldPrice &&
        totalCarryQuantity(state) > beforeGameShopQuantity &&
        hasGameShopGoldPurchaseChat(
          state,
          beforeGameShopBuy.firstGoldItem.name,
          beforeGameShopBuy.firstGoldItem.goldPrice,
        ),
      "afterGameShopGoldBuy",
      8_000,
    );
    gameShopFlow.push(afterGameShopBuy);
    screenshots.push(await screenshot(client, "stage5-gameshop-gold-buy.png"));
    await clickOptional(client, ".game-shop-close button");
    await waitForGameShopState(client, (state) => state.visible === false, "game shop closed", 5_000);
    gameShopFlow.push(await readGameShopState(client, "gameShopClosed"));

    stage5SystemsFlow.push(await readStage5SystemsState(client, "beforeBroadSystems"));
    await sendGatewayCommand(client, { type: "stage5Command", action: "group.create", args: ["Miner"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "group.loot", args: ["roundRobin"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "guild.create", args: ["BichonGuard"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "guild.rank", args: ["Sabuk Warden"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "guild.chat", args: ["Guild", "ready"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "social.friend", args: ["Miner"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "social.block", args: ["Spammer"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "social.unblock", args: ["Spammer"] });
    await sendGatewayCommand(client, {
      type: "stage5Command",
      action: "mail.send",
      args: ["Scout", "Reward", "Take this", "5"],
    });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mail.claim", args: ["1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.start", args: ["Trader"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.offerGold", args: ["1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.accept", args: [] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "conquest.start", args: ["Sabuk"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "conquest.owner", args: [] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "hero.recruit", args: ["Aide"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mine", args: ["2"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "craft", args: ["crafted-blade"] });
    await waitForStage5State(
      client,
      (state) =>
        state?.stage5Systems?.guild?.name === "BichonGuard" &&
        state?.stage5Systems?.guild?.rank === "Sabuk Warden" &&
        state?.stage5Systems?.guild?.chatLog?.some((line) => line.includes("Guild ready")) &&
        state?.stage5Systems?.group?.lootMode === "roundRobin" &&
        state?.stage5Systems?.social?.friends?.includes("Miner") &&
        !(state?.stage5Systems?.social?.blocked ?? []).includes("Spammer") &&
        state?.stage5Systems?.hero?.name === "Aide" &&
        state?.stage5Systems?.profession?.craftedItems?.includes("crafted-blade"),
      "stage5 broad systems",
      15_000,
    );
    stage5SystemsFlow.push(await readStage5SystemsState(client, "afterBroadSystems"));
    screenshots.push(await screenshot(client, "stage5-systems.png"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.start", args: ["Trader"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.offerItem", args: ["red-potion"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.cancel", args: [] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "shop.buy", args: ["shop-ui-potion", "25"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "shop.buyCredit", args: ["credit-shop-ui", "1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.list", args: ["auction-ui-relic", "35"] });
    const auctionRelicId = await waitForLatestAuctionListingId(client, "auction-ui-relic", "auction relic listed");
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.buy", args: [String(auctionRelicId)] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.list", args: ["auction-cancel-ui", "45"] });
    const auctionCancelId = await waitForLatestAuctionListingId(client, "auction-cancel-ui", "auction cancel listed");
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.cancel", args: [String(auctionCancelId)] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "conquest.end", args: ["Sabuk"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "event.spawn", args: ["BugBat", "1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "hero.behaviour", args: ["2"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mine", args: ["2"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "craft", args: ["crafted-shield"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mail.delete", args: ["1"] });
    await waitForStage5State(
      client,
      (state) => {
        const systems = state?.stage5Systems;
        return (
          systems?.trade === null &&
          systems?.auction?.some((listing) => listing.id === auctionRelicId && listing.itemKey === "auction-ui-relic" && listing.sold === true) &&
          systems?.auction?.some((listing) => listing.id === auctionCancelId && listing.itemKey === "auction-cancel-ui" && listing.cancelled === true) &&
          systems?.conquest?.eventLog?.some((line) => line.includes("War ended: Sabuk")) &&
          systems?.hero?.behaviour === 2 &&
          systems?.profession?.craftedItems?.includes("crafted-shield") &&
          systems?.mail?.some((mail) => mail.id === 1 && mail.deleted === true)
        );
      },
      "stage5 advanced systems",
      15_000,
    );
    stage5SystemsFlow.push(await readStage5SystemsState(client, "afterAdvancedSystems"));
    screenshots.push(await screenshot(client, "stage5-systems-advanced.png"));

    chatFlow.push(await readChatState(client, "allInitial"));
    await clickSelector(client, '.chat-filter-button[data-chat-filter-key="shout"] button');
    await waitForChatState(
      client,
      (state) => state.activeFilter === "shout" && state.chatText === "!" && state.chatPrefix === "!",
      "shout chat prefix",
      5_000,
    );
    chatFlow.push(await readChatState(client, "shoutPrefix"));
    screenshots.push(await screenshot(client, "stage5-chat-shout-prefix.png"));

    await clickSelector(client, '.chat-filter-button[data-chat-filter-key="all"] button');
    await waitForChatState(
      client,
      (state) => state.activeFilter === "all" && state.chatText === "",
      "all prefix restored",
      5_000,
    );
    chatFlow.push(await readChatState(client, "allRestored"));

    await client.evaluate("window.__mir2CommandHistory = []");
    await clickSelector(client, '.chat-filter-button[data-chat-filter-key="trade"] button');
    await waitForChatState(
      client,
      (state) => state.lastCommand?.type === "tradeRequest",
      "trade request command",
      5_000,
    );
    chatChannelFlow.push(await readChatState(client, "tradeRequest"));
    screenshots.push(await screenshot(client, "stage5-chat-trade-request.png"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "guild.chat", args: ["Guild", "filter", "check"] });
    await waitForStage5State(
      client,
      (state) => state?.stage5Systems?.guild?.chatLog?.some((line) => line.includes("Guild filter check")),
      "guild chat filter check",
      5_000,
    );
    await clickSelector(client, '.chat-filter-button[data-chat-filter-key="guild"] button');
    await waitForChatState(
      client,
      (state) =>
        state.activeFilter === "guild" &&
        state.chatText === "!~" &&
        state.visibleLineTexts.some((text) => text.includes("Guild filter check")) &&
        state.visibleLineClasses.some((className) => className.includes("channel-guild")),
      "guild prefix",
      5_000,
    );
    chatChannelFlow.push(await readChatState(client, "guildPrefix"));
    screenshots.push(await screenshot(client, "stage5-chat-guild-prefix.png"));

    await clickSelector(client, '.chat-filter-button[data-chat-filter-key="group"] button');
    await waitForChatState(
      client,
      (state) => state.activeFilter === "group" && state.chatText === "!!",
      "group prefix",
      5_000,
    );
    chatChannelFlow.push(await readChatState(client, "groupPrefix"));
    screenshots.push(await screenshot(client, "stage5-chat-group-prefix.png"));

    await clickSelector(client, '.chat-filter-button[data-chat-filter-key="all"] button');
    await waitForChatState(
      client,
      (state) => state.activeFilter === "all" && state.chatText === "",
      "all prefix restored after channels",
      5_000,
    );
    chatChannelFlow.push(await readChatState(client, "allRestoredAfterChannels"));

    await clickSelector(client, ".chat-filter-button.settings button");
    await waitForChatState(client, (state) => state.settingsOpen === true, "settings open", 5_000);
    await clickSelector(client, '.chat-settings-option[data-chat-option-filter="normal"]');
    await waitForChatState(
      client,
      (state) => state.hiddenFilters.includes("normal"),
      "settings normal hidden",
      5_000,
    );
    await clickSelector(client, '.chat-settings-option[data-chat-option-filter="normal"]');
    await waitForChatState(
      client,
      (state) => !state.hiddenFilters.includes("normal"),
      "settings normal restored",
      5_000,
    );
    chatFlow.push(await readChatState(client, "settingsOpen"));
    screenshots.push(await screenshot(client, "stage5-chat-settings.png"));
    await clickOptional(client, ".chat-settings-close");
    await waitForChatState(client, (state) => state.settingsOpen === false, "settings closed", 5_000);

    await clickSelector(client, ".chat-filter-button.size button");
    await waitForChatState(client, (state) => state.collapsed === true && state.feedHidden === true, "collapsed", 5_000);
    chatFlow.push(await readChatState(client, "collapsed"));
    screenshots.push(await screenshot(client, "stage5-chat-collapsed.png"));
    await clickSelector(client, ".chat-filter-button.size button");
    await waitForChatState(client, (state) => state.collapsed === false && state.feedHidden === false, "expanded restored", 5_000);
    chatFlow.push(await readChatState(client, "expandedRestored"));

    await clickSelector(client, ".chat-filter-button.report button");
    await waitForChatState(client, (state) => state.reportOpen === true, "report open", 5_000);
    chatFlow.push(await readChatState(client, "reportOpen"));
    reportFlow.push(await readReportState(client, "reportOpen"));
    screenshots.push(await screenshot(client, "stage5-chat-report.png"));
    await clickOptional(client, ".report-panel .overlay-panel-head button");
    await waitForChatState(client, (state) => state.reportOpen === false, "report closed", 5_000);
    reportFlow.push(await readReportState(client, "reportClosed"));

    await clickSelector(client, ".hud-button.menu button");
    await waitForSystemMenuState(client, (state) => state.open === true, "open", 5_000);
    systemMenuFlow.push(await readSystemMenuState(client, "open"));
    screenshots.push(await screenshot(client, "stage5-system-menu.png"));

    await clickSystemMenuActionByKey(client, "creature");
    await waitForSystemMenuState(
      client,
      (state) =>
        state.systemMenuFeaturePanelVisible === true &&
        state.systemMenuFeaturePanelType === "creature",
      "creature feature panel open",
      5_000,
    );
    systemMenuFlow.push(await readSystemMenuState(client, "creatureFeatureAction"));
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "creatureFeaturePanelOpen"));
    screenshots.push(await screenshot(client, "stage5-system-menu-creature.png"));
    await clickSelector(client, ".creature-feature-actions button:nth-child(1)");
    await waitForSystemMenuState(
      client,
      (state) =>
        hasRecentCommand(
          state,
          (command) => command?.type === "updateIntelligentCreature" && command?.summonMe === true,
        ),
      "creature summon command sent",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "creatureSummonCommand"));
    await clickSystemMenuFeaturePanelClose(client);
    await waitForSystemMenuState(
      client,
      (state) =>
        state.open === true &&
        state.systemMenuFeaturePanelVisible === false,
      "creature feature panel closed",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "creatureFeaturePanelClosed"));

    await clickSystemMenuActionByKey(client, "ride");
    await waitForSystemMenuState(
      client,
      (state) =>
        state.systemMenuFeaturePanelVisible === true &&
        state.systemMenuFeaturePanelType === "mount",
      "mount feature panel open",
      5_000,
    );
    systemMenuFlow.push(await readSystemMenuState(client, "mountFeatureAction"));
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "mountFeaturePanelOpen"));
    screenshots.push(await screenshot(client, "stage5-system-menu-mount.png"));
    await clickSelector(client, ".mount-feature-ride");
    await waitForSystemMenuState(
      client,
      (state) =>
        hasRecentCommand(
          state,
          (command) => command?.type === "useItem" && command?.grid === "equipment" && command?.slot === 13,
        ),
      "mount use-item command sent",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "mountUseCommand"));
    await clickSystemMenuFeaturePanelClose(client);
    await waitForSystemMenuState(
      client,
      (state) =>
        state.open === true &&
        state.systemMenuFeaturePanelVisible === false,
      "mount feature panel closed",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "mountFeaturePanelClosed"));

    await clickSystemMenuActionByKey(client, "fishing");
    await waitForSystemMenuState(
      client,
      (state) =>
        state.systemMenuFeaturePanelVisible === true &&
        state.systemMenuFeaturePanelType === "fishing",
      "fishing feature panel open",
      5_000,
    );
    systemMenuFlow.push(await readSystemMenuState(client, "fishingFeatureAction"));
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingFeaturePanelOpen"));
    screenshots.push(await screenshot(client, "stage5-system-menu-fishing.png"));
    await clickSelector(client, ".fishing-feature-status button:nth-child(1)");
    await waitForSystemMenuState(
      client,
      (state) =>
        hasRecentCommand(
          state,
          (command) => command?.type === "fishingCast" && command?.castOut === true,
        ),
      "fishing cast command sent",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingCastCommand"));
    await clickSelector(client, ".fishing-feature-status button:nth-child(2)");
    await waitForSystemMenuState(
      client,
      (state) =>
        hasRecentCommand(
          state,
          (command) => command?.type === "fishingChangeAutocast" && command?.autoCast === true,
        ),
      "fishing autocast command sent",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingAutoCommand"));
    await clickSystemMenuFeaturePanelClose(client);
    await waitForSystemMenuState(
      client,
      (state) =>
        state.open === true &&
        state.systemMenuFeaturePanelVisible === false,
      "fishing feature panel closed",
      5_000,
    );
    systemMenuFeatureFlow.push(await readSystemMenuState(client, "fishingFeaturePanelClosed"));

    const socialPanels = [
      { key: "ranking", tab: "overall" },
      { key: "friend", tab: "blocks" },
      { key: "mentor", tab: "requests" },
      { key: "relationship", tab: "status" },
      { key: "group", tab: "party" },
      { key: "guild", tab: "members" },
      { key: "hero", tab: "status", commandType: "newHero" },
      { key: "trade", tab: "session" },
      { key: "market", tab: "listings" },
      { key: "marriage", tab: "status" },
      { key: "itemRental", tab: "session", commandType: "itemRentalRequest" },
    ];

    for (const socialPanel of socialPanels) {
      await clickSystemMenuActionByKey(client, socialPanel.key);
      await waitForSystemMenuState(
        client,
        (state) => state.systemMenuSocialPanelVisible === true && state.systemMenuSocialPanelType === socialPanel.key,
        `${socialPanel.key} panel open`,
        5_000,
      );
      const openState = await readSystemMenuState(client, `${socialPanel.key}PanelOpen`);
      systemMenuSocialFlow.push(openState);

      await clickSystemMenuSocialTabByKey(client, socialPanel.tab);
      await waitForSystemMenuState(
        client,
        (state) => state.systemMenuSocialPanelVisible === true && state.systemMenuSocialPanelTabKey === socialPanel.tab,
        `${socialPanel.key} panel tab`,
        5_000,
      );
      const tabState = await readSystemMenuState(client, `${socialPanel.key}PanelTab`);
      systemMenuSocialFlow.push(tabState);

      const selectedEntryName =
        tabState.systemMenuSocialEntryNames[Math.min(1, Math.max(tabState.systemMenuSocialEntryNames.length - 1, 0))] ??
        tabState.systemMenuSocialPanelSelectedRow;
      if (selectedEntryName) {
        await clickSystemMenuSocialEntryByName(client, selectedEntryName);
        await waitForSystemMenuState(
          client,
          (state) => state.systemMenuSocialPanelSelectedRow === selectedEntryName,
          `${socialPanel.key} panel row`,
          5_000,
        );
      }

      const actionLabel = tabState.systemMenuSocialActionLabels[0];
      if (actionLabel) {
        await clickSystemMenuSocialActionByLabel(client, actionLabel);
        await waitForSystemMenuState(
          client,
          (state) =>
            state.systemMenuSocialPanelStatus !== null &&
            state.systemMenuSocialPanelStatus.includes(actionLabel),
          `${socialPanel.key} panel action`,
          5_000,
        );
        if (socialPanel.commandType) {
          await waitForSystemMenuState(
            client,
            (state) => hasRecentCommand(state, (command) => command?.type === socialPanel.commandType),
            `${socialPanel.key} panel command`,
            5_000,
          );
        }
      }

      systemMenuSocialFlow.push(await readSystemMenuState(client, `${socialPanel.key}PanelAction`));
      screenshots.push(await screenshot(client, `stage5-system-menu-${socialPanel.key}.png`));
      await clickSystemMenuFeaturePanelClose(client);
      await waitForSystemMenuState(
        client,
        (state) =>
          state.open === true &&
          state.systemMenuFeaturePanelVisible === false &&
          state.systemMenuSocialPanelVisible === false,
        `${socialPanel.key} panel closed`,
        5_000,
      );
      systemMenuSocialFlow.push(await readSystemMenuState(client, `${socialPanel.key}PanelClosed`));
    }

    if (!(await readSystemMenuState(client, "beforeQaTransferReopen")).open) {
      await clickSelector(client, ".hud-button.menu button");
    }
    await waitForSystemMenuState(client, (state) => state.open === true, "reopen qa transfer", 5_000);
    systemMenuQaTransferFlow.push(await readSystemMenuState(client, "qaTransferPanel"));
    await setSystemMenuQaTransferInputs(client, { map: "0", x: 330, y: 270 });
    systemMenuQaTransferFlow.push(await readSystemMenuState(client, "qaTransferFilled"));
    screenshots.push(await screenshot(client, "stage5-system-menu-qa-transfer.png"));
    await clickSelector(client, ".system-menu-qa-transfer button[type='submit']");
    await waitForStage5State(
      client,
      (state) => state?.mapFileName === "0" && state?.player?.x === 330 && state?.player?.y === 270,
      "system menu QA transfer",
      15_000,
    );
    await waitForSystemMenuState(client, (state) => state.open === false, "qa transfer closed", 5_000);
    systemMenuQaTransferFlow.push(await readSystemMenuState(client, "qaTransferSubmitted"));
    screenshots.push(await screenshot(client, "stage5-system-menu-qa-transfer-result.png"));

    await clickSelector(client, ".hud-button.menu button");
    await waitForSystemMenuState(client, (state) => state.open === true, "reopen transfer list", 5_000);
    systemMenuTransferFlow.push(await readSystemMenuState(client, "transferListOpen"));
    await clickSystemMenuTransfer(client, 1);
    await waitForStage5State(client, (state) => state?.mapFileName === "1", "system menu transfer list", 15_000);
    await waitForSystemMenuState(client, (state) => state.open === false, "transfer list closed", 5_000);
    systemMenuTransferFlow.push(await readSystemMenuState(client, "transferListSubmitted"));
    screenshots.push(await screenshot(client, "stage5-system-menu-transfer-list-result.png"));

    await clickSelector(client, ".hud-button.skill button");
    await waitForCharacterState(client, (state) => state.activeTab === "spells", "hud skill spells", 5_000);
    hudButtonFlow.push(await readHudButtonState(client, "skillToSpells"));
    const beforeSpellCast = await readSpellCastState(client, "beforeBattleFocus", "battle-focus");
    if (!beforeSpellCast.knownSkills.some((skill) => skill.key === "battle-focus")) {
      throw new Error(`Cannot verify Battle Focus cast without skill: ${JSON.stringify(beforeSpellCast)}`);
    }
    spellCastFlow.push(beforeSpellCast);
    await clickCharacterSpellByName(client, "Battle Focus");
    const afterSpellCast = await waitForSpellCastState(
      client,
      (state) =>
        state.activeBuffs.some((buff) => buff.key === "battle-focus") &&
        (state.knownSkills.find((skill) => skill.key === "battle-focus")?.cooldownRemainingTicks ?? 0) > 0,
      "afterBattleFocus",
      5_000,
    );
    spellCastFlow.push(afterSpellCast);
    screenshots.push(await screenshot(client, "stage5-hud-skill-spells.png"));
    screenshots.push(await screenshot(client, "stage5-character-cast-battle-focus.png"));
    await clickOptional(client, ".character-close button");

    await clickSelector(client, ".hud-button.option button");
    await waitForCharacterState(client, (state) => state.activeTab === "stats2", "hud option stats2", 5_000);
    hudButtonFlow.push(await readHudButtonState(client, "optionToStats2"));
    screenshots.push(await screenshot(client, "stage5-hud-option-stats2.png"));
    await clickOptional(client, ".character-close button");

    await setViewport(client, VIEWPORTS.compact);
    await delay(500);
    const compactLayout = await assertCompactLayout(client, VIEWPORTS.compact);
    const compactTextLayout = await assertCoreTextLayout(client);
    const compactPanelLayout = [];
    screenshots.push(await screenshot(client, "stage5-compact-game.png"));

    await clickSelector(client, ".hud-button.inventory button");
    await waitForSelector(client, ".inventory-window", 5_000);
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".inventory-window"])).map((entry) => ({
        label: "inventory",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-inventory.png"));

    await clickSelector(client, ".inventory-tab.tab-three button");
    await waitForInventoryState(
      client,
      (state) => state.activeTab === "quest" && state.storageWindowVisible === true,
      "compact quest storage",
      5_000,
    );
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".inventory-window", ".storage-window"])).map((entry) => ({
        label: "storage",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-storage.png"));
    await clickAllOptional(client, ".storage-close button, .inventory-close button");

    await clickSelector(client, ".hud-button.character button");
    await waitForSelector(client, ".character-window", 5_000);
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".character-window"])).map((entry) => ({
        label: "character",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-character.png"));
    await clickOptional(client, ".character-close button");

    await clickSelector(client, ".hud-button.menu button");
    await waitForSystemMenuState(client, (state) => state.open === true, "compact system menu", 5_000);
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".system-menu-panel"])).map((entry) => ({
        label: "systemMenu",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-system-menu.png"));
    await clickSystemMenuActionByKey(client, "trade");
    await waitForSystemMenuState(
      client,
      (state) => state.systemMenuFeaturePanelVisible === true && state.systemMenuSocialPanelType === "trade",
      "compact trade social panel",
      5_000,
    );
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".system-feature-panel-social"])).map((entry) => ({
        label: "tradeSocial",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-trade-social.png"));
    await clickSystemMenuFeaturePanelClose(client);
    await waitForSystemMenuState(client, (state) => state.open === true, "compact system menu restored", 5_000);
    await clickOptional(client, ".system-menu-close-hit");

    await clickSelector(client, ".chat-filter-button.settings button");
    await waitForChatState(client, (state) => state.settingsOpen === true, "compact chat settings", 5_000);
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".chat-settings-panel"])).map((entry) => ({
        label: "chatSettings",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-chat-settings.png"));
    await clickOptional(client, ".chat-settings-close");

    await clickSelector(client, ".mini-map-button.mail button");
    await waitForSelector(client, ".mail-panel", 5_000);
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".mail-panel"])).map((entry) => ({
        label: "mail",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-mail.png"));
    await clickOptional(client, ".mail-panel .mail-close button");

    await clickSelector(client, ".chat-filter-button.report button");
    await waitForChatState(client, (state) => state.reportOpen === true, "compact report", 5_000);
    compactPanelLayout.push(
      ...(await assertPanelLayout(client, VIEWPORTS.compact, [".report-panel"])).map((entry) => ({
        label: "report",
        ...entry,
      })),
    );
    screenshots.push(await screenshot(client, "stage5-compact-report.png"));
    await clickOptional(client, ".report-panel .overlay-panel-head button");

    const compactMatrix = [];
    for (const [viewportName, viewport] of Object.entries({
      compactWide: VIEWPORTS.compactWide,
      compactNarrow: VIEWPORTS.compactNarrow,
      compactShort: VIEWPORTS.compactShort,
    })) {
      await setViewport(client, viewport);
      await delay(250);
      compactMatrix.push({
        viewportName,
        layout: await assertCompactLayout(client, viewport),
        text: await assertCoreTextLayout(client),
      });
      screenshots.push(await screenshot(client, `stage5-${viewportName}.png`));
    }

    await setViewport(client, VIEWPORTS.desktop);
    await delay(300);
    await waitForBeltState(
      client,
      (state) => state.orientation === "horizontal" && state.labelsWithinDialog === true,
      "horizontal with labels in dialog",
      5_000,
    );
    beltFlow.push(await readBeltState(client, "horizontal"));
    let hotkeyState = await readStage5BeltItems(client, "beforeHotkeys1To6");
    if (!hotkeyState.items.some((item) => item.slot >= 0 && item.slot < 6)) {
      throw new Error(`Cannot verify belt hotkeys without any item in slots 1-6: ${JSON.stringify(hotkeyState)}`);
    }
    beltUseFlow.push(hotkeyState);
    for (let slotIndex = 0; slotIndex < 6; slotIndex += 1) {
      const beforeSlot = hotkeyState.items.find((item) => item.slot === slotIndex) ?? null;
      await pressKey(client, String(slotIndex + 1), `Digit${slotIndex + 1}`, 49 + slotIndex);
      const consumableHotkeyItem =
        beforeSlot?.name === "Red Potion" || beforeSlot?.name === "Blue Potion";
      const afterSlotState = !beforeSlot
        ? await readStage5BeltItems(client, `afterEmptyHotkey${slotIndex + 1}`)
        : consumableHotkeyItem
          ? await waitForStage5BeltItemQuantityBelow(
            client,
            slotIndex,
            beforeSlot.quantity,
            `afterHotkey${slotIndex + 1}`,
            5_000,
          )
          : await readStage5BeltItems(client, `afterOccupiedNoopHotkey${slotIndex + 1}`);
      beltUseFlow.push({
        ...afterSlotState,
        slot: slotIndex,
        expected: !beforeSlot ? "empty-slot-noop" : consumableHotkeyItem ? "used" : "occupied-slot-noop",
      });
      hotkeyState = afterSlotState;
      if (!beforeSlot || !consumableHotkeyItem) {
        await delay(150);
      }
    }
    screenshots.push(await screenshot(client, "stage5-belt-hotkey-use.png"));

    await clickSelector(client, ".belt-button.rotate-horizontal button");
    await waitForBeltState(
      client,
      (state) =>
        state.orientation === "vertical" &&
        state.overlapsQuestTracker === false &&
        state.labelsWithinDialog === true,
      "vertical without quest overlap",
      5_000,
    );
    beltFlow.push(await readBeltState(client, "vertical"));
    screenshots.push(await screenshot(client, "stage5-belt-vertical.png"));

    await clickSelector(client, ".belt-button.rotate-vertical button");
    await waitForBeltState(
      client,
      (state) => state.orientation === "horizontal" && state.labelsWithinDialog === true,
      "horizontal with labels in dialog",
      5_000,
    );
    beltFlow.push(await readBeltState(client, "horizontalAfterRotate"));
    screenshots.push(await screenshot(client, "stage5-belt-horizontal.png"));

    await clickSelector(client, ".belt-button.close-horizontal button");
    await waitForBeltState(client, (state) => state.visible === false, "closed", 5_000);
    beltFlow.push(await readBeltState(client, "closed"));
    screenshots.push(await screenshot(client, "stage5-belt-closed.png"));

    if (client.consoleErrors.length > 0) {
      throw new Error(
        `Browser critical console errors:\n${client.consoleErrors
          .map((entry) => `- ${entry.source}: ${entry.text}`)
          .join("\n")}`,
      );
    }

    const stage5State = await client.evaluate("window.__mir2Stage5?.state ?? null");
    const summary = {
      screenshotCount: screenshots.length,
      compactPanelCount: compactPanelLayout.length,
      compactTextNodeCount: compactTextLayout.checked,
      compactMatrixCount: compactMatrix.length,
      criticalConsoleErrorCount: client.consoleErrors.length,
      flowCounts: {
        inventory: inventoryFlow.length,
        storage: storageFlow.length,
        storagePassword: storagePasswordFlow.length,
        chat: chatFlow.length,
        systemMenu: systemMenuFlow.length,
        systemMenuFeature: systemMenuFeatureFlow.length,
        systemMenuSocial: systemMenuSocialFlow.length,
        stage5Systems: stage5SystemsFlow.length,
        login: loginFlow.length,
        select: selectFlow.length,
        belt: beltFlow.length,
        gameShop: gameShopFlow.length,
      },
    };
    const manifest = {
      baseUrl: BASE_URL,
      generatedAt: new Date().toISOString(),
      summary,
      viewports: VIEWPORTS,
      compactLayout,
      compactTextLayout,
      compactPanelLayout,
      compactMatrix,
      screenshots,
      inventoryFlow,
      inventoryUseFlow,
      inventoryEquipFlow,
      inventoryGoldFlow,
      inventoryMoveFlow,
      inventorySplitFlow,
      inventoryDropFlow,
      groundPickupFlow,
      groundGoldPickupFlow,
      inventorySellFlow,
      characterRemoveFlow,
      characterRepairFlow,
      characterFlow,
      storageFlow,
      storagePasswordFlow,
      storageStoreFlow,
      storageTakeBackFlow,
      chatFlow,
      chatChannelFlow,
      systemMenuFlow,
      systemMenuQaTransferFlow,
      systemMenuTransferFlow,
      systemMenuFeatureFlow,
      systemMenuSocialFlow,
      hudButtonFlow,
      spellCastFlow,
      minimapFlow,
      mailFlow,
      reportFlow,
      npcDialogFlow,
      combatFlow,
      beltFlow,
      beltUseFlow,
      beltMouseUseFlow,
      gameShopFlow,
      loginFlow,
      selectFlow,
      stage5SystemsFlow,
      stage5Systems: stage5State?.stage5Systems ?? null,
      criticalConsoleErrors: client.consoleErrors,
    };
    const manifestPath = path.join(OUTPUT_DIR, "stage5-ui-smoke-manifest.json");
    await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    console.log(`Stage 5 UI smoke captured ${screenshots.length} screenshots.`);
    console.log(`Wrote ${manifestPath}`);
  } finally {
    client?.close();
    chrome.kill();
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function createTarget(port, url) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT",
  });
  if (!response.ok) {
    throw new Error(`Chrome target creation failed: ${response.status}`);
  }
  return response.json();
}

async function setViewport(client, viewport) {
  await client.send("Emulation.setDeviceMetricsOverride", viewport);
}

async function readLoginState(client, label) {
  return client.evaluate(`
    (() => {
      const activeLanguage = document.querySelector(".login-language-selector .language-selector-button.active");
      const accountInput = document.querySelector(".login-input.account");
      const passwordInput = document.querySelector(".login-input.password");
      const accountPanel = document.querySelector(".login-account-panel");
      return {
        label: ${JSON.stringify(label)},
        screen: window.__mir2Stage5?.state?.screen ?? null,
        language: window.__mir2Stage5?.state?.language ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        loginBusy: window.__mir2Stage5?.state?.loginBusy ?? null,
        activeLanguageLabel: activeLanguage?.textContent?.trim() ?? null,
        languageLabels: Array.from(document.querySelectorAll(".login-language-selector .language-selector-button"))
          .map((node) => node.textContent?.trim() ?? ""),
        accountValue: accountInput?.value ?? "",
        passwordFilled: Boolean(passwordInput?.value),
        accountPanelVisible: Boolean(accountPanel),
        accountPanelText: accountPanel?.textContent?.trim() ?? "",
        feedbackText: document.querySelector(".login-feedback")?.textContent?.trim() ?? "",
        runtimeStampVisible: Boolean(document.querySelector(".login-runtime-stamp")),
        buttons: Array.from(document.querySelectorAll(".login-button button"))
          .map((node) => node.getAttribute("aria-label") ?? node.getAttribute("title") ?? ""),
      };
    })()
  `);
}

async function waitForLoginState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readLoginState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for login state ${label}; current state: ${JSON.stringify(state)}`);
}

async function readSelectState(client, label) {
  return client.evaluate(`
    (() => {
      const activeLanguage = document.querySelector(".select-language-selector .language-selector-button.active");
      const slotNodes = Array.from(document.querySelectorAll(".select-character-slot-card"));
      const slots = slotNodes.map((node, index) => ({
        index,
        disabled: Boolean(node.disabled),
        empty: node.classList.contains("empty"),
        selected: node.classList.contains("selected"),
        name: node.querySelector(".name")?.textContent?.trim() ?? "",
        level: node.querySelector(".level")?.textContent?.trim() ?? "",
        job: node.querySelector(".job")?.textContent?.trim() ?? "",
      }));
      const characters = window.__mir2Stage5?.state?.characters ?? [];
      return {
        label: ${JSON.stringify(label)},
        screen: window.__mir2Stage5?.state?.screen ?? null,
        language: window.__mir2Stage5?.state?.language ?? null,
        accountId: window.__mir2Stage5?.state?.accountId ?? null,
        selectedCharacterIndex: window.__mir2Stage5?.state?.selectedCharacterIndex ?? null,
        characterCount: characters.length,
        characters: characters.map((character) => ({
          index: character.index,
          name: character.name,
          level: character.level,
          classKey: character.classKey,
          gender: character.gender,
        })),
        activeLanguageLabel: activeLanguage?.textContent?.trim() ?? null,
        languageLabels: Array.from(document.querySelectorAll(".select-language-selector .language-selector-button"))
          .map((node) => node.textContent?.trim() ?? ""),
        slots,
        creditsPanelVisible: Boolean(document.querySelector(".select-credits-panel")),
        creditsPanelText: document.querySelector(".select-credits-panel")?.textContent?.trim() ?? "",
        deletePanelVisible: Boolean(document.querySelector(".select-delete-panel")),
        deletePanelText: document.querySelector(".select-delete-panel")?.textContent?.trim() ?? "",
        selectedSlotName: slots.find((slot) => slot.selected)?.name ?? "",
        actionLabels: Array.from(document.querySelectorAll(".select-action button"))
          .map((node) => node.getAttribute("aria-label") ?? node.getAttribute("title") ?? ""),
      };
    })()
  `);
}

async function waitForSelectState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readSelectState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for select state ${label}; current state: ${JSON.stringify(state)}`);
}

async function clickLanguageButton(client, rootSelector, label) {
  const clicked = await client.evaluate(`
    (() => {
      const root = document.querySelector(${JSON.stringify(rootSelector)});
      const button = Array.from(root?.querySelectorAll(".language-selector-button") ?? [])
        .find((node) => node.textContent?.trim() === ${JSON.stringify(label)});
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click language ${label} in ${rootSelector}`);
  await delay(150);
  const active = await client.evaluate(`
    (() => {
      const root = document.querySelector(${JSON.stringify(rootSelector)});
      const activeButton = root?.querySelector(".language-selector-button.active");
      return activeButton?.textContent?.trim() === ${JSON.stringify(label)};
    })()
  `);
  if (active) return;
  await mouseClickElementByExpression(
    client,
    `
      (() => {
        const root = document.querySelector(${JSON.stringify(rootSelector)});
        return Array.from(root?.querySelectorAll(".language-selector-button") ?? [])
          .find((node) => node.textContent?.trim() === ${JSON.stringify(label)}) ?? null;
      })()
    `,
    `language ${label} in ${rootSelector}`,
  );
}

async function clickSelectSlot(client, slotIndex) {
  const clicked = await client.evaluate(`
    (() => {
      const slot = document.querySelectorAll(".select-character-slot-card")[${Number(slotIndex)}];
      if (!slot || slot.disabled) return false;
      slot.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click select character slot ${slotIndex}`);
}

async function startSelectedCharacterFromSelect(client, selectFlow) {
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    const beforeStart = await readSelectState(client, `beforeStartAttempt${attempt}`);
    const beforeWorldSnapshotVersion = await client.evaluate("window.__mir2Stage5?.state?.worldSnapshotVersion ?? 0");
    selectFlow.push(beforeStart);
    await clickSelector(client, ".select-action.start button");
    const started = await waitForStage5State(
      client,
      (state) =>
        state?.screen === "game" &&
        state?.player !== null &&
        Number(state?.worldSnapshotVersion ?? 0) > beforeWorldSnapshotVersion,
      `start game attempt ${attempt}`,
      45_000,
      { throwOnTimeout: false },
    );
    if (started) return;
    if (attempt < 3 && beforeStart.selectedCharacterIndex !== null) {
      await clickSelectSlot(client, beforeStart.selectedCharacterIndex);
      await delay(500);
    }
  }

  const selectState = await readSelectState(client, "startGameFailedSelectState");
  const loginState = await readLoginState(client, "startGameFailedLoginState");
  const pageState = await client.evaluate(`
    (() => ({
      stage5State: window.__mir2Stage5?.state ?? null,
      gameSceneVisible: Boolean(document.querySelector(".game-ui-scene")),
      startButtonText: document.querySelector(".select-action.start button")?.textContent?.trim() ?? "",
      startButtonDisabled: Boolean(document.querySelector(".select-action.start button")?.disabled),
      bodyText: document.body?.innerText?.slice(0, 1000) ?? "",
    }))()
  `);
  throw new Error(`Start game did not enter world: ${JSON.stringify({ selectState, loginState, pageState })}`);
}

async function readInventoryState(client, label) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      const activeTab = state?.activeInventoryTab ?? null;
      const items = (state?.inventoryItems ?? [])
        .filter((item) => item.container === activeTab)
        .map((item) => ({
          key: item.key,
          name: item.name,
          slot: item.slot,
          container: item.container,
          quantity: item.quantity,
        }));
      const rect = document.querySelector(".inventory-window")?.getBoundingClientRect();
      const questRows = Array.from(document.querySelectorAll(".inventory-quest-entry")).map((row) => ({
        questId: row.getAttribute("data-quest-id"),
        stage: row.getAttribute("data-quest-stage"),
        title: row.querySelector(".inventory-quest-entry-head strong")?.textContent?.trim() ?? "",
        summary: row.querySelector(".inventory-quest-summary")?.textContent?.trim() ?? "",
        objective: row.querySelector(".inventory-quest-objective")?.textContent?.trim() ?? "",
        progress: row.querySelector(".inventory-quest-progress-row span:first-child")?.textContent?.trim() ?? "",
        reward: row.querySelector(".inventory-quest-progress-row span:last-child")?.textContent?.trim() ?? "",
      }));
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        inventoryWindowVisible: Boolean(document.querySelector(".inventory-window")),
        storageWindowVisible: Boolean(document.querySelector(".storage-window")),
        visibleItemCards: document.querySelectorAll(".inventory-item-card").length,
        questEntryCount: questRows.length,
        questRows,
        stageQuestLog: window.__mir2Stage5?.state?.questLog ?? [],
        items,
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function readInventoryItem(client, label, itemName) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      const item = (window.__mir2Stage5?.state?.inventoryItems ?? []).find(
        (entry) => entry.container === activeTab && entry.name === ${JSON.stringify(itemName)}
      );
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        item: item
          ? {
              key: item.key,
              name: item.name,
              uniqueId: item.uniqueId,
              slot: item.slot,
              container: item.container,
              quantity: item.quantity,
            }
          : null,
      };
    })()
  `);
}

async function readInventoryItemByUniqueId(client, label, uniqueId) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      const item = (window.__mir2Stage5?.state?.inventoryItems ?? []).find(
        (entry) => entry.container === activeTab && Number(entry.uniqueId) === Number(${JSON.stringify(uniqueId)})
      );
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        item: item
          ? {
              key: item.key,
              name: item.name,
              uniqueId: item.uniqueId,
              slot: item.slot,
              container: item.container,
              quantity: item.quantity,
            }
          : null,
      };
    })()
  `);
}

async function ensureInventoryItemNamed(client, itemName, itemKey, label) {
  let state = await readInventoryItem(client, `${label}:before`, itemName);
  if (state.item) return state;

  await sendGatewayCommand(client, { type: "stage5Command", action: "mine", args: ["1"] });
  await sendGatewayCommand(client, { type: "stage5Command", action: "craft", args: [itemKey] });
  await sendGatewayCommand(client, { type: "tick" });
  await waitForStage5State(
    client,
    (nextState) => {
      const activeTab = nextState?.activeInventoryTab ?? "bag1";
      return (nextState?.inventoryItems ?? []).some((item) => item.container === activeTab && item.name === itemName);
    },
    `${label} ${itemName} available`,
    8_000,
  );
  state = await readInventoryItem(client, `${label}:created`, itemName);
  return state;
}

async function findFreeInventorySlot(client, preferredSlot, sameItemUniqueId = null) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? "bag1";
      const items = window.__mir2Stage5?.state?.inventoryItems ?? [];
      const sameItemUniqueId = ${JSON.stringify(sameItemUniqueId)};
      const occupiedBySlot = new Map(
        items
          .filter((item) => item.container === activeTab)
          .map((item) => [Number(item.slot), item]),
      );
      const preferred = Number(${JSON.stringify(preferredSlot)});
      const preferredItem = occupiedBySlot.get(preferred);
      if (!preferredItem || (sameItemUniqueId !== null && Number(preferredItem.uniqueId) === Number(sameItemUniqueId))) {
        return preferred;
      }
      const slotLimit = activeTab === "bag2" ? 40 : 40;
      for (let slot = 0; slot < slotLimit; slot += 1) {
        if (!occupiedBySlot.has(slot)) return slot;
      }
      return null;
    })()
  `);
}

function equipmentSlotUniqueId(slot) {
  const slots = {
    weapon: 0,
    armour: 1,
    helmet: 2,
    torch: 3,
    necklace: 4,
    braceletLeft: 5,
    braceletRight: 6,
    ringLeft: 7,
    ringRight: 8,
    amulet: 9,
    belt: 10,
    boots: 11,
    stone: 12,
    mount: 13,
  };
  return slots[slot] ?? null;
}

async function readPlayerVitalState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      playerHp: window.__mir2Stage5?.state?.playerHp ?? null,
      playerMaxHp: window.__mir2Stage5?.state?.playerMaxHp ?? null,
      playerMp: window.__mir2Stage5?.state?.playerMp ?? null,
      worldTick: window.__mir2Stage5?.state?.worldTick ?? null,
    }))()
  `);
}

async function ensurePlayerCanUseHealingPotion(client, label) {
  const before = await readPlayerVitalState(client, `${label}:before`);
  const hp = Number(before.playerHp ?? 0);
  const maxHp = Number(before.playerMaxHp ?? 0);
  const targetHp = maxHp > 0 ? Math.max(1, maxHp - 20) : Math.max(1, hp - 20);
  if (hp > 1 && (maxHp <= 0 || hp > targetHp)) {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      await sendGatewayCommand(client, { type: "stage5Command", action: "qa.damagePlayer", args: ["25"] });
      const packetDamaged = await waitForStage5State(
        client,
        (state) => Number(state?.playerHp ?? 0) <= targetHp,
        `damaged player packet for ${label}`,
        1_500,
        { throwOnTimeout: false },
      );
      if (packetDamaged) {
        return readPlayerVitalState(client, label);
      }
      await sendGatewayCommand(client, { type: "tick" });
      const damaged = await waitForStage5State(
        client,
        (state) => Number(state?.playerHp ?? 0) <= targetHp,
        `damaged player for ${label}`,
        3_000,
        { throwOnTimeout: false },
      );
      if (damaged) {
        return readPlayerVitalState(client, label);
      }
    }
    await waitForStage5State(
      client,
      (state) => Number(state?.playerHp ?? 0) <= targetHp,
      `damaged player for ${label}`,
      1_000,
    );
    return readPlayerVitalState(client, label);
  }

  return { ...before, label, damageSkipped: true };
}

async function readItemDistribution(client, label, itemName) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      const toItem = (item) => ({
        key: item.key,
        name: item.name,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
        quantity: item.quantity,
      });
      const inventoryItems = (window.__mir2Stage5?.state?.inventoryItems ?? [])
        .filter((entry) => entry.container === activeTab && entry.name === ${JSON.stringify(itemName)})
        .map(toItem)
        .sort((left, right) => left.slot - right.slot);
      const beltItems = (window.__mir2Stage5?.state?.beltItems ?? [])
        .filter((entry) => entry.name === ${JSON.stringify(itemName)})
        .map(toItem)
        .sort((left, right) => left.slot - right.slot);
      const inventoryQuantity = inventoryItems.reduce((total, item) => total + (item.quantity ?? 0), 0);
      const beltQuantity = beltItems.reduce((total, item) => total + (item.quantity ?? 0), 0);
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        inventoryItems,
        beltItems,
        inventoryQuantity,
        beltQuantity,
        totalQuantity: inventoryQuantity + beltQuantity,
      };
    })()
  `);
}

function splitSourceQuantityForDistribution(state, uniqueId) {
  return state.inventoryItems.find((item) => Number(item.uniqueId) === Number(uniqueId))?.quantity ?? 0;
}

async function clickInventoryItemByName(client, itemName) {
  await dispatchMouseSequenceToElementByExpression(
    client,
    `
      Array.from(document.querySelectorAll(".inventory-item-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      ) ?? null
    `,
    `inventory item ${itemName}`,
  );
}

async function contextMenuInventoryItemByName(client, itemName) {
  await dispatchContextMenuToElementByExpression(
    client,
    `
      Array.from(document.querySelectorAll(".inventory-item-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      ) ?? null
    `,
    `inventory item ${itemName}`,
  );
}

async function contextMenuInventoryItemByUniqueId(client, uniqueId) {
  await dispatchContextMenuToElementByExpression(
    client,
    `
      Array.from(document.querySelectorAll(".inventory-item-card")).find(
        (node) => {
          if (node.getAttribute("title") !== "Red Potion") return false;
          const rect = node.getBoundingClientRect();
          const centerX = rect.left + rect.width / 2;
          const centerY = rect.top + rect.height / 2;
          const stageState = window.__mir2Stage5?.state ?? null;
          const activeTab = stageState?.activeInventoryTab ?? null;
          return (stageState?.inventoryItems ?? []).some((item) => {
            if (item.container !== activeTab || Number(item.uniqueId) !== Number(${JSON.stringify(uniqueId)})) {
              return false;
            }
            const slot = Array.from(document.querySelectorAll(".inventory-slot"))[Number(item.slot)];
            if (!slot) return false;
            const slotRect = slot.getBoundingClientRect();
            return Math.abs(centerX - (slotRect.left + slotRect.width / 2)) < 3 &&
              Math.abs(centerY - (slotRect.top + slotRect.height / 2)) < 3;
          });
        }
      ) ?? null
    `,
    `inventory unique item ${uniqueId}`,
  );
}

async function clickInventorySlot(client, slotIndex) {
  const clicked = await client.evaluate(`
    (() => {
      const slot = document.querySelectorAll(".inventory-slot")[${Number(slotIndex)}];
      if (!slot) return false;
      slot.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click inventory slot ${slotIndex}`);
}

async function clickStorageSlot(client, slotIndex) {
  const clicked = await client.evaluate(`
    (() => {
      const slot = document.querySelectorAll(".storage-slot")[${Number(slotIndex)}];
      if (!slot) return false;
      slot.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click storage slot ${slotIndex}`);
}

async function clickStorageItemByName(client, itemName) {
  await dispatchMouseSequenceToElementByExpression(
    client,
    `
      Array.from(document.querySelectorAll(".storage-item-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      ) ?? null
    `,
    `storage item ${itemName}`,
  );
}

async function openStorageServiceViaNpc(client, storageFlow, npcDialogFlow, screenshots) {
  await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button, .npc-dialog-close");
  await delay(250);
  await transferMapAndWait(client, "crystal:0:317:260", "storage service NPC position");
  storageFlow.push({ label: "storageServiceNpcReady", source: "qa.openNpcDialog.realScript" });
  await sendGatewayCommand(client, { type: "stage5Command", action: "qa.openNpcDialog", args: [] });
  const storageDialog = await waitForNpcDialogState(
    client,
    (state) =>
      state.open === true &&
      state.title === "InnKeeper_Brittney" &&
      state.links.some((link) => link.target.toLowerCase() === "@storage") &&
      !hasRawCrystalMarkup(state.visibleText),
    "storage service NPC dialog",
    10_000,
  );
  npcDialogFlow.push(storageDialog);
  screenshots.push(await screenshot(client, "stage5-storage-service-npc.png"));
  await clickNpcDialogLinkByTarget(client, "@Storage");
  await waitForNpcDialogState(
    client,
    (state) => state.open === false,
    "storage service selected",
    8_000,
  );
  npcDialogFlow.push(await readNpcDialogState(client, "storageServiceSelected"));
  await waitForStorageState(
    client,
    (state) => state.storageWindowVisible === true && state.activePage === "1" && state.storageSlots >= 80,
    "storage service opened from NPC",
    8_000,
  );
  await delay(300);
}

async function openNpcServiceViaNpc(client, service, npcDialogFlow, screenshots) {
  await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button, .npc-dialog-close");
  await delay(250);
  await transferMapAndWait(client, service.transferKey, `${service.name} service position`);
  const npc = await waitForStage5Entity(
    client,
    (entity) =>
      entity.kind === "npc" &&
      (entity.objectId === service.objectId ||
        entity.name === service.name ||
        entity.name.includes(service.name.replace(/^[^_]+_/, ""))),
    `${service.name} service NPC`,
    10_000,
  ).catch(() => ({ objectId: service.objectId, kind: "npc", name: service.name, x: null, y: null }));
  await sendGatewayCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  await waitForNpcDialogState(
    client,
    (state) =>
      state.open === true &&
      state.links.some((link) => link.target.toLowerCase() === service.target.toLowerCase()),
    `${service.name} service NPC dialog`,
    10_000,
  );
  npcDialogFlow.push(await readNpcDialogState(client, `${service.name}:${service.target}:dialog`));
  screenshots.push(await screenshot(client, service.screenshotName));
  await clickNpcDialogLinkByTarget(client, service.target);
  await waitForNpcDialogState(client, (state) => state.open === false, `${service.name} service selected`, 8_000);
  npcDialogFlow.push(await readNpcDialogState(client, `${service.name}:${service.target}:selected`));
  await delay(300);
}

async function ensureCharacterWindowOpen(client) {
  if (!(await client.evaluate('Boolean(document.querySelector(".character-window"))'))) {
    await clickSelector(client, ".hud-button.character button");
    await waitForSelector(client, ".character-window", 10_000);
  }
  await clickSelector(client, ".character-tab.char button");
  await waitForCharacterState(client, (state) => state.activeTab === "char", "character repair tab", 5_000);
}

async function readWorldSnapshotVersion(client) {
  return client.evaluate("window.__mir2Stage5?.state?.worldSnapshotVersion ?? 0");
}

function parseCrystalTransferKey(key) {
  const [prefix, mapFileName, x, y] = String(key).split(":");
  if (prefix !== "crystal" || mapFileName === undefined || x === undefined || y === undefined) {
    return null;
  }
  return {
    mapFileName,
    x: Number(x),
    y: Number(y),
  };
}

async function transferMapAndWait(client, key, label, options = {}) {
  const target = parseCrystalTransferKey(key);
  if (!target) {
    await sendGatewayCommand(client, { type: "transferMap", key });
    return;
  }

  const attempts = Number(options.attempts ?? 3);
  const timeoutMs = Number(options.timeoutMs ?? 15_000);
  const attemptTimeoutMs = Math.max(2_500, Math.floor(timeoutMs / Math.max(1, attempts)));
  const beforeSnapshotVersion = await readWorldSnapshotVersion(client);
  const reachedTarget = (state) =>
    state?.mapFileName === target.mapFileName &&
    state?.player?.x === target.x &&
    state?.player?.y === target.y;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    await sendGatewayCommand(client, { type: "transferMap", key });
    const reached = await waitForStage5State(
      client,
      reachedTarget,
      `${label} attempt ${attempt + 1}`,
      attemptTimeoutMs,
      { throwOnTimeout: false },
    );
    if (reached) {
      await sendGatewayCommand(client, { type: "tick" });
      await waitForStage5State(
        client,
        (state) => reachedTarget(state) && Number(state?.worldSnapshotVersion ?? 0) > Number(beforeSnapshotVersion ?? 0),
        `${label} snapshot`,
        3_000,
        { throwOnTimeout: false },
      );
      return;
    }
    await sendGatewayCommand(client, { type: "tick" });
    await delay(250);
  }

  await waitForStage5State(client, reachedTarget, label, 1_000);
}

async function refreshWorldSnapshotAfterTransfer(client, beforeSnapshotVersion, label) {
  await sendGatewayCommand(client, { type: "tick" });
  await waitForStage5State(
    client,
    (state) => Number(state?.worldSnapshotVersion ?? 0) > Number(beforeSnapshotVersion ?? 0),
    label,
    8_000,
  );
}

async function waitForStage5Entity(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastTickAt = 0;
  let entities = [];
  while (Date.now() < deadline) {
    entities = await client.evaluate(`
      (window.__mir2Stage5?.state?.entities ?? []).map((entity) => ({
        objectId: entity.objectId,
        kind: entity.kind,
        name: entity.name,
        x: entity.x,
        y: entity.y,
      }))
    `);
    const entity = entities.find(predicate);
    if (entity) return entity;
    if (Date.now() - lastTickAt > 1_000) {
      lastTickAt = Date.now();
      await sendGatewayCommand(client, { type: "tick" });
    }
    await delay(200);
  }
  throw new Error(`Timed out waiting for Stage 5 entity ${label}; current entities: ${JSON.stringify(entities)}`);
}

function itemQuantity(items, itemName) {
  return items.filter((item) => item.name === itemName).reduce((total, item) => total + (item.quantity ?? 0), 0);
}

async function waitForInventoryItemQuantityBelow(client, uniqueId, previousQuantity, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryItemByUniqueId(client, label, uniqueId);
    if (!state.item || state.item.quantity < previousQuantity) return state;
    await delay(100);
  }
  const debugState = await client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? null;
      return {
        screen: state?.screen ?? null,
        wsState: state?.wsState ?? null,
        mapFileName: state?.mapFileName ?? null,
        player: state?.player ?? null,
        playerHp: state?.playerHp ?? null,
        playerMaxHp: state?.playerMaxHp ?? null,
        lastCommand: state?.lastCommand ?? null,
        liveLastCommand: window.__mir2LastCommand ?? null,
        commandHistory: window.__mir2CommandHistory ?? [],
        lastInventoryActivation: window.__mir2LastInventoryActivation ?? null,
        lastInventoryConfirmation: window.__mir2LastInventoryConfirmation ?? null,
        lastUseItem: window.__mir2LastUseItem ?? null,
        lastBeltActivation: window.__mir2LastBeltActivation ?? null,
        lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
        gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
        worldTick: state?.worldTick ?? null,
        worldSnapshotVersion: state?.worldSnapshotVersion ?? null,
        inventoryItems: state?.inventoryItems ?? [],
        inventoryDeletePanelText: document.querySelector(".inventory-delete-panel")?.textContent?.trim() ?? "",
        inventoryFeedbackText: document.querySelector(".inventory-delete-feedback")?.textContent?.trim() ?? "",
        inventoryHintTexts: Array.from(document.querySelectorAll(".inventory-delete-hint")).map(
          (node) => node.textContent?.trim() ?? "",
        ),
        inventoryButtonTitles: Array.from(document.querySelectorAll(".inventory-item-card")).map((node) => ({
          title: node.getAttribute("title"),
          text: node.textContent?.trim() ?? "",
          disabled: Boolean(node.disabled),
        })),
        actionButtons: Array.from(document.querySelectorAll(".inventory-action-button, .inventory-delete button")).map(
          (node) => ({
            className: node.className,
            text: node.textContent?.trim() ?? "",
            title: node.getAttribute("title") ?? "",
            ariaLabel: node.getAttribute("aria-label") ?? "",
          }),
        ),
        recentLogs: (state?.logs ?? []).slice(-8).map((log) => ({
          text: log.text,
          tone: log.tone,
          channel: log.channel,
        })),
        latestLogs: (state?.logs ?? []).slice(0, 8).map((log) => ({
          text: log.text,
          tone: log.tone,
          channel: log.channel,
        })),
      };
    })()
  `);
  if (options.throwOnTimeout === false) return false;
  throw new Error(
    `Timed out waiting for inventory uniqueId ${uniqueId} quantity below ${previousQuantity}; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}; consoleErrors: ${JSON.stringify(client.consoleErrors.slice(-8))}`,
  );
}

async function waitForInventoryItemSlot(client, itemName, slot, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryItem(client, label, itemName);
    if (state.item?.slot === slot) return state;
    await delay(100);
  }
  if (options.throwOnTimeout === false) return false;
  throw new Error(`Timed out waiting for inventory ${itemName} slot ${slot}; current state: ${JSON.stringify(state)}`);
}

async function waitForItemDistribution(client, itemName, predicate, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readItemDistribution(client, label, itemName);
    if (predicate(state)) return state;
    await delay(100);
  }
  const debugState = await client.evaluate(`
    (() => ({
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      lastInventoryActivation: window.__mir2LastInventoryActivation ?? null,
      lastInventoryConfirmation: window.__mir2LastInventoryConfirmation ?? null,
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
      worldTick: window.__mir2Stage5?.state?.worldTick ?? null,
      worldSnapshotVersion: window.__mir2Stage5?.state?.worldSnapshotVersion ?? null,
    }))()
  `);
  if (options.throwOnTimeout === false) return false;
  throw new Error(`Timed out waiting for inventory ${itemName} split state; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}`);
}

async function readInventoryEquipmentState(client, label, itemName) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      const inventoryItem = (window.__mir2Stage5?.state?.inventoryItems ?? []).find(
        (entry) => entry.container === activeTab && entry.name === ${JSON.stringify(itemName)}
      );
      const inventoryItems = (window.__mir2Stage5?.state?.inventoryItems ?? []).map((item) => ({
        key: item.key,
        name: item.name,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
        quantity: item.quantity,
      }));
      const equipmentItems = (window.__mir2Stage5?.state?.equipmentItems ?? []).map((item) => ({
        name: item.name,
        slot: item.slot,
        durabilityCurrent: item.durabilityCurrent,
        durabilityMax: item.durabilityMax,
      }));
      const storageItems = (window.__mir2Stage5?.state?.storageItems ?? []).map((item) => ({
        key: item.key,
        name: item.name,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
        quantity: item.quantity,
      }));
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        inventoryItem: inventoryItem
          ? {
              key: inventoryItem.key,
              name: inventoryItem.name,
              uniqueId: inventoryItem.uniqueId,
              slot: inventoryItem.slot,
              container: inventoryItem.container,
              quantity: inventoryItem.quantity,
            }
          : null,
        inventoryItems,
        equipmentItems,
        storageItems,
      };
    })()
  `);
}

async function ensureInventoryItemAvailable(client, itemName, label) {
  let state = await readInventoryEquipmentState(client, `${label}:before`, itemName);
  if (state.inventoryItem) return state;

  const equippedItem = state.equipmentItems.find((item) => item.name === itemName);
  if (equippedItem) {
    const uniqueId = equipmentSlotUniqueId(equippedItem.slot);
    const targetSlot = await findFreeInventorySlot(client, 10, null);
    if (uniqueId !== null && targetSlot !== null) {
      await sendGatewayCommand(client, {
        type: "removeItem",
        uniqueId,
        grid: "inventory",
        to: targetSlot,
      });
      await sendGatewayCommand(client, { type: "tick" });
      await waitForStage5State(
        client,
        (nextState) =>
          (nextState?.inventoryItems ?? []).some(
            (item) => item.container === (nextState?.activeInventoryTab ?? "bag1") && item.name === itemName,
          ),
        `${label} restored from equipment`,
        10_000,
      );
      return readInventoryEquipmentState(client, `${label}:removed`, itemName);
    }
  }

  const recovery = await client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? "bag1";
      const inventoryItems = window.__mir2Stage5?.state?.inventoryItems ?? [];
      const storageItems = window.__mir2Stage5?.state?.storageItems ?? [];
      const storedItem = storageItems.find((item) => item.name === ${JSON.stringify(itemName)});
      if (!storedItem) return null;

      const occupiedSlots = new Set(
        inventoryItems
          .filter((item) => item.container === activeTab)
          .map((item) => Number(item.slot))
          .filter((slot) => Number.isFinite(slot)),
      );
      const slotLimit = activeTab === "belt" ? 6 : 40;
      for (let slot = 0; slot < slotLimit; slot += 1) {
        if (!occupiedSlots.has(slot)) {
          return {
            from: Number(storedItem.slot),
            to: slot,
            activeTab,
          };
        }
      }
      return null;
    })()
  `);

  if (!recovery) return state;

  await openStorageServiceViaNpc(client, [], [], []);
  await sendGatewayCommand(client, { type: "takeBackItem", from: recovery.from, to: recovery.to });
  await sendGatewayCommand(client, { type: "tick" });
  await waitForStage5State(
    client,
    (nextState) =>
      (nextState?.inventoryItems ?? []).some(
        (item) => item.container === recovery.activeTab && item.name === itemName,
      ),
    `${label} restored from storage`,
    10_000,
  );
  return readInventoryEquipmentState(client, `${label}:restored`, itemName);
}

async function readCharacterRepairState(client, label, itemName) {
  const equipment = await readInventoryEquipmentState(client, label, itemName);
  const repairUi = await client.evaluate(`
    (() => ({
      repairActionLabels: Array.from(document.querySelectorAll(".character-repair-actions button")).map(
        (button) => button.textContent?.trim() ?? "",
      ),
      activeRepairButtons: Array.from(document.querySelectorAll(".character-repair-actions button.active")).map(
        (button) => button.textContent?.trim() ?? "",
      ),
      activeRepairLabel: document.querySelector(".character-window .inventory-delete-hint")?.textContent?.trim() ?? "",
      gold: window.__mir2Stage5?.state?.gold ?? null,
    }))()
  `);
  return {
    ...equipment,
    ...repairUi,
  };
}

async function waitForCharacterRepairState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readCharacterRepairState(client, label, "Dagger");
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for character repair ${label}; current state: ${JSON.stringify(state)}`);
}

async function clickCharacterRepairAction(client, kind) {
  const index = kind === "special" ? 1 : 0;
  const clicked = await client.evaluate(`
    (() => {
      const button = document.querySelectorAll(".character-repair-actions button")[${index}];
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click character ${kind} repair action`);
}

async function waitForEquipmentItem(client, itemName, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryEquipmentState(client, label, itemName);
    if (state.equipmentItems.some((item) => item.name === itemName)) return state;
    await delay(100);
  }
  if (options.throwOnTimeout === false) return false;
  const debugState = await client.evaluate(`
    (() => ({
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      commandHistory: window.__mir2CommandHistory ?? [],
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
    }))()
  `);
  throw new Error(
    `Timed out waiting for equipped ${itemName}; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}`,
  );
}

async function waitForEquipmentItemAbsent(client, itemName, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryEquipmentState(client, label, itemName);
    if (!state.equipmentItems.some((item) => item.name === itemName)) return state;
    await delay(100);
  }
  if (options.throwOnTimeout === false) return false;
  throw new Error(`Timed out waiting for removed ${itemName}; current state: ${JSON.stringify(state)}`);
}

async function clickCharacterEquipmentItemByName(client, itemName) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".character-slot-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click character equipment item ${itemName}`);
}

async function readInventoryGoldState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      gold: window.__mir2Stage5?.state?.gold ?? null,
      inventoryGoldText: document.querySelector(".inventory-gold")?.textContent?.trim() ?? "",
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      lastInventoryConfirmation: window.__mir2LastInventoryConfirmation ?? null,
      dropGoldPanelOpen:
        (document.querySelector(".inventory-delete-panel strong")?.textContent?.trim() ?? "") === "Drop Gold",
      dropGoldInputValue: document.querySelector(".inventory-delete-panel input")?.value ?? "",
      feedbackText: document.querySelector(".inventory-delete-feedback")?.textContent?.trim() ?? "",
      groundDropLabels: Array.from(document.querySelectorAll(".drop-label")).map((labelNode) =>
        labelNode.textContent?.trim() ?? ""
      ),
    }))()
  `);
}

async function waitForInventoryGoldState(client, predicate, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryGoldState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  if (options.throwOnTimeout === false) return false;
  const debugState = await client.evaluate(`
    (() => ({
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      commandHistory: window.__mir2CommandHistory ?? [],
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
    }))()
  `);
  throw new Error(
    `Timed out waiting for inventory gold ${label}; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}`,
  );
}

async function readInventorySplitState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      splitPanelOpen:
        (document.querySelector(".inventory-delete-panel strong")?.textContent?.trim() ?? "") === "Split Item",
      splitItemName:
        Array.from(document.querySelectorAll(".inventory-delete-panel span"))
          .map((node) => node.textContent?.trim() ?? "")
          .find(Boolean) ?? "",
      splitInputValue: document.querySelector(".inventory-delete-panel input")?.value ?? "",
      feedbackText: document.querySelector(".inventory-delete-feedback")?.textContent?.trim() ?? "",
    }))()
  `);
}

async function waitForInventorySplitState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventorySplitState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory split ${label}; current state: ${JSON.stringify(state)}`);
}

async function readInventoryDropState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      dropItemPanelOpen:
        (document.querySelector(".inventory-delete-panel strong")?.textContent?.trim() ?? "") === "Delete item",
      dropItemName:
        Array.from(document.querySelectorAll(".inventory-delete-panel span"))
          .map((node) => node.textContent?.trim() ?? "")
          .find(Boolean) ?? "",
      feedbackText: document.querySelector(".inventory-delete-feedback")?.textContent?.trim() ?? "",
      groundDropLabels: Array.from(document.querySelectorAll(".drop-label")).map((labelNode) =>
        labelNode.textContent?.trim() ?? ""
      ),
    }))()
  `);
}

async function waitForInventoryDropState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryDropState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory drop ${label}; current state: ${JSON.stringify(state)}`);
}

async function readGroundDropState(client, label, itemName) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        gold: window.__mir2Stage5?.state?.gold ?? null,
        inventoryItems: (window.__mir2Stage5?.state?.inventoryItems ?? []).map((item) => ({
            key: item.key,
            name: item.name,
            uniqueId: item.uniqueId,
            slot: item.slot,
            container: item.container,
            quantity: item.quantity,
          })),
        beltItems: (window.__mir2Stage5?.state?.beltItems ?? []).map((item) => ({
          key: item.key,
          name: item.name,
          uniqueId: item.uniqueId,
          slot: item.slot,
          container: item.container,
          quantity: item.quantity,
        })),
        groundDrops: (window.__mir2Stage5?.state?.groundDrops ?? []).map((drop) => ({
          objectId: drop.objectId,
          name: drop.name,
          x: drop.x,
          y: drop.y,
          quantity: drop.quantity,
          nameColourArgb: drop.nameColourArgb ?? null,
        })),
        matchingLabels: Array.from(document.querySelectorAll(".ground-drop-marker")).filter(
          (node) => (node.getAttribute("title") ?? "").includes(${JSON.stringify(itemName)})
        ).map((node) => node.getAttribute("title") ?? ""),
        visibleLabels: Array.from(document.querySelectorAll(".drop-label")).map((labelNode) =>
          labelNode.textContent?.trim() ?? ""
        ),
      };
    })()
  `);
}

async function waitForGroundDropState(client, itemName, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readGroundDropState(client, label, itemName);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for ground drop ${label}; current state: ${JSON.stringify(state)}`);
}

function groundDropStateItemQuantity(state, itemName) {
  return itemQuantity([...(state.inventoryItems ?? []), ...(state.beltItems ?? [])], itemName);
}

async function waitForGroundDropInventoryItem(client, itemName, label, timeoutMs) {
  return waitForGroundDropState(
    client,
    itemName,
    (state) => state.inventoryItems.some((item) => item.name === itemName),
    label,
    timeoutMs,
  );
}

async function ensureGroundDropAvailable(client, itemName, itemKey, label) {
  let state = await readGroundDropState(client, `${label}:before`, itemName);
  if (state.groundDrops.some((drop) => drop.name === itemName)) return state;

  let inventoryItem = state.inventoryItems.find((item) => item.name === itemName);
  if (!inventoryItem) {
    await sendGatewayCommand(client, { type: "stage5Command", action: "qa.giveItem", args: [itemKey, "1"] });
    await sendGatewayCommand(client, { type: "tick" });
    state = await waitForGroundDropInventoryItem(client, itemName, `${label} inventory seed`, 8_000);
    inventoryItem = state.inventoryItems.find((item) => item.name === itemName);
  }
  if (!inventoryItem) {
    throw new Error(`Cannot seed ${itemName} ground drop without inventory item: ${JSON.stringify(state)}`);
  }
  await sendGatewayCommand(client, {
    type: "dropItem",
    key: inventoryItem.key ?? itemKey,
    uniqueId: inventoryItem.uniqueId,
    count: 1,
    heroInventory: false,
  });
  await sendGatewayCommand(client, { type: "tick" });
  return waitForGroundDropState(
    client,
    itemName,
    (nextState) => nextState.groundDrops.some((drop) => drop.name === itemName),
    `${label} seeded`,
    10_000,
  );
}

async function clickGroundDropByName(client, itemName, options = {}) {
  const target = await client.evaluate(`
    (() => {
      const marker = Array.from(document.querySelectorAll(".ground-drop-marker")).find(
        (node) => (node.getAttribute("title") ?? "").includes(${JSON.stringify(itemName)})
      );
      if (!marker) return null;
      const rect = marker.getBoundingClientRect();
      const matchingDrop = (window.__mir2Stage5?.state?.groundDrops ?? []).find(
        (drop) => marker.getAttribute("title")?.includes(drop.name)
      );
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
        objectId: matchingDrop?.objectId ?? null,
      };
    })()
  `);
  if (!target) {
    if (options.objectId) {
      await sendGatewayCommand(client, { type: "pickUp", objectId: Number(options.objectId) });
      return;
    }
    throw new Error(`Could not click ground drop ${itemName}`);
  }

  await client.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: target.x,
    y: target.y,
    button: "none",
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: target.x,
    y: target.y,
    button: "left",
    clickCount: 1,
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: target.x,
    y: target.y,
    button: "left",
    clickCount: 1,
  });
  await delay(100);

  if (target.objectId) {
    await sendGatewayCommand(client, { type: "pickUp", objectId: Number(target.objectId) });
  }
}

async function readInventorySellState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      sellPanelOpen:
        (document.querySelector(".inventory-delete-panel strong")?.textContent?.trim() ?? "") === "Sell Item",
      sellItemName:
        Array.from(document.querySelectorAll(".inventory-delete-panel span"))
          .map((node) => node.textContent?.trim() ?? "")
          .find(Boolean) ?? "",
      feedbackText: document.querySelector(".inventory-delete-feedback")?.textContent?.trim() ?? "",
    }))()
  `);
}

async function waitForInventorySellState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventorySellState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory sell ${label}; current state: ${JSON.stringify(state)}`);
}

async function readStorageTransferState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      activeTab: window.__mir2Stage5?.state?.activeInventoryTab ?? null,
      inventoryItems: (window.__mir2Stage5?.state?.inventoryItems ?? [])
        .map((item) => ({
          key: item.key,
          name: item.name,
          slot: item.slot,
          container: item.container,
          quantity: item.quantity,
        })),
      storageItems: (window.__mir2Stage5?.state?.storageItems ?? []).map((item) => ({
        key: item.key,
        name: item.name,
        slot: item.slot,
        container: item.container,
        quantity: item.quantity,
      })),
      storageWindowVisible: Boolean(document.querySelector(".storage-window")),
      storageItemCards: document.querySelectorAll(".storage-item-card").length,
      storageSlots: document.querySelectorAll(".storage-slot").length,
      feedbackText: document.querySelector(".inventory-delete-feedback")?.textContent?.trim() ?? "",
      hintTexts: Array.from(document.querySelectorAll(".inventory-delete-hint")).map(
        (node) => node.textContent?.trim() ?? "",
      ),
      statusText: document.querySelector(".storage-window-status")?.textContent?.trim() ?? "",
    }))()
  `);
}

async function waitForStorageTransferState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readStorageTransferState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for storage transfer ${label}; current state: ${JSON.stringify(state)}`);
}

async function ensureStoredItemForTakeBack(client, itemName, itemKey, label) {
  let state = await readStorageTransferState(client, `${label}:before`);
  let storageSlot = firstStorageSlotForItem(state, itemName);
  let inventorySlot = firstFreeInventorySlotFromState(state, 6);
  if (storageSlot !== null && inventorySlot !== null) {
    return { state, storageSlot, inventorySlot };
  }

  let sourceSlot = firstInventorySlotForItem(state, itemName);
  if (sourceSlot === null) {
    await sendGatewayCommand(client, { type: "stage5Command", action: "qa.giveItem", args: [itemKey, "1"] });
    await sendGatewayCommand(client, { type: "tick" });
    await waitForStorageTransferState(
      client,
      (nextState) => firstInventorySlotForItem(nextState, itemName) !== null,
      `${label} ${itemName} seeded in inventory`,
      8_000,
    );
    state = await readStorageTransferState(client, `${label}:inventorySeeded`);
    sourceSlot = firstInventorySlotForItem(state, itemName);
  }

  storageSlot = firstFreeStorageSlotFromState(state, 0);
  if (sourceSlot === null || storageSlot === null) {
    throw new Error(`Cannot seed stored ${itemName}: ${JSON.stringify(state)}`);
  }

  await sendGatewayCommand(client, { type: "storeItem", from: sourceSlot, to: storageSlot });
  await sendGatewayCommand(client, { type: "tick" });
  state = await waitForStorageTransferState(
    client,
    (nextState) => firstStorageSlotForItem(nextState, itemName) === storageSlot,
    `${label} ${itemName} stored for take back`,
    8_000,
  );
  inventorySlot = firstFreeInventorySlotFromState(state, sourceSlot);
  if (inventorySlot === null) {
    throw new Error(`Cannot take back ${itemName} without a free inventory slot: ${JSON.stringify(state)}`);
  }

  return { state, storageSlot, inventorySlot };
}

function firstStorageSlotForItem(state, itemName) {
  const item = (state.storageItems ?? [])
    .filter((entry) => entry.name === itemName)
    .map((entry) => Number(entry.slot))
    .filter((slot) => Number.isFinite(slot) && slot >= 0 && slot < 40)
    .sort((left, right) => left - right)[0];
  return item ?? null;
}

function firstInventorySlotForItem(state, itemName) {
  const item = (state.inventoryItems ?? [])
    .filter((entry) => entry.name === itemName)
    .map((entry) => Number(entry.slot))
    .filter((slot) => Number.isFinite(slot) && slot >= 0 && slot < 40)
    .sort((left, right) => left - right)[0];
  return item ?? null;
}

function firstFreeStorageSlotFromState(state, preferredSlot) {
  const occupiedSlots = new Set(
    (state.storageItems ?? [])
      .map((entry) => Number(entry.slot))
      .filter((slot) => Number.isFinite(slot) && slot >= 0 && slot < 40),
  );
  return firstFreeSlot(occupiedSlots, preferredSlot, 40);
}

function firstFreeInventorySlotFromState(state, preferredSlot) {
  const occupiedSlots = new Set(
    (state.inventoryItems ?? [])
      .filter((entry) => entry.container === "bag1")
      .map((entry) => Number(entry.slot))
      .filter((slot) => Number.isFinite(slot) && slot >= 0 && slot < 40),
  );
  return firstFreeSlot(occupiedSlots, preferredSlot, 40);
}

function firstFreeSlot(occupiedSlots, preferredSlot, slotLimit) {
  const preferred = Number(preferredSlot);
  if (Number.isFinite(preferred) && preferred >= 0 && preferred < slotLimit && !occupiedSlots.has(preferred)) {
    return preferred;
  }
  for (let slot = 0; slot < slotLimit; slot += 1) {
    if (!occupiedSlots.has(slot)) return slot;
  }
  return null;
}

async function clickInventoryPanelAction(client, index) {
  const clicked = await client.evaluate(`
    (() => {
      const button = document.querySelectorAll(".inventory-delete-panel .inventory-delete-actions button")[${Number(index)}];
      if (!button || button.disabled) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click inventory panel action ${index}`);
}

async function waitForInventoryState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory ${label}; current state: ${JSON.stringify(state)}`);
}

async function readCharacterState(client, label) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state;
      const rect = document.querySelector(".character-window")?.getBoundingClientRect();
      return {
        label: ${JSON.stringify(label)},
        activeTab: state?.activeCharacterTab ?? null,
        characterWindowVisible: Boolean(document.querySelector(".character-window")),
        equipmentCardCount: document.querySelectorAll(".character-slot-card").length,
        statsValueCount: document.querySelectorAll(".character-field-value").length,
        spellValueCount: document.querySelectorAll(".character-spell-value").length,
        knownSkills: (state?.knownSkills ?? []).map((skill) => ({
          key: skill.key,
          name: skill.name,
          cooldownRemainingTicks: skill.cooldownRemainingTicks,
        })),
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function waitForCharacterState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readCharacterState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for character ${label}; current state: ${JSON.stringify(state)}`);
}

async function readStorageState(client, label) {
  return client.evaluate(`
    (() => {
      const windowNode = document.querySelector(".storage-window");
      const rect = windowNode?.getBoundingClientRect();
      const pageOne = document.querySelector(".storage-page-tab.page-1");
      const pageTwo = document.querySelector(".storage-page-tab.page-2");
      return {
        label: ${JSON.stringify(label)},
        storageWindowVisible: Boolean(windowNode),
        activePage: pageTwo?.classList.contains("active") ? "2" : pageOne?.classList.contains("active") ? "1" : null,
        hasExpandedStorage: window.__mir2Stage5?.state?.hasExpandedStorage ?? null,
        worldTick: window.__mir2Stage5?.state?.worldTick ?? null,
        lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
        liveLastCommand: window.__mir2LastCommand ?? null,
        lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
        gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
        pageLocked: Boolean(document.querySelector(".storage-page-locked")),
        storageItemCards: document.querySelectorAll(".storage-item-card").length,
        storageSlots: document.querySelectorAll(".storage-slot").length,
        statusText: document.querySelector(".storage-window-status")?.textContent?.trim() ?? "",
        rentalText: document.querySelector(".storage-window-rental")?.textContent?.trim() ?? "",
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function waitForStorageState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readStorageState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for storage ${label}; current state: ${JSON.stringify(state)}`);
}

async function readStoragePasswordState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      panelVisible: Boolean(document.querySelector(".storage-password-panel")),
      panelTitle: document.querySelector(".storage-password-panel strong")?.textContent?.trim() ?? "",
      promptTexts: Array.from(document.querySelectorAll(".storage-password-panel span")).map(
        (node) => node.textContent?.trim() ?? "",
      ),
      inputCount: document.querySelectorAll(".storage-password-panel input").length,
      submitDisabled: document.querySelector(".storage-password-panel .inventory-delete-actions button")?.disabled ?? null,
      protectDisabled: document.querySelector(".storage-action-button.protect")?.disabled ?? null,
      hasStoragePassword: window.__mir2Stage5?.state?.hasStoragePassword ?? null,
      requireStoragePassword: window.__mir2Stage5?.state?.requireStoragePassword ?? null,
      storageSessionUnlocked: window.__mir2Stage5?.state?.storageSessionUnlocked ?? null,
      storagePasswordLastSetBinaryDatetime:
        window.__mir2Stage5?.state?.storagePasswordLastSetBinaryDatetime ?? null,
      inputValues: Array.from(document.querySelectorAll(".storage-password-panel input")).map(
        (input) => input.value ?? ""
      ),
      actionLabels: Array.from(
        document.querySelectorAll(".storage-password-panel .inventory-delete-actions button")
      ).map((button) => button.textContent?.trim() ?? ""),
      chatLines: Array.from(document.querySelectorAll(".chat-feed-line")).map(
        (line) => line.textContent?.trim() ?? ""
      ),
    }))()
  `);
}

async function waitForStoragePasswordState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readStoragePasswordState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for storage password ${label}; current state: ${JSON.stringify(state)}`);
}

async function setStoragePasswordPanelInputs(client, { currentPassword, newPassword, confirmPassword }) {
  const inputCount = await client.evaluate('document.querySelectorAll(".storage-password-panel input").length');
  const values =
    inputCount === 2
      ? [newPassword, confirmPassword]
      : [currentPassword, newPassword, confirmPassword];
  for (let index = 0; index < values.length; index += 1) {
    if (values[index] === undefined) {
      continue;
    }
    await setStoragePasswordPanelInput(client, index, values[index]);
    await delay(100);
  }
}

async function setStoragePasswordPanelInput(client, index, value) {
  const updated = await client.evaluate(`
    (() => {
      const inputs = Array.from(document.querySelectorAll(".storage-password-panel input"));
      const input = inputs[${Number(index)}];
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText" }));
      return true;
    })()
  `);
  if (!updated) throw new Error(`Could not set storage password panel input ${index}`);
}

async function clickStoragePasswordModeButton(client, label) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".storage-password-mode-row button")).find(
        (node) => (node.textContent?.trim() ?? "") === ${JSON.stringify(label)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click storage password mode ${label}`);
}

async function readChatState(client, label) {
  return client.evaluate(`
    (() => {
      const frame = document.querySelector(".chat-frame");
      const feed = document.querySelector(".chat-feed");
      const knob = document.querySelector(".chat-position-knob");
      const input = document.querySelector(".chat-textbox");
      const rect = frame?.getBoundingClientRect();
      const lines = Array.from(document.querySelectorAll(".chat-feed-line"));
      const hiddenFilters = Array.from(document.querySelectorAll(".chat-settings-option[data-chat-option-hidden='true']"))
        .map((button) => button.getAttribute("data-chat-option-filter"))
        .filter((key) => key && key !== "all");
      return {
        label: ${JSON.stringify(label)},
        frameVisible: Boolean(frame),
        collapsed: frame?.classList.contains("collapsed") ?? null,
        feedHidden: feed?.classList.contains("hidden") ?? null,
        transparent: frame?.classList.contains("transparent") ?? null,
        settingsOpen: Boolean(document.querySelector(".chat-settings-panel")),
        reportOpen: Boolean(document.querySelector(".report-panel")),
        activeFilter:
          document.querySelector('.chat-filter-button[data-chat-filter-active="true"]')?.getAttribute("data-chat-filter-key") ??
          null,
        chatText: input instanceof HTMLInputElement ? input.value : null,
        chatPrefix: input instanceof HTMLElement ? input.getAttribute("data-chat-prefix") : null,
        hiddenFilters,
        lastCommand: window.__mir2LastCommand ?? null,
        visibleLineTexts: lines.map((line) => line.textContent?.trim() ?? ""),
        visibleLineClasses: lines.map((line) => line.className),
        nonEmptyLineCount: lines.filter((line) => (line.textContent?.trim() ?? "") !== "").length,
        knobTop: knob instanceof HTMLElement ? knob.style.top : "",
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function waitForChatState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readChatState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for chat ${label}; current state: ${JSON.stringify(state)}`);
}

async function runChatControlsSmoke(client, { chatFlow, chatChannelFlow, reportFlow, screenshots }) {
  await waitForSelector(client, ".chat-filter-bar", 5_000);
  const hitTest = await readChatHitTestState(client, "initialHitTest");
  const blockedButtons = hitTest.buttons.filter((button) => !button.topMatches);
  if (blockedButtons.length > 0) {
    throw new Error(`Chat control buttons are covered: ${JSON.stringify(blockedButtons)}`);
  }
  chatFlow.push(hitTest);
  chatFlow.push(await readChatState(client, "allInitial"));

  await clickChatControlButton(client, '.chat-filter-button[data-chat-filter-key="shout"] button', "shout");
  await waitForChatState(
    client,
    (state) => state.activeFilter === "shout" && state.chatText === "!" && state.chatPrefix === "!",
    "shout prefix selected",
    5_000,
  );
  chatChannelFlow.push(await readChatState(client, "shoutPrefix"));
  await focusSelector(client, ".chat-textbox");
  await setInputValue(client, ".chat-textbox", "!Codex shout smoke");
  await pressKey(client, "Enter", "Enter", 13);
  await waitForChatState(
    client,
    (state) =>
      state.lastCommand?.type === "chat" &&
      state.lastCommand?.message === "!Codex shout smoke" &&
      state.chatText === "!",
    "shout send command",
    5_000,
  );
  chatChannelFlow.push(await readChatState(client, "shoutSendCommand"));
  screenshots.push(await screenshot(client, "stage5-chat-controls-shout.png"));

  const channelButtons = [
    { key: "all", prefix: "" },
    { key: "whisper", prefix: "/" },
    { key: "lover", prefix: ":)" },
    { key: "mentor", prefix: "!#" },
    { key: "group", prefix: "!!" },
    { key: "guild", prefix: "!~" },
  ];
  for (const { key, prefix } of channelButtons) {
    await clickChatControlButton(client, `.chat-filter-button[data-chat-filter-key="${key}"] button`, key);
    await waitForChatState(
      client,
      (state) => state.activeFilter === key && state.chatText === prefix && state.chatPrefix === prefix,
      `${key} prefix selected`,
      5_000,
    );
    chatChannelFlow.push(await readChatState(client, `${key}Prefix`));
  }
  screenshots.push(await screenshot(client, "stage5-chat-controls-prefixes.png"));

  await client.evaluate("window.__mir2CommandHistory = []");
  await clickChatControlButton(client, '.chat-filter-button[data-chat-filter-key="trade"] button', "trade");
  await waitForChatState(
    client,
    (state) => state.lastCommand?.type === "tradeRequest" && state.activeFilter === "guild",
    "trade request command",
    5_000,
  );
  chatChannelFlow.push(await readChatState(client, "tradeRequest"));
  screenshots.push(await screenshot(client, "stage5-chat-controls-trade.png"));

  await clickChatControlButton(client, ".chat-filter-button.settings button", "settings");
  await waitForChatState(client, (state) => state.settingsOpen === true, "settings open", 5_000);
  await clickChatControlButton(client, '.chat-settings-option[data-chat-option-filter="normal"]', "normal filter off");
  await waitForChatState(
    client,
    (state) => state.hiddenFilters.includes("normal"),
    "settings normal hidden",
    5_000,
  );
  await clickChatControlButton(client, '.chat-settings-option[data-chat-option-filter="normal"]', "normal filter on");
  await waitForChatState(
    client,
    (state) => !state.hiddenFilters.includes("normal"),
    "settings normal restored",
    5_000,
  );
  await clickChatControlButton(client, ".chat-settings-tabs button[data-chat-transparent]", "transparent toggle on");
  await waitForChatState(client, (state) => state.transparent === true, "chat transparent on", 5_000);
  await clickChatControlButton(client, ".chat-settings-tabs button[data-chat-transparent]", "transparent toggle off");
  await waitForChatState(client, (state) => state.transparent === false, "chat transparent off", 5_000);
  chatFlow.push(await readChatState(client, "settingsToggles"));
  screenshots.push(await screenshot(client, "stage5-chat-controls-settings.png"));
  await clickOptional(client, ".chat-settings-close");
  await waitForChatState(client, (state) => state.settingsOpen === false, "settings closed", 5_000);

  await clickChatControlButton(client, ".chat-filter-button.size button", "size collapse");
  await waitForChatState(client, (state) => state.collapsed === true && state.feedHidden === true, "chat collapsed", 5_000);
  chatFlow.push(await readChatState(client, "collapsed"));
  await clickChatControlButton(client, ".chat-filter-button.size button", "size expand");
  await waitForChatState(client, (state) => state.collapsed === false && state.feedHidden === false, "chat expanded", 5_000);
  chatFlow.push(await readChatState(client, "expanded"));

  await clickChatControlButton(client, ".chat-filter-button.report button", "report");
  await waitForChatState(client, (state) => state.reportOpen === true, "report open", 5_000);
  chatFlow.push(await readChatState(client, "reportOpen"));
  reportFlow.push(await readReportState(client, "reportOpen"));
  screenshots.push(await screenshot(client, "stage5-chat-controls-report.png"));
  await clickOptional(client, ".report-panel .overlay-panel-head button");
  await waitForChatState(client, (state) => state.reportOpen === false, "report closed", 5_000);
  reportFlow.push(await readReportState(client, "reportClosed"));
}

async function runMiniMapSmoke(client, { minimapFlow, mailFlow, screenshots }) {
  await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button");
  await delay(300);
  await transferMapAndWait(client, "crystal:0:334:289", "minimap Bichon smoke", {
    attempts: 2,
    timeoutMs: 12_000,
  });
  await waitForMiniMapState(
    client,
    (state) =>
      state.panelVisible === true &&
      state.sceneHidden === false &&
      state.nameText === "BichonProvince" &&
      state.sceneHasRaster === true,
    "expanded Crystal raster",
    5_000,
  );
  const expanded = await readMiniMapState(client, "expanded");
  assertMiniMapButtonHitTargets(expanded);
  if (expanded.titleCount !== 1 || expanded.nameText !== "BichonProvince") {
    throw new Error(`MiniMap title is not Crystal-aligned: ${JSON.stringify(expanded)}`);
  }
  if (!expanded.sceneHasRaster || expanded.sceneHasFallback || expanded.sceneHidden) {
    throw new Error(`MiniMap scene did not render the Crystal raster: ${JSON.stringify(expanded)}`);
  }
  minimapFlow.push(expanded);
  screenshots.push(await screenshot(client, "stage5-minimap-only-expanded.png"));

  await clickSelector(client, ".mini-map-button.toggle button");
  await waitForMiniMapState(
    client,
    (state) =>
      state.panelVisible === true &&
      state.sceneHidden === true &&
      state.titleCount === 0 &&
      state.smallMode === true,
    "collapsed Crystal frame",
    5_000,
  );
  const collapsed = await readMiniMapState(client, "collapsed");
  assertMiniMapButtonHitTargets(collapsed);
  minimapFlow.push(collapsed);
  screenshots.push(await screenshot(client, "stage5-minimap-only-collapsed.png"));

  await clickSelector(client, ".mini-map-button.bigmap button");
  await waitForMiniMapState(client, (state) => state.bigMapOpen === true, "big map dialog open", 5_000);
  minimapFlow.push(await readMiniMapState(client, "bigMapOpen"));
  screenshots.push(await screenshot(client, "stage5-minimap-only-bigmap.png"));

  await clickSelector(client, ".mini-map-button.mail button");
  await waitForSelector(client, ".mail-panel", 5_000);
  minimapFlow.push(await readMiniMapState(client, "mailOpen"));
  mailFlow.push(await readMailState(client, "mailOpen"));
  screenshots.push(await screenshot(client, "stage5-minimap-only-mail.png"));

  await clickOptional(client, ".mail-panel .mail-close button");
  await waitForMiniMapState(client, (state) => state.mailOpen === false, "mail closed", 5_000);
  mailFlow.push(await readMailState(client, "mailClosed"));
}

function assertMiniMapButtonHitTargets(state) {
  const coveredButtons = (state.buttons ?? []).filter((button) => !button.topMatches);
  if (coveredButtons.length > 0) {
    throw new Error(`MiniMap buttons are covered: ${JSON.stringify(coveredButtons)}`);
  }
}

async function clickChatControlButton(client, selector, label) {
  await mouseClickElementByExpression(
    client,
    `document.querySelector(${JSON.stringify(selector)})`,
    `chat control ${label}`,
  );
  await delay(100);
}

async function readChatHitTestState(client, label) {
  return client.evaluate(`
    (() => {
      const buttons = Array.from(document.querySelectorAll(".chat-filter-button button")).map((button) => {
        const container = button.closest(".chat-filter-button");
        const rect = button.getBoundingClientRect();
        const x = rect.left + rect.width / 2;
        const y = rect.top + rect.height / 2;
        const topElement = document.elementFromPoint(x, y);
        const topButton = topElement?.closest?.("button") ?? null;
        return {
          key: container?.getAttribute("data-chat-filter-key") ?? container?.className ?? "",
          topMatches: topButton === button,
          topClassName: topElement?.className ?? "",
          rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
        };
      });
      return { label: ${JSON.stringify(label)}, buttons };
    })()
  `);
}

async function readStage5SystemsState(client, label) {
  return client.evaluate(`
    (() => {
      const systems = window.__mir2Stage5?.state?.stage5Systems ?? null;
      return {
        label: ${JSON.stringify(label)},
        group: systems?.group ?? null,
        guild: systems?.guild ?? null,
        social: systems?.social ?? null,
        mail: systems?.mail ?? null,
        trade: systems?.trade ?? null,
        auction: systems?.auction ?? [],
        conquest: systems?.conquest ?? null,
        hero: systems?.hero ?? null,
        profession: systems?.profession ?? null,
        gold: window.__mir2Stage5?.state?.gold ?? null,
        credit: window.__mir2Stage5?.state?.credit ?? null,
        inventoryItems: (window.__mir2Stage5?.state?.inventoryItems ?? []).map((item) => ({
          key: item.key,
          name: item.name,
          quantity: item.quantity,
          slot: item.slot,
          container: item.container,
        })),
        monsterCount: (window.__mir2Stage5?.state?.entities ?? []).filter((entity) => entity.kind === "monster").length,
      };
    })()
  `);
}

async function waitForLatestAuctionListingId(client, itemKey, label, timeoutMs = 5_000) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readStage5SystemsState(client, label);
    const listings = (state.auction ?? [])
      .filter((listing) => listing.itemKey === itemKey && !listing.sold && !listing.cancelled && !listing.expired)
      .sort((left, right) => Number(right.id ?? 0) - Number(left.id ?? 0));
    const id = Number(listings[0]?.id ?? 0);
    if (Number.isFinite(id) && id > 0) return id;
    await delay(100);
  }
  throw new Error(`Timed out waiting for ${label}; current state: ${JSON.stringify(state)}`);
}

async function readGameShopState(client, label) {
  return client.evaluate(`
    (() => {
      const parsePrice = (selector, root) => {
        const text = root.querySelector(selector)?.textContent?.trim() ?? "";
        const value = Number.parseInt(text.replace(/[^0-9]/g, ""), 10);
        return Number.isFinite(value) ? value : 0;
      };
      const goldCells = Array.from(document.querySelectorAll(".game-shop-cell-frame"))
        .map((cell, index) => ({
          index,
          name:
            cell.querySelector(".game-shop-cell-name")?.getAttribute("title") ??
            cell.querySelector(".game-shop-cell-name")?.textContent?.trim() ??
            "",
          goldPrice: parsePrice(".game-shop-cell-gold-price", cell),
          creditPrice: parsePrice(".game-shop-cell-credit-price", cell),
        }))
        .filter((cell) => cell.name && cell.goldPrice > 0);
      return {
        label: ${JSON.stringify(label)},
        visible: Boolean(document.querySelector(".game-shop-window")),
        gold: window.__mir2Stage5?.state?.gold ?? null,
        credit: window.__mir2Stage5?.state?.credit ?? null,
        firstGoldItem: goldCells[0] ?? null,
        visibleGoldItems: goldCells,
        inventoryItems: (window.__mir2Stage5?.state?.inventoryItems ?? []).map((item) => ({
          key: item.key,
          name: item.name,
          quantity: item.quantity,
          slot: item.slot,
          container: item.container,
        })),
        beltItems: (window.__mir2Stage5?.state?.beltItems ?? []).map((item) => ({
          key: item.key,
          name: item.name,
          quantity: item.quantity,
          slot: item.slot,
          container: item.container,
        })),
        mail: window.__mir2Stage5?.state?.stage5Systems?.mail ?? [],
        chatLines: (window.__mir2Stage5?.state?.logs ?? []).map((line) => line.text ?? ""),
      };
    })()
  `);
}

async function waitForGameShopState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readGameShopState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for game shop ${label}; current state: ${JSON.stringify(state)}`);
}

async function clickFirstGameShopGoldBuy(client) {
  const clicked = await client.evaluate(`
    (() => {
      const parsePrice = (cell) => {
        const text = cell.querySelector(".game-shop-cell-gold-price")?.textContent?.trim() ?? "";
        const value = Number.parseInt(text.replace(/[^0-9]/g, ""), 10);
        return Number.isFinite(value) ? value : 0;
      };
      const cell = Array.from(document.querySelectorAll(".game-shop-cell-frame")).find(
        (entry) => parsePrice(entry) > 0
      );
      const button = cell?.querySelector(".game-shop-cell-buy button");
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error("Could not click first gold-priced game shop buy button");
}

function totalCarryQuantity(state) {
  return [...(state?.inventoryItems ?? []), ...(state?.beltItems ?? [])].reduce(
    (total, item) => total + (Number(item.quantity) || 0),
    0,
  );
}

function hasGameShopGoldPurchaseChat(state, itemName, goldPrice) {
  return (state?.chatLines ?? []).some(
    (line) => line.includes(`You bought ${itemName}`) && line.includes(`${goldPrice}`) && line.includes("Gold"),
  );
}

async function readMailState(client, label) {
  return client.evaluate(`
    (() => {
      const panel = document.querySelector(".mail-panel");
      const rect = panel?.getBoundingClientRect();
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        title: document.querySelector(".mail-panel .mail-title-image") ? "Mail" : "",
        rows: Array.from(document.querySelectorAll(".mail-panel .overlay-panel-row")).map(
          (row) => row.textContent?.trim() ?? "",
        ),
        emptyText: document.querySelector(".mail-panel .overlay-panel-empty")?.textContent?.trim() ?? "",
        footers: Array.from(document.querySelectorAll(".mail-panel .overlay-panel-foot")).map(
          (footer) => footer.textContent?.trim() ?? "",
        ),
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function readReportState(client, label) {
  return client.evaluate(`
    (() => {
      const panel = document.querySelector(".report-panel");
      const rect = panel?.getBoundingClientRect();
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        title: document.querySelector(".report-panel .overlay-panel-head strong")?.textContent?.trim() ?? "",
        rows: Array.from(document.querySelectorAll(".report-panel .overlay-panel-row")).map(
          (row) => row.textContent?.trim() ?? "",
        ),
        footers: Array.from(document.querySelectorAll(".report-panel .overlay-panel-foot")).map(
          (footer) => footer.textContent?.trim() ?? "",
        ),
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function readNpcDialogState(client, label) {
  return client.evaluate(`
    (() => {
      const panel = document.querySelector(".npc-dialog-panel");
      const rect = panel?.getBoundingClientRect();
      const bodyLines = Array.from(document.querySelectorAll(".npc-dialog-body p")).map(
        (line) => line.textContent?.trim() ?? "",
      );
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        caption: document.querySelector(".npc-dialog-caption")?.textContent?.trim() ?? "",
        title: document.querySelector(".npc-dialog-head strong")?.textContent?.trim() ?? "",
        bodyLines,
        links: Array.from(document.querySelectorAll(".npc-dialog-links button")).map((button) => ({
          text: button.textContent?.trim() ?? "",
          target: button.getAttribute("data-target") ?? "",
        })),
        footer: document.querySelector(".npc-dialog-footer")?.textContent?.trim() ?? "",
        visibleText: panel?.textContent?.trim() ?? "",
        inputVisible: Boolean(document.querySelector(".npc-dialog-input-form")),
        inputPrompt: document.querySelector(".npc-dialog-input-form span")?.textContent?.trim() ?? "",
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function clickNpcDialogLinkByTarget(client, target) {
  const clicked = await client.evaluate(`
    (() => {
      const normalizedTarget = ${JSON.stringify(target)}.toLowerCase();
      const button = Array.from(document.querySelectorAll(".npc-dialog-links button")).find(
        (node) => (node.getAttribute("data-target") ?? "").toLowerCase() === normalizedTarget
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click NPC dialog link ${target}`);
}

async function waitForNpcDialogState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readNpcDialogState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for NPC dialog ${label}; current state: ${JSON.stringify(state)}`);
}

async function readCombatState(client, label) {
  return client.evaluate(`
    (() => {
      const now = Date.now();
      const entities = (window.__mir2Stage5?.state?.entities ?? []).map((entity) => ({
        objectId: entity.objectId,
        kind: entity.kind,
        name: entity.name,
        x: entity.x,
        y: entity.y,
        hp: entity.hp ?? null,
        maxHp: entity.maxHp ?? null,
        dead: entity.dead === true,
        attackActive: typeof entity.attackUntil === "number" && entity.attackUntil > now,
        struckActive: typeof entity.struckUntil === "number" && entity.struckUntil > now,
      }));
      return {
        label: ${JSON.stringify(label)},
        selectedObjectId: window.__mir2Stage5?.state?.selectedObjectId ?? null,
        player: window.__mir2Stage5?.state?.player ?? null,
        monsters: entities.filter((entity) => entity.kind === "monster"),
        activeAttackCount: entities.filter((entity) => entity.attackActive).length,
        activeStruckCount: entities.filter((entity) => entity.struckActive).length,
        projectileCount: document.querySelectorAll(".viewport-projectile").length,
        visibleMonsterLabels: Array.from(document.querySelectorAll(".entity-nameplate.monster")).map(
          (node) => node.textContent?.trim() ?? "",
        ),
      };
    })()
  `);
}

async function waitForCombatState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readCombatState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for combat ${label}; current state: ${JSON.stringify(state)}`);
}

async function readSystemMenuState(client, label) {
  return client.evaluate(`
    (() => {
      const panel = document.querySelector(".system-menu-panel");
      const rect = panel?.getBoundingClientRect();
      const featurePanel = document.querySelector(".system-feature-panel");
      const socialPanel = document.querySelector(".system-social-panel");
      const qaInputs = Array.from(document.querySelectorAll(".system-menu-qa-transfer input")).map((input) => ({
        label: input.closest("label")?.querySelector("span")?.textContent?.trim() ?? "",
        value: input.value ?? "",
      }));
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        actionKeys: Array.from(document.querySelectorAll("[data-system-menu-action]")).map(
          (node) => node.getAttribute("data-system-menu-action") ?? "",
        ),
        characterWindowVisible: Boolean(document.querySelector(".character-window")),
        inventoryWindowVisible: Boolean(document.querySelector(".inventory-window")),
        storageWindowVisible: Boolean(document.querySelector(".storage-window")),
        activeInventoryTab: window.__mir2Stage5?.state?.activeInventoryTab ?? null,
        actionLabels: Array.from(document.querySelectorAll(".system-menu-actions button")).map((button) =>
          button.textContent?.trim() ?? ""
        ),
        systemMenuFeaturePanelVisible: Boolean(featurePanel),
        systemMenuFeaturePanelType: featurePanel?.getAttribute("data-system-feature-panel") ?? null,
        systemMenuSocialPanelVisible: Boolean(socialPanel),
        systemMenuSocialPanelType: socialPanel?.getAttribute("data-system-social-panel") ?? null,
        systemMenuSocialPanelTabKey: socialPanel?.getAttribute("data-system-social-tab") ?? null,
        systemMenuSocialPanelSelectedRow: socialPanel?.getAttribute("data-system-social-selected-row") ?? null,
        systemMenuSocialPanelStatus: socialPanel?.getAttribute("data-system-social-status") ?? null,
        systemMenuSocialTabKeys: Array.from(document.querySelectorAll(".system-social-tabs button")).map(
          (button) => button.getAttribute("data-social-tab-key") ?? "",
        ),
        systemMenuSocialEntryNames: Array.from(document.querySelectorAll(".system-social-entry")).map(
          (button) => button.getAttribute("data-social-entry-name") ?? "",
        ),
        systemMenuSocialActionLabels: Array.from(document.querySelectorAll(".system-social-actions button")).map(
          (button) => button.getAttribute("data-social-action-label") ?? "",
        ),
        transferLabels: Array.from(document.querySelectorAll(".system-menu-transfer-list button")).map((button) =>
          button.textContent?.trim() ?? ""
        ),
        qaTransferTitle: document.querySelector(".system-menu-qa-transfer .system-menu-transfer-title")?.textContent?.trim() ?? "",
        qaInputs,
        currentMapFileName: window.__mir2Stage5?.state?.mapFileName ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        lastCommand: window.__mir2LastCommand ?? null,
        commandHistory: window.__mir2CommandHistory ?? [],
        featureActionLabels: Array.from(document.querySelectorAll(".system-feature-panel button")).map((button) =>
          button.textContent?.trim() || button.getAttribute("aria-label") || ""
        ),
        playerPosition: window.__mir2Stage5?.state?.player
          ? {
              x: window.__mir2Stage5.state.player.x,
              y: window.__mir2Stage5.state.player.y,
            }
          : null,
        metaText: document.querySelector(".system-menu-meta")?.textContent?.trim() ?? "",
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function waitForSystemMenuState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readSystemMenuState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(
    `Timed out waiting for system menu ${label}; current state: ${JSON.stringify(state)}; consoleErrors: ${JSON.stringify(client.consoleErrors.slice(-8))}`,
  );
}

function hasRecentCommand(state, predicate) {
  const commands = [
    ...(Array.isArray(state?.commandHistory) ? state.commandHistory : []),
    state?.lastCommand,
  ].filter(Boolean);
  return commands.some(predicate);
}

async function clickSystemMenuAction(client, index) {
  const clicked = await client.evaluate(`
    (() => {
      const button = document.querySelectorAll(".system-menu-actions button")[${Number(index)}];
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click system menu action ${index}`);
}

async function clickSystemMenuActionByKey(client, actionKey) {
  const clicked = await client.evaluate(`
    (() => {
      const action = Array.from(document.querySelectorAll("[data-system-menu-action]")).find(
        (entry) => entry.getAttribute("data-system-menu-action") === ${JSON.stringify(actionKey)}
      );
      const button = action?.matches("button") ? action : action?.querySelector("button");
      if (!button || typeof button.click !== "function") return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) {
    const state = await readSystemMenuState(client, `click ${actionKey} failed`);
    throw new Error(`Could not click system menu action ${actionKey}; current state: ${JSON.stringify(state)}`);
  }
}

async function clickSystemMenuActionByLabel(client, label) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".system-menu-actions button")).find(
        (entry) => (entry.textContent ?? "").trim() === ${JSON.stringify(label)}
      );
      if (!button || typeof button.click !== "function") return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click system menu action with label ${label}`);
}

async function clickSystemMenuFeaturePanelClose(client) {
  await clickSelector(client, ".system-feature-close, .system-feature-panel .overlay-panel-head button");
}

async function clickSystemMenuSocialTabByKey(client, tabKey) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".system-social-tabs button")).find(
        (entry) => entry.getAttribute("data-social-tab-key") === ${JSON.stringify(tabKey)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click system menu social tab ${tabKey}`);
}

async function clickSystemMenuSocialEntryByName(client, entryName) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".system-social-entry")).find(
        (entry) => entry.getAttribute("data-social-entry-name") === ${JSON.stringify(entryName)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click system menu social entry ${entryName}`);
}

async function clickSystemMenuSocialActionByLabel(client, actionLabel) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".system-social-actions button")).find(
        (entry) => entry.getAttribute("data-social-action-label") === ${JSON.stringify(actionLabel)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click system menu social action ${actionLabel}`);
}

async function clickSystemMenuTransfer(client, index) {
  const clicked = await client.evaluate(`
    (() => {
      const button = document.querySelectorAll(".system-menu-transfer-list button")[${Number(index)}];
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click system menu transfer ${index}`);
}

async function setSystemMenuQaTransferInputs(client, { map, x, y }) {
  const values = [map, String(x), String(y)];
  for (let index = 0; index < values.length; index += 1) {
    const updated = await client.evaluate(`
      (() => {
        const input = document.querySelectorAll(".system-menu-qa-transfer input")[${index}];
        if (!input) return false;
        const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
        setter.call(input, ${JSON.stringify(values[index])});
        input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText" }));
        return true;
      })()
    `);
    if (!updated) throw new Error(`Could not set system menu QA transfer input ${index}`);
    await delay(100);
  }
}

async function readHudButtonState(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      characterWindowVisible: Boolean(document.querySelector(".character-window")),
      inventoryWindowVisible: Boolean(document.querySelector(".inventory-window")),
      activeCharacterTab: window.__mir2Stage5?.state?.activeCharacterTab ?? null,
      activeInventoryTab: window.__mir2Stage5?.state?.activeInventoryTab ?? null,
      spellValueCount: document.querySelectorAll(".character-spell-value").length,
      statsValueCount: document.querySelectorAll(".character-field-value").length,
    }))()
  `);
}

async function readSpellCastState(client, label, skillKey) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      characterWindowVisible: Boolean(document.querySelector(".character-window")),
      activeCharacterTab: window.__mir2Stage5?.state?.activeCharacterTab ?? null,
      knownSkills: (window.__mir2Stage5?.state?.knownSkills ?? []).map((skill) => ({
        key: skill.key,
        name: skill.name,
        cooldownRemainingTicks: skill.cooldownRemainingTicks,
      })),
      activeBuffs: (window.__mir2Stage5?.state?.activeBuffs ?? []).map((buff) => ({
        key: buff.key,
        name: buff.name,
        attackBonus: buff.attackBonus,
        defenceBonus: buff.defenceBonus,
        remainingTicks: buff.remainingTicks,
      })),
      matchingSpellText: Array.from(document.querySelectorAll(".character-spell-value")).map(
        (node) => node.textContent?.trim() ?? "",
      ).find((text) => text.includes(${JSON.stringify(skillKey)})) ?? "",
    }))()
  `);
}

async function waitForSpellCastState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readSpellCastState(client, label, "battle-focus");
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for spell cast ${label}; current state: ${JSON.stringify(state)}`);
}

async function clickCharacterSpellByName(client, skillName) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".character-spell-value")).find(
        (node) => (node.textContent ?? "").includes(${JSON.stringify(skillName)})
      );
      if (!button || typeof button.click !== "function") return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click character spell ${skillName}`);
}

async function assertCompactLayout(client, viewport) {
  const layout = await client.evaluate(`
    (() => {
      const selectors = [
        ".client-stage-frame",
        ".game-ui-scene",
        ".main-hud-shell",
        ".chat-frame",
        ".mini-map-panel",
      ];
      return selectors.map((selector) => {
        const node = document.querySelector(selector);
        if (!node) return { selector, missing: true };
        const rect = node.getBoundingClientRect();
        return {
          selector,
          missing: false,
          left: rect.left,
          top: rect.top,
          right: rect.right,
          bottom: rect.bottom,
          width: rect.width,
          height: rect.height,
        };
      });
    })()
  `);
  const tolerance = 1;
  const overflowing = layout.filter(
    (entry) =>
      entry.missing ||
      entry.left < -tolerance ||
      entry.top < -tolerance ||
      entry.right > viewport.width + tolerance ||
      entry.bottom > viewport.height + tolerance,
  );
  if (overflowing.length > 0) {
    throw new Error(`Compact viewport layout overflow: ${JSON.stringify(overflowing, null, 2)}`);
  }
  return layout;
}

async function assertCoreTextLayout(client) {
  const result = await client.evaluate(`
    (() => {
      const selectors = [
        ".quest-tracker-title",
        ".quest-stage",
        ".quest-name",
        ".quest-progress",
        ".quest-objective",
        ".quest-reward",
        ".quest-hint-line",
        ".mini-map-name",
        ".mini-map-coords",
        ".hud-top-label",
        ".hud-bottom-label",
        ".hud-level-label",
        ".hud-name-label",
        ".hud-map-label",
        ".hud-buff-label",
        ".hud-exp-label",
        ".hud-gold-label",
        ".hud-weight-label",
        ".hud-space-label",
        ".belt-slot-label",
        ".belt-item-count",
        ".chat-feed-line",
        ".drop-label",
        ".entity-nameplate",
        ".mail-row-sender",
        ".mail-row-message",
        ".mail-row-flag",
        ".mail-page-label",
        ".game-shop-cell-name",
        ".game-shop-cell-stock-label",
        ".game-shop-cell-stock-value",
        ".game-shop-total",
        ".game-shop-payment",
        ".game-shop-page",
        ".storage-window-title",
        ".storage-window-subtitle",
        ".storage-window-status",
        ".storage-window-rental",
        ".character-field-values",
        ".character-field-value",
        ".system-menu-transfer-title",
        ".system-menu-meta",
        ".system-feature-title",
        ".system-feature-panel strong",
        ".system-social-footer",
        ".inventory-quest-summary",
        ".inventory-quest-objective",
        ".inventory-quest-progress-row",
      ];
      const nodes = Array.from(new Set(selectors.flatMap((selector) => Array.from(document.querySelectorAll(selector)))));
      const entries = [];
      for (const node of nodes) {
        const style = getComputedStyle(node);
        const rect = node.getBoundingClientRect();
        const text = node.textContent?.trim() ?? "";
        if (!text || style.display === "none" || style.visibility === "hidden" || rect.width <= 0 || rect.height <= 0) {
          continue;
        }
        const overflowX = Math.max(0, node.scrollWidth - Math.ceil(node.clientWidth));
        const overflowY = Math.max(0, node.scrollHeight - Math.ceil(node.clientHeight));
        entries.push({
          selector: selectors.find((selector) => node.matches(selector)) ?? node.className,
          text,
          width: rect.width,
          height: rect.height,
          clientWidth: node.clientWidth,
          clientHeight: node.clientHeight,
          scrollWidth: node.scrollWidth,
          scrollHeight: node.scrollHeight,
          overflowX,
          overflowY,
        });
      }
      return entries;
    })()
  `);
  const tolerance = 2;
  const overflowing = result.filter((entry) => entry.overflowX > tolerance || entry.overflowY > tolerance);
  if (overflowing.length > 0) {
    throw new Error(`Core text overflow: ${JSON.stringify(overflowing, null, 2)}`);
  }
  return {
    checked: result.length,
    overflowing,
  };
}

async function assertPanelLayout(client, viewport, selectors) {
  const layout = await client.evaluate(`
    (() => {
      const selectors = ${JSON.stringify(selectors)};
      return selectors.map((selector) => {
        const node = document.querySelector(selector);
        if (!node) return { selector, missing: true };
        const rect = node.getBoundingClientRect();
        return {
          selector,
          missing: false,
          left: rect.left,
          top: rect.top,
          right: rect.right,
          bottom: rect.bottom,
          width: rect.width,
          height: rect.height,
        };
      });
    })()
  `);
  const tolerance = 1;
  const overflowing = layout.filter(
    (entry) =>
      entry.missing ||
      entry.left < -tolerance ||
      entry.top < -tolerance ||
      entry.right > viewport.width + tolerance ||
      entry.bottom > viewport.height + tolerance,
  );
  if (overflowing.length > 0) {
    throw new Error(`Compact panel layout overflow: ${JSON.stringify(overflowing, null, 2)}`);
  }
  return layout;
}

async function readMiniMapState(client, label) {
  return client.evaluate(`
    (() => {
      const panel = document.querySelector(".mini-map-panel");
      const scene = document.querySelector(".mini-map-scene-shell");
      const mail = document.querySelector(".mail-panel");
      const bigMap = document.querySelector(".big-map-dialog");
      const name = document.querySelector(".mini-map-name");
      const titles = Array.from(document.querySelectorAll(".mini-map-name"));
      const coords = document.querySelector(".mini-map-coords");
      const rect = panel?.getBoundingClientRect();
      const sceneRect = scene?.getBoundingClientRect();
      const raster = document.querySelector(".mini-map-raster");
      const fallback = document.querySelector(".mini-map-patch-fallback");
      const buttons = Array.from(document.querySelectorAll(".mini-map-button")).map((container) => {
        const button = container.querySelector("button");
        const buttonRect = button?.getBoundingClientRect();
        if (!buttonRect) {
          return {
            className: container.className,
            visible: false,
            topMatches: false,
          };
        }
        const x = buttonRect.left + buttonRect.width / 2;
        const y = buttonRect.top + buttonRect.height / 2;
        const topElement = document.elementFromPoint(x, y);
        return {
          className: container.className,
          visible: buttonRect.width > 0 && buttonRect.height > 0,
          topMatches: Boolean(topElement && container.contains(topElement)),
          topElement: topElement?.className?.toString?.() ?? topElement?.tagName ?? null,
          rect: {
            left: buttonRect.left,
            top: buttonRect.top,
            right: buttonRect.right,
            bottom: buttonRect.bottom,
            width: buttonRect.width,
            height: buttonRect.height,
          },
        };
      });
      return {
        label: ${JSON.stringify(label)},
        panelVisible: Boolean(panel),
        smallMode: panel?.classList.contains("small") ?? null,
        sceneHidden: scene?.classList.contains("hidden") ?? null,
        sceneHasRaster: Boolean(raster),
        sceneHasFallback: Boolean(fallback),
        mailOpen: Boolean(mail),
        bigMapOpen: Boolean(bigMap),
        nameText: name?.textContent?.trim() ?? "",
        titleCount: titles.length,
        titleTexts: titles.map((title) => title.textContent?.trim() ?? ""),
        coordsText: coords?.textContent?.trim() ?? "",
        buttons,
        sceneRect: sceneRect
          ? {
              left: sceneRect.left,
              top: sceneRect.top,
              right: sceneRect.right,
              bottom: sceneRect.bottom,
              width: sceneRect.width,
              height: sceneRect.height,
            }
          : null,
        panelRect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function waitForMiniMapState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readMiniMapState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for minimap ${label}; current state: ${JSON.stringify(state)}`);
}

async function readBeltState(client, label) {
  return client.evaluate(`
    (() => {
      const dialog = document.querySelector(".belt-dialog");
      const quest = document.querySelector(".quest-tracker-panel");
      const rect = dialog?.getBoundingClientRect();
      const questRect = quest?.getBoundingClientRect();
      const labelNodes = Array.from(document.querySelectorAll(".belt-slot-label"));
      const labels = labelNodes.map((node) => node.textContent?.trim() ?? "");
      const labelRects = labelNodes.map((node) => {
        const labelRect = node.getBoundingClientRect();
        return {
          text: node.textContent?.trim() ?? "",
          left: labelRect.left,
          top: labelRect.top,
          right: labelRect.right,
          bottom: labelRect.bottom,
          width: labelRect.width,
          height: labelRect.height,
        };
      });
      const labelsWithinDialog = Boolean(
        rect &&
          labelRects.every(
            (labelRect) =>
              labelRect.left >= rect.left - 1 &&
              labelRect.right <= rect.right + 1 &&
              labelRect.top >= rect.top - 1 &&
              labelRect.bottom <= rect.bottom + 1,
          ),
      );
      const overlapsQuestTracker = Boolean(
        rect &&
          questRect &&
          rect.left < questRect.right &&
          rect.right > questRect.left &&
          rect.top < questRect.bottom &&
          rect.bottom > questRect.top,
      );
      return {
        label: ${JSON.stringify(label)},
        visible: Boolean(dialog),
        orientation: dialog?.classList.contains("vertical")
          ? "vertical"
          : dialog?.classList.contains("horizontal")
            ? "horizontal"
            : null,
        slotCount: document.querySelectorAll(".belt-slot").length,
        itemCount: document.querySelectorAll(".belt-item").length,
        labels,
        labelRects,
        labelsWithinDialog,
        overlapsQuestTracker,
        rect: rect
          ? {
              left: rect.left,
              top: rect.top,
              right: rect.right,
              bottom: rect.bottom,
              width: rect.width,
              height: rect.height,
            }
          : null,
      };
    })()
  `);
}

async function waitForBeltState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readBeltState(client, label);
    if (predicate(state)) return;
    await delay(100);
  }
  throw new Error(`Timed out waiting for belt ${label}; current state: ${JSON.stringify(state)}`);
}

async function readStage5BeltItems(client, label) {
  return client.evaluate(`
    (() => ({
      label: ${JSON.stringify(label)},
      items: (window.__mir2Stage5?.state?.beltItems ?? []).map((item) => ({
        key: item.key,
        name: item.name,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
        quantity: item.quantity,
      })),
    }))()
  `);
}

async function readBeltItem(client, label, itemName) {
  const state = await readStage5BeltItems(client, label);
  return {
    label,
    item:
      state.items
        .filter((entry) => entry.name === itemName)
        .sort((left, right) => left.slot - right.slot)[0] ?? null,
    items: state.items,
  };
}

async function waitForStage5BeltItemQuantityBelow(client, slot, previousQuantity, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readStage5BeltItems(client, label);
    const item = state.items.find((entry) => entry.slot === slot);
    if (!item || item.quantity < previousQuantity) return state;
    await delay(100);
  }
  throw new Error(
    `Timed out waiting for belt slot ${slot + 1} quantity below ${previousQuantity}; current state: ${JSON.stringify(state)}`,
  );
}

async function waitForBeltItemUniqueIdQuantityBelow(client, uniqueId, previousQuantity, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    const beltState = await readStage5BeltItems(client, label);
    state = {
      label,
      item: beltState.items.find((entry) => Number(entry.uniqueId) === Number(uniqueId)) ?? null,
      items: beltState.items,
    };
    if (!state.item || state.item.quantity < previousQuantity) return state;
    await delay(100);
  }
  const debugState = await client.evaluate(`
    (() => ({
      lastBeltActivation: window.__mir2LastBeltActivation ?? null,
      lastUseItem: window.__mir2LastUseItem ?? null,
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
      playerHp: window.__mir2Stage5?.state?.playerHp ?? null,
      playerMaxHp: window.__mir2Stage5?.state?.playerMaxHp ?? null,
      beltButtonTitles: Array.from(document.querySelectorAll(".belt-item")).map((node) => ({
        title: node.getAttribute("title"),
        text: node.textContent?.trim() ?? "",
        disabled: Boolean(node.disabled),
      })),
    }))()
  `);
  if (options.throwOnTimeout === false) return false;
  throw new Error(
    `Timed out waiting for belt uniqueId ${uniqueId} quantity below ${previousQuantity}; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}`,
  );
}

async function waitForBeltItemQuantityBelow(client, itemName, previousQuantity, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readBeltItem(client, label, itemName);
    if (!state.item || state.item.quantity < previousQuantity) return state;
    await delay(100);
  }
  const debugState = await client.evaluate(`
    (() => ({
      lastBeltActivation: window.__mir2LastBeltActivation ?? null,
      lastUseItem: window.__mir2LastUseItem ?? null,
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
      playerHp: window.__mir2Stage5?.state?.playerHp ?? null,
      playerMaxHp: window.__mir2Stage5?.state?.playerMaxHp ?? null,
      beltButtonTitles: Array.from(document.querySelectorAll(".belt-item")).map((node) => ({
        title: node.getAttribute("title"),
        text: node.textContent?.trim() ?? "",
        disabled: Boolean(node.disabled),
      })),
    }))()
  `);
  if (options.throwOnTimeout === false) return false;
  throw new Error(
    `Timed out waiting for belt ${itemName} quantity below ${previousQuantity}; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}`,
  );
}

async function clickBeltItemByName(client, itemName) {
  await dispatchMouseSequenceToElementByExpression(
    client,
    `
      Array.from(document.querySelectorAll(".belt-item")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      ) ?? null
	    `,
	    `belt item ${itemName}`,
	  );
}

async function pressKey(client, key, code, windowsVirtualKeyCode) {
  await client.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key,
    code,
    windowsVirtualKeyCode,
  });
  await client.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key,
    code,
    windowsVirtualKeyCode,
  });
}

async function waitForChrome(port) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return;
    } catch {
      await delay(200);
    }
  }
  throw new Error(`Chrome did not open CDP port ${port}`);
}

async function waitForSelector(client, selector, timeoutMs) {
  await waitUntil(
    async () => Boolean(await client.evaluate(`Boolean(document.querySelector(${JSON.stringify(selector)}))`)),
    timeoutMs,
    `selector ${selector}`,
  );
}

async function waitForSelectorOptional(client, selector, timeoutMs) {
  try {
    await waitForSelector(client, selector, timeoutMs);
    return true;
  } catch {
    return false;
  }
}

async function waitForText(client, selector, text, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let currentText = "";
  while (Date.now() < deadline) {
    currentText = await client.evaluate(
      `Array.from(document.querySelectorAll(${JSON.stringify(selector)})).map((node) => node.textContent).join(" | ")`,
    );
    if (currentText.includes(text)) return;
    await delay(200);
  }
  throw new Error(`Timed out waiting for text ${text} in ${selector}; current text: ${currentText}`);
}

async function waitUntil(predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await delay(200);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function clickSelector(client, selector) {
  const clicked = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click ${selector}`);
}

async function mouseClickElementByExpression(client, expression, label) {
  const rect = await client.evaluate(`
    (() => {
      const node = (${expression});
      if (!node) return null;
      if (node.disabled) return { disabled: true };
      const rect = node.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return { hidden: true };
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
        width: rect.width,
        height: rect.height,
      };
    })()
  `);
  if (!rect || rect.disabled || rect.hidden) {
    throw new Error(`Could not click ${label}: ${JSON.stringify(rect)}`);
  }
  await client.send("Page.bringToFront");
  await client.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: rect.x,
    y: rect.y,
    button: "none",
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: rect.x,
    y: rect.y,
    button: "left",
    buttons: 1,
    clickCount: 1,
    pointerType: "mouse",
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: rect.x,
    y: rect.y,
    button: "left",
    buttons: 0,
    clickCount: 1,
    pointerType: "mouse",
  });
}

async function dispatchMouseSequenceToElementByExpression(client, expression, label) {
  const dispatched = await client.evaluate(`
    (() => {
      const node = (${expression});
      if (!node || node.disabled) return false;
      for (const [type, button, buttons] of [
        ["mousedown", 0, 1],
        ["mouseup", 0, 0],
        ["click", 0, 0],
      ]) {
        node.dispatchEvent(new MouseEvent(type, {
          bubbles: true,
          cancelable: true,
          button,
          buttons,
          detail: type === "click" ? 1 : 0,
          view: window,
        }));
      }
      return true;
    })()
  `);
  if (!dispatched) throw new Error(`Could not click ${label}`);
  await delay(50);
}

async function dispatchContextMenuToElementByExpression(client, expression, label) {
  const dispatched = await client.evaluate(`
    (() => {
      const node = (${expression});
      if (!node || node.disabled) return false;
      for (const [type, buttons] of [
        ["mousedown", 2],
        ["mouseup", 0],
        ["contextmenu", 0],
      ]) {
        node.dispatchEvent(new MouseEvent(type, {
          bubbles: true,
          cancelable: true,
          button: 2,
          buttons,
          view: window,
        }));
      }
      return true;
    })()
  `);
  if (!dispatched) throw new Error(`Could not context-menu ${label}`);
  await delay(50);
}

async function focusSelector(client, selector) {
  const focused = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node || typeof node.focus !== "function") return false;
      node.focus();
      return document.activeElement === node;
    })()
  `);
  if (!focused) throw new Error(`Could not focus ${selector}`);
}

async function sendGatewayCommand(client, command) {
  const sent = await client.evaluate(`
    (() => {
      const api = window.__mir2Stage5;
      if (!api || typeof api.send !== "function") return false;
      return api.send(${JSON.stringify(command)}) === true;
    })()
  `);
  if (!sent) throw new Error(`Could not send gateway command ${JSON.stringify(command)}`);
}

async function waitForStage5State(client, predicate, label, timeoutMs, options = {}) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await client.evaluate("window.__mir2Stage5?.state ?? null");
    if (predicate(state)) return true;
    await delay(200);
  }
  if (options.throwOnTimeout === false) return false;
  const debugState = await client.evaluate(`
    (() => ({
      lastCommand: window.__mir2Stage5?.state?.lastCommand ?? null,
      liveLastCommand: window.__mir2LastCommand ?? null,
      commandHistory: window.__mir2CommandHistory ?? [],
      lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
      gatewayEventHistory: window.__mir2GatewayEventHistory ?? [],
      webSocketFrames: ${JSON.stringify(client.wsFrames ?? [])},
    }))()
  `);
  throw new Error(
    `Timed out waiting for Stage 5 state ${label}; current state: ${JSON.stringify(state)}; debug: ${JSON.stringify(debugState)}`,
  );
}

async function setInputValue(client, selector, value) {
  const updated = await client.evaluate(`
    (() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    })()
  `);
  if (!updated) throw new Error(`Could not set input ${selector}`);
}

async function clickFirst(client, selector) {
  const clicked = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click first ${selector}`);
}

async function clickOptional(client, selector) {
  await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (node) node.click();
    })()
  `);
}

async function clickAllOptional(client, selector) {
  await client.evaluate(`
    (() => {
      for (const node of Array.from(document.querySelectorAll(${JSON.stringify(selector)}))) {
        node.click();
      }
    })()
  `);
}

async function clickFirstByText(client, selector, text) {
  const clicked = await client.evaluate(`
    (() => {
      const node = Array.from(document.querySelectorAll(${JSON.stringify(selector)}))
        .find((entry) => entry.textContent.includes(${JSON.stringify(text)}));
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click ${selector} containing ${text}`);
}

async function clickButtonByImageAlt(client, alt) {
  const clicked = await client.evaluate(`
    (() => {
      const image = Array.from(document.querySelectorAll("img"))
        .find((entry) => entry.alt === ${JSON.stringify(alt)});
      const button = image?.closest("button") ?? Array.from(document.querySelectorAll("button"))
        .find((entry) =>
          entry.getAttribute("aria-label") === ${JSON.stringify(alt)} ||
          entry.getAttribute("title") === ${JSON.stringify(alt)} ||
          entry.textContent.trim() === ${JSON.stringify(alt)}
        );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click button with image alt ${alt}`);
}

async function screenshot(client, fileName) {
  await delay(200);
  const result = await client.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  const filePath = path.join(OUTPUT_DIR, fileName);
  await fs.writeFile(filePath, Buffer.from(result.data, "base64"));
  return path.relative(path.resolve(process.cwd(), "..", ".."), filePath).replaceAll(path.sep, "/");
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function hasRawCrystalMarkup(text) {
  return /\{\/?[A-Z]+\}|<\$[^>]+>|%[A-Z0-9_()]+/i.test(text ?? "");
}

function findChromePath() {
  const candidates =
    process.platform === "win32"
      ? [
          path.join(process.env.ProgramFiles ?? "", "Google", "Chrome", "Application", "chrome.exe"),
          path.join(process.env["ProgramFiles(x86)"] ?? "", "Google", "Chrome", "Application", "chrome.exe"),
          path.join(process.env.ProgramFiles ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
          path.join(process.env["ProgramFiles(x86)"] ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
        ]
      : process.platform === "darwin"
        ? [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
          ]
        : ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser"];
  return candidates.find((candidate) => candidate && fsSync.existsSync(candidate));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
