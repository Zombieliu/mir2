#!/usr/bin/env node

const wsUrl = process.env.MIR2_WORLD_DIRECTOR_PLAYER_WS_URL ?? "ws://127.0.0.1:27110/ws";
const operatorUrl =
  process.env.MIR2_WORLD_DIRECTOR_OPERATOR_URL ?? "http://127.0.0.1:29201";
const managementToken = process.env.MIR2_WORLD_DIRECTOR_MANAGEMENT_TOKEN;
const qaControlToken = process.env.MIR2_WORLD_DIRECTOR_QA_CONTROL_TOKEN;
const timeoutMs = Number(process.env.MIR2_WORLD_DIRECTOR_PLAYER_TIMEOUT_MS ?? 20_000);

if (!managementToken) {
  throw new Error("MIR2_WORLD_DIRECTOR_MANAGEMENT_TOKEN is required");
}

const accountId = `director-player-${process.pid}-${Date.now()}`;
const password = "director-acceptance-pass";
const characterName = `Dir${String(Date.now()).slice(-9)}`;

class PlayerClient {
  constructor(label) {
    this.label = label;
    this.messages = [];
    this.errors = [];
    this.characters = [];
    this.characterIndex = null;
    this.loginSuccess = false;
    this.inGame = false;
    this.mapFileName = null;
    this.playerPosition = null;
    this.mapInformationCount = 0;
    this.visibleEventMonsters = new Map();
    this.combatPackets = [];
  }

  async connect() {
    this.socket = new WebSocket(wsUrl);
    this.socket.addEventListener("message", (event) => this.onMessage(String(event.data)));
    await waitForEvent(this.socket, "open", `${this.label} WebSocket open`);
  }

  onMessage(raw) {
    let message;
    try {
      message = JSON.parse(raw);
    } catch {
      return;
    }
    this.messages.push(message);
    if (message.type === "error") {
      this.errors.push(String(message.message ?? "unknown Gateway error"));
    }
    if (message.packet === "LoginSuccess") {
      this.loginSuccess = true;
      this.characters = message.payload?.characters ?? [];
      const first = this.characters[0]?.index;
      if (Number.isInteger(first)) this.characterIndex = first;
    }
    if (message.packet === "NewCharacterSuccess") {
      this.characterIndex = message.payload?.character?.index ?? 0;
    }
    if (message.packet === "UserInformation") this.inGame = true;
    if (message.packet === "MapInformation") {
      this.mapInformationCount += 1;
      this.mapFileName =
        message.payload?.fileName ?? message.payload?.mapFileName ?? this.mapFileName;
    }
    if (message.packet === "UserLocation") {
      this.playerPosition = {
        x: Number(message.payload?.x),
        y: Number(message.payload?.y),
      };
    }
    if (message.type === "worldSnapshot") {
      this.mapFileName = message.payload?.mapFileName ?? this.mapFileName;
      if (message.payload?.player) {
        this.playerPosition = {
          x: Number(message.payload.player.x),
          y: Number(message.payload.player.y),
        };
      }
    }
    if (message.packet === "ObjectMonster") {
      this.visibleEventMonsters.set(Number(message.payload?.objectId), message.payload);
    }
    if (
      ["ObjectAttack", "ObjectStruck", "ObjectHealth", "ObjectDied"].includes(message.packet)
    ) {
      this.combatPackets.push(message);
    }
  }

  send(value) {
    this.socket.send(JSON.stringify(value));
  }

  async bootstrap(createAccount) {
    this.send({ type: "clientVersion" });
    if (createAccount) {
      this.send({
        type: "newAccount",
        accountId,
        password,
        birthDateBinary: 0,
        userName: accountId,
        secretQuestion: "",
        secretAnswer: "",
        emailAddress: "",
      });
    }
    this.send({ type: "login", accountId, password });
    await waitFor(() => this.loginSuccess, `${this.label} login`);
    if (this.characterIndex === null) {
      this.send({
        type: "newCharacter",
        name: characterName,
        gender: "Male",
        class: "Warrior",
      });
      await waitFor(() => this.characterIndex !== null, `${this.label} character creation`);
    }
    this.send({ type: "startGame", characterIndex: this.characterIndex });
    await waitFor(() => this.inGame, `${this.label} start game`);
  }

