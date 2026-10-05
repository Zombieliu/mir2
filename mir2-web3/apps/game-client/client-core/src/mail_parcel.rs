//! Shared parcel rules. Local cell locks are UI reservations, never server escrow.
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pricing {
    pub template_index: i32,
    pub item_type: u8,
    pub shape: i16,
    pub template_price: u32,
    pub template_durability: u16,
    pub quantity: u32,
    pub durability_current: Option<u16>,
    pub durability_max: Option<u16>,
    pub added_stats: Vec<(u8, i32)>,
}
#[derive(Clone, Debug)]
pub struct ParcelItem { pub unique_id: Option<u64>, pub container: u8, pub slot: u32,
    pub pricing: Option<Pricing>, pub stamp: bool }
#[derive(Clone, Debug, Default)]
pub struct ParcelInventory { pub bag_capacity: u16, pub items: Vec<ParcelItem> }
impl ParcelInventory {
    pub fn item(&self, id: u64) -> Option<&ParcelItem> {
        if id == 0 { return None; }
        let mut rows=self.items.iter().filter(|r| r.unique_id==Some(id));
        let row=rows.next()?;
        if rows.next().is_some() || row.container!=0 || row.slot>=u32::from(self.bag_capacity)
            || self.items.iter().filter(|r|r.container==0&&r.slot==row.slot).count()!=1 { return None; }
        Some(row)
    }
    pub fn stamp_available(&self)->bool { self.items.iter().any(|r| r.stamp&&r.pricing.as_ref().is_some_and(|p|p.template_index==838&&p.item_type==0&&p.shape==1&&p.quantity>0)&&r.unique_id.is_some_and(|id|self.item(id).is_some())) }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint { pub gold: u32, pub attachment_unique_ids: Vec<u64>, pub stamped: bool,
    pub attachment_pricing: Vec<(u64, Pricing)> }
impl Fingerprint {
    pub fn current(gold:u32,ids:&[u64],stamped:bool,inventory:&ParcelInventory)->Option<Self> {
        if ids.len()>if stamped {5}else{1} || stamped&&!inventory.stamp_available() {return None;}
        let mut seen=BTreeSet::new();let mut pricing=Vec::with_capacity(ids.len());
        for id in ids {if !seen.insert(*id){return None;}let p=inventory.item(*id)?.pricing.as_ref()?;
            if p.template_index<0||p.quantity==0||p.added_stats.iter().try_fold(0_u32,|n,(_,v)|n.checked_add(v.checked_abs()? as u32)).is_none() {return None;}
            if p.template_durability>0 && !p.durability_current.zip(p.durability_max).is_some_and(|(a,b)|a<=b){return None;}
            pricing.push((*id,p.clone()));}
        Some(Self {gold,attachment_unique_ids:ids.to_vec(),stamped,attachment_pricing:pricing})
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParcelOwner {pub connection:u64,pub session:u64,pub revision:u64,pub player:u32}
/// Local authorization identity. Never serialized or derived from a draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeQuoteToken(u64);
impl NativeQuoteToken { pub fn value(self)->u64 {self.0} }
#[derive(Clone, Debug)]
pub struct NativeQuoteReservation {pub token:NativeQuoteToken,pub fingerprint:Fingerprint}
#[derive(Clone, Debug)]
struct PendingCost {fingerprint:Fingerprint,requested_at_ms:u64,owner:Option<ParcelOwner>,entered:bool,epoch:u64,native_token:Option<NativeQuoteToken>}
#[derive(Debug, Default)]
pub struct MailParcelLedger {
    stamped:bool, selected:BTreeSet<u64>, locked:BTreeSet<u64>,
    quote:Option<(Fingerprint,u32)>, pending:Option<PendingCost>, desired:Option<Fingerprint>,
    error:Option<String>, owner:Option<ParcelOwner>,epoch:u64,
    native_token_highwater:u64,native_epoch_exhausted:bool,
}
impl MailParcelLedger {
    pub fn sync_owner(&mut self,owner:Option<ParcelOwner>) {
        if self.owner==owner{return;}
        let retain=self.pending.as_ref().is_some_and(|p|p.native_token.is_some()||(p.entered&&match(owner,p.owner){(Some(a),Some(b))=>a.connection==b.connection,(None,Some(_))=>true,_=>false}));
        self.release_all();if !retain{self.pending=None;}self.owner=owner;
    }
    pub fn stamped(&self)->bool{self.stamped}
    pub fn slot_limit(&self)->usize{if self.stamped{5}else{1}}
    pub fn toggle_stamp(&mut self,inventory:&ParcelInventory)->bool {
        if !self.stamped&&!inventory.stamp_available(){return false;}
        self.stamped=!self.stamped;self.invalidate_quote();true
    }
    pub fn blocks_item(&self,id:u64)->bool{id!=0&&(self.selected.contains(&id)||self.locked.contains(&id))}
    pub fn selected_ids(&self)->impl Iterator<Item=u64>+'_ {self.selected.iter().copied()}
    pub fn apply_lock_receipt(&mut self,id:u64,locked:bool){if id==0{return;}if locked{if self.blocks_item(id){self.locked.insert(id);}}else{self.locked.remove(&id);}}
    pub fn attach(&mut self,ids:&mut Vec<u64>,inventory:&ParcelInventory,id:u64)->bool {
        if inventory.item(id).is_none()||ids.contains(&id)||ids.len()>=self.slot_limit(){return false;}
        ids.push(id);self.selected.insert(id);self.invalidate_quote();true
    }
    pub fn detach_at(&mut self,ids:&mut Vec<u64>,slot:usize)->Option<u64>{let id=*ids.get(slot)?;ids.remove(slot);self.selected.remove(&id);self.locked.remove(&id);self.invalidate_quote();Some(id)}
    pub fn reconcile(&mut self,ids:&mut Vec<u64>,inventory:&ParcelInventory)->(Vec<u64>,Vec<u64>) {
        let mut unlock=Vec::new();let mut changed=false;
        if self.stamped&&!inventory.stamp_available(){self.stamped=false;changed=true;}
        let mut seen=BTreeSet::new();ids.retain(|id|{let valid=seen.insert(*id)&&inventory.item(*id).is_some();if !valid{changed=true;if self.selected.remove(id){self.locked.remove(id);unlock.push(*id);}}valid});
        while ids.len()>self.slot_limit(){let id=ids.pop().unwrap();changed=true;if self.selected.remove(&id){self.locked.remove(&id);unlock.push(id);}}
        let mut lock=Vec::new();for id in ids {if self.selected.insert(*id){lock.push(*id);}}
        if changed{self.invalidate_quote();} (unlock,lock)
    }
    pub fn quote_is_current(&self,current:Option<&Fingerprint>)->bool {self.pending.is_none()&&self.quote.as_ref().is_some_and(|(fp,cost)|Some(fp)==current&&*cost!=u32::MAX)}
    pub fn postage(&self)->Option<u32>{self.quote.as_ref().map(|(_,c)|*c).filter(|c|*c!=u32::MAX)}
    pub fn quote_error(&self)->Option<&str>{self.error.as_deref()}
    pub fn has_pending_quote(&self)->bool{self.pending.is_some()}
    pub fn reserve_current_quote(&mut self,gold:u32,ids:&[u64],inventory:&ParcelInventory,now:u64)->Option<Fingerprint>{
        let Some(fp)=Fingerprint::current(gold,ids,self.stamped,inventory)else{self.error=Some("Attachment pricing or stamp is unavailable; draft kept".into());return None;};
        self.begin_quote(fp.clone(),now).then_some(fp)
    }
    pub fn begin_quote(&mut self,current:Fingerprint,now:u64)->bool {
        if self.desired.as_ref()!=Some(&current){self.desired=Some(current.clone());self.quote=None;if self.pending.is_none(){self.error=None;}}
        if self.pending.is_some()||self.quote.as_ref().is_some_and(|(fp,_)|fp==&current){return false;}
        self.error=None;self.pending=Some(PendingCost{fingerprint:current,requested_at_ms:now,owner:self.owner,entered:false,epoch:self.epoch,native_token:None});true
    }
    pub fn enter_quote(&mut self,current:&Fingerprint)->bool {let Some(p)=self.pending.as_mut()else{return false;};if p.native_token.is_some()||p.entered||p.owner!=self.owner||p.epoch!=self.epoch||&p.fingerprint!=current{return false;}p.entered=true;true}
    pub fn cancel_unsent_quote(&mut self){if self.pending.as_ref().is_some_and(|p|p.native_token.is_none()&&!p.entered){self.pending=None;self.error=Some("Postage request was not sent".into());}}
    pub fn reserve_native_quote(&mut self,gold:u32,ids:&[u64],inventory:&ParcelInventory,now:u64)->Option<NativeQuoteReservation>{
        if self.native_epoch_exhausted{return None;}
        let next=self.native_token_highwater.checked_add(1)?;
        let fingerprint=self.reserve_current_quote(gold,ids,inventory,now)?;
        let token=NativeQuoteToken(next);self.native_token_highwater=next;
        self.pending.as_mut()?.native_token=Some(token);
        Some(NativeQuoteReservation{token,fingerprint})
    }
    pub fn pending_native_token(&self)->Option<NativeQuoteToken>{self.pending.as_ref()?.native_token}
    /// Only a host's exact prebound ticket receipt may use this trusted API.
    /// An old owner can enter its tombstone but cannot price the new owner.
    pub fn enter_native_quote(&mut self,token:NativeQuoteToken,entered_at_ms:u64)->bool{
        let Some(p)=self.pending.as_mut()else{return false;};
        if p.native_token!=Some(token)||p.entered{return false;}
        p.entered=true;p.requested_at_ms=entered_at_ms;true
    }
    pub fn cancel_native_unsent_quote(&mut self,token:NativeQuoteToken)->bool{
        if !self.pending.as_ref().is_some_and(|p|p.native_token==Some(token)&&!p.entered){return false;}
        self.pending=None;self.error=Some("Postage request was not sent".into());true
    }
    pub fn tick_quote_timeout(&mut self,now:u64){if self.pending.as_ref().is_some_and(|p|(p.native_token.is_none()||p.entered)&&now.saturating_sub(p.requested_at_ms)>=5000){self.error=Some("Postage quote unavailable".into());}}
    pub fn apply_cost(&mut self,cost:u32,current:Option<&Fingerprint>){
        if !self.pending.as_ref().is_some_and(|p|p.entered){return;}
        let p=self.pending.take().unwrap();
        if (p.native_token.is_none()||!self.native_epoch_exhausted)&&p.owner==self.owner&&p.epoch==self.epoch&&Some(&p.fingerprint)==current{self.quote=Some((p.fingerprint,cost));self.error=(cost==u32::MAX).then(||"Postage quote unavailable".into());}
    }
    pub fn invalidate_quote(&mut self){self.quote=None;self.desired=None;self.error=None;}
    pub fn release_all(&mut self)->Vec<u64>{let ids=self.selected.iter().copied().collect();self.selected.clear();self.locked.clear();self.stamped=false;if let Some(next)=self.epoch.checked_add(1){self.epoch=next;}else{self.native_epoch_exhausted=true;}self.invalidate_quote();ids}
    pub fn complete_send(&mut self)->Vec<u64>{self.release_all()}
    /// Only a host boundary that discards its old packet stream may call this.
    pub fn reset_stream(&mut self){self.release_all();self.pending=None;}
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn native_mail_provisional_owner_reset_entry_timeout_and_late_cost_are_exact(){
  let mut l=MailParcelLedger::default();let a=ParcelOwner{connection:1,session:2,revision:0,player:3};l.sync_owner(Some(a));
  let first=l.reserve_native_quote(700,&[],&inventory(),1).unwrap();
  l.sync_owner(Some(ParcelOwner{session:9,..a}));l.tick_quote_timeout(900_000);
  assert!(l.has_pending_quote());assert_eq!(l.quote_error(),None);assert!(!l.enter_quote(&first.fingerprint));
  assert!(l.enter_native_quote(first.token,900_000));l.tick_quote_timeout(904_999);assert_eq!(l.quote_error(),None);
  l.tick_quote_timeout(905_000);assert_eq!(l.quote_error(),Some("Postage quote unavailable"));
  l.apply_cost(70,Some(&first.fingerprint));assert!(!l.quote_is_current(Some(&first.fingerprint)));
  let next=l.reserve_native_quote(700,&[],&inventory(),905_001).unwrap();assert_ne!(first.token,next.token);assert_eq!(first.fingerprint,next.fingerprint);
  assert!(!l.enter_native_quote(first.token,905_001));assert!(!l.cancel_native_unsent_quote(first.token));
  assert!(l.enter_native_quote(next.token,905_001));assert!(!l.cancel_native_unsent_quote(next.token));
  l.apply_cost(140,Some(&next.fingerprint));assert!(l.quote_is_current(Some(&next.fingerprint)));assert_eq!(l.postage(),Some(140));
 }
 #[test] fn native_mail_unsent_and_real_stream_reset_do_not_reuse_token(){
  let mut l=MailParcelLedger::default();let first=l.reserve_native_quote(1,&[],&inventory(),1).unwrap();
  l.apply_cost(12,Some(&first.fingerprint));assert!(l.has_pending_quote());assert_eq!(l.postage(),None);
  l.cancel_unsent_quote();assert!(l.has_pending_quote(),"Web cancellation cannot cancel a tracked Native flight");
  assert!(!l.cancel_native_unsent_quote(NativeQuoteToken(0)));assert!(l.cancel_native_unsent_quote(first.token));
  let second=l.reserve_native_quote(1,&[],&inventory(),2).unwrap();l.reset_stream();
  let third=l.reserve_native_quote(1,&[],&inventory(),3).unwrap();assert_ne!(second.token,third.token);
  assert!(!l.enter_native_quote(second.token,3));assert!(!l.cancel_native_unsent_quote(second.token));assert_eq!(l.pending_native_token(),Some(third.token));
 }
 #[test] fn native_mail_checked_token_and_epoch_exhaustion_fail_closed(){
  let mut l=MailParcelLedger::default();l.native_token_highwater=u64::MAX-1;
  let last=l.reserve_native_quote(1,&[],&inventory(),1).unwrap();assert_eq!(last.token.value(),u64::MAX);
  assert!(l.cancel_native_unsent_quote(last.token));l.reset_stream();assert!(l.reserve_native_quote(1,&[],&inventory(),2).is_none());
  let mut l=MailParcelLedger::default();l.epoch=u64::MAX;let old=l.reserve_native_quote(1,&[],&inventory(),1).unwrap();
  l.release_all();assert!(l.enter_native_quote(old.token,2));l.apply_cost(12,Some(&old.fingerprint));assert_eq!(l.postage(),None);
  assert!(l.reserve_native_quote(1,&[],&inventory(),3).is_none());
 }
 #[test] fn web_mail_unentered_owner_cancel_and_request_time_timeout_stay_unchanged(){
  let mut l=MailParcelLedger::default();let fp=Fingerprint::current(1,&[],false,&inventory()).unwrap();
  assert!(l.begin_quote(fp.clone(),1));l.tick_quote_timeout(5001);assert_eq!(l.quote_error(),Some("Postage quote unavailable"));
  l.sync_owner(Some(ParcelOwner{connection:1,session:2,revision:0,player:3}));assert!(!l.has_pending_quote());
  assert!(l.begin_quote(fp.clone(),7000));assert!(l.enter_quote(&fp));l.tick_quote_timeout(12000);assert_eq!(l.quote_error(),Some("Postage quote unavailable"));
 }
 fn inventory()->ParcelInventory{ParcelInventory{bag_capacity:80,items:(1..=6).map(|id|ParcelItem{unique_id:Some(id),container:0,slot:id as u32+39,stamp:id==6,pricing:Some(Pricing{template_index:if id==6{838}else{id as i32},item_type:0,shape:1,template_price:100,template_durability:0,quantity:1,durability_current:None,durability_max:None,added_stats:vec![]})}).collect()}}
 #[test] fn selection_slots_echo_and_loss_are_shared(){let mut l=MailParcelLedger::default();let mut inv=inventory();let mut ids=vec![];assert!(l.attach(&mut ids,&inv,1));assert!(!l.attach(&mut ids,&inv,2));l.apply_lock_receipt(1,false);assert!(l.blocks_item(1));l.apply_lock_receipt(5,true);assert!(!l.blocks_item(5));assert!(l.toggle_stamp(&inv));for id in 2..=5{assert!(l.attach(&mut ids,&inv,id));}assert!(!l.attach(&mut ids,&inv,6));inv.items.pop();let(unlock,_)=l.reconcile(&mut ids,&inv);assert_eq!(ids,vec![1]);assert_eq!(unlock,vec![5,4,3,2]);}
 #[test] fn late_quotes_compare_live_price_and_keep_uncorrelated_slot(){let mut l=MailParcelLedger::default();let mut inv=inventory();let fp=Fingerprint::current(700,&[1],false,&inv).unwrap();assert!(l.begin_quote(fp.clone(),1));assert!(l.enter_quote(&fp));l.tick_quote_timeout(5002);l.release_all();assert!(!l.begin_quote(fp.clone(),5003));inv.items[0].pricing.as_mut().unwrap().added_stats=vec![(1,2)];let changed=Fingerprint::current(700,&[1],false,&inv).unwrap();l.apply_cost(12,Some(&changed));assert!(!l.quote_is_current(Some(&changed)));assert!(l.begin_quote(changed.clone(),5004));assert!(l.enter_quote(&changed));l.apply_cost(u32::MAX,Some(&changed));assert!(!l.begin_quote(changed,5005));}
 #[test] fn owner_tombstone_and_definitely_unsent_are_distinct(){let mut l=MailParcelLedger::default();let a=ParcelOwner{connection:1,session:2,revision:0,player:3};l.sync_owner(Some(a));let fp=Fingerprint::current(1,&[],false,&inventory()).unwrap();assert!(l.begin_quote(fp.clone(),1));l.cancel_unsent_quote();assert!(l.begin_quote(fp.clone(),2));assert!(l.enter_quote(&fp));l.sync_owner(Some(ParcelOwner{session:9,..a}));assert!(!l.begin_quote(fp.clone(),3));l.apply_cost(100,Some(&fp));assert!(!l.quote_is_current(Some(&fp)));assert!(l.begin_quote(fp,4));}
 #[test] fn every_live_pricing_field_and_order_bind_the_receipt(){
  let inv=inventory();let original=Fingerprint::current(700,&[1],false,&inv).unwrap();
  for field in 0..9{let mut changed=inv.clone();let p=changed.items[0].pricing.as_mut().unwrap();match field{0=>p.template_index+=1,1=>p.item_type+=1,2=>p.shape+=1,3=>p.template_price+=1,4=>p.template_durability=1,5=>p.quantity+=1,6=>p.durability_current=Some(1),7=>p.durability_max=Some(2),_=>p.added_stats=vec![(1,1),(2,-1)]};
   if p.template_durability>0{p.durability_current=Some(1);p.durability_max=Some(2);}
   let fp=Fingerprint::current(700,&[1],false,&changed).unwrap();assert_ne!(fp,original);let mut l=MailParcelLedger::default();assert!(l.begin_quote(original.clone(),1));assert!(l.enter_quote(&original));l.apply_cost(70,Some(&fp));assert!(!l.quote_is_current(Some(&fp)));
  }
  let mut l=MailParcelLedger::default();assert!(l.toggle_stamp(&inv));assert_ne!(Fingerprint::current(700,&[1,2],true,&inv),Fingerprint::current(700,&[2,1],true,&inv));
 }
 #[test] fn identity_missing_metadata_and_overflow_cannot_authorize_parcels(){
  let mut inv=inventory();assert!(Fingerprint::current(700,&[],false,&inv).is_some());assert!(Fingerprint::current(700,&[1,1],true,&inv).is_none());
  inv.items[0].pricing=None;assert!(Fingerprint::current(700,&[],false,&inv).is_some());assert!(Fingerprint::current(700,&[1],false,&inv).is_none());
  inv=inventory();inv.items[0].pricing.as_mut().unwrap().added_stats=vec![(1,i32::MIN)];assert!(Fingerprint::current(0,&[1],false,&inv).is_none());
  inv.items[0].pricing.as_mut().unwrap().added_stats=vec![(1,i32::MAX),(2,i32::MAX),(3,2)];assert!(Fingerprint::current(0,&[1],false,&inv).is_none());
  inv=inventory();inv.items.push(inv.items[0].clone());assert!(Fingerprint::current(0,&[1],false,&inv).is_none());
 }
}
