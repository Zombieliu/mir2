import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

const sha = data => createHash('sha256').update(data).digest('hex');
const roles = ['primary', 'peer'];
const amounts = {primary:137, peer:59};
const partnerOf = role => role === 'primary' ? 'peer' : 'primary';
const sortBag = bag => bag.sort((a,b) => String(a.container).localeCompare(String(b.container)) || String(a.slot).localeCompare(String(b.slot)) || a.uidHash.localeCompare(b.uidHash));

const normalize = (items) => (items ?? []).map((item) => ({
  slot: item.slot, uidHash: sha(String(item.uniqueId)), quantity: item.quantity, container: item.container, key: item.key,
})).sort((a, b) => String(a.container).localeCompare(String(b.container)) || String(a.slot).localeCompare(String(b.slot)) || a.uidHash.localeCompare(b.uidHash));
export function economy(s) {
  return { gold: s.gold, bag: normalize(s.inventoryItems), belt: normalize(s.beltItems), equipment: normalize(s.equipmentItems) };
}

export function exactSettlement(before, items, trace) {
  const packets = (role,name) => trace.filter(row => row.phase === 'settlement' && row.role === role && row.packet === name);
  const expected = structuredClone(before), received = {};
  for (const role of roles) {
    const other = partnerOf(role);
    const sentItem = items[role], incoming = items[other];
    const terminal = packets(role,'TradeConfirm');
    const credit = packets(role,'GainedGold'), debit = packets(role,'LoseGold');
    const goods = packets(role,'GainedItem');
    assert.equal(terminal.length,1,role+' terminal');
    assert.equal(credit.length,1,role+' incoming gold');
    assert.equal(debit.length,1,role+' outgoing gold');
    assert.equal(goods.length,1,role+' incoming item');
    assert.equal(credit[0].gold,amounts[other]);
    assert.equal(debit[0].gold,amounts[role]);
    assert(credit[0].order < terminal[0].order && goods[0].order < terminal[0].order);
    assert.equal(incoming.key,'crystal-item-'+goods[0].item.itemIndex);
    assert.equal(goods[0].item.count,incoming.quantity);
    const ownUid=sha(String(sentItem.uniqueId)), deliveryUid=goods[0].item.uidHash, incomingUid=sha(String(incoming.uniqueId));
    const outgoing=before[role].bag.find(i=>i.uidHash===ownUid);
    const source=before[other].bag.find(i=>i.uidHash===incomingUid);
    assert(outgoing&&source);
    expected[role].bag=expected[role].bag.filter(i=>i.uidHash!==ownUid);
    const existing=[...expected[role].bag,...before[role].belt,...before[role].equipment];
    assert(!existing.some(i=>i.uidHash===deliveryUid),'receiver delivery collides with existing UID');
    const collision=existing.some(i=>i.uidHash===incomingUid);
    assert(collision?deliveryUid!==incomingUid:deliveryUid===incomingUid);
    const taken=new Set(expected[role].bag.filter(i=>i.container==='bag1').map(i=>i.slot));
    const target=taken.has(source.slot)?Array.from({length:40},(_,i)=>i).find(i=>!taken.has(i)):source.slot;
    expected[role].bag.push({...source,uidHash:deliveryUid,slot:target}); sortBag(expected[role].bag);
    expected[role].gold+=amounts[other]-amounts[role];
    received[role]={itemIndex:goods[0].item.itemIndex,count:incoming.quantity,fromUidHash:incomingUid,toUidHash:deliveryUid,bagSlot:target,
      normalizedCollision:collision,debit:debit[0].gold,credit:credit[0].gold,creditOrder:credit[0].order,itemOrder:goods[0].order,terminalOrder:terminal[0].order};
  }
  return {expected,received};
}

