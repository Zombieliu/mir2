//! Strict, Bevy-independent host for the actual shared parcel ledger.
use mir2_client_core::mail_parcel::{MailParcelLedger,ParcelInventory,ParcelItem,Pricing,Fingerprint,ParcelOwner};
use serde_json::{json,Value};
use wasm_bindgen::prelude::*;
const SAFE:u64=9_007_199_254_740_991;
fn number(v:&Value,max:u64)->Option<u64>{v.as_u64().filter(|n|*n<=max)}
fn keys(v:&Value,names:&[&str])->bool{v.as_object().is_some_and(|o|o.len()==names.len()&&names.iter().all(|k|o.contains_key(*k)))}
fn owner(v:&Value)->Option<ParcelOwner>{if !keys(v,&["connectionGeneration","sessionGeneration","ownerRevision","playerObjectId"]){return None;}Some(ParcelOwner{connection:number(&v["connectionGeneration"],SAFE).filter(|n|*n>0)?,session:number(&v["sessionGeneration"],SAFE).filter(|n|*n>0)?,revision:number(&v["ownerRevision"],SAFE)?,player:u32::try_from(number(&v["playerObjectId"],u32::MAX.into()).filter(|n|*n>0)?).ok()?})}
fn pricing(v:&Value)->Option<Pricing>{
 if !keys(v,&["templateIndex","itemType","shape","templatePrice","templateDurability","quantity","currentDura","maxDura","addedStats"]){return None;}
 let optional=|v:&Value|if v.is_null(){Some(None)}else{Some(Some(number(v,u16::MAX.into())? as u16))};
 let stats=v["addedStats"].as_array()?;if stats.len()>256{return None;}
 let mut added_stats=Vec::new();for s in stats{if !keys(s,&["stat","value"]){return None;}added_stats.push((number(&s["stat"],255)? as u8,i32::try_from(s["value"].as_i64()?).ok()?));}
 Some(Pricing{template_index:i32::try_from(v["templateIndex"].as_i64()?).ok()?,item_type:number(&v["itemType"],255)? as u8,shape:i16::try_from(v["shape"].as_i64()?).ok()?,template_price:number(&v["templatePrice"],u32::MAX.into())? as u32,
 template_durability:number(&v["templateDurability"],u16::MAX.into())? as u16,quantity:number(&v["quantity"],u32::MAX.into())? as u32,
 durability_current:optional(&v["currentDura"])?,durability_max:optional(&v["maxDura"])?,added_stats})
}
fn inventory(v:&Value)->Option<ParcelInventory>{
 if !keys(v,&["bagCapacity","items"]){return None;}let cap=number(&v["bagCapacity"],250)? as u16;if cap==0{return None;}
 let rows=v["items"].as_array()?;if rows.len()>1024{return None;}let mut items=Vec::new();let mut ids=std::collections::BTreeSet::new();let mut cells=std::collections::BTreeSet::new();
 for r in rows{if !keys(r,&["uniqueId","container","slot","pricing","stamp"]){return None;}
  let id=if r["uniqueId"].is_null(){None}else{Some(number(&r["uniqueId"],SAFE).filter(|n|*n>0)?)};
  let container=number(&r["container"],4)? as u8;let slot=number(&r["slot"],u32::MAX.into())? as u32;
  if id.is_some_and(|id|!ids.insert(id))||!cells.insert((container,slot))||container==0&&slot>=u32::from(cap){return None;}
  let p=if r["pricing"].is_null(){None}else{Some(pricing(&r["pricing"])?)};let stamp=r["stamp"].as_bool()?;
  if stamp&&(id.is_none()||container!=0||p.as_ref().is_none_or(|p|p.template_index!=838||p.item_type!=0||p.shape!=1||p.quantity==0)){return None;}
  items.push(ParcelItem{unique_id:id,container,slot,pricing:p,stamp});
 }Some(ParcelInventory{bag_capacity:cap,items})
}
fn cost_payload(fp:&Fingerprint)->Value{let mut ids=fp.attachment_unique_ids.clone();ids.resize(5,0);json!({"gold":fp.gold,"itemsIdx":ids,"stamped":fp.stamped})}
#[wasm_bindgen]
pub fn mail_parcel_abi_version()->u32{1}
#[wasm_bindgen]
#[derive(Default)]
pub struct MailParcelBridge {ledger:MailParcelLedger,inventory:Option<ParcelInventory>,owner:Option<ParcelOwner>,ids:Vec<u64>,gold:u32,review:bool,notice:Option<String>,serial:u64,ticket:Option<(u64,Fingerprint)>}
#[wasm_bindgen]
impl MailParcelBridge {
 #[wasm_bindgen(constructor)] pub fn new()->Self{Self::default()}
 pub fn transact(&mut self,input:&str)->String{self.handle(input).unwrap_or_else(||json!({"ok":false,"error":"Invalid parcel input"}).to_string())}
}
impl MailParcelBridge {
 fn current(&self)->Option<Fingerprint>{Fingerprint::current(self.gold,&self.ids,self.ledger.stamped(),self.inventory.as_ref()?)}
 fn output(&self,locks:Vec<Value>,quote:Option<Value>,token:Option<u64>)->String{
  let pricing_notice=(!self.ids.is_empty()&&self.inventory.is_some()&&self.current().is_none()).then_some("Attachment pricing or stamp is unavailable; draft kept");
  let mut blocked:Vec<u64>=self.inventory.as_ref().map(|inv|inv.items.iter().filter_map(|r|r.unique_id.filter(|id|self.ledger.blocks_item(*id))).collect()).unwrap_or_default();
  for id in self.ledger.selected_ids(){if !blocked.contains(&id){blocked.push(id);}}
  json!({"ok":true,"state":{"attachmentUniqueIds":self.ids,"stamped":self.ledger.stamped(),"slotLimit":self.ledger.slot_limit(),"blockedUniqueIds":blocked,
   "postage":self.ledger.postage(),"pendingQuote":self.ledger.has_pending_quote(),"quoteReady":!self.review&&self.ledger.quote_is_current(self.current().as_ref()),"reviewRequired":self.review,
   "notice":self.notice.as_deref().or(self.ledger.quote_error()).or(pricing_notice)},"locks":locks,"quote":quote,"token":token}).to_string()
 }
 fn handle(&mut self,input:&str)->Option<String>{
  if input.len()>262144{return None;}let v:Value=serde_json::from_str(input).ok()?;let action=v["action"].as_str()?;let mut locks=Vec::new();let mut quote=None;let mut token=None;
  match action {
   "sync"=>{
    if !keys(&v,&["action","owner","snapshot","gold","now"]){return None;}let next=if v["owner"].is_null(){None}else{Some(owner(&v["owner"])?)};
    let inv=if v["snapshot"].is_null(){None}else{Some(inventory(&v["snapshot"])?)};let gold=number(&v["gold"],u32::MAX.into())? as u32;let now=number(&v["now"],SAFE)?;
    if next!=self.owner{self.ids.clear();self.notice=None;self.review=false;self.ticket=None;self.ledger.sync_owner(next);self.owner=next;}
    if self.gold!=gold{self.ledger.invalidate_quote();self.gold=gold;}
    self.inventory=inv;if self.inventory.is_none(){self.ledger.invalidate_quote();}else{
     let old=(self.ids.clone(),self.ledger.stamped());let(unlock,lock)=self.ledger.reconcile(&mut self.ids,self.inventory.as_ref().unwrap());
     for id in unlock{locks.push(json!({"uniqueId":id,"locked":false}));}for id in lock{locks.push(json!({"uniqueId":id,"locked":true}));}
     if old!=(self.ids.clone(),self.ledger.stamped()){self.review=true;self.notice=Some("Parcel inventory changed; review attachments and stamp".into());}
    }self.ledger.tick_quote_timeout(now);
   }
   "attach"=>{if !keys(&v,&["action","uniqueId"]){return None;}let id=number(&v["uniqueId"],SAFE).filter(|n|*n>0)?;
    if self.owner.is_none()||!self.ledger.attach(&mut self.ids,self.inventory.as_ref()?,id){return Some(json!({"ok":false,"error":"Attachment is unavailable or parcel slots are full"}).to_string());}locks.push(json!({"uniqueId":id,"locked":true}));self.review=false;self.notice=None;}
   "detach"=>{if !keys(&v,&["action","slot"]){return None;}let slot=number(&v["slot"],4)? as usize;let id=self.ledger.detach_at(&mut self.ids,slot)?;locks.push(json!({"uniqueId":id,"locked":false}));self.review=false;self.notice=None;}
   "stamp"=>{if !keys(&v,&["action"]){return None;}if !self.ledger.toggle_stamp(self.inventory.as_ref()?){return Some(json!({"ok":false,"error":"A live carried stamp is required"}).to_string());}
    let(unlock,lock)=self.ledger.reconcile(&mut self.ids,self.inventory.as_ref()?);for id in unlock{locks.push(json!({"uniqueId":id,"locked":false}));}for id in lock{locks.push(json!({"uniqueId":id,"locked":true}));}self.review=false;self.notice=None;}
   "review"=>{if !keys(&v,&["action"]){return None;}self.review=false;self.notice=None;}
   "quote"=>{if !keys(&v,&["action","now"]){return None;}let now=number(&v["now"],SAFE)?;if !self.review&&self.owner.is_some(){if let Some(inv)=self.inventory.as_ref(){if let Some(fp)=self.ledger.reserve_current_quote(self.gold,&self.ids,inv,now){self.serial=self.serial.checked_add(1).filter(|s|*s<=SAFE)?;token=Some(self.serial);quote=Some(cost_payload(&fp));self.ticket=Some((self.serial,fp));}}}}
   "enter"=>{if !keys(&v,&["action","token"]){return None;}let n=number(&v["token"],SAFE)?;let (_,fp)=self.ticket.as_ref().filter(|(id,_)|*id==n)?;if self.current().as_ref()!=Some(fp)||!self.ledger.enter_quote(fp){return Some(json!({"ok":false,"error":"Postage proof is stale"}).to_string());}}
   "finish"=>{if !keys(&v,&["action","token"]){return None;}let n=number(&v["token"],SAFE)?;if self.ticket.as_ref().is_some_and(|(id,_)|*id==n){self.ledger.cancel_unsent_quote();self.ticket=None;}}
   "cost"=>{if !keys(&v,&["action","cost"]){return None;}let cost=number(&v["cost"],u32::MAX.into())? as u32;let current=self.current();self.ledger.apply_cost(cost,current.as_ref());if !self.ledger.has_pending_quote(){self.ticket=None;}}
   "lock"=>{if !keys(&v,&["action","uniqueId","locked"]){return None;}let id=number(&v["uniqueId"],SAFE).filter(|n|*n>0)?;self.ledger.apply_lock_receipt(id,v["locked"].as_bool()?);}
   "cancel"|"complete"=>{if !keys(&v,&["action"]){return None;}for id in self.ledger.release_all(){locks.push(json!({"uniqueId":id,"locked":false}));}self.ids.clear();self.review=false;self.notice=None;}
   "status"=>{if !keys(&v,&["action"]){return None;}}
   _=>return None,
  }Some(self.output(locks,quote,token))
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn actual_bridge_rejects_schema_and_invalid_numbers(){let mut b=MailParcelBridge::new();for input in ["{}","[]","{\"action\":\"attach\",\"uniqueId\":0}","{\"action\":\"cost\",\"cost\":1.5}","{\"action\":\"status\",\"extra\":true}"]{assert_eq!(serde_json::from_str::<Value>(&b.transact(input)).unwrap()["ok"],false);}}
 #[test]fn actual_gold_only_quote_keeps_unknown_slot_until_cost(){let mut b=MailParcelBridge::new();let owner=json!({"connectionGeneration":1,"sessionGeneration":2,"ownerRevision":0,"playerObjectId":3});let mut step=|v:Value|serde_json::from_str::<Value>(&b.transact(&v.to_string())).unwrap();
  let s=step(json!({"action":"sync","owner":owner,"snapshot":{"bagCapacity":40,"items":[{"uniqueId":null,"container":0,"slot":0,"pricing":null,"stamp":false}]},"gold":700,"now":1}));assert_eq!(s["ok"],true);
  let q=step(json!({"action":"quote","now":2}));assert_eq!(q["quote"],json!({"gold":700,"itemsIdx":[0,0,0,0,0],"stamped":false}));assert_eq!(q["token"],1);
  step(json!({"action":"enter","token":1}));step(json!({"action":"finish","token":1}));step(json!({"action":"cancel"}));assert_eq!(step(json!({"action":"quote","now":7000}))["quote"],Value::Null);
  let got=step(json!({"action":"cost","cost":70}));assert_eq!(got["state"]["quoteReady"],false);assert_eq!(step(json!({"action":"quote","now":7001}))["token"],2);
 }
 #[test]
 fn unsolicited_cost_before_socket_keeps_cancelable_ticket(){let mut b=MailParcelBridge::new();let mut step=|v:Value|serde_json::from_str::<Value>(&b.transact(&v.to_string())).unwrap();
  step(json!({"action":"sync","owner":{"connectionGeneration":1,"sessionGeneration":2,"ownerRevision":0,"playerObjectId":3},"snapshot":{"bagCapacity":40,"items":[]},"gold":700,"now":1}));
  assert_eq!(step(json!({"action":"quote","now":2}))["token"],1);assert_eq!(step(json!({"action":"cost","cost":70}))["state"]["pendingQuote"],true);
  assert_eq!(step(json!({"action":"finish","token":1}))["state"]["pendingQuote"],false);assert_eq!(step(json!({"action":"quote","now":3}))["token"],2);
 }
 #[test]
 #[ignore="Root explicitly supplies actual wrapper/consumer JSON transcripts"]
 fn actual_web_parcel_transcripts_match_shared_ledger(){let path=std::env::var("MIR2_MAIL_PARCEL_FIXTURE_PATH").expect("explicit Root path");let sessions:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();let sessions=sessions.as_array().unwrap();assert!(!sessions.is_empty());for session in sessions{let mut b=MailParcelBridge::new();for row in session.as_array().unwrap(){assert_eq!(serde_json::from_str::<Value>(&b.transact(row["inputJson"].as_str().unwrap())).unwrap(),row["output"]);}}}
}
