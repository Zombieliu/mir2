/** Hero UI authority. All custody changes remain server-owned. */
export type HeroUiOwner = Readonly<{
  connectionGeneration: number; sessionGeneration: number; playerObjectId: number;
  socket: object; sceneRevision: number; mapFileName: string;
}>;
type Row = Record<string, unknown>;
export type HeroActor = Readonly<{ objectId: number; name: string; class: string; gender: string; generation: number }>;
export type HeroItem = Readonly<{
  slot: number; uniqueId: number | string; itemIndex: number; count: number; name: string; icon: number;
  userItem: Readonly<Row>; info: Readonly<Row>; tooltipSource: Readonly<Row>;
}>;
export type HeroMagic = Readonly<{ spell: string; name: string; icon: number; key: number; level: number; raw: Readonly<Row> }>;
export type HeroSkillKeyAck = Readonly<{ requestId: number; spell: string; key: number; oldKey: number; accepted: boolean }>;
export type HeroPlayerModel = Readonly<{
  owner: HeroUiOwner; actor: HeroActor; sourceKey: string; authoritySerial: number;
  informationSerial: number; snapshotSerial: number; personalSerial: number; skillSnapshotSerial: number;
  inventoryCapacity: number; inventory: readonly (HeroItem | null)[]; equipment: readonly (HeroItem | null)[];
  personalInventory: readonly (HeroItem | null)[] | null; magics: readonly HeroMagic[];
  hp: number; mp: number; level: number; experience: number | string; maxExperience: number | string; spawned: boolean; riding: boolean | null;
  autoPot: boolean; hpPercent: number; mpPercent: number; hpItemIndex: number; mpItemIndex: number;
  stats: readonly Readonly<{ stat: number; value: number }>[];
  weights: Readonly<{ bag: number; wear: number; hand: number }>;
  skillKeyAck: HeroSkillKeyAck | null;
}>;
export type HeroGrid = "HeroInventory" | "HeroEquipment" | "Inventory";
export type HeroCell = Readonly<{ grid: HeroGrid; slot: number }>;
export type HeroUiAction =
  | Readonly<{ kind: "move"; from: number; to: number }>
  | Readonly<{ kind: "equip"; from: number; to: number }>
  | Readonly<{ kind: "remove"; from: number; to?: number }>
  | Readonly<{ kind: "merge"; from: HeroCell; to: HeroCell }>
  | Readonly<{ kind: "transfer"; from: number; to: number }>
  | Readonly<{ kind: "takeBack"; from: number; to: number }>
  | Readonly<{ kind: "use"; slot: number; confirmed?: boolean }>
  | Readonly<{ kind: "autoPotValue"; stat: 12 | 13; value: number }>
  | Readonly<{ kind: "autoPotItem"; grid: "HeroHpItem" | "HeroMpItem"; slot: number | null }>
  | Readonly<{ kind: "magicKey"; spell: string; key: number }>;
export type HeroWire = Readonly<Record<string, string | number>>;
export type HeroActionPlan = Readonly<{ wire: HeroWire; confirmationRequired: boolean; crossPlayer: boolean }>;
export type HeroActionDto = Readonly<{ actor: HeroActor; owner: HeroUiOwner; sourceKey: string; action: HeroUiAction }>;

export type HeroBasisWindows = Readonly<{inventoryOpen:boolean;characterOpen:boolean;characterPage:"equipment"|"status"|"state"|"skills";beltVisible:boolean;beltVertical:boolean}>;
export type HeroActionBasis = Readonly<{version:1;actor:Readonly<{objectId:number;name:string;class:string;gender:string;spawned:boolean}>;
  windows:HeroBasisWindows;cells:readonly Readonly<{grid:HeroGrid;slot:number;capacity:number;item:null|Readonly<{uid:string;itemIndex:number;count:number}>}>[];
  facts:Readonly<Record<string,unknown>>;resolvedWire:Readonly<Record<string,string|number>>;confirmationRequired:boolean;crossPlayer:boolean}>;
/** An action witness, independently derived from the frozen Web owner model.
 * It never supplies a model to the Rust planner or broadens the legacy wire ABI. */
