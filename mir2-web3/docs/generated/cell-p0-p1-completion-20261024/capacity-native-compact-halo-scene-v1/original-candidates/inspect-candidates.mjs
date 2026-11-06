import { loadGeometry, PARAMETERS, ROOT, key, pin, writeNew, geometrySelfChecks } from './static-lib.mjs';
const g = await loadGeometry(), legal = [], rejected = {};
for (let y = PARAMETERS.bounds.minY; y <= PARAMETERS.bounds.maxY; y++) for (let x = PARAMETERS.bounds.minX; x <= PARAMETERS.bounds.maxX; x++) {
  const home = { x, y }, reasons = g.issues(home);
  if (reasons.length) { for (const reason of reasons) rejected[reason] = (rejected[reason] ?? 0) + 1; continue; }
  const targets = g.localTargets(home), scarecrows = targets.filter(t => t.name === 'Scarecrow').length;
  legal.push({ home, targets: targets.length, scarecrows, ids: targets.map(t => t.objectId) });
}
const selected = [];
for (const candidate of legal.sort((a, b) => b.targets - a.targets || b.scarecrows - a.scarecrows || b.home.y - a.home.y || a.home.x - b.home.x)) {
  if (selected.some(c => Math.max(Math.abs(c.home.x - candidate.home.x), Math.abs(c.home.y - candidate.home.y)) < 8)) continue;
  const bootstrap = g.bootstrap(candidate.home);
  if (!bootstrap.found || !bootstrap.noTransfer || !bootstrap.noInitialDanger) continue;
  selected.push({ ...candidate, bootstrap }); if (selected.length === 20) break;
}
const out = writeNew(ROOT + '/candidate-inventory-v1.json', { schemaVersion: 1, createdAt: new Date().toISOString(), parameters: PARAMETERS,
  geometrySelfChecks: geometrySelfChecks(), legalCount: legal.length, rejectedReasons: rejected, canonicalWeakSlots: g.targets.length,
  targetsInsideBounds: g.targets.filter(t => t.x >= 208 && t.x <= 376 && t.y >= 512 && t.y <= 682),
  topSpatiallySeparatedHomes: selected, sourcePins: [...g.sourcePins, pin(import.meta.filename)], accepted: false, capacityAccepted: false });
console.log(JSON.stringify({ out, legalCount: legal.length, top: selected, canonicalWeakSlots: g.targets.length }));
