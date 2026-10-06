// Shared helpers for the [fe-packets] extended server->client packet handlers.
//
// These are pure, framework-free utilities that normalise the JSON payload
// shapes emitted by the gateway (apps/gateway/src/web.rs `server_packet_to_event`)
// into the lightweight value objects consumed by the web client's WorldState.
//
// Field-name notes (derived from the gateway serializer + protocol serde attrs):
//   * Enum-level variant fields are camelCase (#[serde(rename_all_fields)]).
//   * `UserItem` has no rename_all, so its fields arrive snake_case
//     (`unique_id`, `item_index`, `current_dura`, `max_dura`, `count`).
//   * `ClientFriend` / `ClientMail` use rename_all = "camelCase".

export type PacketRecord = Record<string, unknown>;

export function packetNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

export function packetString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

export function packetBool(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

/** Normalised view of a protocol `UserItem` payload. */
export type NormalizedUserItem = {
  uniqueId: number;
  itemIndex?: number;
  currentDura?: number;
  maxDura?: number;
  count: number;
};

/**
 * Reads a `UserItem` payload tolerating both snake_case (the wire shape) and
 * camelCase (in case a future serializer change normalises field names).
 */
export function normalizeUserItem(raw: unknown): NormalizedUserItem | null {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const record = raw as PacketRecord;
  const uniqueId =
    packetNumber(record.uniqueId) ?? packetNumber(record.unique_id);
  if (typeof uniqueId !== "number") {
    return null;
  }
  return {
    uniqueId,
    itemIndex: packetNumber(record.itemIndex) ?? packetNumber(record.item_index),
    currentDura:
      packetNumber(record.currentDura) ?? packetNumber(record.current_dura),
    maxDura: packetNumber(record.maxDura) ?? packetNumber(record.max_dura),
    count: packetNumber(record.count) ?? 1,
  };
}

/** Minimal item shape the client tracks inside its bag/belt/storage lists. */
export type ExtendedWorldItemPatch = {
  uniqueId: number;
  quantity?: number;
  durabilityCurrent?: number;
  durabilityMax?: number;
  name?: string;
  icon?: number;
};

/**
 * Applies a durability/quantity patch to every item in a list that matches the
 * given uniqueId. Returns the original list reference when nothing matched so
 * callers can avoid needless React state churn.
 */
export function patchItemsByUniqueId<
  T extends {
    uniqueId: number;
    quantity: number;
    durabilityCurrent?: number;
    durabilityMax?: number;
    name: string;
  },
>(items: T[], patch: ExtendedWorldItemPatch): T[] {
  let changed = false;
  const next = items.map((item) => {
    if (item.uniqueId !== patch.uniqueId) {
      return item;
    }
    changed = true;
    return {
      ...item,
      quantity: patch.quantity ?? item.quantity,
      durabilityCurrent: patch.durabilityCurrent ?? item.durabilityCurrent,
      durabilityMax: patch.durabilityMax ?? item.durabilityMax,
      name: patch.name ?? item.name,
    };
  });
  return changed ? next : items;
}

/** Removes (or decrements) an item across a list by uniqueId. */
export function removeItemByUniqueId<
  T extends { uniqueId: number; quantity: number },
>(items: T[], uniqueId: number, count: number): T[] {
  let changed = false;
  const next = items.flatMap((item) => {
    if (changed || item.uniqueId !== uniqueId) {
      return [item];
    }
    changed = true;
    if (count > 0 && item.quantity > count) {
      return [{ ...item, quantity: item.quantity - count }];
    }
    return [];
  });
  return changed ? next : items;
}

/** Normalised friend record for the social panel. */
export type NormalizedFriend = {
  index?: number;
  name: string;
  memo: string;
  blocked: boolean;
  online: boolean;
};

export function normalizeFriendList(raw: unknown): NormalizedFriend[] {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const record = entry as PacketRecord;
    const name = packetString(record.name);
    if (!name) {
      return [];
    }
    return [
      {
        ...(typeof record.index === "number" && Number.isSafeInteger(record.index) && record.index >= 0 && record.index <= 0x7fff_ffff ? {index: record.index} : {}),
        name,
        memo: packetString(record.memo) ?? "",
        blocked: packetBool(record.blocked) ?? false,
        online: packetBool(record.online) ?? false,
      },
    ];
  });
}

export type NormalizedMailAttachment = {uniqueId:number|null;itemIndex:number|null;name:string|null;key:string|null;
  count:number;currentDura:number;maxDura:number;soulBoundId:number;identified:boolean|null;cursed:boolean;gemCount:number};
