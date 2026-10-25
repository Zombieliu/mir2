import fs from 'node:fs/promises';
import crypto from 'node:crypto';
const file='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/apps/web/scripts/playtest-capacity-soak.mjs';
const base='C:/mir2-build/p01-completion-20261024';
const original=await fs.readFile(file),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
if(original.length!==144416||sha(original)!=='2251b3a9cbdc391ecc0e68f17f7568468d677550a0f7425604a1ec0a7a485a42')throw Error('Original committed main source pin differs');
let source=original.toString('utf8');const rules=[],lines=s=>s.replaceAll('\n','\r\n');
function replace(before,after){const variants=[...new Set([before,lines(before)])].filter(value=>source.includes(value));if(variants.length!==1||source.split(variants[0]).length!==2)throw Error('Expected exactly one source anchor: '+before.slice(0,100));before=variants[0];const index=source.indexOf(before),crlf=before.includes('\r\n')||(!before.includes('\n')&&source.slice(index+before.length).startsWith('\r\n'));after=crlf?lines(after):after;source=source.replace(before,after);rules.push({before,after});}
replace('const COMBAT_CHASE_MAX_MS = 30000;',`const COMBAT_CHASE_MAX_MS = 30000;
const RECENT_CHASE_PREFERENCE_MS = 3000;`);
replace('    startedAt: now, activeElapsedMs: 0, confirmedMoves: 0, visited: new Map() };',`    startedAt: now, activeElapsedMs: 0, confirmedMoves: 0, visited: new Map(),
    coordinationFirstReceiptMonoMs: null, coordinationLastReceiptMonoMs: null };`);
replace(`  pursuit.confirmedMoves++;
  pursuit.lastMovementAt = now;`,`  pursuit.confirmedMoves++;
  pursuit.lastMovementAt = now;
  // Only the authoritative receipt's own clock can establish this optional
  // planning preference. Processing an old ACK does not refresh its age.
  const receiptMonoMs = receipt.monotonicMs, previousMonoMs = pursuit.coordinationLastReceiptMonoMs;
  if (Number.isFinite(receiptMonoMs) && receiptMonoMs >= 0 &&
      (!Number.isFinite(previousMonoMs) || receiptMonoMs >= previousMonoMs)) {
    if (!Number.isFinite(previousMonoMs) || receiptMonoMs - previousMonoMs > RECENT_CHASE_PREFERENCE_MS ||
        !Number.isFinite(pursuit.coordinationFirstReceiptMonoMs) || pursuit.coordinationFirstReceiptMonoMs < 0 ||
        pursuit.coordinationFirstReceiptMonoMs > previousMonoMs) pursuit.coordinationFirstReceiptMonoMs = receiptMonoMs;
    pursuit.coordinationLastReceiptMonoMs = receiptMonoMs;
  } else {
    pursuit.coordinationFirstReceiptMonoMs = null; pursuit.coordinationLastReceiptMonoMs = null;
  }`);
replace('    startedAt: now, lastMovementAt: now, targetPoint: lifecyclePoint(target), routeProgress: null,',`    startedAt: now, lastMovementAt: now, targetPoint: lifecyclePoint(target), routeProgress: null,
    coordinationFirstReceiptMonoMs: null, coordinationLastReceiptMonoMs: null,`);
