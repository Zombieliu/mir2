import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const BASE_URL = process.env.MIR2_WEB_BASE_URL ?? process.argv[2] ?? "http://127.0.0.1:3002";
const OUTPUT_DIR = path.resolve(process.cwd(), "..", "..", "docs", "stage5-screenshots");
const CHROME_PATH = process.env.MIR2_CHROME_PATH ?? findChromePath();
const DEBUG_PORT = Number(process.env.MIR2_CHROME_DEBUG_PORT ?? 9400 + (process.pid % 1000));
const VIEWPORTS = {
  desktop: { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false },
  compact: { width: 820, height: 640, deviceScaleFactor: 1, mobile: false },
};

if (!CHROME_PATH) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH to run the Stage 5 UI smoke.");
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
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
      this.consoleErrors.push({
        source: "exception",
        text: message.params?.exceptionDetails?.text ?? "runtime exception",
      });
    }

    if (message.method === "Log.entryAdded") {
      const entry = message.params?.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        const url = entry.url ? ` (${entry.url})` : "";
        this.consoleErrors.push({ source: entry.source ?? "log", text: `${entry.text ?? ""}${url}` });
      }
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
    await setViewport(client, VIEWPORTS.desktop);
    await client.send("Page.navigate", { url: BASE_URL });
    await waitForSelector(client, ".login-overlay", 15_000);
    loginFlow.push(await readLoginState(client, "initial"));
    screenshots.push(await screenshot(client, "stage5-login.png"));

    const accountId = `stage5-${process.pid}-${Date.now()}`;
    await clickLanguageButton(client, ".login-language-selector", "简体中文");
    await waitForLoginState(client, (state) => state.activeLanguageLabel === "简体中文", "login zh-CN language", 5_000);
    loginFlow.push(await readLoginState(client, "zhCnLanguage"));
    screenshots.push(await screenshot(client, "stage5-login-language-zh.png"));
    await clickLanguageButton(client, ".login-language-selector", "English");
    await waitForLoginState(client, (state) => state.activeLanguageLabel === "English", "login English language", 5_000);
    loginFlow.push(await readLoginState(client, "englishLanguageRestored"));

    await setInputValue(client, ".login-input.account", accountId);
    await setInputValue(client, ".login-input.password", "stage5-pass");
    loginFlow.push(await readLoginState(client, "credentialsFilled"));
    await clickSelector(client, ".login-button.view button");
    await waitForLoginState(client, (state) => state.accountPanelVisible === true, "view key panel", 5_000);
    loginFlow.push(await readLoginState(client, "viewKeyPanel"));
    screenshots.push(await screenshot(client, "stage5-login-view-key.png"));
    await clickSelector(client, ".login-account-panel button");
    await waitForLoginState(client, (state) => state.accountPanelVisible === false, "view key closed", 5_000);
    loginFlow.push(await readLoginState(client, "viewKeyClosed"));

    await clickSelector(client, ".login-button.account button");
    await delay(1_200);
    loginFlow.push(await readLoginState(client, "accountCreated"));
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

    await clickSelector(client, ".select-action.start button");
    await waitForSelector(client, ".game-ui-scene", 15_000);
    await waitForSelector(client, ".hud-button.inventory button", 10_000);
    await waitForStage5State(client, (state) => state?.mapFileName === "0", "starter map", 15_000);
    screenshots.push(await screenshot(client, "stage5-game.png"));

    await clickSelector(client, ".hud-button.inventory button");
    await waitForSelector(client, ".inventory-window", 10_000);
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "bag1", 5_000);
    inventoryFlow.push(await readInventoryState(client, "bag1"));
    screenshots.push(await screenshot(client, "stage5-inventory.png"));

    const beforeInventoryUse = await readInventoryItem(client, "beforeInventoryUse", "Red Potion");
    if (!beforeInventoryUse.item) {
      throw new Error(`Cannot verify inventory item use without Red Potion: ${JSON.stringify(beforeInventoryUse)}`);
    }
    inventoryUseFlow.push(beforeInventoryUse);
    await clickInventoryItemByName(client, "Red Potion");
    const afterInventoryUse = await waitForInventoryItemQuantityBelow(
      client,
      "Red Potion",
      beforeInventoryUse.item.quantity,
      "afterInventoryUse",
      5_000,
    );
    inventoryUseFlow.push(afterInventoryUse);
    screenshots.push(await screenshot(client, "stage5-inventory-use-red-potion.png"));

    const beforeInventoryEquip = await readInventoryEquipmentState(client, "beforeInventoryEquip", "Dagger");
    if (!beforeInventoryEquip.inventoryItem) {
      throw new Error(`Cannot verify inventory equip without Dagger: ${JSON.stringify(beforeInventoryEquip)}`);
    }
    inventoryEquipFlow.push(beforeInventoryEquip);
    await clickInventoryItemByName(client, "Dagger");
    const afterInventoryEquip = await waitForEquipmentItem(client, "Dagger", "afterInventoryEquip", 5_000);
    inventoryEquipFlow.push(afterInventoryEquip);
    screenshots.push(await screenshot(client, "stage5-inventory-equip-dagger.png"));

    const beforeGoldDrop = await readInventoryGoldState(client, "beforeDropGold");
    inventoryGoldFlow.push(beforeGoldDrop);
    await clickButtonByImageAlt(client, "Drop Gold");
    await waitForInventoryGoldState(client, (state) => state.dropGoldPanelOpen === true, "drop gold panel", 5_000);
    inventoryGoldFlow.push(await readInventoryGoldState(client, "dropGoldPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-drop-gold-panel.png"));
    await clickInventoryPanelAction(client, 0);
    const afterGoldDrop = await waitForInventoryGoldState(
      client,
      (state) => state.gold === beforeGoldDrop.gold - 100,
      "after drop gold",
      5_000,
    );
    inventoryGoldFlow.push(afterGoldDrop);
    screenshots.push(await screenshot(client, "stage5-inventory-drop-gold.png"));

    const beforeInventoryMove = await readInventoryItem(client, "beforeInventoryMove", "Wooden Sword");
    if (!beforeInventoryMove.item) {
      throw new Error(`Cannot verify inventory move without Wooden Sword: ${JSON.stringify(beforeInventoryMove)}`);
    }
    inventoryMoveFlow.push(beforeInventoryMove);
    await contextMenuInventoryItemByName(client, "Wooden Sword");
    await clickInventorySlot(client, 10);
    const afterInventoryMove = await waitForInventoryItemSlot(client, "Wooden Sword", 10, "afterInventoryMove", 5_000);
    inventoryMoveFlow.push(afterInventoryMove);
    screenshots.push(await screenshot(client, "stage5-inventory-move-wooden-sword.png"));

    const beforeInventorySplit = await readItemDistribution(client, "beforeInventorySplit", "Red Potion");
    if (beforeInventorySplit.inventoryItems.length !== 1 || beforeInventorySplit.inventoryQuantity < 2) {
      throw new Error(`Cannot verify inventory split with Red Potion state: ${JSON.stringify(beforeInventorySplit)}`);
    }
    inventorySplitFlow.push(beforeInventorySplit);
    await contextMenuInventoryItemByName(client, "Red Potion");
    await waitForInventorySplitState(client, (state) => state.splitPanelOpen === true, "split panel", 5_000);
    inventorySplitFlow.push(await readInventorySplitState(client, "splitPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-split-red-potion-panel.png"));
    await clickInventoryPanelAction(client, 0);
    const afterInventorySplit = await waitForItemDistribution(
      client,
      "Red Potion",
      (state) =>
        state.inventoryItems.length === 1 &&
        state.beltItems.some((item) => item.quantity === 1) &&
        state.totalQuantity === beforeInventorySplit.totalQuantity &&
        state.inventoryQuantity === beforeInventorySplit.inventoryQuantity - 1,
      "afterInventorySplit",
      5_000,
    );
    inventorySplitFlow.push(afterInventorySplit);
    screenshots.push(await screenshot(client, "stage5-inventory-split-red-potion.png"));

    const beforeInventoryDrop = await readInventoryItem(client, "beforeInventoryDrop", "Blue Potion");
    if (!beforeInventoryDrop.item) {
      throw new Error(`Cannot verify inventory drop without Blue Potion: ${JSON.stringify(beforeInventoryDrop)}`);
    }
    inventoryDropFlow.push(beforeInventoryDrop);
    await clickSelector(client, ".inventory-delete button");
    await clickInventoryItemByName(client, "Blue Potion");
    await waitForInventoryDropState(client, (state) => state.dropItemPanelOpen === true, "drop item panel", 5_000);
    inventoryDropFlow.push(await readInventoryDropState(client, "dropItemPanel"));
    screenshots.push(await screenshot(client, "stage5-inventory-drop-blue-potion-panel.png"));
    await clickInventoryPanelAction(client, 0);
    const afterInventoryDrop = await waitForInventoryItemQuantityBelow(
      client,
      "Blue Potion",
      beforeInventoryDrop.item.quantity,
      "afterInventoryDrop",
      5_000,
    );
    const afterInventoryDropState = await readInventoryDropState(client, "afterInventoryDrop");
    if (!afterInventoryDropState.groundDropLabels.some((label) => label.includes("Blue Potion"))) {
      throw new Error(`Blue Potion ground drop label was not visible: ${JSON.stringify(afterInventoryDropState)}`);
    }
    inventoryDropFlow.push({ ...afterInventoryDrop, dropState: afterInventoryDropState });
    screenshots.push(await screenshot(client, "stage5-inventory-drop-blue-potion.png"));

    const beforeBluePotionPickup = await readGroundDropState(client, "beforeBluePotionPickup", "Blue Potion");
    const bluePotionDrop = beforeBluePotionPickup.groundDrops.find((drop) => drop.name === "Blue Potion");
    if (!bluePotionDrop) {
      throw new Error(`Cannot verify ground pickup without Blue Potion drop: ${JSON.stringify(beforeBluePotionPickup)}`);
    }
    groundPickupFlow.push(beforeBluePotionPickup);
    await sendGatewayCommand(client, { type: "moveTo", x: bluePotionDrop.x, y: bluePotionDrop.y, mode: "walk" });
    await waitForStage5State(
      client,
      (state) => state?.player?.x === bluePotionDrop.x && state?.player?.y === bluePotionDrop.y,
      "move to Blue Potion drop",
      8_000,
    );
    groundPickupFlow.push(await readGroundDropState(client, "atBluePotionDrop", "Blue Potion"));
    await clickGroundDropByName(client, "Blue Potion");
    const afterBluePotionPickup = await waitForGroundDropState(
      client,
      "Blue Potion",
      (state) =>
        !state.groundDrops.some((drop) => drop.name === "Blue Potion") &&
        itemQuantity(state.inventoryItems, "Blue Potion") > (afterInventoryDrop.item?.quantity ?? 0),
      "afterBluePotionPickup",
      5_000,
    );
    groundPickupFlow.push(afterBluePotionPickup);
    screenshots.push(await screenshot(client, "stage5-ground-pickup-blue-potion.png"));

    const beforeGoldPickup = await readGroundDropState(client, "beforeGoldPickup", "100 Gold");
    const goldDrop = beforeGoldPickup.groundDrops.find((drop) => drop.name === "100 Gold");
    if (!goldDrop) {
      throw new Error(`Cannot verify ground gold pickup without 100 Gold drop: ${JSON.stringify(beforeGoldPickup)}`);
    }
    groundGoldPickupFlow.push(beforeGoldPickup);
    await sendGatewayCommand(client, { type: "moveTo", x: goldDrop.x, y: goldDrop.y, mode: "walk" });
    await waitForStage5State(
      client,
      (state) => state?.player?.x === goldDrop.x && state?.player?.y === goldDrop.y,
      "move to 100 Gold drop",
      8_000,
    );
    groundGoldPickupFlow.push(await readGroundDropState(client, "atGoldDrop", "100 Gold"));
    await clickGroundDropByName(client, "100 Gold");
    const afterGoldPickup = await waitForGroundDropState(
      client,
      "100 Gold",
      (state) =>
        !state.groundDrops.some((drop) => drop.name === "100 Gold") &&
        state.gold > beforeGoldPickup.gold,
      "afterGoldPickup",
      5_000,
    );
    groundGoldPickupFlow.push(afterGoldPickup);
    screenshots.push(await screenshot(client, "stage5-ground-pickup-gold.png"));

    const beforeBeltMouseUse = await readBeltItem(client, "beforeBeltMouseUse", "Red Potion");
    if (!beforeBeltMouseUse.item) {
      throw new Error(`Cannot verify belt mouse use without Red Potion: ${JSON.stringify(beforeBeltMouseUse)}`);
    }
    beltMouseUseFlow.push(beforeBeltMouseUse);
    await clickBeltItemByName(client, "Red Potion");
    const afterBeltMouseUse = await waitForBeltItemQuantityBelow(
      client,
      "Red Potion",
      beforeBeltMouseUse.item.quantity,
      "afterBeltMouseUse",
      5_000,
    );
    beltMouseUseFlow.push(afterBeltMouseUse);
    screenshots.push(await screenshot(client, "stage5-belt-mouse-use-red-potion.png"));

    await clickSelector(client, ".inventory-tab.tab-two button");
    await waitForInventoryState(client, (state) => state.activeTab === "bag2", "bag2", 5_000);
    inventoryFlow.push(await readInventoryState(client, "bag2"));
    screenshots.push(await screenshot(client, "stage5-inventory-bag2.png"));

    await clickSelector(client, ".inventory-tab.tab-three button");
    await waitForInventoryState(
      client,
      (state) => state.activeTab === "quest" && state.storageWindowVisible === true,
      "quest",
      5_000,
    );
    inventoryFlow.push(await readInventoryState(client, "quest"));
    storageFlow.push(await readStorageState(client, "page1"));
    screenshots.push(await screenshot(client, "stage5-inventory-quest.png"));

    await clickSelector(client, ".storage-page-tab.page-2");
    await waitForStorageState(client, (state) => state.activePage === "2" && state.pageLocked === true, "page2 locked", 5_000);
    storageFlow.push(await readStorageState(client, "page2Locked"));
    screenshots.push(await screenshot(client, "stage5-storage-page2-locked.png"));

    await clickSelector(client, ".storage-action-button.rent");
    await waitForStorageState(
      client,
      (state) => state.activePage === "2" && state.pageLocked === false && state.hasExpandedStorage === true,
      "page2 rented",
      5_000,
    );
    storageFlow.push(await readStorageState(client, "page2Rented"));
    screenshots.push(await screenshot(client, "stage5-storage-page2-rented.png"));

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
    await waitForStoragePasswordState(
      client,
      (state) =>
        state.panelVisible === true &&
        state.hasStoragePassword === false &&
        state.chatLines.some((line) => line.includes("Storage password service is not available.")),
      "storage password no-service submit",
      5_000,
    );
    storagePasswordFlow.push(await readStoragePasswordState(client, "storagePasswordSubmitNoService"));
    screenshots.push(await screenshot(client, "stage5-storage-password-submit-no-service.png"));

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

    const beforeCharacterRepair = await readCharacterRepairState(client, "beforeCharacterRepair", "Dagger");
    if (!beforeCharacterRepair.equipmentItems.some((item) => item.name === "Dagger")) {
      throw new Error(`Cannot verify repair mode without equipped Dagger: ${JSON.stringify(beforeCharacterRepair)}`);
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
      (state) => !state.activeRepairLabel && state.equipmentItems.some((item) => item.name === "Dagger"),
      "normal repair submitted",
      5_000,
    );
    characterRepairFlow.push(await readCharacterRepairState(client, "normalRepairSubmitted", "Dagger"));

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
      (state) => !state.activeRepairLabel && state.equipmentItems.some((item) => item.name === "Dagger"),
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
    const afterCharacterRemove = await waitForEquipmentItemAbsent(client, "Dagger", "afterCharacterRemove", 5_000);
    if (!afterCharacterRemove.inventoryItem) {
      throw new Error(`Dagger was removed from equipment but not found in inventory: ${JSON.stringify(afterCharacterRemove)}`);
    }
    characterRemoveFlow.push(afterCharacterRemove);
    screenshots.push(await screenshot(client, "stage5-character-remove-dagger.png"));

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

    const beforeStorageStore = {
      item: await readInventoryItem(client, "beforeStorageStore", "Dagger"),
      storage: await readStorageTransferState(client, "beforeStorageStore"),
    };
    if (!beforeStorageStore.item.item) {
      throw new Error(`Cannot verify storage store without Dagger: ${JSON.stringify(beforeStorageStore)}`);
    }
    storageStoreFlow.push(beforeStorageStore);
    await clickButtonByImageAlt(client, "Store Item");
    await waitForStorageTransferState(client, (state) => state.storageWindowVisible === true, "store mode open", 5_000);
    await clickInventoryItemByName(client, "Dagger");
    await waitForStorageTransferState(
      client,
      (state) => state.feedbackText.includes("Dagger") && state.hintTexts.some((hint) => hint.includes("select target slot")),
      "store item selected",
      5_000,
    );
    storageStoreFlow.push(await readStorageTransferState(client, "storeItemSelected"));
    screenshots.push(await screenshot(client, "stage5-storage-store-dagger-selected.png"));
    await clickStorageSlot(client, 0);
    const beforeStorageDaggerCount = beforeStorageStore.storage.storageItems.filter((item) => item.name === "Dagger").length;
    const afterStorageStoreState = await waitForStorageTransferState(
      client,
      (state) => {
        const activeInventoryDagger = state.inventoryItems.some((item) => item.name === "Dagger");
        const storageDaggerCount = state.storageItems.filter((item) => item.name === "Dagger").length;
        return (
          state.feedbackText.includes("Dagger") &&
          state.feedbackText.includes("1") &&
          ((!activeInventoryDagger && storageDaggerCount > beforeStorageDaggerCount) ||
            (activeInventoryDagger && storageDaggerCount === beforeStorageDaggerCount))
        );
      },
      "afterStorageStore",
      8_000,
    );
    const afterStorageDaggerCount = afterStorageStoreState.storageItems.filter((item) => item.name === "Dagger").length;
    const storageStoreResult =
      afterStorageDaggerCount > beforeStorageDaggerCount ? "stored" : "preserved-without-service";
    const afterStorageStore = {
      item: await readInventoryItem(client, "afterStorageStore", "Dagger"),
      storage: afterStorageStoreState,
      result: storageStoreResult,
    };
    if (
      (storageStoreResult === "stored" && afterStorageStore.item.item) ||
      (storageStoreResult === "preserved-without-service" &&
        (!afterStorageStore.item.item || afterStorageStore.item.item.slot !== beforeStorageStore.item.item.slot))
    ) {
      throw new Error(`Store item result was inconsistent: ${JSON.stringify(afterStorageStore)}`);
    }
    storageStoreFlow.push(afterStorageStore);
    screenshots.push(await screenshot(client, "stage5-storage-store-dagger-result.png"));
    await clickSelector(client, ".storage-close button");
    await waitForStorageTransferState(
      client,
      (state) => state.storageWindowVisible === false && state.feedbackText.includes("Dagger"),
      "storageStoreFeedback",
      5_000,
    );
    storageStoreFlow.push(await readStorageTransferState(client, "storageStoreFeedback"));
    screenshots.push(await screenshot(client, "stage5-storage-store-dagger-feedback.png"));

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
      await clickButtonByImageAlt(client, "Store Item");
    }
    await waitForSelector(client, ".storage-window", 10_000);
    screenshots.push(await screenshot(client, "stage5-storage.png"));

    const beforeStorageTakeBack = await readStorageTransferState(client, "beforeStorageTakeBack");
    const beforeTakeBackInventoryQuantity = itemQuantity(beforeStorageTakeBack.inventoryItems, "Red Potion");
    const beforeTakeBackStorageQuantity = itemQuantity(beforeStorageTakeBack.storageItems, "Red Potion");
    if (beforeTakeBackStorageQuantity < 1) {
      throw new Error(`Cannot verify Take Back without stored Red Potion: ${JSON.stringify(beforeStorageTakeBack)}`);
    }
    storageTakeBackFlow.push(beforeStorageTakeBack);
    await clickButtonByImageAlt(client, "Take Back");
    await clickStorageItemByName(client, "Red Potion");
    await waitForStorageTransferState(
      client,
      (state) => state.feedbackText.includes("Red Potion") && state.hintTexts.some((hint) => hint.includes("select target slot")),
      "takeBackItemSelected",
      5_000,
    );
    storageTakeBackFlow.push(await readStorageTransferState(client, "takeBackItemSelected"));
    screenshots.push(await screenshot(client, "stage5-storage-takeback-red-potion-selected.png"));
    await clickInventorySlot(client, 6);
    const afterStorageTakeBackState = await waitForStorageTransferState(
      client,
      (state) => {
        const inventoryQuantity = itemQuantity(state.inventoryItems, "Red Potion");
        const storageQuantity = itemQuantity(state.storageItems, "Red Potion");
        return (
          state.feedbackText.includes("Red Potion") &&
          ((inventoryQuantity === beforeTakeBackInventoryQuantity && storageQuantity === beforeTakeBackStorageQuantity) ||
            (inventoryQuantity > beforeTakeBackInventoryQuantity && storageQuantity < beforeTakeBackStorageQuantity))
        );
      },
      "afterStorageTakeBack",
      8_000,
    );
    const afterTakeBackInventoryQuantity = itemQuantity(afterStorageTakeBackState.inventoryItems, "Red Potion");
    const afterTakeBackStorageQuantity = itemQuantity(afterStorageTakeBackState.storageItems, "Red Potion");
    storageTakeBackFlow.push({
      ...afterStorageTakeBackState,
      result:
        afterTakeBackInventoryQuantity > beforeTakeBackInventoryQuantity &&
        afterTakeBackStorageQuantity < beforeTakeBackStorageQuantity
          ? "taken-back"
          : "preserved-without-service",
    });
    screenshots.push(await screenshot(client, "stage5-storage-takeback-red-potion-result.png"));
    await clickSelector(client, ".storage-close button");
    await waitForStorageTransferState(
      client,
      (state) => state.storageWindowVisible === false && state.feedbackText.includes("Red Potion"),
      "storageTakeBackFeedback",
      5_000,
    );
    storageTakeBackFlow.push(await readStorageTransferState(client, "storageTakeBackFeedback"));
    screenshots.push(await screenshot(client, "stage5-storage-takeback-red-potion-feedback.png"));

    await waitForSelector(client, ".entity-nameplate.npc", 10_000);
    await clickFirst(client, ".entity-nameplate.npc");
    await waitForSelector(client, ".npc-dialog-panel", 10_000);
    npcDialogFlow.push(await readNpcDialogState(client, "open"));
    screenshots.push(await screenshot(client, "stage5-npc.png"));
    if ((await readNpcDialogState(client, "linkOptional")).links.length > 0) {
      screenshots.push(await screenshot(client, "stage5-npc-links.png"));
    }
    await clickOptional(client, ".npc-dialog-close");
    await waitForNpcDialogState(client, (state) => state.open === false, "closed", 5_000);
    npcDialogFlow.push(await readNpcDialogState(client, "closed"));

    await waitForSelector(client, ".entity-nameplate.monster", 10_000);
    const beforeCombat = await readCombatState(client, "beforeCombat");
    const combatTarget = beforeCombat.monsters[0] ?? null;
    if (!combatTarget) {
      throw new Error(`Cannot verify combat without visible monster: ${JSON.stringify(beforeCombat)}`);
    }
    combatFlow.push(beforeCombat);
    await clickFirst(client, ".entity-nameplate.monster");
    const afterCombat = await waitForCombatState(
      client,
      (state) => {
        const selectedMonster = state.monsters.find((monster) => monster.objectId === state.selectedObjectId);
        const damagedMonster = state.monsters.find((monster) => {
          const beforeMonster = beforeCombat.monsters.find((entry) => entry.objectId === monster.objectId);
          return (
            typeof monster.hp === "number" &&
            typeof beforeMonster?.hp === "number" &&
            monster.hp < beforeMonster.hp
          );
        });
        return (
          Boolean(selectedMonster) ||
          state.monsters.some((monster) => monster.struckActive) ||
          Boolean(damagedMonster)
        );
      },
      "afterCombat",
      2_000,
    );
    combatFlow.push(afterCombat);
    screenshots.push(await screenshot(client, "stage5-combat.png"));

    await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button");
    await delay(300);
    await sendGatewayCommand(client, { type: "transferMap", key: "crystal:1:315:82" });
    await waitForStage5State(client, (state) => state?.mapFileName === "1", "mapFileName 1", 15_000);
    screenshots.push(await screenshot(client, "stage5-map-transfer-1.png"));
    minimapFlow.push(await readMiniMapState(client, "expanded"));

    await clickSelector(client, ".mini-map-button.toggle button");
    await waitForMiniMapState(client, (state) => state.sceneHidden === true, "collapsed", 5_000);
    minimapFlow.push(await readMiniMapState(client, "collapsed"));
    screenshots.push(await screenshot(client, "stage5-minimap-collapsed.png"));

    await clickSelector(client, ".mini-map-button.bigmap button");
    await waitForMiniMapState(client, (state) => state.sceneHidden === false, "expanded by big map button", 5_000);
    minimapFlow.push(await readMiniMapState(client, "expandedAfterBigMap"));
    screenshots.push(await screenshot(client, "stage5-minimap-expanded.png"));

    await clickSelector(client, ".mini-map-button.mail button");
    await waitForSelector(client, ".mail-panel", 5_000);
    minimapFlow.push(await readMiniMapState(client, "mailOpen"));
    mailFlow.push(await readMailState(client, "mailOpen"));
    screenshots.push(await screenshot(client, "stage5-minimap-mail.png"));
    await clickOptional(client, ".mail-panel .overlay-panel-head button");
    await waitForMiniMapState(client, (state) => state.mailOpen === false, "mail closed", 5_000);
    mailFlow.push(await readMailState(client, "mailClosed"));

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
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.buy", args: ["1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.list", args: ["auction-cancel-ui", "45"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "auction.cancel", args: ["2"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "conquest.end", args: ["Sabuk"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "event.spawn", args: ["Field Wasp", "1"] });
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
          systems?.auction?.some((listing) => listing.itemKey === "auction-ui-relic" && listing.sold === true) &&
          systems?.auction?.some((listing) => listing.itemKey === "auction-cancel-ui" && listing.cancelled === true) &&
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
    await clickSelector(client, '.chat-filter-button[style*="34px"] button');
    await waitForChatState(client, (state) => state.visibleLineTexts.every((text) => text === ""), "shout filter", 5_000);
    chatFlow.push(await readChatState(client, "shoutFilter"));
    screenshots.push(await screenshot(client, "stage5-chat-shout-filter.png"));

    await clickSelector(client, '.chat-filter-button[style*="12px"] button');
    await waitForChatState(client, (state) => state.visibleLineTexts.some((text) => text !== ""), "all filter restored", 5_000);
    chatFlow.push(await readChatState(client, "allRestored"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "guild.chat", args: ["Guild", "filter", "check"] });
    await waitForStage5State(
      client,
      (state) => state?.stage5Systems?.guild?.chatLog?.some((line) => line.includes("Guild filter check")),
      "guild chat filter check",
      5_000,
    );
    await clickSelector(client, '.chat-filter-button[style*="144px"] button');
    await waitForChatState(
      client,
      (state) =>
        state.visibleLineTexts.some((text) => text.includes("Guild filter check")) &&
        state.visibleLineClasses.every((className) => className.includes("channel-guild")),
      "guild filter",
      5_000,
    );
    chatChannelFlow.push(await readChatState(client, "guildFilter"));
    screenshots.push(await screenshot(client, "stage5-chat-guild-filter.png"));

    await clickSelector(client, '.chat-filter-button[style*="122px"] button');
    await waitForChatState(client, (state) => state.visibleLineTexts.every((text) => text === ""), "group filter empty", 5_000);
    chatChannelFlow.push(await readChatState(client, "groupFilterEmpty"));
    screenshots.push(await screenshot(client, "stage5-chat-group-filter-empty.png"));

    await clickSelector(client, '.chat-filter-button[style*="12px"] button');
    await waitForChatState(client, (state) => state.visibleLineTexts.some((text) => text !== ""), "all restored after channels", 5_000);
    chatChannelFlow.push(await readChatState(client, "allRestoredAfterChannels"));

    await clickSelector(client, ".chat-filter-button.settings button");
    await waitForChatState(client, (state) => state.settingsOpen === true, "settings open", 5_000);
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

    await clickSystemMenuAction(client, 0);
    await waitForSelector(client, ".character-window", 5_000);
    systemMenuFlow.push(await readSystemMenuState(client, "characterAction"));
    screenshots.push(await screenshot(client, "stage5-system-menu-character.png"));
    await clickOptional(client, ".character-close button");

    await clickSelector(client, ".hud-button.menu button");
    await waitForSystemMenuState(client, (state) => state.open === true, "reopen inventory", 5_000);
    await clickSystemMenuAction(client, 1);
    await waitForInventoryState(client, (state) => state.activeTab === "bag1", "system menu inventory", 5_000);
    systemMenuFlow.push(await readSystemMenuState(client, "inventoryAction"));
    screenshots.push(await screenshot(client, "stage5-system-menu-inventory.png"));
    await clickOptional(client, ".inventory-close button");

    await clickSelector(client, ".hud-button.menu button");
    await waitForSystemMenuState(client, (state) => state.open === true, "reopen quest", 5_000);
    await clickSystemMenuAction(client, 2);
    await waitForInventoryState(
      client,
      (state) => state.activeTab === "quest" && state.storageWindowVisible === true,
      "system menu quest",
      5_000,
    );
    systemMenuFlow.push(await readSystemMenuState(client, "questAction"));
    screenshots.push(await screenshot(client, "stage5-system-menu-quest.png"));
    await clickAllOptional(client, ".storage-close button, .inventory-close button");
    await clickOptional(client, ".inventory-close button");
    await waitUntil(
      async () =>
        !Boolean(
          await client.evaluate('Boolean(document.querySelector(".inventory-window, .storage-window"))'),
        ),
      5_000,
      "inventory and storage closed",
    );

    await clickSelector(client, ".hud-button.menu button");
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
    await clickOptional(client, ".system-menu-panel .overlay-panel-head button");

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
    await clickOptional(client, ".mail-panel .overlay-panel-head button");

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

    await setViewport(client, VIEWPORTS.desktop);
    await delay(300);
    await waitForBeltState(
      client,
      (state) => state.orientation === "horizontal" && state.labelsWithinDialog === true,
      "horizontal with labels in dialog",
      5_000,
    );
    beltFlow.push(await readBeltState(client, "horizontal"));
    const beforeHotkeyUse = await readStage5BeltItems(client, "beforeHotkey1");
    const beforeSlotOne = beforeHotkeyUse.items.find((item) => item.slot === 0);
    if (!beforeSlotOne) {
      throw new Error(`Cannot verify belt hotkey use without an item in slot 1: ${JSON.stringify(beforeHotkeyUse)}`);
    }
    beltUseFlow.push(beforeHotkeyUse);
    await pressKey(client, "1", "Digit1", 49);
    const afterHotkeyUse = await waitForStage5BeltItemQuantityBelow(
      client,
      0,
      beforeSlotOne.quantity,
      "afterHotkey1",
      5_000,
    );
    beltUseFlow.push(afterHotkeyUse);
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
      criticalConsoleErrorCount: client.consoleErrors.length,
      flowCounts: {
        inventory: inventoryFlow.length,
        storage: storageFlow.length,
        storagePassword: storagePasswordFlow.length,
        chat: chatFlow.length,
        systemMenu: systemMenuFlow.length,
        stage5Systems: stage5SystemsFlow.length,
        login: loginFlow.length,
        select: selectFlow.length,
        belt: beltFlow.length,
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
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        inventoryWindowVisible: Boolean(document.querySelector(".inventory-window")),
        storageWindowVisible: Boolean(document.querySelector(".storage-window")),
        visibleItemCards: document.querySelectorAll(".inventory-item-card").length,
        questEntryCount: document.querySelectorAll(".inventory-quest-entry").length,
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
              slot: item.slot,
              container: item.container,
              quantity: item.quantity,
            }
          : null,
      };
    })()
  `);
}

async function readItemDistribution(client, label, itemName) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      const toItem = (item) => ({
        key: item.key,
        name: item.name,
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

async function clickInventoryItemByName(client, itemName) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".inventory-item-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click inventory item ${itemName}`);
}