  async transferNear(monster) {
    for (let attempt = 0; attempt < 8; attempt += 1) {
      const liveMonster = await liveEventMonster(monster.objectId);
      const candidates = adjacentTransferCandidates(liveMonster.position);
      const [x, y] = candidates[attempt % candidates.length];
      const previousMapInformationCount = this.mapInformationCount;
      this.send({ type: "transferMap", key: `crystal:D022:${x}:${y}` });
      const reached = await waitFor(
        () =>
          this.mapFileName === "D022" &&
          this.mapInformationCount > previousMapInformationCount &&
          Number.isFinite(this.playerPosition?.x) &&
          Number.isFinite(this.playerPosition?.y),
        `${this.label} D022 transfer`,
        timeoutMs,
        false,
      );
      if (!reached) continue;

      const refreshedMonster = await liveEventMonster(monster.objectId);
      const direction = directionToward(this.playerPosition, refreshedMonster.position);
      if (direction && tileDistance(this.playerPosition, refreshedMonster.position) <= 1) {
        return {
          requested: { x, y },
          authoritative: { ...this.playerPosition },
          monster: { ...refreshedMonster.position },
          direction,
        };
      }
    }
    throw new Error(
      `${this.label} could not transfer adjacent to D022 monster: ${JSON.stringify(
        this.diagnosticSnapshot(),
      )}`,
    );
  }

  async prepareCombatState() {
    if (!qaControlToken) return;
    const state = {
      character: {
        name: characterName,
        level: 50,
        class: "Warrior",
        gender: "Male",
      },
      mapFileName: this.mapFileName ?? "0",
      mapTitle: "",
      position: this.playerPosition ?? { x: 330, y: 270 },
      direction: "Down",
      hp: 2_000,
      maxHp: 2_000,
      mp: 200,
      maxMp: 200,
      experience: 0,
      maxExperience: 1_000_000,
      gold: 0,
      credit: 0,
      inventoryItemsJson: [],
      beltItemsJson: [],
      storageItemsJson: [],
      equipmentItemsJson: [],
    };
    this.send({
      type: "qaControl",
      token: qaControlToken,
      action: {
        type: "stage5Command",
        action: "qa.applyNativeState",
        args: [JSON.stringify(state)],
      },
    });
    await delay(1_000);
  }

  close() {
    this.socket.close();
  }

  diagnosticSnapshot() {
    return {
      mapFileName: this.mapFileName,
      playerPosition: this.playerPosition,
      errors: this.errors,
      recentMessages: this.messages.slice(-12).map((message) => ({
        type: message.type ?? null,
        packet: message.packet ?? null,
        payload: message.payload ?? null,
        message: message.message ?? null,
      })),
    };
  }
}

async function main() {
  const initialStatus = await directorStatus();
  const initialMonsters = initialStatus.worldEventMonsters?.["map:D022"] ?? [];
  const target = initialMonsters.find((monster) => !monster.dead && monster.hp > 0);
  if (!target) throw new Error("director status has no live D022 world-event monster");

  const first = new PlayerClient("first session");
  await first.connect();
  await first.bootstrap(true);
  await first.prepareCombatState();
  const firstPosition = await first.transferNear(target);
  await waitFor(
    () => first.visibleEventMonsters.has(target.objectId),
    "first session ObjectMonster",
  );
  await delay(1_000);
  const combatStatus = await directorStatus();
  const combatTarget =
    combatStatus.worldEventMonsters?.["map:D022"]?.find(
      (monster) => monster.objectId === target.objectId && !monster.dead,
    ) ?? target;
  const combatPosition = await first.transferNear(combatTarget);
  first.send({ type: "turn", direction: combatPosition.direction });
  await delay(700);
  first.combatPackets.length = 0;
  for (let index = 0; index < 6; index += 1) {
    first.send({ type: "attack", objectId: target.objectId });
    await delay(700);
    first.send({ type: "tick" });
  }
  const attacked = await waitFor(
    () =>
      first.combatPackets.some(
        (message) =>
          ["ObjectStruck", "ObjectHealth", "ObjectDied"].includes(message.packet) &&
          Number(message.payload?.objectId) === target.objectId,
      ),
    "event monster combat result",
    timeoutMs,
    false,
  );
  if (!attacked) {
    throw new Error(
      `player saw the event monster but no combat packet referenced it: ${JSON.stringify({
        targetObjectId: target.objectId,
        combatPackets: first.combatPackets.map((message) => ({
          packet: message.packet,
          payload: message.payload ?? null,
        })),
        session: first.diagnosticSnapshot(),
      })}`,
    );
  }
  first.close();
  await delay(1_000);

  const refreshedStatus = await directorStatus();
  const refreshedTarget =
    refreshedStatus.worldEventMonsters?.["map:D022"]?.find(
      (monster) => monster.objectId === target.objectId && !monster.dead,
    ) ?? target;
  const second = new PlayerClient("reconnected session");
  await second.connect();
  await second.bootstrap(false);
  const secondPosition = await second.transferNear(refreshedTarget);
  await waitFor(
    () => second.visibleEventMonsters.has(target.objectId),
    "reconnected session ObjectMonster",
  );

  const evidence = {
    accepted: true,
    accountId,
    characterIndex: second.characterIndex,
    mapFileName: second.mapFileName,
    targetMonster: {
      objectId: target.objectId,
      name: target.name,
      initialHp: target.hp,
      refreshedHp: refreshedTarget.hp,
    },
    firstSession: {
      transferPosition: firstPosition,
      combatPosition,
      objectMonsterVisible: first.visibleEventMonsters.has(target.objectId),
      combatPackets: first.combatPackets.map((message) => ({
        packet: message.packet,
        objectId: message.payload?.objectId ?? null,
        attackerId: message.payload?.attackerId ?? null,
        percent: message.payload?.percent ?? null,
      })),
      errors: first.errors,
    },
    reconnectedSession: {
      transferPosition: secondPosition,
      objectMonsterVisible: second.visibleEventMonsters.has(target.objectId),
      errors: second.errors,
    },
  };
  second.close();
  process.stdout.write(`${JSON.stringify(evidence, null, 2)}\n`);
}