export function captureHeroActionBasis(model:HeroPlayerModel,action:HeroUiAction,windows:HeroBasisWindows):HeroActionBasis|null {
  try {
    const plan=planHeroAction(model,action);if(!plan)return null;
    const selected:HeroCell[]=[],policySelected:HeroCell[]=[];let target:{mode:"none"|"explicit"|"automatic";slot:number|null}={mode:"none",slot:null};
    let requirement:Readonly<{passed:boolean;statValue:number|null}>|null=null;
    let restock:Readonly<{belt:number;from:number;uid:string;itemIndex:number}>|null=null;
    const add=(grid:HeroGrid,slot:number,usesPolicy=true)=>{const cell={grid,slot};if(!available(model,cell))throw Error("Hero basis cell unavailable");
      if(!selected.some(c=>c.grid===grid&&c.slot===slot))selected.push(cell);
      if(usesPolicy&&!policySelected.some(c=>c.grid===grid&&c.slot===slot))policySelected.push(cell);};
    const policy=(item:HeroItem)=>{const base=item.tooltipSource.info as Row,effective=item.info;
      return {base:{itemIndex:base.item_index,itemType:base.item_type,shape:base.shape,stackSize:base.stack_size},
        effective:{itemIndex:effective.item_index,itemType:effective.item_type,shape:effective.shape,stackSize:effective.stack_size,
          requiredClass:effective.required_class,requiredGender:effective.required_gender,requiredType:effective.required_type,requiredAmount:effective.required_amount}};};
    const requireItem=(slot:number)=>{const item=at(model,{grid:"HeroInventory",slot});if(!item)throw Error("Hero basis source absent");
      const stat=({1:1,2:3,3:5,4:7,5:9,7:0,8:2,9:4,10:6,11:8} as Record<number,number>)[item.info.required_type as number];
      requirement={passed:heroRequirements(model,item),statValue:stat===undefined?null:model.stats.find(s=>s.stat===stat)?.value??0};return item;};
    switch(action.kind){
      case "move":case "equip":add("HeroInventory",action.from);add(action.kind==="move"?"HeroInventory":"HeroEquipment",action.to);
        target={mode:"explicit",slot:action.to};if(action.kind==="equip")requireItem(action.from);break;
      case "merge":add(action.from.grid,action.from.slot);add(action.to.grid,action.to.slot);target={mode:"explicit",slot:action.to.slot};break;
      case "transfer":case "takeBack":add(action.kind==="transfer"?"Inventory":"HeroInventory",action.from);
        add(action.kind==="transfer"?"HeroInventory":"Inventory",action.to);target={mode:"explicit",slot:action.to};break;
      case "remove":add("HeroEquipment",action.from);target={mode:action.to===undefined?"automatic":"explicit",slot:plan.wire.to as number};
        if(action.to===undefined){for(let slot=0;slot<model.inventory.length;slot++)add("HeroInventory",slot,false);}else add("HeroInventory",action.to);break;
      case "use":{
        add("HeroInventory",action.slot);const item=requireItem(action.slot),type=item.info.item_type;
        if(type===6)add("HeroEquipment",6);if(type===7)add("HeroEquipment",8);
        if(plan.wire.type==="equipItem"){target={mode:"automatic",slot:plan.wire.to as number};add("HeroEquipment",target.slot!);}
        if(plan.wire.type==="mergeItem"){const slot=model.equipment.findIndex(i=>i&&String(i.uniqueId)===String(plan.wire.idTo));
          if(slot<0)throw Error("Hero basis target absent");target={mode:"automatic",slot};add("HeroEquipment",slot);}
        if(plan.wire.type==="useItem"&&action.slot<=1&&item.count===1){for(let slot=0;slot<model.inventory.length;slot++)add("HeroInventory",slot,false);
          const candidate=heroRestockCandidate(model,action.slot);if(candidate)restock={belt:candidate.belt,from:candidate.from,uid:String(candidate.uniqueId),itemIndex:candidate.itemIndex};}
        break;}
      case "autoPotItem":if(action.slot!==null){add("HeroInventory",action.slot);target={mode:"explicit",slot:action.slot};}break;
      case "autoPotValue":case "magicKey":break;
    }
    const rank={HeroInventory:0,HeroEquipment:1,Inventory:2};selected.sort((a,b)=>rank[a.grid]-rank[b.grid]||a.slot-b.slot);
    const cells=selected.map(cell=>{const item=at(model,cell),list=cell.grid==="HeroInventory"?model.inventory:cell.grid==="HeroEquipment"?model.equipment:model.personalInventory!;
      if(item&&!exactId(item.uniqueId))throw Error("Hero basis invalid UID");
      return {...cell,capacity:list.length,item:item?{uid:String(item.uniqueId),itemIndex:item.itemIndex,count:item.count}:null};});
    policySelected.sort((a,b)=>rank[a.grid]-rank[b.grid]||a.slot-b.slot);
    const policies=policySelected.flatMap(cell=>{const item=at(model,cell);return item?[{...cell,...policy(item)}]:[];});
    const keys=action.kind==="magicKey"?model.magics.map(m=>({spell:m.spell,key:m.key})).sort((a,b)=>a.spell<b.spell?-1:a.spell>b.spell?1:0):[];
    const resolvedWire=Object.fromEntries(Object.entries(plan.wire).map(([k,v])=>[k,["uniqueId","idFrom","idTo"].includes(k)?String(v):v]));
    const basis:HeroActionBasis={version:1,actor:{objectId:model.actor.objectId,name:model.actor.name,class:model.actor.class,gender:model.actor.gender,spawned:model.spawned},
      windows:{...windows},cells,facts:{hp:model.hp,level:model.level,riding:model.riding,autoPot:model.autoPot,policies,requirement,target,keys,restock,confirmed:action.kind==="use"&&action.confirmed===true},
      resolvedWire,confirmationRequired:plan.confirmationRequired,crossPlayer:plan.crossPlayer};
    return new TextEncoder().encode(canonical(basis)).byteLength<=16384?freeze(basis):null;
  }catch{return null;}
}
export function heroActionBasisMatches(basis:unknown,model:HeroPlayerModel,action:HeroUiAction,windows:HeroBasisWindows):boolean {
  const expected=captureHeroActionBasis(model,action,windows);
  try{return expected!==null&&canonical(basis)===canonical(expected);}catch{return false;}
}

export type HeroRestockCandidate = Readonly<{ actor: HeroActor; owner: HeroUiOwner; belt: number; from: number; uniqueId: number; itemIndex: number }>;

