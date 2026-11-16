import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PGlite } from '@electric-sql/pglite';

const root = dirname(fileURLToPath(import.meta.url));
const repo = process.argv[2] ?? 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const migrationPath = resolve(repo, 'infra/postgres/migrations/0018_procurement_authority_guard.sql');
const migrationBytes = await readFile(migrationPath);
const migration = migrationBytes.toString('utf8');
const packageMetadata = JSON.parse(await readFile(resolve(root, 'node_modules/@electric-sql/pglite/package.json'), 'utf8'));
assert.equal(packageMetadata.version, '0.5.13');
const collectionNames = ['orders', 'locks', 'credits', 'outbox', 'deliveries', 'chain_inbox', 'absence_inbox', 'requests', 'quota_quotes', 'quota_purchases'];
const original = {
  account_id: 'protected', gold: 100,
  procurement: Object.fromEntries(collectionNames.map(name => [name, { existing: { state: 'before', kind: name } }])),
};
const started = new Date();
const tests = [];
let pg;
let engine;
async function test(name, action) {
  const begin = performance.now();
  try {
    await action();
    tests.push({ name, passed: true, elapsedMs: Math.round(performance.now() - begin) });
    console.log(`PASS ${name}`);
  } catch (error) {
    tests.push({ name, passed: false, elapsedMs: Math.round(performance.now() - begin), error: String(error.stack ?? error) });
    console.log(`FAIL ${name}: ${error.message}`);
  }
}
async function getAccount(id = 'protected') {
  const result = await pg.query('SELECT raw_json FROM accounts WHERE account_id = $1', [id]);
  return result.rows[0]?.raw_json;
}
async function updateAccount(value, id = 'protected') {
  return pg.query('UPDATE accounts SET raw_json = $1::jsonb WHERE account_id = $2', [JSON.stringify(value), id]);
}
async function rejectsGuard(action, id = 'protected') {
  const before = await getAccount(id);
  await assert.rejects(action, error => {
    assert.equal(error.code, '23514');
    assert.match(error.message, /procurement authority/);
    return true;
  });
  assert.deepEqual(await getAccount(id), before);
}
function changed(action) {
  const next = structuredClone(original);
  action(next);
  return next;
}

