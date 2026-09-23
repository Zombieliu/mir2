/** Crystal Server/Settings.cs:37,112,429 reads Configs/Setup.ini [Game] DropRate.
 * A missing key keeps the source default 1F. Invalid/nonpositive rates are
 * rejected deliberately: division by them cannot define a valid reward draw.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function parseCreatureSettings(bytes) {
  let section = '';
  let rawRate;
  for (const raw of bytes.toString('utf8').replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith(';') || line.startsWith('#')) continue;
    if (line.startsWith('[') && line.endsWith(']')) {
      section = line.slice(1, -1).trim().toLowerCase();
      continue;
    }
    const equals = line.indexOf('=');
    if (section === 'game' && equals >= 0 && line.slice(0, equals).trim().toLowerCase() === 'droprate') {
      rawRate = line.slice(equals + 1).trim();
    }
  }
  const value = rawRate ?? '1';
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(value)) {
    throw new Error('Invalid source configuration Configs/Setup.ini [Game] DropRate');
  }
  const dropRate = Math.fround(Number(value));
  if (!Number.isFinite(dropRate) || dropRate <= 0) {
    throw new Error('Invalid source configuration: DropRate must be finite positive f32');
  }
  return {
    source: 'Crystal/Build/Server/Debug/Configs/Setup.ini',
    sourceSha256: createHash('sha256').update(bytes).digest('hex'),
    dropRate,
  };
}

export function generateCreatureSettings(source, output, check = false) {
  const encoded = JSON.stringify(parseCreatureSettings(readFileSync(source)), null, 2) + '\n';
  if (check) {
    if (readFileSync(output, 'utf8') !== encoded) throw new Error('Creature settings differ from source');
  } else {
    writeFileSync(output, encoded);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const root = resolve(import.meta.dirname, '../../..');
  const args = process.argv.slice(2);
  const positional = args.filter(arg => arg !== '--check');
  if (positional.length > 2 || positional.some(arg => arg.startsWith('--'))) throw new Error('Usage: [source.ini] [output.json] [--check]');
  const source = positional[0] ?? resolve(root, '../Crystal/Build/Server/Debug/Configs/Setup.ini');
  const output = positional[1] ?? resolve(root, 'packages/game-data/data/generated/crystal_creature_settings.json');
  generateCreatureSettings(source, output, args.includes('--check'));
  console.log(`Creature settings ${args.includes('--check') ? 'verified' : 'generated'}: ${output}`);
}
