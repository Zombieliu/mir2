import { randomBytes } from "node:crypto";
import { lstat, mkdir, open, readdir, realpath } from "node:fs/promises";
import { isAbsolute, relative, resolve, sep } from "node:path";

const SCHEMA = "mir2-companion-supply-local/1";
const CONFIG = "supply-server.local.json";
const JOURNAL = "supply-server.journal.json";
const ERROR = "The private supply server configuration is unavailable; it was not reset or replaced.";
const ref = value => typeof value === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(value);
const exact = (value, keys) => value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));

export function withinPrivateRoot(root, path, { allowRoot = false } = {}) {
  const difference = relative(root, path);
  return (allowRoot || Boolean(difference)) && difference !== ".." && !difference.startsWith(`..\\`) && !difference.startsWith("../") && !isAbsolute(difference);
}

/** Resolve every existing directory component before creating its children,
 * so an intermediate junction cannot make mkdir write outside the root. */
export async function canonicalPrivateDirectory(directory, allowedRoot) {
  if (typeof directory !== "string" || !isAbsolute(directory) || typeof allowedRoot !== "string" || !isAbsolute(allowedRoot) || !withinPrivateRoot(resolve(allowedRoot), resolve(directory), { allowRoot: true })) throw new Error(ERROR);
  const root = await realpath(allowedRoot);
  let current = root;
  const parts = relative(resolve(allowedRoot), resolve(directory)).split(sep).filter(Boolean);
  for (const part of parts) {
    const next = resolve(current, part);
    let metadata;
    try { metadata = await lstat(next); }
    catch (error) { if (error.code !== "ENOENT") throw new Error(ERROR); await mkdir(next); metadata = await lstat(next); }
    if (!metadata.isDirectory() && !metadata.isSymbolicLink()) throw new Error(ERROR);
    current = await realpath(next);
    if (!withinPrivateRoot(root, current, { allowRoot: true })) throw new Error(ERROR);
  }
  return current;
}

async function readConfig(path) {
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 8192) throw new Error(ERROR);
  const file = await open(path, "r");
  try {
    const bytes = Buffer.alloc(8193); let length = 0;
    while (length < bytes.length) { const result = await file.read(bytes, length, bytes.length - length, null); if (!result.bytesRead) break; length += result.bytesRead; }
    if (!length || length > 8192) throw new Error(ERROR);
    return JSON.parse(bytes.subarray(0, length).toString("utf8"));
  } finally { await file.close(); }
}

function validate(value, realmId, worldEpoch) {
  if (!exact(value, ["schema", "realmId", "worldEpoch", "journalFileName", "macKeyHex"]) || value.schema !== SCHEMA || value.realmId !== realmId || value.worldEpoch !== worldEpoch || value.journalFileName !== JOURNAL || typeof value.macKeyHex !== "string" || !/^[0-9a-f]{64}$/.test(value.macKeyHex) || /^0+$/.test(value.macKeyHex)) throw new Error(ERROR);
  return value;
}

/** Only the Gateway receives this key. The host must never merge this result
 * into actor/native environments, profiles, public receipts, or archives. */
export async function localSupplyServerEnvironment({ directory, allowedRoot, realmId, worldEpoch }) {
  if (typeof directory !== "string" || !isAbsolute(directory) || typeof allowedRoot !== "string" || !isAbsolute(allowedRoot) || !ref(realmId) || !ref(worldEpoch) || !withinPrivateRoot(resolve(allowedRoot), resolve(directory), { allowRoot: true })) throw new Error(ERROR);
  try {
    const privateDirectory = await canonicalPrivateDirectory(directory, allowedRoot);
    const configPath = resolve(privateDirectory, CONFIG);
    let configuration;
    try { configuration = validate(await readConfig(configPath), realmId, worldEpoch); }
    catch (error) {
      if (error.code !== "ENOENT") throw new Error(ERROR);
      // A surviving journal, signed head, lock, or temporary publication is
      // evidence of earlier initialization. Losing its key cannot mint budget.
      const names = await readdir(privateDirectory);
      if (names.some(name => name === JOURNAL || name.startsWith(`${JOURNAL}.`))) throw new Error(ERROR);
      const key = randomBytes(32);
      if (key.every(value => value === 0)) throw new Error(ERROR);
      const candidate = { schema: SCHEMA, realmId, worldEpoch, journalFileName: JOURNAL, macKeyHex: key.toString("hex") };
      key.fill(0);
      let writer;
      try { writer = await open(configPath, "wx", 0o600); await writer.writeFile(JSON.stringify(candidate, null, 2)); await writer.sync(); }
      catch (createError) { if (createError.code !== "EEXIST") throw new Error(ERROR); }
      finally { await writer?.close(); }
      configuration = validate(await readConfig(configPath), realmId, worldEpoch);
    }
    return {
      MIR2_COMPANION_SUPPLY_JOURNAL_PATH: resolve(privateDirectory, JOURNAL),
      MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY: configuration.macKeyHex,
    };
  } catch { throw new Error(ERROR); }
}