async function contextMenuInventoryItemByName(client, itemName) {
  const opened = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".inventory-item-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      );
      if (!button) return false;
      button.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2 }));
      return true;
    })()
  `);
  if (!opened) throw new Error(`Could not context-menu inventory item ${itemName}`);
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
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".storage-item-card")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click storage item ${itemName}`);
}

function itemQuantity(items, itemName) {
  return items.filter((item) => item.name === itemName).reduce((total, item) => total + (item.quantity ?? 0), 0);
}

async function waitForInventoryItemQuantityBelow(client, itemName, previousQuantity, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryItem(client, label, itemName);
    if (!state.item || state.item.quantity < previousQuantity) return state;
    await delay(100);
  }
  throw new Error(
    `Timed out waiting for inventory ${itemName} quantity below ${previousQuantity}; current state: ${JSON.stringify(state)}`,
  );
}

async function waitForInventoryItemSlot(client, itemName, slot, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryItem(client, label, itemName);
    if (state.item?.slot === slot) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory ${itemName} slot ${slot}; current state: ${JSON.stringify(state)}`);
}

async function waitForItemDistribution(client, itemName, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readItemDistribution(client, label, itemName);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory ${itemName} split state; current state: ${JSON.stringify(state)}`);
}

async function readInventoryEquipmentState(client, label, itemName) {
  return client.evaluate(`
    (() => {
      const activeTab = window.__mir2Stage5?.state?.activeInventoryTab ?? null;
      const inventoryItem = (window.__mir2Stage5?.state?.inventoryItems ?? []).find(
        (entry) => entry.container === activeTab && entry.name === ${JSON.stringify(itemName)}
      );
      const equipmentItems = (window.__mir2Stage5?.state?.equipmentItems ?? []).map((item) => ({
        name: item.name,
        slot: item.slot,
        durabilityCurrent: item.durabilityCurrent,
        durabilityMax: item.durabilityMax,
      }));
      return {
        label: ${JSON.stringify(label)},
        activeTab,
        inventoryItem: inventoryItem
          ? {
              key: inventoryItem.key,
              name: inventoryItem.name,
              slot: inventoryItem.slot,
              container: inventoryItem.container,
              quantity: inventoryItem.quantity,
            }
          : null,
        equipmentItems,
      };
    })()
  `);
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

async function waitForEquipmentItem(client, itemName, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryEquipmentState(client, label, itemName);
    if (state.equipmentItems.some((item) => item.name === itemName)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for equipped ${itemName}; current state: ${JSON.stringify(state)}`);
}

async function waitForEquipmentItemAbsent(client, itemName, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryEquipmentState(client, label, itemName);
    if (!state.equipmentItems.some((item) => item.name === itemName)) return state;
    await delay(100);
  }
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

async function waitForInventoryGoldState(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readInventoryGoldState(client, label);
    if (predicate(state)) return state;
    await delay(100);
  }
  throw new Error(`Timed out waiting for inventory gold ${label}; current state: ${JSON.stringify(state)}`);
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
        inventoryItems: (window.__mir2Stage5?.state?.inventoryItems ?? [])
          .filter((item) => item.container === activeTab)
          .map((item) => ({
            key: item.key,
            name: item.name,
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

async function clickGroundDropByName(client, itemName) {
  const clicked = await client.evaluate(`
    (() => {
      const marker = Array.from(document.querySelectorAll(".ground-drop-marker")).find(
        (node) => (node.getAttribute("title") ?? "").includes(${JSON.stringify(itemName)})
      );
      if (!marker) return false;
      marker.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click ground drop ${itemName}`);
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
        .filter((item) => item.container === (window.__mir2Stage5?.state?.activeInventoryTab ?? null))
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

async function clickInventoryPanelAction(client, index) {
  const clicked = await client.evaluate(`
    (() => {
      const button = document.querySelectorAll(".inventory-delete-panel .inventory-delete-actions button")[${Number(index)}];
      if (!button) return false;
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

async function readChatState(client, label) {
  return client.evaluate(`
    (() => {
      const frame = document.querySelector(".chat-frame");
      const feed = document.querySelector(".chat-feed");
      const knob = document.querySelector(".chat-position-knob");
      const rect = frame?.getBoundingClientRect();
      const lines = Array.from(document.querySelectorAll(".chat-feed-line"));
      return {
        label: ${JSON.stringify(label)},
        frameVisible: Boolean(frame),
        collapsed: frame?.classList.contains("collapsed") ?? null,
        feedHidden: feed?.classList.contains("hidden") ?? null,
        settingsOpen: Boolean(document.querySelector(".chat-settings-panel")),
        reportOpen: Boolean(document.querySelector(".report-panel")),
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

async function readMailState(client, label) {
  return client.evaluate(`
    (() => {
      const panel = document.querySelector(".mail-panel");
      const rect = panel?.getBoundingClientRect();
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        title: document.querySelector(".mail-panel .overlay-panel-head strong")?.textContent?.trim() ?? "",
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
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        caption: document.querySelector(".npc-dialog-caption")?.textContent?.trim() ?? "",
        title: document.querySelector(".npc-dialog-head strong")?.textContent?.trim() ?? "",
        bodyLines: Array.from(document.querySelectorAll(".npc-dialog-body p")).map(
          (line) => line.textContent?.trim() ?? "",
        ),
        links: Array.from(document.querySelectorAll(".npc-dialog-links button")).map((button) => ({
          text: button.textContent?.trim() ?? "",
          target: button.getAttribute("data-target") ?? "",
        })),
        footer: document.querySelector(".npc-dialog-footer")?.textContent?.trim() ?? "",
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
      const qaInputs = Array.from(document.querySelectorAll(".system-menu-qa-transfer input")).map((input) => ({
        label: input.closest("label")?.querySelector("span")?.textContent?.trim() ?? "",
        value: input.value ?? "",
      }));
      return {
        label: ${JSON.stringify(label)},
        open: Boolean(panel),
        characterWindowVisible: Boolean(document.querySelector(".character-window")),
        inventoryWindowVisible: Boolean(document.querySelector(".inventory-window")),
        storageWindowVisible: Boolean(document.querySelector(".storage-window")),
        activeInventoryTab: window.__mir2Stage5?.state?.activeInventoryTab ?? null,
        actionLabels: Array.from(document.querySelectorAll(".system-menu-actions button")).map((button) =>
          button.textContent?.trim() ?? ""
        ),
        transferLabels: Array.from(document.querySelectorAll(".system-menu-transfer-list button")).map((button) =>
          button.textContent?.trim() ?? ""
        ),
        qaTransferTitle: document.querySelector(".system-menu-qa-transfer .system-menu-transfer-title")?.textContent?.trim() ?? "",
        qaInputs,
        currentMapFileName: window.__mir2Stage5?.state?.mapFileName ?? null,
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
  throw new Error(`Timed out waiting for system menu ${label}; current state: ${JSON.stringify(state)}`);
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
      const name = document.querySelector(".mini-map-name");
      const coords = document.querySelector(".mini-map-coords");
      const rect = panel?.getBoundingClientRect();
      return {
        label: ${JSON.stringify(label)},
        panelVisible: Boolean(panel),
        sceneHidden: scene?.classList.contains("hidden") ?? null,
        mailOpen: Boolean(mail),
        nameText: name?.textContent?.trim() ?? "",
        coordsText: coords?.textContent?.trim() ?? "",
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

async function waitForBeltItemQuantityBelow(client, itemName, previousQuantity, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await readBeltItem(client, label, itemName);
    if (!state.item || state.item.quantity < previousQuantity) return state;
    await delay(100);
  }
  throw new Error(
    `Timed out waiting for belt ${itemName} quantity below ${previousQuantity}; current state: ${JSON.stringify(state)}`,
  );
}

async function clickBeltItemByName(client, itemName) {
  const clicked = await client.evaluate(`
    (() => {
      const button = Array.from(document.querySelectorAll(".belt-item")).find(
        (node) => node.getAttribute("title") === ${JSON.stringify(itemName)}
      );
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click belt item ${itemName}`);
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

async function waitForStage5State(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await client.evaluate("window.__mir2Stage5?.state ?? null");
    if (predicate(state)) return;
    await delay(200);
  }
  throw new Error(`Timed out waiting for Stage 5 state ${label}; current state: ${JSON.stringify(state)}`);
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
      const button = image?.closest("button");
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