export type NormalizedMail = {mailId:number;senderName:string;message:string;subject:string;opened:boolean;locked:boolean;
  canReply:boolean;collected:boolean;gold:number;items:NormalizedMailAttachment[];itemCount:number;
  dateSentBinaryDatetime:string|null;metadataKnown:boolean};
const mailObject=(v:unknown):v is Record<string,unknown>=>Boolean(v)&&typeof v==="object"&&!Array.isArray(v);
const mailInt=(v:unknown,min:number,max:number):v is number=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=min&&v<=max;
/** ClientMail and the existing Stage5 bootstrap use one loss-aware representation.
 * Unsafe numeric identities cannot be recovered by converting the rounded number to text. */
export function parseMailList(raw:unknown):NormalizedMail[]|null {
  if(!Array.isArray(raw)||raw.length>256)return null;
  const ids=new Set<number>(),out:NormalizedMail[]=[];
  for(const entry of raw){
    if(!mailObject(entry))return null;
    if(entry.deleted===true)continue;
    const id=entry.mailId??entry.mail_id??entry.id;
    if(!mailInt(id,1,Number.MAX_SAFE_INTEGER)||ids.has(id))return null;ids.add(id);
    const sender=entry.senderName??entry.sender_name??entry.sender??entry.from??"",subjectSource=entry.subject??"",bodySource=entry.body??"";
    const body=entry.message??(typeof subjectSource==="string"&&subjectSource&&typeof bodySource==="string"?bodySource?`${subjectSource}\n${bodySource}`:subjectSource:bodySource);
    if(typeof sender!=="string"||typeof body!=="string"||sender.length>256||body.length>65536)return null;
    let sourceItems:unknown=entry.items??[];
    const states=entry.itemStatesJson??entry.item_states_json;
    if(states!==undefined){
      if(!Array.isArray(states)||states.length>5)return null;
      if(states.length){const decoded=[];for(const rawState of states){
        if(typeof rawState!=="string"||rawState.length>262144)return null;
        let state:unknown;try{state=JSON.parse(rawState);}catch{return null;}
        if(!mailObject(state)||!mailInt(state.unique_id,1,Number.MAX_SAFE_INTEGER)||typeof state.name!=="string"||typeof state.key!=="string")return null;
        const metadata=state.user_item_metadata;if(metadata!==undefined&&metadata!==null&&!mailObject(metadata))return null;
        decoded.push({unique_id:state.unique_id,item_index:mailObject(metadata)?metadata.item_index??null:null,name:state.name,key:state.key,count:state.quantity,
          current_dura:state.durability_current??0,max_dura:state.durability_max??0,soul_bound_id:state.soul_bound_id??-1,gem_count:state.gem_count??0,
          identified:typeof state.identified==="boolean"?state.identified:null,cursed:state.cursed===true});
      }sourceItems=decoded;}
    }
    if(!Array.isArray(sourceItems)||sourceItems.length>5)return null;
    const items:NormalizedMailAttachment[]=[];
    for(const item of sourceItems){
      if(typeof item==="string"){if(item.length>512)return null;items.push({uniqueId:null,itemIndex:null,name:item,key:item,count:1,currentDura:0,maxDura:0,soulBoundId:0,identified:false,cursed:false,gemCount:0});continue;}
      if(!mailObject(item))return null;
      const uid=item.unique_id??item.uniqueId??null,index=item.item_index??item.itemIndex??null;
      if(uid!==null&&!mailInt(uid,1,Number.MAX_SAFE_INTEGER)||index!==null&&!mailInt(index,-2147483648,2147483647))return null;
      const count=item.count??0,current=item.current_dura??item.currentDura??0,max=item.max_dura??item.maxDura??0,
        soul=item.soul_bound_id??item.soulBoundId??0,gem=item.gem_count??item.gemCount??0;
      if(![count,current,max,gem].every(v=>mailInt(v,0,65535))||!mailInt(soul,-2147483648,2147483647))return null;
      const name=item.name??null,key=item.key??null;
      if(name!==null&&(typeof name!=="string"||name.length>512)||key!==null&&(typeof key!=="string"||key.length>512))return null;
      items.push({uniqueId:uid as number|null,itemIndex:index as number|null,name:name as string|null,key:key as string|null,count:count as number,
        currentDura:current as number,maxDura:max as number,soulBoundId:soul,gemCount:gem as number,identified:typeof item.identified==="boolean"?item.identified:null,cursed:item.cursed===true});
    }
    const gold=entry.gold??0;if(!mailInt(gold,0,0xffffffff))return null;
    const date=entry.dateSentBinaryDatetime??entry.date_sent_binary_datetime;
    // Decimal text is lossless only when it was received as text. A JS-safe integer is also exact.
    let dateText:string|null=null;
    if(typeof date==="string"&&/^-?\d{1,19}$/.test(date)){try{const n=BigInt(date);if(n>=BigInt("-9223372036854775808")&&n<=BigInt("9223372036854775807"))dateText=date;}catch{/* unknown */}}
    else if(mailInt(date,Number.MIN_SAFE_INTEGER,Number.MAX_SAFE_INTEGER))dateText=String(date);
    const reply=entry.canReply??entry.can_reply;
    const subject=entry.subject??"";if(typeof subject!=="string"||subject.length>4096)return null;
    out.push({mailId:id,senderName:sender,message:body,subject,opened:(entry.opened??entry.read)===true,locked:entry.locked===true,
      canReply:reply===true,collected:(entry.collected??entry.claimed)===true,gold,items,itemCount:items.length,dateSentBinaryDatetime:dateText,
      metadataKnown:entry.metadataKnown!==false&&typeof reply==="boolean"&&dateText!==null});
  }
  return out;
}
/** The caller must qualify the same connection/session/player before providing previous rows.
 * Only absent Stage5 metadata is retained; authoritative flags/items always come from this payload. */
