import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import { archiveRecords, copy, facts, getArchiveRecord, locales, marketRows, mapPoints } from '../lib/atlas-data.mjs';
import { atlasDataState, displayCopy, getDisplayMetrics, getPolicyLinks, metricOrder } from '../lib/atlas-display.mjs';

assert.equal(locales.length, 16);
assert.deepEqual(atlasDataState, {kind:'prototype',connected:false});
assert.deepEqual(metricOrder,['onlinePlayers','activeRegions','publicEvents','marketVolume']);
assert.equal(displayCopy.en.metrics.onlinePlayers,'Adventurers online');
assert.equal(displayCopy.en.metrics.marketVolume,'Market volume');
for (const locale of locales) {
  assert.ok(facts[locale]?.[0] && facts[locale]?.[1]);
  assert.equal(copy[locale].hero[3], facts[locale][1]);
  assert.equal(copy[locale].record[1], facts[locale][0]);
  assert.equal(copy[locale].record[3], facts[locale][0]);
  const metrics=getDisplayMetrics(locale,facts[locale][0]);
  assert.deepEqual(metrics.map(metric=>metric.id),metricOrder);
  assert.ok(metrics.every(metric=>metric.value==='—'&&metric.note===facts[locale][0]));
  assert.equal(new Set(metrics.map(metric=>metric.label)).size,4);
  assert.deepEqual(getPolicyLinks(locale).map(link=>link.href),['legal/terms','legal/privacy','legal/refunds','support'].map(path=>`https://numeron.obelisk.build/${locale}/${path}`));
  for (const {kind, slug} of archiveRecords) {
    const record = getArchiveRecord(locale,kind,slug);
    assert.ok(record);
    assert.equal(record.summary,facts[locale][0]);
    assert.ok(record.summary.length < 65, 'List rows must retain short prototype descriptions');
    assert.ok(record.metrics.every(metric => metric.value === '—'));
    assert.deepEqual(record.metrics.map(metric=>metric.id),metricOrder);
    assert.ok(record.timeline.every(entry => entry.time === '—'));
    assert.ok(!/\b(?:Mir|Bichon|Sabuk|Verified|LIVE)\b|已验证|確認済み/.test(JSON.stringify(record)));
    for (const related of record.related) assert.ok(getArchiveRecord(locale,related.kind,related.slug));
  }
}
assert.ok(marketRows.every(row => row[2] === '—' && row[3] === '—'));
assert.ok(mapPoints.every(row => row[2] === '—'));
const dashboard=await readFile(new URL('../app/components/explorer-dashboard.js',import.meta.url),'utf8');
assert.ok(!/883921|#WR-|RED MOON|children:\s*"0%"/.test(dashboard),'Fabricated progress/phase/ledger IDs must not remain in presentation source');
assert.match(dashboard,/style:\s*\{\s*width:\s*0\s*\}/,'Prototype progress bar must have no fabricated fill');
console.log(`Truth content check passed: ${locales.length} locales × ${archiveRecords.length} prototype records; no connected player/economic metrics.`);