try {
  // No TCP socket, external PostgreSQL daemon, credentials, or production data.
  pg = await PGlite.create({ dataDir: 'memory://' });
  engine = (await pg.query("SELECT version() AS version, current_setting('server_version') AS server_version")).rows[0];
  await pg.exec('CREATE TEMP TABLE accounts (account_id TEXT PRIMARY KEY, raw_json JSONB NOT NULL) ON COMMIT PRESERVE ROWS;');
  await pg.query('INSERT INTO accounts VALUES ($1, $2::jsonb)', ['protected', JSON.stringify(original)]);
  await pg.query('INSERT INTO accounts VALUES ($1, $2::jsonb)', ['plain', JSON.stringify({ gold: 100 })]);

  await test('migration creates PL/pgSQL function and trigger', async () => {
    await pg.exec(migration);
    const result = await pg.query("SELECT l.lanname FROM pg_proc p JOIN pg_language l ON l.oid = p.prolang WHERE p.proname = 'mir2_guard_procurement_authority'");
    assert.equal(result.rows.length, 1);
    assert.equal(result.rows[0].lanname, 'plpgsql');
  });
  await test('migration reapplication is idempotent', async () => {
    await pg.exec(migration);
    await pg.exec(migration);
    const result = await pg.query("SELECT COUNT(*)::int AS count FROM pg_trigger WHERE tgrelid = 'accounts'::regclass AND tgname = 'accounts_procurement_authority_guard'");
    assert.equal(result.rows[0].count, 1);
  });
  await test('plain account update is allowed', async () => {
    await updateAccount({ gold: 150, character: 'test only' }, 'plain');
    assert.equal((await getAccount('plain')).gold, 150);
  });
  await test('plain account deletion is allowed', async () => {
    await pg.query('INSERT INTO accounts VALUES ($1, $2::jsonb)', ['plain-delete', '{"gold":1}']);
    await pg.query('DELETE FROM accounts WHERE account_id = $1', ['plain-delete']);
    assert.equal(await getAccount('plain-delete'), undefined);
  });
  await test('legacy procurement:null account update and deletion are allowed', async () => {
    await pg.query('INSERT INTO accounts VALUES ($1, $2::jsonb)', ['legacy-null', '{"procurement":null,"gold":1}']);
    await updateAccount({ gold: 2 }, 'legacy-null');
    await pg.query('DELETE FROM accounts WHERE account_id = $1', ['legacy-null']);
    assert.equal(await getAccount('legacy-null'), undefined);
  });
  await test('procurement account deletion is blocked', async () => {
    await rejectsGuard(() => pg.query('DELETE FROM accounts WHERE account_id = $1', ['protected']));
  });
  await test('procurement:null downgrade is blocked', async () => {
    await rejectsGuard(() => updateAccount(changed(a => { a.procurement = null; })));
  });
  await test('omitted procurement downgrade is blocked', async () => {
    await rejectsGuard(() => updateAccount(changed(a => { delete a.procurement; })));
  });
  await test('empty procurement authority is blocked', async () => {
    await rejectsGuard(() => updateAccount(changed(a => { a.procurement = {}; })));
  });
  for (const name of collectionNames) {
    await test(`${name}: removing the history collection is blocked`, async () => {
      await rejectsGuard(() => updateAccount(changed(a => { delete a.procurement[name]; })));
    });
    await test(`${name}: clearing existing history is blocked`, async () => {
      await rejectsGuard(() => updateAccount(changed(a => { a.procurement[name] = {}; })));
    });
    await test(`${name}: replacing an existing key is blocked`, async () => {
      await rejectsGuard(() => updateAccount(changed(a => { a.procurement[name] = { replacement: {} }; })));
    });
  }
  await test('old schema11 writer cannot omit the new quota quote and purchase receipts', async () => {
    await rejectsGuard(() => updateAccount(changed(a => {
      delete a.procurement.quota_quotes;
      delete a.procurement.quota_purchases;
    })));
  });
  await test('normal state progression and appended history are allowed', async () => {
    const next = changed(a => {
      a.gold = 50;
      for (const name of collectionNames) {
        a.procurement[name].existing.state = 'after';
        a.procurement[name].appended = { state: 'new' };
      }
      a.procurement.outbox.existing.failed_transactions = { 'failed-digest': 45 };
      a.procurement.outbox.existing.archived_transactions = { 'failed-digest': { step: 'expire' } };
      a.procurement.outbox.existing.signed_transactions = { expire: { transaction_digest: 'retry-digest' } };
    });
    await updateAccount(next);
    assert.deepEqual(await getAccount(), next);
  });
  await test('transaction trigger rejection rolls back prior account debit and appended history', async () => {
    const protectedBefore = await getAccount();
    const plainBefore = await getAccount('plain');
    await assert.rejects(() => pg.transaction(async tx => {
      await tx.query("UPDATE accounts SET raw_json = jsonb_set(raw_json, '{gold}', '5'::jsonb) WHERE account_id = $1", ['plain']);
      await tx.query("UPDATE accounts SET raw_json = jsonb_set(raw_json, '{procurement,orders,transaction-added}', '{}'::jsonb) WHERE account_id = $1", ['protected']);
      await tx.query("UPDATE accounts SET raw_json = jsonb_set(raw_json, '{procurement,quota_purchases,transaction-grant}', '{\"source\":\"Test\",\"units\":10}'::jsonb) WHERE account_id = $1", ['protected']);
      await tx.query('DELETE FROM accounts WHERE account_id = $1', ['protected']);
    }), error => error.code === '23514');
    assert.deepEqual(await getAccount(), protectedBefore);
    assert.deepEqual(await getAccount('plain'), plainBefore);
  });
  await test('application error rolls back an otherwise allowed authority transition', async () => {
    const before = await getAccount();
    await assert.rejects(() => pg.transaction(async tx => {
      await tx.query("UPDATE accounts SET raw_json = jsonb_set(raw_json, '{gold}', '7'::jsonb) WHERE account_id = $1", ['protected']);
      throw new Error('deliberate callback failure');
    }), /deliberate callback failure/);
    assert.deepEqual(await getAccount(), before);
  });
  await test('a first procurement ledger can be created but cannot later be dropped', async () => {
    await pg.query('INSERT INTO accounts VALUES ($1, $2::jsonb)', ['new-ledger', '{"gold":1}']);
    await updateAccount({ gold: 1, procurement: {} }, 'new-ledger');
    await rejectsGuard(() => pg.query('DELETE FROM accounts WHERE account_id = $1', ['new-ledger']), 'new-ledger');
    await rejectsGuard(() => updateAccount({ gold: 1 }, 'new-ledger'), 'new-ledger');
  });
} catch (error) {
  tests.push({ name: 'harness setup/execution', passed: false, error: String(error.stack ?? error) });
  console.error(error);
} finally {
  if (pg) await pg.close();
}
const report = {
  startedAt: started.toISOString(), finishedAt: new Date().toISOString(),
  executionKind: 'Embedded PostgreSQL WASM SQL/PLpgSQL execution; not native/server PostgreSQL or Rust/libpq integration',
  pglite: { package: '@electric-sql/pglite', version: packageMetadata.version, wasm: true, dataDir: 'memory://', engine },
  dependenciesInstalledWithIgnoreScripts: true,
  migration: { sourcePath: migrationPath, sha256: createHash('sha256').update(migrationBytes).digest('hex') },
  officialReferences: ['https://pglite.dev/docs/about', 'https://pglite.dev/docs/api'],
  scope: 'Temporary minimal accounts(raw_json JSONB) table; exact unmodified migration 0018; no game runtime, native libpq, daemon, Docker, production database, or network database access',
  totals: { passed: tests.filter(t => t.passed).length, failed: tests.filter(t => !t.passed).length }, tests,
};
await writeFile(resolve(root, '0018-migration-tested.sql'), migrationBytes);
await writeFile(resolve(root, 'results.json'), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ ...report.totals, pglite: packageMetadata.version, engine, migrationSha256: report.migration.sha256 }));
if (report.totals.failed) process.exitCode = 1;