export type MailRowsResolver=(json:string)=>string|null|undefined;
/** A nullable flag is a CURRENT template default. Only the Rust catalogue facade can resolve it. */
export function mergeMailList(raw:unknown,previous:unknown,resolver?:MailRowsResolver):NormalizedMail[]|null {
  let next=parseMailList(raw),old=parseMailList(previous);if(!next)return null;
  if(resolver){try{
    const result=resolver(JSON.stringify(next));if(typeof result!=="string")return null;
    const resolved=parseMailList(JSON.parse(result));
    if(!resolved||resolved.length!==next.length||resolved.some((m,n)=>m.mailId!==next![n].mailId))return null;
    next=resolved;
    if(old){const prior=resolver(JSON.stringify(old));old=typeof prior==="string"?parseMailList(JSON.parse(prior)):null;}
  }catch{return null;}}
  if(!old)return next;
  const stableAttachment=(a:NormalizedMailAttachment,b:NormalizedMailAttachment)=>a.uniqueId===b.uniqueId
    &&(a.uniqueId!==null||a.name===b.name&&a.key===b.key)&&a.itemIndex!==null&&a.itemIndex===b.itemIndex
    &&a.identified!==null&&a.identified===b.identified&&a.count===b.count&&a.currentDura===b.currentDura
    &&a.maxDura===b.maxDura&&a.soulBoundId===b.soulBoundId&&a.gemCount===b.gemCount&&a.cursed===b.cursed
    // Before actual catalogue resolution a changed canonical key cannot be hidden by the old explicit index.
    &&(Boolean(resolver)||a.key===b.key||!a.key&&!b.key);
  return next.map(row=>{const source=(raw as unknown[]).find(v=>mailObject(v)&&(v.mailId??v.mail_id??v.id)===row.mailId) as Record<string,unknown>|undefined;
    const prior=old.find(m=>m.mailId===row.mailId&&m.senderName===row.senderName&&m.message===row.message&&m.gold===row.gold&&m.items.length===row.items.length
      &&m.items.every((i,n)=>stableAttachment(i,row.items[n])));
    if(!prior||!source||!prior.metadataKnown||Object.hasOwn(source,"canReply")||Object.hasOwn(source,"can_reply")||Object.hasOwn(source,"dateSentBinaryDatetime")||Object.hasOwn(source,"date_sent_binary_datetime"))return row;
    return{...row,canReply:prior.canReply,dateSentBinaryDatetime:prior.dateSentBinaryDatetime,metadataKnown:true};
  });
}
/** Invalid current payload withdraws the mailbox, rather than retaining stale actionable rows. */
export function normalizeMailList(raw:unknown):Array<Record<string,unknown>> {return parseMailList(raw)??[];}

/** Recover only the single-field UserItemExpireInfo carrier. The value remains
 * JSON data; interpretation and validation belong to the shared Rust codec. */