replace('function combatPlan(client, context) {',`function recentNativePursuitLeaders(context, mapFileName, nowMonoMs) {
  const leaders = new Map();
  if (!Number.isFinite(nowMonoMs) || nowMonoMs < 0) return leaders;
  for (const hunter of context.clients) {
    if (hunter.role !== 'combat' || typeof hunter.label !== 'string' || !activeGameplayOwner(hunter, mapFileName)) continue;
    const pursuit = hunter.combatPursuit;
    if (!pursuit || pursuit.attackSent || pursuit.mapFileName !== mapFileName) continue;
    const first = pursuit.coordinationFirstReceiptMonoMs, last = pursuit.coordinationLastReceiptMonoMs;
    if (!Number.isFinite(first) || !Number.isFinite(last) || first < 0 || first > last ||
        last > nowMonoMs || nowMonoMs - last > RECENT_CHASE_PREFERENCE_MS) continue;
    const target = hunter.snapshot.entities.find(entity => entity.objectId === pursuit.objectId && entity.name === pursuit.name);
    if (!target || !availableCombatTarget(hunter, context, target, Date.now())) continue;
    const prior = leaders.get(pursuit.objectId);
    if (!prior || first < prior.firstReceiptMonoMs || (first === prior.firstReceiptMonoMs && hunter.label < prior.label)) {
      leaders.set(pursuit.objectId, { label: hunter.label, firstReceiptMonoMs: first });
    }
  }
  return leaders;
}

function combatPlan(client, context, nowMonoMs = performance.now()) {`);
replace(`    candidates = [...new Set([...(selected && adjacent.includes(selected) ? [selected] : []), ...adjacent,
      ...(selected ? [selected] : []), ...candidates])];`,`    candidates = [...new Set([...(selected && adjacent.includes(selected) ? [selected] : []), ...adjacent,
      ...(selected ? [selected] : []), ...candidates])];
    // Reorder only the original four-candidate window. A claimed target with
    // the only usable path remains a fallback within the same search budget.
    // This is a group planning preference, never an exclusive combat claim.
    const leaders = recentNativePursuitLeaders(context, client.snapshot.mapFileName, nowMonoMs);
    const immediate = new Set(adjacent.map(target => target.objectId));
    const priority = target => immediate.has(target.objectId) ? 0 :
      leaders.has(target.objectId) && leaders.get(target.objectId).label !== client.label ? 2 : 1;
    candidates = candidates.slice(0, 4).sort((a, b) => priority(a) - priority(b));`);
replace("  return client.role === 'combat' ? combatPlan(client, context) : patrol(client, context);", "  return client.role === 'combat' ? combatPlan(client, context, now) : patrol(client, context);");
replace("  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativeAttackWitness', { enumerable: true,",`  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativeChasePreference', { enumerable: true,
    value: Object.freeze({ freshReceiptMs: RECENT_CHASE_PREFERENCE_MS,
      scope: 'Client-side preference within the original first4 candidates, not an exclusive claim or a capacity result.',
      evidenceRule: 'Only fresh matching nonzero chase UserLocation receipts establish monotonic first/last ACK priority; active living same-map fighters and live targets only.',
      selectionRule: 'Keep adjacent witnessed attacks first, then prefer targets without an earlier recent pursuer; retain every original first4 candidate and path fallback.',
      budgetRule: 'Original4-target/8-goal/4000-node searches, cadence, offered/completed accounting and finite chase budgets are unchanged.',
      restoreRule: 'Resume clears receipt preference only; original chase elapsed time, ACK count and visited positions remain.' }) });
  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativeAttackWitness', { enumerable: true,`);
let inverse=source;for(const rule of [...rules].reverse()){if(inverse.split(rule.after).length!==2)throw Error('Ambiguous inverse');inverse=inverse.replace(rule.after,rule.before);}if(!Buffer.from(inverse).equals(original))throw Error('Inverse does not restore exact original');
await fs.writeFile(base+'/native-chase-preference-main-before-v1.mjs.txt',original,{flag:'wx'});
await fs.writeFile(file,source);
const actual=await fs.readFile(file);if(!actual.equals(Buffer.from(source)))throw Error('Source write differs');
const receipt={schemaVersion:1,createdAt:new Date().toISOString(),file,before:{bytes:original.length,sha256:sha(original)},after:{bytes:actual.length,sha256:sha(actual)},ruleCount:rules.length,inverseRestoresOriginal:true,rules,backendChanged:false,thresholdsChanged:false,testsNotYetRun:true};
await fs.writeFile(base+'/native-chase-preference-main-mutation-v1.json',JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({changed:true,before:receipt.before,after:receipt.after,ruleCount:rules.length,inverseRestoresOriginal:true}));
