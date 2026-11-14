import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function parseGuildSettings(bytes) {
  const sections = new Map();
  let section;
  for (const raw of bytes.toString('utf8').replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith(';') || line.startsWith('#')) continue;
    if (line.startsWith('[') && line.endsWith(']')) {
      section = new Map();
      sections.set(line.slice(1, -1).toLowerCase(), section);
      continue;
    }
    const equals = line.indexOf('=');
    if (!section || equals < 0) throw new Error(`Invalid GuildSettings line: ${line}`);
    section.set(line.slice(0, equals).trim().toLowerCase(), line.slice(equals + 1).trim());
  }
  const get = (s, k, fallback) => sections.get(s.toLowerCase())?.get(k.toLowerCase()) ?? fallback;
  const integer = (s, k, fallback, min, max) => {
    const value = get(s, k, String(fallback));
    if (!/^-?\d+$/.test(value)) throw new Error(`Invalid integer ${s}.${k}`);
    const result = BigInt(value);
    if (result < BigInt(min) || result > BigInt(max)) throw new Error(`Out of range ${s}.${k}`);
    return result;
  };
  const levels = (s, max) => {
    const result = [];
    for (let index = 0; ; index++) {
      const value = integer(s, `Level-${index}`, -1, -1, max);
      if (value === -1n) return result;
      result.push(value);
    }
  };
  const costs = [];
  for (let index = 0; ; index++) {
    const amount = Number(integer(`Required-${index}`, 'Amount', 0, 0, 4294967295n));
    if (amount === 0) break;
    costs.push({ itemName: get(`Required-${index}`, 'ItemName', ''), amount });
  }
  const rate = Number(get('Guilds', 'ExpRate', '0.01'));
  if (!Number.isFinite(rate) || !Number.isFinite(Math.fround(rate)) || rate < 0) throw new Error('Invalid Guilds.ExpRate');
  const boolean = get('Guilds', 'NewbieGuildBuffEnabled', 'True').toLowerCase();
  if (!['true', 'false'].includes(boolean)) throw new Error('Invalid guild newbie boolean');
  return {
    source: 'Crystal/Build/Server/Debug/Configs/GuildSettings.ini',
    sourceSha256: createHash('sha256').update(bytes).digest('hex'),
    minimumLevel: Number(integer('Guilds', 'MinimumLevel', 22, 0, 255)),
    experienceRate: rate,
    pointsPerLevel: Number(integer('Guilds', 'PointPerLevel', 0, 0, 255)),
    warTime: integer('Guilds', 'WarTime', 180, 0, 9223372036854775807n),
    warCost: Number(integer('Guilds', 'WarCost', 3000, 0, 4294967295n)),
    newbieGuildBuffEnabled: boolean === 'true',
    newbieGuildExpBuff: Number(integer('Guilds', 'NewbieGuildExpBuff', 5, -2147483648n, 2147483647n)),
    totalBuffs: Number(integer('Guilds', 'TotalBuffs', 0, 0, 255)),
    creationCosts: costs,
    experienceLevels: levels('Exp', 9223372036854775807n),
    memberCaps: levels('Cap', 2147483647n).map(Number),
  };
}

export function serializeGuildSettings(settings) {
  // Keep Int64 thresholds exact: the original final threshold exceeds JS's
  // safe integer range, but serde_json reads this integer token losslessly.
  return JSON.stringify(settings, (_, value) => {
    if (typeof value === 'string' && /^__I64_-?\d+__$/.test(value)) {
      throw new Error('Reserved integer serialization marker in guild settings');
    }
    return typeof value === 'bigint' ? `__I64_${value}__` : value;
  }, 2).replace(/"__I64_(-?\d+)__"/g, '$1') + '\n';
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const root = resolve(import.meta.dirname, '../../..');
  const positional = process.argv.slice(2).filter(value => value !== '--check');
  const source = positional[0] ?? resolve(root, '../Crystal/Build/Server/Debug/Configs/GuildSettings.ini');
  const output = positional[1] ?? resolve(root, 'packages/game-data/data/generated/crystal_guild_settings.json');
  const encoded = serializeGuildSettings(parseGuildSettings(readFileSync(source)));
  if (process.argv.includes('--check')) {
    if (readFileSync(output, 'utf8') !== encoded) throw new Error('Guild settings differ from source');
  } else writeFileSync(output, encoded);
  console.log(`Guild settings ${process.argv.includes('--check') ? 'verified' : 'generated'}: ${output}`);
}