function gatewayItemExpiryReviver(this:Record<string,unknown>,key:string,value:unknown,context?:{source?:string}):unknown {
  if(key!=="expiry_binary_datetime"||!mailObject(this)||Object.keys(this).length!==1||!Object.hasOwn(this,key)
    ||typeof value!=="number"||Number.isSafeInteger(value))return value;
  const source=context?.source;
  if(typeof source!=="string"||!/^(?:0|[1-9]\d{0,18}|-[1-9]\d{0,18})$/.test(source))return null;
  try{const n=BigInt(source);return n>=-9223372036854775808n&&n<=9223372036854775807n?source:null;}catch{return null;}
}

/** Preserve only the three i64 values in a complete creature-shaped carrier.
 * An unsafe identity elsewhere remains a Number and fails its normal validator. */
function gatewayCreatureTimeReviver(this:Record<string,unknown>,key:string,value:unknown,context?:{source?:string}):unknown {
  if(!["expireBinaryDatetime","blackstoneTime","maintainFoodTime"].includes(key)
    ||!mailObject(this)||!mailInt(this.petType,0,255)||!mailInt(this.slotIndex,0,9)
    ||typeof this.customName!=="string"||!mailObject(this.creatureRules)||!mailObject(this.filter)
    ||typeof value!=="number"||Number.isSafeInteger(value))return value;
  const source=context?.source;
  if(typeof source!=="string"||!/^(?:0|[1-9]\d{0,18}|-[1-9]\d{0,18})$/.test(source))return null;
  try{const n=BigInt(source);return n>=-9223372036854775808n&&n<=9223372036854775807n?source:null;}catch{return null;}
}

function gatewayGameShopDateReviver(this:Record<string,unknown>,key:string,value:unknown,context?:{source?:string}):unknown {
  if(key!=="date_binary_datetime"||!mailObject(this)||!mailInt(this.g_index,0,2147483647)
    ||!mailInt(this.item_index,0,2147483647)||!mailObject(this.info)||typeof this.can_buy_gold!=="boolean"
    ||typeof this.can_buy_credit!=="boolean"||typeof value!=="number"||Number.isSafeInteger(value))return value;
  const source=context?.source;
  if(typeof source!=="string"||!/^(?:0|[1-9]\d{0,18}|-[1-9]\d{0,18})$/.test(source))return null;
  try{const n=BigInt(source);return n>=-9223372036854775808n&&n<=9223372036854775807n?source:null;}catch{return null;}
}

/** Preserve plain item expiry on every packet and the existing ReceiveMail date.
 * Without reviver source support, an unsafe plain expiry becomes unknown. */
export function parseGatewayMailDates(text:string):unknown {
  const parse=JSON.parse as (text:string,reviver:(this:Record<string,unknown>,key:string,value:unknown,context?:{source?:string})=>unknown)=>unknown;
  const parsed:unknown=parse(text,function(key,value,context){
    return gatewayItemExpiryReviver.call(this,key,gatewayCreatureTimeReviver.call(this,key,gatewayGameShopDateReviver.call(this,key,value,context),context),context);
  });
  if(mailObject(parsed)&&parsed.type==="packet"&&parsed.packet==="ChangePasswordBanned") {
    return parse(text,function(key,value,context){
      if(key!=="expiryBinaryDatetime"||!mailObject(this)||typeof this.reason!=="string"
        ||Object.keys(this).sort().join()!=="expiryBinaryDatetime,reason") return value;
      const source=typeof value==="string"?value:typeof value==="number"
        ? Number.isSafeInteger(value)?String(value):context?.source : undefined;
      if(typeof source!=="string"||!/^(?:0|[1-9][0-9]{0,18}|-[1-9][0-9]{0,18})$/.test(source))return null;
      try { const n=BigInt(source); return n>=-9223372036854775808n&&n<=9223372036854775807n?source:null; }
      catch { return null; }
    });
  }
  if(!mailObject(parsed)||parsed.type!=="packet"||parsed.packet!=="ReceiveMail")return parsed;
  return parse(text,function(key,value,context){
    if((key==="dateSentBinaryDatetime"||key==="date_sent_binary_datetime")&&typeof value==="number"&&mailInt(this.mailId??this.mail_id,1,Number.MAX_SAFE_INTEGER)&&typeof context?.source==="string"&&/^-?\d{1,19}$/.test(context.source)){
      try{const n=BigInt(context.source);return n>=BigInt("-9223372036854775808")&&n<=BigInt("9223372036854775807")?context.source:null;}catch{return null;}
    }return gatewayItemExpiryReviver.call(this,key,value,context);
  });
}

