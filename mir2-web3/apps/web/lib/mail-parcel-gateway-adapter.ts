import {projectEquipmentGatewaySnapshot} from "./equipment-gateway-adapter";
import type {MailParcelSnapshot} from "./client-core-runtime";
const record=(v:unknown):v is Record<string,unknown>=>typeof v==="object"&&v!==null&&!Array.isArray(v);
const integer=(v:unknown,max=Number.MAX_SAFE_INTEGER):v is number=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0&&v<=max;
/** Raw full snapshots only. Unknown unselected metadata stays display-only. */
export function projectMailParcelSnapshot(raw:unknown):MailParcelSnapshot|null {
 const layout=projectEquipmentGatewaySnapshot(raw);if(!layout||!record(raw))return null;
 const sources=[raw.inventoryItems,raw.beltItems,raw.equipmentItems,raw.storageItems??[]];let index=0;
 const items:MailParcelSnapshot['items']=[];const ids=new Set<number>(),cells=new Set<string>();
 for(const group of sources){if(!Array.isArray(group))return null;for(const source of group){if(!record(source))return null;const placement=layout.placements[index++];
  const id=placement.uniqueId&&placement.uniqueId>0?placement.uniqueId:null,cell=`${placement.container}:${placement.slot}`;
  if(id!==null&&ids.has(id)||cells.has(cell)||placement.container===0&&placement.slot>=layout.capacity-6)return null;if(id!==null)ids.add(id);cells.add(cell);
  let pricing:MailParcelSnapshot['items'][number]['pricing']=null,stamp=false;const tooltip=source.tooltipSource;
  if(id!==null&&record(tooltip)&&record(tooltip.info)&&record(tooltip.userItem)){
   const info=tooltip.info,user=tooltip.userItem,stats=user.added_stats;
   if(user.unique_id===id&&user.item_index===info.item_index&&integer(info.item_index,0x7fffffff)&&integer(info.item_type,255)&&typeof info.shape==='number'&&Number.isInteger(info.shape)&&info.shape>=-32768&&info.shape<=32767&&integer(info.price,0xffffffff)&&integer(info.durability,65535)
    &&integer(source.quantity,0xffffffff)&&source.quantity>0&&Array.isArray(stats)&&stats.length<=256&&stats.every(s=>record(s)&&integer(s.stat,255)&&typeof s.value==='number'&&Number.isSafeInteger(s.value)&&s.value>=-2147483647&&s.value<=2147483647)
    &&(info.durability===0||integer(source.durabilityCurrent,65535)&&integer(source.durabilityMax,65535)&&source.durabilityCurrent<=source.durabilityMax)){
     pricing={templateIndex:info.item_index,itemType:info.item_type,shape:info.shape,templatePrice:info.price,templateDurability:info.durability,quantity:source.quantity,currentDura:integer(source.durabilityCurrent,65535)?source.durabilityCurrent:null,maxDura:integer(source.durabilityMax,65535)?source.durabilityMax:null,addedStats:stats.map(s=>({stat:(s as Record<string,number>).stat,value:(s as Record<string,number>).value}))};
     stamp=placement.container===0&&info.item_index===838&&info.item_type===0&&info.shape===1&&integer(user.count,65535)&&user.count>0;
   }
  }items.push({uniqueId:id,container:placement.container,slot:placement.slot,pricing,stamp});
 }}return{bagCapacity:layout.capacity-6,items};
}
/** Item mutations preserve authoritative UID 0; parcel attachment IDs keep their existing sentinel rules. */
export function projectMailItemMutationSnapshot(raw:unknown):MailParcelSnapshot|null {
 const layout=projectEquipmentGatewaySnapshot(raw);if(!layout)return null;
 return{bagCapacity:layout.capacity-6,items:layout.placements.map(placement=>({
  uniqueId:placement.uniqueId,container:placement.container,slot:placement.slot,pricing:null,stamp:false,
 }))};
}
const mutationTypes=new Set(['useItem','equipItem','removeItem','moveItem','mergeItem','splitItem','dropItem','sellItem','storeItem','takeBackItem','storeItemV2','takeBackItemV2','depositTradeItem','retrieveTradeItem','repairItem','specialRepairItem','combineItem','refineItem','awakenItem','sealItem','unsealItem','disassembleItem','upgradeItem','downgradeItem','lockItem','unlockItem','buyItem','sortBag','autoArrangeBag']);
export function isMailItemMutation(command:Record<string,unknown>):boolean{if(command.type==='mailLockedItem')return false;return mutationTypes.has(String(command.type))||/item/i.test(String(command.type))&&(Object.hasOwn(command,'uniqueId')||Object.hasOwn(command,'from')||Object.hasOwn(command,'to'));}
/** Decode both touched cells from the immutable wire DTO against raw authority. */
export function mailMutationAllowed(command:Record<string,unknown>,snapshot:MailParcelSnapshot|null,blocked:readonly number[],blockedCells:readonly {container:number;slot:number}[]=[]):boolean {
 if((!blocked.length&&!blockedCells.length)||!isMailItemMutation(command))return true;if(!snapshot)return false;
 const grid=(v:unknown):number|null=>typeof v==='string'?({inventory:0,bag:0,belt:1,equipment:2,questinventory:3,storage:4} as Record<string,number>)[v.toLowerCase()]??null:null;
 const cell=(container:number|null,slot:unknown,required=false):boolean=>{
  if(container===null||!integer(slot,0xffffffff)||blockedCells.some(c=>c.container===container&&c.slot===slot))return false;const matches=snapshot.items.filter(r=>r.container===container&&r.slot===slot);
  return matches.length===0?!required:matches.length===1&&matches[0].uniqueId!==null&&!blocked.includes(matches[0].uniqueId!);
 };
 const uid=(id:unknown,container?:number|null):boolean=>{
  if(!integer(id)||id===0)return false;const matches=snapshot.items.filter(r=>r.uniqueId===id);
  return matches.length===1&&(container===undefined||container===matches[0].container)&&!blocked.includes(id)&&!blockedCells.some(c=>c.container===matches[0].container&&c.slot===matches[0].slot);
 };
 switch(command.type){
  case 'moveItem':{
   if(String(command.grid).toLowerCase()==='belt'){
    // Belt's MoveItem endpoints address Crystal's unified inventory: the first
    // six cells are Belt, followed by the global Bag index. Decode both ends.
    const unified=(slot:unknown,required=false)=>integer(snapshot.bagCapacity,80)&&integer(slot,85)
     &&slot<snapshot.bagCapacity+6&&cell(slot<6?1:0,slot<6?slot:slot-6,required);
    return unified(command.from,true)&&unified(command.to);
   }
   return cell(grid(command.grid),command.from,true)&&cell(grid(command.grid),command.to);
  }
  case 'storeItem':case 'storeItemV2':return cell(0,command.from,true)&&cell(4,command.to);
  case 'takeBackItem':case 'takeBackItemV2':return cell(4,command.from,true)&&cell(0,command.to);
  case 'depositTradeItem':return cell(0,command.from,true);
  case 'retrieveTradeItem':return cell(0,command.to);
  case 'repairItem':case 'specialRepairItem':return uid(command.uniqueId,0);
  case 'equipItem':return uid(command.uniqueId,grid(command.grid))&&cell(2,command.to);
  case 'removeItem':return uid(command.uniqueId,2)&&cell(0,command.to);
  case 'mergeItem':return uid(command.idFrom,grid(command.gridFrom))&&uid(command.idTo,grid(command.gridTo));
  case 'useItem':return String(command.grid).toLowerCase()==='equipment'?cell(2,command.slot,true):uid(command.uniqueId,grid(command.grid));
  case 'dropItem':case 'sellItem':return command.heroInventory===true?true:uid(command.uniqueId);
  case 'splitItem':return uid(command.uniqueId,grid(command.grid));
  default:return false; // Unresolved mutation could touch a locked carried cell.
 }
}
export function mailInventoryMutationPacket(packet:string):boolean{return !packet.startsWith('Mail')&&/Item|Inventory|Belt|Equipment|Durability|UserSlots|UserStorage/.test(packet);}
