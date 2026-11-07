import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { buildR16NativeLocalization } from "./generate-r16-native-localization.mjs";
import { buildPortableNativeLocalization, encodePortableNativeLocalization, PORTABLE_NATIVE_LANGUAGES,
  PORTABLE_NATIVE_MAX_BYTES, isPortableNativeKey } from "../../../packages/tooling/scripts/generate-portable-native-localization.mjs";

const native = buildR16NativeLocalization();
test("shared Native presentation is a deterministic, source-exact ten-locale subset within its budget", () => {
  const value = buildPortableNativeLocalization({ native });
  assert.deepEqual(value.languages, PORTABLE_NATIVE_LANGUAGES);
  assert.equal(readFileSync(new URL("../../../packages/game-data/data/generated/portable-native-i18n.json", import.meta.url), "utf8"), encodePortableNativeLocalization(value));
  assert(Buffer.byteLength(encodePortableNativeLocalization(value)) <= PORTABLE_NATIVE_MAX_BYTES);
  for (const row of value.entries) {
    const original = native.entries.find(entry => entry.key === row.key);
    for (const [index, code] of value.languages.entries())
      assert.equal(row.values[index], original.values[native.languages.findIndex(language => language.code === code)], `${row.key}/${code}`);
    assert(row.aliases.every(alias => original.aliases.includes(alias)));
  }
});
test("the shared vocabulary excludes NPC prose, credentials, user content and the broad legacy client catalog", () => {
  const value = buildPortableNativeLocalization({ native });
  for (const key of ["npc.prose.test", "npc.menu.test", "client.Chat", "billing.stripe", "account.password", "player.name",
    "content.item.245.name", "content.map.185.name"])
    assert.equal(isPortableNativeKey(key), false);
  assert(value.entries.every(row => isPortableNativeKey(row.key)));
  assert(value.entries.some(row => row.key === "content.magic.Fencing.name"));
  for (const key of ["content.item.244.name", "content.item.259.name", "content.item.272.name", "content.map.186.name"])
    assert(value.entries.some(row => row.key === key), `graduation display key missing: ${key}`);
});
test("missing languages, duplicate keys, malformed selected values and oversized data fail instead of falling back", () => {
  const mutated = change => { const copy = structuredClone(native); change(copy); return copy; };
  assert.throws(() => buildPortableNativeLocalization({ native: mutated(value => value.languages.pop()) }));
  assert.throws(() => buildPortableNativeLocalization({ native: mutated(value => value.entries.push(value.entries[0])) }), /duplicate/);
  assert.throws(() => buildPortableNativeLocalization({ native: mutated(value => { value.entries.find(row => row.key === "game.060").values.pop(); }) }));
  assert.throws(() => buildPortableNativeLocalization({ native: mutated(value => { value.entries.find(row => row.key === "game.060").values[0] = "x".repeat(PORTABLE_NATIVE_MAX_BYTES); }) }), /producer cap/);
});
