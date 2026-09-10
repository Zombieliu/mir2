import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, writeFileSync, statSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { parseCreatureSettings } from './generate-crystal-creature-settings.mjs';

test('missing key uses source 1F default; exact input bytes determine SHA', () => {
  const bytes = Buffer.from('\uFEFF[Other]\r\nDropRate=7\r\n[Game]\r\n; DropRate=9\r\n');
  const result = parseCreatureSettings(bytes);
  assert.equal(result.dropRate, 1);
  assert.equal(result.sourceSha256, createHash('sha256').update(bytes).digest('hex'));
  assert.equal(result.source, 'Crystal/Build/Server/Debug/Configs/Setup.ini');
  assert.equal(parseCreatureSettings(Buffer.from('[Game]\nDropRate=1')).dropRate, 1);
  assert.equal(parseCreatureSettings(Buffer.from('[Game]\nDropRate=.1')).dropRate, Math.fround(.1));
});

test('invalid source rates cannot become reward probabilities', () => {
  for (const rate of ['0', '-1', 'NaN', 'Infinity', '1e39', '1e-99', '', 'junk', '0x10']) {
    assert.throws(() => parseCreatureSettings(Buffer.from(`[Game]\nDropRate=${rate}`)), /Invalid source configuration/);
  }
});

test('--check never writes on success, mismatch, or missing output', () => {
  const root = mkdtempSync(join(tmpdir(), 'mir2-creature-settings-'));
  assert.equal(dirname(resolve(root)), resolve(tmpdir()));
  try {
    const source = join(root, 'Setup.ini');
    const output = join(root, 'settings.json');
    const script = resolve(import.meta.dirname, 'generate-crystal-creature-settings.mjs');
    const run = (...args) => spawnSync(process.execPath, [script, source, output, ...args], { encoding: 'utf8' });
    writeFileSync(source, '[Game]\nDropRate=1\n');
    assert.notEqual(run('--check').status, 0);
    assert.equal(existsSync(output), false);
    assert.equal(run().status, 0);
    const original = readFileSync(output);
    const modified = statSync(output).mtimeMs;
    assert.equal(run('--check').status, 0);
    assert.deepEqual(readFileSync(output), original);
    assert.equal(statSync(output).mtimeMs, modified);
    writeFileSync(source, '[Game]\nDropRate=2\n');
    assert.notEqual(run('--check').status, 0);
    assert.deepEqual(readFileSync(output), original);
    assert.equal(statSync(output).mtimeMs, modified);
  } finally {
    // Only this test-created directory, verified inside the system temp root.
    rmSync(root, { recursive: true, force: true });
  }
});