/** Maps the numeric attack mode (ChangeAMode) to a human-readable label. */
export function attackModeLabel(mode: number): string {
  switch (mode) {
    case 0:
      return "Peaceful";
    case 1:
      return "Group";
    case 2:
      return "Guild";
    case 3:
      return "EnemyGuild";
    case 4:
      return "RedBrown";
    case 5:
      return "All";
    default:
      return `Mode ${mode}`;
  }
}

/** Maps the numeric pet mode (ChangePMode) to a human-readable label. */
export function petModeLabel(mode: number): string {
  switch (mode) {
    case 0:
      return "Both";
    case 1:
      return "MoveOnly";
    case 2:
      return "AttackOnly";
    case 3:
      return "None";
    case 4:
      return "FocusMasterTarget";
    default:
      return `Pet Mode ${mode}`;
  }
}

export type CrystalModeChatMessage = {
  localizationKey: string;
  fallback: string;
};

/** Mirrors GameScene.ChangeAMode, including the startup mode-sync message. */
export function attackModeChatMessage(mode: number): CrystalModeChatMessage | null {
  const messages: CrystalModeChatMessage[] = [
    { localizationKey: "client.AttackMode_Peace", fallback: "[Mode: Peaceful]" },
    { localizationKey: "client.AttackMode_Group", fallback: "[Mode: Group]" },
    { localizationKey: "client.AttackMode_Guild", fallback: "[Mode: Guild]" },
    { localizationKey: "client.AttackMode_EnemyGuild", fallback: "[Mode: Enemy Guild]" },
    { localizationKey: "client.AttackMode_RedBrown", fallback: "[Mode: Red/Brown]" },
    { localizationKey: "client.AttackMode_All", fallback: "[Mode: Attack All]" },
  ];
  return Number.isInteger(mode) ? messages[mode] ?? null : null;
}

/** Mirrors GameScene.ChangePMode, including the startup pet-mode message. */
export function petModeChatMessage(mode: number): CrystalModeChatMessage | null {
  const messages: CrystalModeChatMessage[] = [
    { localizationKey: "client.PetMode_Both", fallback: "[Pet: Attack and Move]" },
    { localizationKey: "client.PetMode_MoveOnly", fallback: "[Pet: Do Not Attack]" },
    { localizationKey: "client.PetMode_AttackOnly", fallback: "[Pet: Do Not Move]" },
    { localizationKey: "client.PetMode_None", fallback: "[Pet: Do Not Attack or Move]" },
    {
      localizationKey: "client.PetMode_FocusMasterTarget",
      fallback: "[Pet: Focus Master Target]",
    },
  ];
  return Number.isInteger(mode) ? messages[mode] ?? null : null;
}

/** Matches Crystal's `{0:#0.##%}` main-HUD experience label. */
export function formatCrystalExperiencePercent(ratio: number): string {
  const safeRatio = Number.isFinite(ratio) ? Math.max(0, ratio) : 0;
  const percent = (safeRatio * 100).toFixed(2).replace(/\.?0+$/, "");
  return `${percent}%`;
}

/** Result-code text for MailSent / ParcelCollected. */
export function mailResultMessage(result: number, parcel: boolean): string {
  if (result === 1) {
    return parcel ? "Parcel collected." : "Mail sent.";
  }
  switch (result) {
    case 0:
      return parcel ? "Parcel could not be collected." : "Mail could not be sent.";
    case -1:
      return "Recipient not found.";
    case -2:
      return "Not enough gold.";
    case -3:
      return "Mailbox is full.";
    default:
      return parcel
        ? `Parcel collection returned ${result}.`
        : `Mail send returned ${result}.`;
  }
}

/** NewHero / NewCharacter style result -> message. */
export function heroCreateResultMessage(result: number): string {
  switch (result) {
    case 1:
      return "Hero name is too long.";
    case 2:
      return "Hero name contains banned words.";
    case 3:
      return "A hero with that name already exists.";
    case 4:
      return "Maximum number of heroes reached.";
    case 8:
      return "Hero created successfully.";
    default:
      return `Hero creation returned ${result}.`;
  }
}

/** Builds the immutable stage5 group slice update for membership changes. */
export function groupMembersAfterChange(
  current: string[] | undefined,
  change: { add?: string; remove?: string; replace?: string[] },
): string[] {
  if (change.replace) {
    return [...change.replace];
  }
  const base = current ? [...current] : [];
  if (change.remove) {
    return base.filter((member) => member !== change.remove);
  }
  if (change.add && !base.includes(change.add)) {
    return [...base, change.add];
  }
  return base;
}