const row = (v: unknown): v is Row => typeof v === "object" && v !== null && !Array.isArray(v);
const int = (v: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
const text = (v: unknown): v is string => typeof v === "string" && v.length > 0 && v.length <= 256 && !v.includes("\0");
const key = (v: unknown): v is number => v === 0 || int(v, 17, 24);
const classes = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"];
const genders = ["Male", "Female"];
const capacities = [10, 18, 26, 34, 42];
const i32 = (v: unknown): v is number => int(v, -2147483648, 2147483647);
const exactId = (v: unknown): v is number | string => int(v, 1) || typeof v === "string"
  && /^[1-9][0-9]{0,19}$/.test(v) && BigInt(v) <= 18446744073709551615n;
const exactExperience = (v: unknown): v is number | string => int(v, 0) || typeof v === "string"
  && /^(?:0|[1-9][0-9]{0,18})$/.test(v) && BigInt(v) <= 9223372036854775807n;
const sameId = (a: unknown, b: unknown) => exactId(a) && exactId(b) && String(a) === String(b);
/** Stable serialization is a proof of the entire source, never an instance ID. */
function canonical(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number" && Number.isFinite(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (row(value)) return `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${canonical(value[k])}`).join(",")}}`;
  throw new Error("Incomplete Hero source");
}
function copy<T>(value: T): T { return JSON.parse(canonical(value)) as T; }
function freeze<T>(value: T): T {
  if (value && typeof value === "object") { for (const v of Object.values(value)) freeze(v); Object.freeze(value); }
  return value;
}
function validOwner(owner: HeroUiOwner): boolean {
  return row(owner) && int(owner.connectionGeneration) && int(owner.sessionGeneration)
    && int(owner.playerObjectId, 1, 4294967295) && row(owner.socket)
    && int(owner.sceneRevision) && text(owner.mapFileName);
}
export function sameHeroSession(a: HeroUiOwner, b: HeroUiOwner): boolean {
  return a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId;
}
export function sameHeroOwner(a: HeroUiOwner, b: HeroUiOwner): boolean {
  return sameHeroSession(a, b) && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
}
function sameActor(a: HeroActor, b: HeroActor): boolean {
  return a.objectId === b.objectId && a.name === b.name && a.class === b.class && a.gender === b.gender && a.generation === b.generation;
}
function userItem(v: unknown, depth = 0): v is Row {
  if (!row(v) || depth > 8 || !exactId(v.unique_id) || !i32(v.item_index) || !int(v.count, 1, 65535)) return false;
  for (const k of ["current_dura", "max_dura", "gem_count"]) if (!int(v[k], 0, 65535)) return false;
  for (const k of ["soul_bound_id", "refine_success_chance", "wedding_ring"]) if (!i32(v[k])) return false;
  for (const k of ["awake_type", "refined_value", "refine_added"]) if (!int(v[k], 0, 255)) return false;
  for (const k of ["identified", "cursed", "is_shop_item", "gm_made"]) if (typeof v[k] !== "boolean") return false;
  if (!Array.isArray(v.slots) || v.slots.length > 256 || !v.slots.every(x => x === null || userItem(x, depth + 1))
    || !Array.isArray(v.added_stats) || !v.added_stats.every(x => row(x) && int(x.stat, 0, 255) && i32(x.value))
    || !Array.isArray(v.awake_values) || !v.awake_values.every(x => int(x, 0, 255))) return false;
  return ["expire_info", "rental_information", "sealed_info"].every(k => v[k] === null || row(v[k]));
}
function nullableItems(v: unknown, count: number): v is (Row | null)[] {
  return Array.isArray(v) && v.length === count && Array.from(v).every(x => x === null || userItem(x));
}
function custodyIds(items: readonly (HeroItem | Row | null)[]): string[] {
  const result: string[] = [];
  const visit = (raw: Row) => {
    result.push(String(raw.unique_id));
    for (const child of raw.slots as (Row | null)[]) if (child) visit(child);
  };
  for (const item of items) if (item) visit("userItem" in item ? item.userItem as Row : item as Row);
  return result;
}
function magic(v: unknown): v is Row {
  if (!row(v) || !text(v.name) || !text(v.spell) || !key(v.key)) return false;
  for (const k of ["base_cost", "level_cost", "icon", "level1", "level2", "level3", "level", "range"]) if (!int(v[k], 0, 255)) return false;
  for (const k of ["need1", "need2", "need3", "experience"]) if (!int(v[k], 0, 65535)) return false;
  return int(v.delay, -Number.MAX_SAFE_INTEGER) && int(v.cast_time, -Number.MAX_SAFE_INTEGER);
}
function information(v: unknown): v is Row {
  if (!row(v) || !int(v.object_id, 1, 4294967295) || !text(v.name) || !classes.includes(String(v.class))
    || !genders.includes(String(v.gender)) || !int(v.level, 0, 65535) || !int(v.hair, 0, 255)
    || !i32(v.hp) || !i32(v.mp) || !exactExperience(v.experience) || !exactExperience(v.max_experience)
    || !Array.isArray(v.inventory) || !capacities.includes(v.inventory.length) || !nullableItems(v.inventory, v.inventory.length)
    || !nullableItems(v.equipment, 14) || !Array.isArray(v.magics) || v.magics.length > 256 || !v.magics.every(magic)
    || new Set(v.magics.map(m => m.spell)).size !== v.magics.length || typeof v.auto_pot !== "boolean"
    || !int(v.auto_hp_percent, 0, 99) || !int(v.auto_mp_percent, 0, 99) || !i32(v.hp_item_index) || !i32(v.mp_item_index)) return false;
  const ids = custodyIds([...v.inventory, ...v.equipment]);
  return new Set(ids).size === ids.length;
}
function itemInfo(v: unknown, index: unknown): v is Row {
  if (!row(v) || !i32(v.item_index) || v.item_index !== index || !text(v.name) || !int(v.shape, -32768, 32767)
    || !int(v.bind, -32768, 32767) || !int(v.unique, -32768, 32767) || !int(v.stack_size, 1, 65535)
    || !int(v.image, 0, 65535) || !int(v.durability, 0, 65535) || !int(v.price, 0, 4294967295)) return false;
  for (const k of ["item_type", "grade", "required_type", "required_class", "required_gender", "item_set", "weight", "light",
    "required_amount", "effect", "random_stats_id", "slots"]) if (!int(v[k], 0, 255)) return false;
  for (const k of ["start_item", "need_identify", "show_group_pickup", "class_based", "level_based", "can_mine",
    "global_drop_notify", "can_fast_run", "can_awakening"]) if (typeof v[k] !== "boolean") return false;
  return statBlock(v.stats) !== null && (v.tooltip === null || typeof v.tooltip === "string");
}
function sparseItems(v: unknown, capacity: number, personal = false): (HeroItem | null)[] | null {
  if (!Array.isArray(v) || v.length > capacity) return null;
  const result: (HeroItem | null)[] = Array(capacity).fill(null), ids = new Set<string>();
  for (const raw of v) {
    if (personal && row(raw) && raw.container === "quest") continue;
    if (!row(raw) || !["bag1", "bag2", "belt"].includes(String(raw.container)) || !int(raw.slot, 0, personal ? 39 : capacity - 1)) return null;
    const slot = personal && raw.container === "bag2" ? raw.slot + 40 : raw.slot;
    if ((personal && raw.container === "belt") || slot >= capacity || result[slot] !== null
      || !exactId(raw.uniqueId) || ids.has(String(raw.uniqueId)) || !int(raw.quantity, 1, 65535)
      || !text(raw.name) || !int(raw.icon, 0, 65535) || !row(raw.tooltipSource)
      || !userItem(raw.tooltipSource.userItem) || !sameId(raw.tooltipSource.userItem.unique_id, raw.uniqueId)
      || raw.tooltipSource.userItem.count !== raw.quantity || !itemInfo(raw.tooltipSource.info, raw.tooltipSource.userItem.item_index)
      || (raw.tooltipSource.realInfo != null && (!row(raw.tooltipSource.realInfo)
        || !itemInfo(raw.tooltipSource.realInfo, raw.tooltipSource.realInfo.item_index)))) return null;
    const info = raw.tooltipSource.realInfo ?? raw.tooltipSource.info;
    ids.add(String(raw.uniqueId));
    result[slot] = { slot, uniqueId: raw.uniqueId, itemIndex: raw.tooltipSource.userItem.item_index as number,
      count: raw.quantity, name: raw.name, icon: raw.icon, userItem: raw.tooltipSource.userItem,
      info: info as Row, tooltipSource: raw.tooltipSource };
  }
  return result;
}
function enrich(items: (Row | null)[], catalog: readonly (HeroItem | null)[]): (HeroItem | null)[] | null {
  const result: (HeroItem | null)[] = [];
  for (const [slot, raw] of items.entries()) {
    if (!raw) { result.push(null); continue; }
    const view = catalog.find(v => v !== null && sameId(v.uniqueId, raw.unique_id) && v.itemIndex === raw.item_index);
    if (!view) return null;
    result.push({ ...view, slot, count: raw.count as number, userItem: raw,
      tooltipSource: { ...view.tooltipSource, userItem: raw } });
  }
  return result;
}
function statBlock(v: unknown): { stat: number; value: number }[] | null {
  if (!Array.isArray(v) || v.length > 256 || !v.every(x => row(x) && int(x.stat, 0, 255) && i32(x.value))) return null;
  return new Set(v.map(x => x.stat)).size === v.length ? v as { stat: number; value: number }[] : null;
}
function skillAck(v: unknown): HeroSkillKeyAck | null {
  return row(v) && int(v.requestId, 1) && text(v.spell) && key(v.key) && key(v.oldKey) && typeof v.accepted === "boolean"
    ? v as HeroSkillKeyAck : null;
}

/** Receivers must supply the physical authenticated owner; read uses the stricter current scene owner. */
export class HeroPlayerAuthority {
  private owner: HeroUiOwner | null = null;
  private info: Row | null = null;
  private world: Row | null = null;
  private generation = 0;
  private serial = 0;
  private infoSerial = 0;
  private worldSerial = 0;
  private actorIdentity = "";
  private actorState: { objectId: number; spawned?: boolean; riding?: boolean } | null = null;
  get authoritySerial(): number { return this.serial; }
  receiveInformation(payload: unknown, owner: HeroUiOwner): boolean {
    if (!this.acceptSession(owner)) return false;
    const value = row(payload) && "info" in payload ? payload.info : payload;
    ++this.serial; this.infoSerial = this.serial;
    if (!information(value)) { this.info = null; return false; }
    let cloned: Row;
    try { cloned = copy(value); } catch { this.info = null; return false; }
    const identity = canonical([value.object_id, value.name, value.class, value.gender]);
    if (identity !== this.actorIdentity) {
      if (this.actorIdentity) this.world = null;
      ++this.generation; this.actorIdentity = identity; this.actorState = null;
    }
    this.info = cloned;
    return true;
  }
  receiveSnapshot(payload: unknown, owner: HeroUiOwner): boolean {
    if (!this.acceptSession(owner)) return false;
    ++this.serial; this.worldSerial = this.serial;
    this.owner = { ...owner };
    if (!row(payload) || payload.playerObjectId !== owner.playerObjectId || payload.mapFileName !== owner.mapFileName) {
      this.world = null; return false;
    }
    try { this.world = copy(payload); return true; } catch { this.world = null; return false; }
  }
  receiveActor(packet: string, payload: unknown, owner: HeroUiOwner): boolean {
    if (!this.owner || !sameHeroSession(this.owner, owner) || !this.info || !row(payload)
      || payload.objectId !== this.info.object_id) return false;
    const state = this.actorState ?? { objectId: payload.objectId as number };
    if (packet === "ObjectHero") {
      if (payload.name !== this.info.name || payload.class !== this.info.class || payload.gender !== this.info.gender
        || typeof payload.ridingMount !== "boolean") return false;
      this.actorState = { ...state, spawned: true, riding: payload.ridingMount };
    } else if (packet === "ObjectRemove" || packet === "ObjectDied") this.actorState = { ...state, spawned: false };
    else if (packet === "MountUpdate" && typeof payload.ridingMount === "boolean") this.actorState = { ...state, riding: payload.ridingMount };
    else return false;
    ++this.serial; return true;
  }
  /** Only a different real authenticated session releases cached authority. */
  retireSession(nextOwner: HeroUiOwner): boolean {
    if (!validOwner(nextOwner) || !this.owner || sameHeroSession(this.owner, nextOwner)) return false;
    this.owner = { ...nextOwner }; this.info = this.world = null; this.actorIdentity = ""; this.actorState = null;
    this.infoSerial = this.worldSerial = 0; ++this.generation; ++this.serial;
    return true;
  }
  private acceptSession(owner: HeroUiOwner): boolean {
    if (!validOwner(owner)) return false;
    if (!this.owner) this.owner = { ...owner };
    return sameHeroSession(this.owner, owner);
  }
  read(owner: HeroUiOwner): HeroPlayerModel | null {
    try {
      const h = this.info, w = this.world;
      if (!h || !w || !this.owner || !sameHeroOwner(this.owner, owner) || !row(w.stage5Systems) || !row(w.stage5Systems.hero)) return null;
      const stage = w.stage5Systems.hero;
      if (stage.name !== h.name || stage.class !== h.class || stage.gender !== h.gender || stage.spawned !== true
        || !int(stage.level, 0, 65535) || !capacities.includes(w.heroInventoryCapacity as number)) return null;
      if (this.actorState?.spawned === false) return null;
      const inv = sparseItems(w.heroInventoryItems, w.heroInventoryCapacity as number);
      const gear = sparseItems(w.heroEquipmentItems, 14);
      const stats = statBlock(w.heroStats), vitals = w.heroVitals, weights = w.heroWeights;
      if (!inv || !gear || !stats || !row(vitals) || !i32(vitals.hp) || !i32(vitals.mp)
        || !row(weights) || ![weights.bag, weights.wear, weights.hand].every(i32)) return null;
      let inventory = inv, equipment = gear;
      const infoNewer = this.infoSerial > this.worldSerial;
      const snapshotExperience = Object.hasOwn(w, "heroMaxExperience");
      const experience = infoNewer || !snapshotExperience ? h.experience : stage.experience;
      const maxExperience = infoNewer || !snapshotExperience ? h.max_experience : w.heroMaxExperience;
      if (!exactExperience(experience) || !exactExperience(maxExperience)) return null;
      if (infoNewer) {
        if (h.level !== stage.level) return null;
        const nextInv = enrich(h.inventory as (Row | null)[], [...inv, ...gear]);
        const nextGear = enrich(h.equipment as (Row | null)[], [...inv, ...gear]);
        if (!nextInv || !nextGear) return null;
        inventory = nextInv; equipment = nextGear;
      }
      const ids = custodyIds([...inventory, ...equipment]);
      if (new Set(ids).size !== ids.length) return null;
      const learned = w.stage5Systems.heroLearnedMagics;
      if (!Array.isArray(learned) || learned.length > 256 || !learned.every(m => row(m) && text(m.spell) && key(m.key)
        && int(m.level, 0, 255) && int(m.experience, 0, 65535))
        || new Set(learned.map(m => m.spell)).size !== learned.length) return null;
      const magics: HeroMagic[] = [];
      for (const raw of h.magics as Row[]) {
        const current = learned.find(m => m.spell === raw.spell);
        if (!current) return null;
        magics.push({ spell: raw.spell as string, name: raw.name as string, icon: raw.icon as number,
          key: infoNewer ? raw.key as number : current.key, level: infoNewer ? raw.level as number : current.level, raw });
      }
      if (learned.length !== magics.length || new Set(magics.filter(m => m.key !== 0).map(m => m.key)).size !== magics.filter(m => m.key !== 0).length) return null;
      let personal: (HeroItem | null)[] | null = null;
      if (int(w.maxBagSlots, 0, 80) && int(w.inventoryCapacity, 46, 86)
        && (w.inventoryCapacity === 46 || (w.inventoryCapacity >= 54 && (w.inventoryCapacity - 54) % 4 === 0))
        && w.maxBagSlots === w.inventoryCapacity - 6) personal = sparseItems(w.inventoryItems, w.maxBagSlots, true);
      if (personal) {
        const personalIds = custodyIds(personal);
        if (new Set(personalIds).size !== personalIds.length || personalIds.some(id => ids.includes(id))) personal = null;
      }
      const config = infoNewer ? h : stage;
      const autoPot = config[infoNewer ? "auto_pot" : "autoPot"];
      const hpPercent = config[infoNewer ? "auto_hp_percent" : "autoHpPercent"];
      const mpPercent = config[infoNewer ? "auto_mp_percent" : "autoMpPercent"];
      const hpItemIndex = config[infoNewer ? "hp_item_index" : "hpItemIndex"];
      const mpItemIndex = config[infoNewer ? "mp_item_index" : "mpItemIndex"];
      if (typeof autoPot !== "boolean" || !int(hpPercent, 0, 99) || !int(mpPercent, 0, 99) || !i32(hpItemIndex) || !i32(mpItemIndex)) return null;
      const actor: HeroActor = { objectId: h.object_id as number, name: h.name as string,
        class: h.class as string, gender: h.gender as string, generation: this.generation };
      const data = { actor, inventory, equipment, personalInventory: personal, magics,
        hp: infoNewer ? h.hp as number : vitals.hp, mp: infoNewer ? h.mp as number : vitals.mp,
        level: infoNewer ? h.level as number : stage.level, experience,
        maxExperience, spawned: true, riding: this.actorState?.riding ?? null,
        autoPot, hpPercent, mpPercent, hpItemIndex, mpItemIndex, stats, weights: weights as { bag: number; wear: number; hand: number } };
      const sourceKey = canonical(data);
      // Preserve socket identity; never freeze an external transport object.
      const model = freeze({ ...data, sourceKey, authoritySerial: this.serial, informationSerial: this.infoSerial,
        snapshotSerial: this.worldSerial, personalSerial: personal ? this.worldSerial : 0,
        skillSnapshotSerial: this.worldSerial, inventoryCapacity: inventory.length, skillKeyAck: skillAck(w.skillKeyAck) });
      return Object.freeze({ ...model, owner: Object.freeze({ ...owner }) });
    } catch { return null; }
  }
}

function at(model: HeroPlayerModel, cell: HeroCell): HeroItem | null {
  if (!["HeroInventory", "HeroEquipment", "Inventory"].includes(cell.grid)) return null;
  const list = cell.grid === "HeroInventory" ? model.inventory : cell.grid === "HeroEquipment" ? model.equipment : model.personalInventory;
  return list && int(cell.slot, 0, list.length - 1) ? list[cell.slot] : null;
}
function available(model: HeroPlayerModel, cell: HeroCell): boolean {
  if (!["HeroInventory", "HeroEquipment", "Inventory"].includes(cell.grid)) return false;
  const list = cell.grid === "HeroInventory" ? model.inventory : cell.grid === "HeroEquipment" ? model.equipment : model.personalInventory;
  return !!list && int(cell.slot, 0, list.length - 1);
}
export function heroRequirements(model: HeroPlayerModel, item: HeroItem): boolean {
  const i = item.info;
  if (!((i.required_class as number) & (1 << classes.indexOf(model.actor.class)))
    || !((i.required_gender as number) & (1 << genders.indexOf(model.actor.gender)))) return false;
  const amount = i.required_amount as number, type = i.required_type as number;
  if (type === 0) return model.level >= amount;
  if (type === 6) return model.level <= amount;
  const stat = ({ 1: 1, 2: 3, 3: 5, 4: 7, 5: 9, 7: 0, 8: 2, 9: 4, 10: 6, 11: 8 } as Record<number, number>)[type];
  return stat !== undefined && (model.stats.find(s => s.stat === stat)?.value ?? 0) >= amount;
}
function equipTarget(model: HeroPlayerModel, item: HeroItem): number | null {
  const type = item.info.item_type as number;
  if (type === 6) return !model.equipment[6] || (model.equipment[6]?.tooltipSource.info as Row | undefined)?.item_type === 8 ? 6 : 5;
  if (type === 7) return !model.equipment[8] ? 8 : 7;
  return ({ 1: 0, 2: 1, 4: 2, 5: 4, 8: 9, 9: 10, 10: 11, 11: 12, 12: 3, 19: 13 } as Record<number, number>)[type] ?? null;
}
function canEquip(model: HeroPlayerModel, item: HeroItem, to: number): boolean {
  const type = item.info.item_type as number;
  const targets = type === 6 ? [5, 6] : type === 7 ? [7, 8] : [equipTarget(model, item)];
  return model.hp > 0 && !(model.riding === true && type !== 12) && targets.includes(to) && heroRequirements(model, item);
}
function mergePlan(model: HeroPlayerModel, from: HeroCell, to: HeroCell): HeroActionPlan | null {
  if (from.grid === "Inventory" && to.grid === "Inventory") return null;
  if ([from.grid, to.grid].includes("Inventory") && [from.grid, to.grid].includes("HeroEquipment")) return null;
  const a = at(model, from), b = at(model, to);
  if (!a || !b || !int(a.uniqueId, 1) || !int(b.uniqueId, 1) || a.uniqueId === b.uniqueId || a.itemIndex !== b.itemIndex || b.count >= ((b.tooltipSource.info as Row).stack_size as number)) return null;
  return { wire: { type: "mergeItem", gridFrom: from.grid, gridTo: to.grid, idFrom: a.uniqueId, idTo: b.uniqueId },
    confirmationRequired: false, crossPlayer: from.grid === "Inventory" || to.grid === "Inventory" };
}
/** Concrete reachable commands only; unsupported attachments and manual casts have no plan. */
export function planHeroAction(model: HeroPlayerModel, action: HeroUiAction): HeroActionPlan | null {
  if (!model.spawned) return null;
  // Legacy mutation transport still requires numeric instance IDs. Exact text
  // remains visible, but never becomes a rounded or invalid legacy command.
  const result = (wire: HeroWire, confirmationRequired = false, crossPlayer = false): HeroActionPlan | null =>
    ["uniqueId", "idFrom", "idTo"].some(field => field in wire && !int(wire[field], 1))
      ? null : ({ wire, confirmationRequired, crossPlayer });
  switch (action.kind) {
    case "move": {
      if (action.from === action.to || !at(model, { grid: "HeroInventory", slot: action.from }) || !available(model, { grid: "HeroInventory", slot: action.to })) return null;
      return result({ type: "moveItem", grid: "HeroInventory", from: action.from, to: action.to });
    }
    case "merge": return mergePlan(model, action.from, action.to);
    case "equip": {
      const item = at(model, { grid: "HeroInventory", slot: action.from });
      return item && int(action.to, 0, 13) && canEquip(model, item, action.to)
        ? result({ type: "equipItem", grid: "HeroInventory", uniqueId: item.uniqueId, to: action.to }) : null;
    }
    case "remove": {
      const item = at(model, { grid: "HeroEquipment", slot: action.from });
      if (!item || model.hp <= 0 || (model.riding === true && action.from !== 3)) return null;
      const to = action.to ?? [...Array.from({ length: Math.max(0, model.inventory.length - 2) }, (_, n) => n + 2), 0, 1].find(n => !model.inventory[n]);
      return to !== undefined && available(model, { grid: "HeroInventory", slot: to }) && !model.inventory[to]
        ? result({ type: "removeItem", grid: "HeroInventory", uniqueId: item.uniqueId, to }) : null;
    }
    case "transfer": case "takeBack": {
      const from: HeroCell = { grid: action.kind === "transfer" ? "Inventory" : "HeroInventory", slot: action.from };
      const to: HeroCell = { grid: action.kind === "transfer" ? "HeroInventory" : "Inventory", slot: action.to };
      if (!at(model, from) || !available(model, to)) return null;
      if (at(model, to)) return mergePlan(model, from, to);
      return result({ type: action.kind === "transfer" ? "transferHeroItem" : "takeBackHeroItem", from: action.from, to: action.to }, false, true);
    }
    case "use": {
      const item = at(model, { grid: "HeroInventory", slot: action.slot });
      if (!item || !heroRequirements(model, item)) return null;
      const target = equipTarget(model, item);
      if (target !== null) {
        if (!canEquip(model, item, target)) return null;
        if (item.info.item_type === 8 && model.equipment[target]?.itemIndex === item.info.item_index) {
          const merge = mergePlan(model, { grid: "HeroInventory", slot: action.slot }, { grid: "HeroEquipment", slot: target });
          if (merge) return merge;
        }
        return result({ type: "equipItem", grid: "HeroInventory", uniqueId: item.uniqueId, to: target });
      }
      if (![13, 17, 20, 21, 27, 36, 37, 38, 40, 42].includes(item.info.item_type as number)) return null;
      return result({ type: "useItem", grid: "HeroInventory", uniqueId: item.uniqueId }, item.info.item_type === 13 && item.info.shape === 4 && action.confirmed !== true);
    }
    case "autoPotValue":
      return model.autoPot && [12, 13].includes(action.stat) && int(action.value, 0, 99)
        ? result({ type: "setAutoPotValue", stat: action.stat, value: action.value }) : null;
    case "autoPotItem": {
      if (!model.autoPot || !["HeroHpItem", "HeroMpItem"].includes(action.grid)) return null;
      const item = action.slot === null ? null : at(model, { grid: "HeroInventory", slot: action.slot });
      if (action.slot !== null && (!item || model.hp <= 0 || (item.tooltipSource.info as Row).item_type !== 13 || ((item.tooltipSource.info as Row).shape as number) > 1)) return null;
      return result({ type: "setAutoPotItem", grid: action.grid, itemIndex: item?.itemIndex ?? 0 });
    }
    case "magicKey": {
      const spell = model.magics.find(m => m.spell === action.spell);
      if (model.hp <= 0 || !spell || !key(action.key) || (action.key === 0 && spell.key === 0)) return null;
      return result({ type: "magicKey", spell: spell.spell, key: action.key, oldKey: spell.key });
    }
  }
  return null;
}
export function captureHeroAction(model: HeroPlayerModel, action: HeroUiAction): HeroActionDto | null {
  if (!planHeroAction(model, action)) return null;
  return Object.freeze({ actor: model.actor, owner: model.owner, sourceKey: model.sourceKey, action: freeze(copy(action)) });
}
export function heroActionCurrent(dto: HeroActionDto, model: HeroPlayerModel): boolean {
  return sameHeroOwner(dto.owner, model.owner) && sameActor(dto.actor, model.actor) && dto.sourceKey === model.sourceKey;
}
export function heroRestockCandidate(model: HeroPlayerModel, belt: number): HeroRestockCandidate | null {
  const item = int(belt, 0, 1) ? model.inventory[belt] : null;
  if (!item || item.count !== 1 || !int(item.uniqueId, 1)) return null;
  const source = model.inventory.find(i => i && i.slot >= 2 && i.itemIndex === item.itemIndex && int(i.uniqueId, 1));
  return source && int(source.uniqueId, 1) ? Object.freeze({ actor: model.actor, owner: model.owner, belt, from: source.slot, uniqueId: source.uniqueId, itemIndex: source.itemIndex }) : null;
}
/** Explicit subsequent action only; observing consumption never automatically sends a restock. */
export function heroRestockAction(candidate: HeroRestockCandidate, model: HeroPlayerModel): HeroActionDto | null {
  const source = model.inventory[candidate.from];
  return sameHeroOwner(candidate.owner, model.owner) && sameActor(candidate.actor, model.actor) && !model.inventory[candidate.belt]
    && source?.uniqueId === candidate.uniqueId && source.itemIndex === candidate.itemIndex
    ? captureHeroAction(model, { kind: "move", from: candidate.from, to: candidate.belt }) : null;
}

let nextRequest = 1;
/** Shared by every Hero ledger in this JS realm; never resets on close/map changes. */
export function allocateHeroRequestId(): number | null {
  return Number.isSafeInteger(nextRequest) ? nextRequest++ : null;
}
export type HeroOperationProof = Readonly<{
  id: number; dto: HeroActionDto; wire: HeroWire; crossPlayer: boolean;
  beforeSerial: number; beforeSkillSerial: number;
}>;
export type HeroOperationPending = Readonly<{ proof: HeroOperationProof; state: "reserved" | "entered" | "unknown" | "acknowledged"; ackSerial: number | null; success: boolean | null }>;
export class HeroPlayerOperations {
  private value: HeroOperationPending | null = null;
  private lastSerial = 0;
  private restock: Readonly<{ useId: number; candidate: HeroRestockCandidate; ready: boolean }> | null = null;
  get pending(): HeroOperationPending | null { return this.value; }
  /** Intent belongs to the accepted belt use, independently of window visibility. */
  get restockCandidate(): HeroRestockCandidate | null { return this.restock?.candidate ?? null; }
  /** Host-triggered successor, never a replay of the use or an entered move. */
  reserveRestock(model: HeroPlayerModel): HeroOperationProof | null {
    const intent = this.restock;
    if (this.value || !intent?.ready) return null;
    const candidate = intent.candidate;
    // Rebase only the scene of the same physical owner. The fresh DTO still uses
    // the current complete model and passes the ordinary last-claim source gate.
    const dto = sameHeroSession(candidate.owner, model.owner) && sameActor(candidate.actor, model.actor)
      ? heroRestockAction({ ...candidate, owner: model.owner }, model) : null;
    this.restock = null;
    return dto ? this.reserve(model, dto) : null;
  }
  reserve(model: HeroPlayerModel, action: HeroUiAction | HeroActionDto): HeroOperationProof | null {
    if (this.value) return null;
    const dto = "action" in action ? action : captureHeroAction(model, action);
    if (!dto || !heroActionCurrent(dto, model)) return null;
    const plan = planHeroAction(model, dto.action), id = allocateHeroRequestId();
    if (!plan || plan.confirmationRequired || id === null) return null;
    const wire = freeze({ ...plan.wire, ...(plan.wire.type === "magicKey" ? { requestId: id } : {}) });
    const proof = Object.freeze({ id, dto, wire, crossPlayer: plan.crossPlayer, beforeSerial: model.authoritySerial, beforeSkillSerial: model.skillSnapshotSerial });
    this.lastSerial = model.authoritySerial;
    this.value = Object.freeze({ proof, state: "reserved", ackSerial: null, success: null });
    return proof;
  }
  allows(proof: HeroOperationProof, model: HeroPlayerModel, wire: unknown = proof.wire): boolean {
    if (this.value?.proof !== proof || this.value.state !== "reserved" || !heroActionCurrent(proof.dto, model)) return false;
    try { return canonical(wire) === canonical(proof.wire) && !!planHeroAction(model, proof.dto.action); } catch { return false; }
  }
  claim(proof: HeroOperationProof, model: HeroPlayerModel, wire: unknown = proof.wire): boolean {
    if (!this.allows(proof, model, wire)) return false;
    if (proof.wire.type === "useItem" && proof.dto.action.kind === "use") {
      const candidate = heroRestockCandidate(model, proof.dto.action.slot);
      this.restock = candidate ? Object.freeze({ useId: proof.id, candidate, ready: false }) : null;
    }
    this.value = Object.freeze({ ...this.value!, state: "entered" }); return true;
  }
  cancelDefinitelyUnsent(proof: HeroOperationProof): boolean {
    if (this.value?.proof !== proof || this.value.state !== "reserved") return false;
    this.value = null; return true;
  }
  outcomeUnknown(proof: HeroOperationProof): boolean {
    if (this.value?.proof !== proof || this.value.state !== "entered") return false;
    this.value = Object.freeze({ ...this.value, state: "unknown" }); return true;
  }
  /** Pass the receiver's latest authority serial even when strict UI read is unavailable. */
  receipt(packet: string, payload: unknown, owner: HeroUiOwner, authoritySerial: number): boolean {
    const pending = this.value;
    if (!pending || !["entered", "unknown"].includes(pending.state) || !sameHeroSession(pending.proof.dto.owner, owner)
      || !int(authoritySerial) || !row(payload)) return false;
    const w = pending.proof.wire;
    const expected = ({ moveItem: "MoveItem", equipItem: "EquipItem", removeItem: "RemoveItem", mergeItem: "MergeItem",
      transferHeroItem: "TransferHeroItem", takeBackHeroItem: "TakeBackHeroItem", useItem: "UseItem",
      setAutoPotValue: "SetAutoPotValue", setAutoPotItem: "SetAutoPotItem" } as Record<string, string>)[String(w.type)];
    if (packet !== expected) return false;
    const fields = Object.keys(w).filter(k => k !== "type");
    for (const field of fields) {
      const actual = field === "grid" && packet === "SetAutoPotItem"
        ? payload.grid === (w.grid === "HeroHpItem" ? 23 : 24) ? w.grid : null
        : field === "itemIndex" ? payload.itemIndex ?? payload.item_index : payload[field];
      if (actual !== w[field]) return false;
    }
    const config = packet === "SetAutoPotValue" || packet === "SetAutoPotItem";
    if (!config && typeof payload.success !== "boolean") return false;
    if (this.restock?.useId === pending.proof.id && payload.success === false) this.restock = null;
    this.value = Object.freeze({ ...pending, state: "acknowledged", success: config || payload.success === true,
      ackSerial: Math.max(authoritySerial, this.lastSerial, pending.proof.beforeSerial) });
    return true;
  }
  observe(model: HeroPlayerModel): boolean {
    this.lastSerial = Math.max(this.lastSerial, model.authoritySerial);
    if (this.restock) {
      const candidate = this.restock.candidate, source = model.inventory[candidate.from];
      if (!sameHeroSession(candidate.owner, model.owner) || !sameActor(candidate.actor, model.actor)
        || !source || source.uniqueId !== candidate.uniqueId || source.itemIndex !== candidate.itemIndex) this.restock = null;
    }
    const p = this.value;
    if (!p || !sameHeroSession(p.proof.dto.owner, model.owner) || !sameActor(p.proof.dto.actor, model.actor)) return false;
    const w = p.proof.wire;
    if (w.type === "magicKey" && ["entered", "unknown"].includes(p.state)) {
      const ack = model.skillKeyAck, spell = model.magics.find(m => m.spell === w.spell);
      if (model.skillSnapshotSerial <= p.proof.beforeSkillSerial || !ack || !spell || ack.requestId !== w.requestId
        || ack.spell !== w.spell || ack.key !== w.key || ack.oldKey !== w.oldKey) return false;
      if (ack.accepted && spell.key !== w.key) return false;
      if (!ack.accepted && spell.key !== w.oldKey) return false;
      this.value = null; return true;
    }
    if (p.state !== "acknowledged" || p.ackSerial === null
      || Math.max(model.informationSerial, model.snapshotSerial) <= p.ackSerial
      || (p.proof.crossPlayer && model.personalSerial <= p.ackSerial)) return false;
    if (this.restock?.useId === p.proof.id) {
      const candidate = this.restock.candidate;
      const ready = p.success === true && !!heroRestockAction({ ...candidate, owner: model.owner }, model);
      this.restock = ready ? Object.freeze({ ...this.restock, ready: true }) : null;
    }
    this.value = null; return true;
  }
  /** Losing focus, scene, window, or in-game status is deliberately not a retirement event. */
  retireSession(nextOwner: HeroUiOwner): boolean {
    const owner = this.value?.proof.dto.owner ?? this.restock?.candidate.owner;
    if (!validOwner(nextOwner) || !owner || sameHeroSession(owner, nextOwner)) return false;
    this.value = null; this.restock = null; this.lastSerial = 0; return true;
  }
}
