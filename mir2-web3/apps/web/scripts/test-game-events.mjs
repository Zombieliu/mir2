// Behavioral tests for lib/game-events/ (bus + sound-subscriber + vfx-subscriber).
//
// Covers:
//   1. Bus: emit + on (per-type), onAny, unsubscribe (idempotent), multiple
//      handlers, order guarantee.
//   2. SoundSubscriber: each event type calls the right AudioSink method;
//      gainedGold=0 does NOT call playGoldSound; unsubscribe stops calls.
//   3. VfxSubscriber: damageDealt calls addDamageFloater with correct payload;
//      entityStruck calls markEntityStruck; unsubscribe stops calls.
//   4. Integration: bus.emit drives both subscribers simultaneously.
//
// Pure logic: no DOM, no React, no network. Run with plain `node`.
// The .ts modules are transpiled in-memory via the `typescript` devDependency
// (same harness used by test-stage5-adapters.mjs, test-vfx-fallback.mjs, etc.).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// ---------------------------------------------------------------------------
// In-process TypeScript loader (mirrors the pattern in other test-*.mjs files)
// ---------------------------------------------------------------------------

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) in ${url}: ${specifier}`);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

// ---------------------------------------------------------------------------
// Load modules under test
//
// events.ts re-exports SoundEntityRef from original-sound-triggers; we stub
// that module to keep tests dependency-free.
// ---------------------------------------------------------------------------

const soundTriggersStub = {
  playEntityAttackSound: () => {},
  playEntityStruckSound: () => {},
  playEntityDieSound: () => {},
  playMagicSoundId: () => false,
  monsterImageFromBodyLibrary: () => null,
};

const BASE = new URL("../lib/game-events/", import.meta.url);

const eventsModule = loadTypeScriptModule(new URL("events.ts", BASE), {
  "../original-sound-triggers": soundTriggersStub,
});

const busModule = loadTypeScriptModule(new URL("bus.ts", BASE), {
  "./events": eventsModule,
});

const soundSubscriberModule = loadTypeScriptModule(new URL("sound-subscriber.ts", BASE), {
  "./bus": busModule,
  "./events": eventsModule,
});

const vfxSubscriberModule = loadTypeScriptModule(new URL("vfx-subscriber.ts", BASE), {
  "./bus": busModule,
  "./events": eventsModule,
});

const { createGameEventBus } = busModule;
const { registerSoundSubscriber } = soundSubscriberModule;
const { registerVfxSubscriber } = vfxSubscriberModule;

// ---------------------------------------------------------------------------
// Minimal test harness
// ---------------------------------------------------------------------------

let passed = 0;
let failed = 0;

function check(label, fn) {
  try {
    fn();
    passed += 1;
  } catch (err) {
    failed += 1;
    console.error(`FAIL: ${label}`);
    console.error(err);
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a no-op AudioSink with per-call recording. */
function makeAudioSink(entityRef = null) {
  const calls = [];
  return {
    calls,
    sink: {
      playOriginalSoundId(soundId) { calls.push({ fn: "playOriginalSoundId", soundId }); },
      setOriginalMusicId(musicId) { calls.push({ fn: "setOriginalMusicId", musicId }); },
      playEntityAttackSound(entity) { calls.push({ fn: "playEntityAttackSound", entity }); },
      playEntityStruckSound(entity) { calls.push({ fn: "playEntityStruckSound", entity }); },
      playEntityDieSound(entity) { calls.push({ fn: "playEntityDieSound", entity }); },
      playMagicSoundId(spell) { calls.push({ fn: "playMagicSoundId", spell }); },
      soundEntityRefFor(_objectId) { return entityRef; },
      playGoldSound() { calls.push({ fn: "playGoldSound" }); },
    },
  };
}

/** Build a VfxSink with per-call recording. */
function makeVfxSink() {
  const calls = [];
  return {
    calls,
    sink: {
      addDamageFloater(payload) { calls.push({ fn: "addDamageFloater", ...payload }); },
      markEntityStruck(objectId) { calls.push({ fn: "markEntityStruck", objectId }); },
    },
  };
}

// ---------------------------------------------------------------------------
// 1. Bus: core pub/sub
// ---------------------------------------------------------------------------

check("bus.on handler is called when matching event is emitted", () => {
  const bus = createGameEventBus();
  const received = [];
  bus.on("entityStruck", (e) => received.push(e));
  bus.emit({ type: "entityStruck", objectId: "42" });
  assert.equal(received.length, 1);
  assert.equal(received[0].objectId, "42");
});

check("bus.on handler is NOT called for a different event type", () => {
  const bus = createGameEventBus();
  const received = [];
  bus.on("entityStruck", (e) => received.push(e));
  bus.emit({ type: "entityDied", objectId: "99" });
  assert.equal(received.length, 0);
});

check("multiple handlers for the same type are all called (in insertion order)", () => {
  const bus = createGameEventBus();
  const order = [];
  bus.on("entityAttack", () => order.push(1));
  bus.on("entityAttack", () => order.push(2));
  bus.on("entityAttack", () => order.push(3));
  bus.emit({ type: "entityAttack", objectId: "1" });
  assert.deepEqual(order, [1, 2, 3]);
});

check("handlers for multiple different types are independent", () => {
  const bus = createGameEventBus();
  const attackCalls = [];
  const struckCalls = [];
  bus.on("entityAttack", (e) => attackCalls.push(e.objectId));
  bus.on("entityStruck", (e) => struckCalls.push(e.objectId));
  bus.emit({ type: "entityAttack", objectId: "A" });
  bus.emit({ type: "entityStruck", objectId: "B" });
  assert.deepEqual(attackCalls, ["A"]);
  assert.deepEqual(struckCalls, ["B"]);
});

check("unsubscribe stops the handler from being called", () => {
  const bus = createGameEventBus();
  const received = [];
  const unsub = bus.on("entityDied", (e) => received.push(e));
  bus.emit({ type: "entityDied", objectId: "1" });
  unsub();
  bus.emit({ type: "entityDied", objectId: "2" });
  assert.equal(received.length, 1, "second emit ignored after unsub");
  assert.equal(received[0].objectId, "1");
});

check("unsubscribe is idempotent (calling twice is safe)", () => {
  const bus = createGameEventBus();
  const received = [];
  const unsub = bus.on("playSound", (e) => received.push(e));
  unsub();
  unsub(); // must not throw
  bus.emit({ type: "playSound", soundId: 200 });
  assert.equal(received.length, 0);
});

check("onAny handler receives every event type", () => {
  const bus = createGameEventBus();
  const types = [];
  bus.onAny((e) => types.push(e.type));
  bus.emit({ type: "entityAttack", objectId: "1" });
  bus.emit({ type: "entityStruck", objectId: "2" });
  bus.emit({ type: "entityDied", objectId: "3" });
  bus.emit({ type: "magicCast", objectId: "4", spell: 31 });
  bus.emit({ type: "playSound", soundId: 100 });
  bus.emit({ type: "gainedGold", amount: 500 });
  bus.emit({ type: "mapMusicChanged", musicId: 146 });
  bus.emit({ type: "damageDealt", objectId: "5", damage: -100, damageType: 0 });
  assert.deepEqual(types, [
    "entityAttack",
    "entityStruck",
    "entityDied",
    "magicCast",
    "playSound",
    "gainedGold",
    "mapMusicChanged",
    "damageDealt",
  ]);
});

check("onAny unsubscribe stops delivery", () => {
  const bus = createGameEventBus();
  const types = [];
  const unsub = bus.onAny((e) => types.push(e.type));
  bus.emit({ type: "playSound", soundId: 1 });
  unsub();
  bus.emit({ type: "playSound", soundId: 2 });
  assert.equal(types.length, 1);
});

check("onAny + per-type handler both fire for the same event", () => {
  const bus = createGameEventBus();
  const typed = [];
  const any = [];
  bus.on("entityAttack", (e) => typed.push(e.objectId));
  bus.onAny((e) => any.push(e.type));
  bus.emit({ type: "entityAttack", objectId: "X" });
  assert.deepEqual(typed, ["X"]);
  assert.deepEqual(any, ["entityAttack"]);
});

check("emitting with no handlers registered is a no-op (no throw)", () => {
  const bus = createGameEventBus();
  bus.emit({ type: "entityDied", objectId: "ghost" }); // must not throw
});

// ---------------------------------------------------------------------------
// 2. SoundSubscriber: event -> AudioSink mapping
// ---------------------------------------------------------------------------

check("entityAttack -> playEntityAttackSound with resolved entity ref", () => {
  const bus = createGameEventBus();
  const fakeRef = { kind: "monster", sprite: { bodyLibrary: "Monster/004" }, genderKey: null };
  const { calls, sink } = makeAudioSink(fakeRef);
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "entityAttack", objectId: "42" });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playEntityAttackSound");
  assert.deepEqual(calls[0].entity, fakeRef);
});

check("entityStruck -> playEntityStruckSound", () => {
  const bus = createGameEventBus();
  const fakeRef = { kind: "selfPlayer", genderKey: "male" };
  const { calls, sink } = makeAudioSink(fakeRef);
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "entityStruck", objectId: "7" });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playEntityStruckSound");
  assert.equal(calls[0].entity, fakeRef);
});

check("entityDied -> playEntityDieSound", () => {
  const bus = createGameEventBus();
  const fakeRef = { kind: "player", genderKey: "female" };
  const { calls, sink } = makeAudioSink(fakeRef);
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "entityDied", objectId: "9" });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playEntityDieSound");
  assert.equal(calls[0].entity, fakeRef);
});

check("magicCast -> playMagicSoundId with the spell value", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "magicCast", objectId: "5", spell: 31 });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playMagicSoundId");
  assert.equal(calls[0].spell, 31);
});

check("magicCast with null spell still calls playMagicSoundId (graceful no-op path)", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "magicCast", objectId: "5", spell: null });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playMagicSoundId");
  assert.equal(calls[0].spell, null);
});

check("playSound -> playOriginalSoundId with correct id", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "playSound", soundId: 10110 });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playOriginalSoundId");
  assert.equal(calls[0].soundId, 10110);
});

check("gainedGold with amount > 0 -> playGoldSound", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "gainedGold", amount: 100 });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "playGoldSound");
});

check("gainedGold with amount = 0 -> does NOT call playGoldSound", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "gainedGold", amount: 0 });
  assert.equal(calls.length, 0);
});

check("mapMusicChanged -> setOriginalMusicId with the musicId", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "mapMusicChanged", musicId: 146 });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "setOriginalMusicId");
  assert.equal(calls[0].musicId, 146);
});

check("mapMusicChanged with null musicId -> setOriginalMusicId(null) (stops track)", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  registerSoundSubscriber(bus, sink);
  bus.emit({ type: "mapMusicChanged", musicId: null });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "setOriginalMusicId");
  assert.equal(calls[0].musicId, null);
});

check("soundSubscriber unsubscribe stops all sound handlers", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeAudioSink();
  const unsub = registerSoundSubscriber(bus, sink);
  bus.emit({ type: "entityAttack", objectId: "1" });
  assert.equal(calls.length, 1);
  unsub();
  bus.emit({ type: "entityAttack", objectId: "2" });
  bus.emit({ type: "entityDied", objectId: "3" });
  bus.emit({ type: "playSound", soundId: 999 });
  assert.equal(calls.length, 1, "no new calls after unsub");
});

// ---------------------------------------------------------------------------
// 3. VfxSubscriber: event -> VfxSink mapping
// ---------------------------------------------------------------------------

check("damageDealt -> addDamageFloater with full payload (hit)", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  registerVfxSubscriber(bus, sink);
  bus.emit({ type: "damageDealt", objectId: "33", damage: -200, damageType: 0 });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "addDamageFloater");
  assert.equal(calls[0].objectId, "33");
  assert.equal(calls[0].damage, -200);
  assert.equal(calls[0].damageType, 0);
});

check("damageDealt damageType=1 (miss) -> addDamageFloater with damageType 1", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  registerVfxSubscriber(bus, sink);
  bus.emit({ type: "damageDealt", objectId: "55", damage: 0, damageType: 1 });
  assert.equal(calls[0].damageType, 1);
});

check("damageDealt damageType=2 (crit) -> addDamageFloater with damageType 2", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  registerVfxSubscriber(bus, sink);
  bus.emit({ type: "damageDealt", objectId: "77", damage: -500, damageType: 2 });
  assert.equal(calls[0].damageType, 2);
  assert.equal(calls[0].damage, -500);
});

check("damageDealt positive damage (heal) -> addDamageFloater with positive damage", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  registerVfxSubscriber(bus, sink);
  bus.emit({ type: "damageDealt", objectId: "11", damage: 300, damageType: 0 });
  assert.equal(calls[0].damage, 300);
});

check("entityStruck -> markEntityStruck with correct objectId", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  registerVfxSubscriber(bus, sink);
  bus.emit({ type: "entityStruck", objectId: "99" });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].fn, "markEntityStruck");
  assert.equal(calls[0].objectId, "99");
});

check("vfxSubscriber does NOT fire for non-VFX events (attack, sound, etc.)", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  registerVfxSubscriber(bus, sink);
  bus.emit({ type: "entityAttack", objectId: "1" });
  bus.emit({ type: "entityDied", objectId: "2" });
  bus.emit({ type: "playSound", soundId: 100 });
  bus.emit({ type: "gainedGold", amount: 50 });
  bus.emit({ type: "mapMusicChanged", musicId: 146 });
  bus.emit({ type: "magicCast", objectId: "3", spell: 5 });
  assert.equal(calls.length, 0, "no VFX side-effects for non-VFX events");
});

check("vfxSubscriber unsubscribe stops all VFX handlers", () => {
  const bus = createGameEventBus();
  const { calls, sink } = makeVfxSink();
  const unsub = registerVfxSubscriber(bus, sink);
  bus.emit({ type: "entityStruck", objectId: "1" });
  assert.equal(calls.length, 1);
  unsub();
  bus.emit({ type: "entityStruck", objectId: "2" });
  bus.emit({ type: "damageDealt", objectId: "3", damage: -10, damageType: 0 });
  assert.equal(calls.length, 1, "no new calls after unsub");
});

// ---------------------------------------------------------------------------
// 4. Integration: both subscribers on the same bus
// ---------------------------------------------------------------------------

check("entityStruck fires both sound AND vfx subscribers", () => {
  const bus = createGameEventBus();
  const audio = makeAudioSink({ kind: "monster", sprite: null, genderKey: null });
  const vfx = makeVfxSink();
  registerSoundSubscriber(bus, audio.sink);
  registerVfxSubscriber(bus, vfx.sink);
  bus.emit({ type: "entityStruck", objectId: "42" });
  assert.equal(audio.calls.length, 1, "sound subscriber fired");
  assert.equal(audio.calls[0].fn, "playEntityStruckSound");
  assert.equal(vfx.calls.length, 1, "vfx subscriber fired");
  assert.equal(vfx.calls[0].fn, "markEntityStruck");
  assert.equal(vfx.calls[0].objectId, "42");
});

check("damageDealt fires vfx but NOT sound subscriber", () => {
  const bus = createGameEventBus();
  const audio = makeAudioSink();
  const vfx = makeVfxSink();
  registerSoundSubscriber(bus, audio.sink);
  registerVfxSubscriber(bus, vfx.sink);
  bus.emit({ type: "damageDealt", objectId: "7", damage: -50, damageType: 0 });
  assert.equal(audio.calls.length, 0, "no sound for damageDealt");
  assert.equal(vfx.calls.length, 1, "vfx fires for damageDealt");
});

check("full combat sequence wires both subscribers correctly", () => {
  const bus = createGameEventBus();
  const fakeRef = { kind: "monster", sprite: { bodyLibrary: "Monster/042" }, genderKey: null };
  const audio = makeAudioSink(fakeRef);
  const vfx = makeVfxSink();
  registerSoundSubscriber(bus, audio.sink);
  registerVfxSubscriber(bus, vfx.sink);

  bus.emit({ type: "entityAttack", objectId: "mob-1" });
  bus.emit({ type: "entityStruck", objectId: "player-1" });
  bus.emit({ type: "damageDealt", objectId: "player-1", damage: -120, damageType: 0 });
  bus.emit({ type: "entityDied", objectId: "player-1" });

  assert.equal(audio.calls.filter((c) => c.fn === "playEntityAttackSound").length, 1);
  assert.equal(audio.calls.filter((c) => c.fn === "playEntityStruckSound").length, 1);
  assert.equal(audio.calls.filter((c) => c.fn === "playEntityDieSound").length, 1);
  assert.equal(vfx.calls.filter((c) => c.fn === "markEntityStruck").length, 1);
  assert.equal(vfx.calls.filter((c) => c.fn === "addDamageFloater").length, 1);
  assert.equal(vfx.calls.find((c) => c.fn === "addDamageFloater").damage, -120);
});

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

if (failed > 0) {
  console.error(`\ngame-events tests: ${passed} passed, ${failed} FAILED`);
  process.exit(1);
} else {
  console.log(`game-events tests passed (${passed} groups)`);
}