async function directorStatus() {
  const response = await fetch(`${operatorUrl}/v1/world-director`, {
    headers: { authorization: `Bearer ${managementToken}` },
  });
  if (!response.ok) {
    throw new Error(`world director status failed: HTTP ${response.status} ${await response.text()}`);
  }
  return response.json();
}

async function liveEventMonster(objectId) {
  const status = await directorStatus();
  const monster = status.worldEventMonsters?.["map:D022"]?.find(
    (candidate) => candidate.objectId === objectId && !candidate.dead && candidate.hp > 0,
  );
  if (!monster) throw new Error(`D022 world-event monster ${objectId} is no longer alive`);
  return monster;
}

function adjacentTransferCandidates(position) {
  return [
    [position.x + 1, position.y],
    [position.x - 1, position.y],
    [position.x, position.y + 1],
    [position.x, position.y - 1],
    [position.x + 1, position.y + 1],
    [position.x - 1, position.y - 1],
    [position.x + 1, position.y - 1],
    [position.x - 1, position.y + 1],
  ];
}

function tileDistance(left, right) {
  return Math.max(Math.abs(left.x - right.x), Math.abs(left.y - right.y));
}

function directionToward(from, to) {
  const dx = Math.sign(to.x - from.x);
  const dy = Math.sign(to.y - from.y);
  return new Map([
    ["0,-1", "Up"],
    ["1,-1", "UpRight"],
    ["1,0", "Right"],
    ["1,1", "DownRight"],
    ["0,1", "Down"],
    ["-1,1", "DownLeft"],
    ["-1,0", "Left"],
    ["-1,-1", "UpLeft"],
  ]).get(`${dx},${dy}`);
}

function waitFor(predicate, label, waitMs = timeoutMs, throwOnTimeout = true) {
  return new Promise((resolve, reject) => {
    const deadline = Date.now() + waitMs;
    const timer = setInterval(() => {
      if (predicate()) {
        clearInterval(timer);
        resolve(true);
      } else if (Date.now() >= deadline) {
        clearInterval(timer);
        if (throwOnTimeout) reject(new Error(`timed out waiting for ${label}`));
        else resolve(false);
      }
    }, 25);
  });
}

function waitForEvent(target, eventName, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`timed out waiting for ${label}`)), timeoutMs);
    target.addEventListener(
      eventName,
      () => {
        clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
    target.addEventListener(
      "error",
      (event) => {
        clearTimeout(timer);
        reject(event.error ?? new Error(`${label} failed`));
      },
      { once: true },
    );
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  process.stderr.write(`world director player acceptance failed: ${error.stack ?? error}\n`);
  process.exit(1);
});
