#!/usr/bin/env node

const gatewayUrl = process.argv[2] ?? "ws://127.0.0.1:7110/ws";
const timeoutMs = Number(process.env.MIR2_NATIVE_PROBE_TIMEOUT_MS ?? 5_000);
const socket = new WebSocket(gatewayUrl);

const valueType = (value) =>
  value === null ? "null" : Array.isArray(value) ? "array" : typeof value;

const summarizeRecord = (value, fields) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return { type: valueType(value), value };
  }

  return {
    keys: Object.keys(value).sort(),
    fields: Object.fromEntries(
      fields
        .filter((field) => Object.hasOwn(value, field))
        .map((field) => [
          field,
          { type: valueType(value[field]), value: value[field] },
        ]),
    ),
  };
};

const summarizeList = (value, fields, limit) => ({
  type: valueType(value),
  length: Array.isArray(value) ? value.length : null,
  samples: Array.isArray(value)
    ? value.slice(0, limit).map((entry) => summarizeRecord(entry, fields))
    : [],
});

const timeout = setTimeout(() => {
  console.error(`native snapshot probe timed out after ${timeoutMs} ms`);
  socket.close();
  process.exitCode = 1;
}, timeoutMs);

socket.onmessage = (event) => {
  const message = JSON.parse(String(event.data));
  if (message.type !== "worldSnapshot") return;

  const payload = message.payload ?? {};
  const summary = {
    keys: Object.keys(payload).sort(),
    scalars: Object.fromEntries(
      [
        "playerHp",
        "playerMaxHp",
        "playerMp",
        "playerMaxMp",
        "gold",
        "level",
        "mapTitle",
        "mapFileName",
        "selectedObjectId",
      ].map((field) => [
        field,
        { type: valueType(payload[field]), value: payload[field] },
      ]),
    ),
    inventory: summarizeList(
      payload.inventoryItems,
      ["slot", "index", "uniqueId", "itemId", "name", "count", "image", "imageIndex"],
      3,
    ),
    belt: summarizeList(
      payload.beltItems,
      ["slot", "index", "uniqueId", "itemId", "name", "count", "image", "imageIndex"],
      3,
    ),
    equipment: summarizeList(
      payload.equipmentItems,
      ["slot", "index", "uniqueId", "itemId", "name", "count", "image", "imageIndex"],
      6,
    ),
    drops: summarizeList(
      payload.groundDrops,
      ["key", "objectId", "itemId", "name", "count", "mapFileName", "x", "y"],
      3,
    ),
    entities: summarizeList(
      payload.entities,
      ["id", "objectId", "kind", "name", "hp", "maxHp", "x", "y", "direction"],
      4,
    ),
  };

  clearTimeout(timeout);
  console.log(JSON.stringify(summary, null, 2));
  socket.close();
};

socket.onerror = () => {
  clearTimeout(timeout);
  console.error("native snapshot probe failed");
  process.exitCode = 1;
};
