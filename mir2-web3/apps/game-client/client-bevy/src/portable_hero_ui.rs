//! Portable Hero windows use the Native Crystal painter and semantic intents.
//! Custody and operation settlement remain in the host's single authoritative ledger.
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use serde::{Deserialize, Serialize};
use crate::{hero_ingress::HeroScope, hero_model::HeroModel, inventory::{InventoryModel, ItemModel}, read_model::PlayerStats};
use crate::crystal_ui::{
    amount_input::CrystalAmountInput, bag_paint::{self, BagCellPolicy, BagPaintAction, BagPaintOptions},
    character_materials::CrystalCharacterWingMaterials, hero_dialog::{self, geometry::HeroWindow, render::HeroAction, HeroDialogModel, HeroPage},
    item_image::layout_original_item_images, shared_hud::absolute_node, spec::CrystalRect,
};
use mir2_protocol::{ClientPacket, MirGridType};
const MAX_SAFE: u64 = 9_007_199_254_740_991;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeroWindows {
    pub inventory_open: bool, pub character_open: bool, pub character_page: HeroPageName,
    pub belt_visible: bool, pub belt_vertical: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeroPageName { #[default] Equipment, Status, State, Skills }
impl HeroPageName { fn page(self) -> HeroPage { match self { Self::Equipment=>HeroPage::Equipment,Self::Status=>HeroPage::Status,Self::State=>HeroPage::State,Self::Skills=>HeroPage::Skills } } }
impl From<HeroPage> for HeroPageName { fn from(v:HeroPage)->Self {match v {HeroPage::Equipment=>Self::Equipment,HeroPage::Status=>Self::Status,HeroPage::State=>Self::State,HeroPage::Skills=>Self::Skills}} }
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct HeroPresentation { pub logical_width:f32,pub logical_height:f32,pub stage_css_scale:f32,pub touch:bool }
impl HeroPresentation { pub fn fits(self)->bool { [self.logical_width,self.logical_height,self.stage_css_scale].into_iter().all(f32::is_finite)
    && self.logical_width>=1024. && self.logical_width<=16384. && self.logical_height>=768. && self.logical_height<=16384.
    && self.stage_css_scale>0. && self.stage_css_scale<=16.
    && (!self.touch || self.stage_css_scale*14.>=44. && self.stage_css_scale*8.>=14.) } }
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct HeroStamp {
    pub scope:HeroScope,pub hero_object_id:u32,pub hero_generation:u64,pub hud_generation:u64,
    pub model_revision:u64,pub presentation_revision:u64,pub window_epochs:[u64;3],
}
impl HeroStamp { pub fn valid(&self)->bool { self.scope.valid() && self.hero_object_id!=0
    && [self.hero_generation,self.hud_generation,self.model_revision,self.presentation_revision].into_iter().all(|v|v>0 && v<=MAX_SAFE)
    && self.window_epochs.iter().all(|v|*v<=MAX_SAFE) } }
#[derive(Resource, Debug, Clone, Default)]
pub struct HeroUiContext { pub stamp:HeroStamp,pub revision:u64,pub frame_sequence:u64,pub windows:HeroWindows,
    pub presentation:Option<HeroPresentation>,pub active:bool,pub ready:bool,pub input_enabled:bool,pub pending:bool,pub now_ms:u64 }
#[derive(Resource,Debug,Clone,Default)]
pub struct HeroUiReadModel { pub hero:HeroModel,pub personal:Option<InventoryModel>,pub player:PlayerStats }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
pub enum HeroGrid { HeroInventory,HeroEquipment,Inventory }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeroCell { pub grid:HeroGrid,pub slot:u32 }
#[derive(Debug,Clone,PartialEq,Eq,Serialize)]
#[serde(tag="kind",rename_all="camelCase")]
pub enum HeroSemanticAction {
    Move{from:u32,to:u32},Equip{from:u32,to:u32},Remove{from:u32,to:Option<u32>},
    Merge{from:HeroCell,to:HeroCell},Transfer{from:u32,to:u32},TakeBack{from:u32,to:u32},
    Use{slot:u32,confirmed:bool},AutoPotValue{stat:u8,value:u32},AutoPotItem{grid:HeroPotGrid,slot:Option<u32>},
    MagicKey{spell:String,key:u8},
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
pub enum HeroPotGrid { HeroHpItem,HeroMpItem }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum HeroOrigin { Inventory,Character,Belt }
#[derive(Debug,Clone,Serialize)]
#[serde(tag="type",rename_all="camelCase")]
pub enum HeroIntentBody { Action{origin:HeroOrigin,action:HeroSemanticAction,#[serde(rename="oldKey")] old_key:Option<u8>,basis:crate::hero_action_basis::HeroActionBasis},Windows{windows:HeroWindows} }
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroUiIntent { #[serde(flatten)] pub stamp:HeroStamp,pub intent_sequence:u64,#[serde(flatten)]pub body:HeroIntentBody }
#[derive(Resource,Default)]
pub struct HeroIntentQueue(pub Vec<HeroUiIntent>);
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct HeroInputEdge {
    pub stamp:HeroStamp,pub sequence:u64,pub pointer_id:u64,pub phase:String,pub x:f32,pub y:f32,pub button:u8,
    pub key:String,pub text:String,pub control:bool,pub shift:bool,
}
impl HeroInputEdge { pub fn valid(&self)->bool { self.stamp.valid() && self.sequence>0 && self.sequence<=MAX_SAFE && self.pointer_id<=MAX_SAFE
    && self.x.is_finite() && self.y.is_finite() && self.x.abs()<=16384. && self.y.abs()<=16384. && matches!(self.button,0|2)
    && matches!(self.phase.as_str(),"down"|"move"|"up"|"cancel"|"key") && self.key.len()<=64 && self.text.len()<=32 } }
#[derive(Resource,Default)]
pub struct HeroInputQueue(pub Vec<HeroInputEdge>);
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum Hit { Hero(Option<HeroWindow>,Option<HeroAction>),PersonalCell(u32),PersonalPage(u8),PersonalClose,PersonalBackground }
#[derive(Debug,Clone)]
struct Selected { cell:HeroCell,item:ItemModel,origin:HeroOrigin }
#[derive(Debug,Clone)]
struct Held { pointer:u64,hit:Hit,button:u8,dragged:bool }
#[derive(Resource,Debug,Clone,Default)]
pub struct HeroUiState { pub ui:HeroDialogModel,pub personal_open:bool,pub personal_page:u8,pub personal_position:[i32;2],
    stamp:Option<HeroStamp>,selected:Option<Selected>,held:Option<Held>,last_click:Option<(HeroCell,u64,u64)>,
    sequence:u64,intent_sequence:u64,next_use_ms:u64,confirmation:Option<Selected>,personal_drag:Option<[f32;2]>,
}
impl HeroUiState {
    /// Escape belongs to this exact reducer state, including personal selection.
    pub fn captures_escape(&self) -> bool { self.ui.modal() || self.selected.is_some() || self.held.is_some() }
    pub fn cancel(&mut self) { self.held=None;self.selected=None;self.last_click=None;self.confirmation=None;
        self.ui.armed=None;self.ui.hovered=None;self.ui.dragging=None;self.personal_drag=None;self.ui.selected=None;self.ui.amount=None;
        self.ui.use_confirmation=None;self.ui.assign=Default::default(); }
    pub fn reconcile(&mut self,c:&HeroUiContext,m:&HeroUiReadModel) {
        if self.stamp.as_ref()!=Some(&c.stamp) {
            let same=self.stamp.as_ref().is_some_and(|old|old.scope==c.stamp.scope && old.hero_object_id==c.stamp.hero_object_id && old.hero_generation==c.stamp.hero_generation);
            self.cancel();if !same { self.ui=Default::default();self.sequence=0;self.intent_sequence=0;self.next_use_ms=0;self.personal_page=0;self.personal_position=[0,470];self.personal_open=true; }
            self.stamp=Some(c.stamp.clone());
        }
        self.ui.info=m.hero.info.clone();self.ui.hero_generation=m.hero.hero_generation;self.ui.epoch=m.hero.session_epoch;
        self.ui.packet_actor_object_id=m.hero.info.as_ref().map(|i|i.object_id);self.ui.checkpoint_present=m.hero.snapshot_identity.is_some();
        if !self.ui.inventory_open && c.windows.inventory_open {self.personal_open=true;}
        self.ui.spawned=m.hero.spawned;self.ui.inventory_open=c.windows.inventory_open;self.ui.character_open=c.windows.character_open;
        self.ui.page=c.windows.character_page.page();self.ui.belt_visible=c.windows.belt_visible;self.ui.belt_vertical=c.windows.belt_vertical;
        if !m.personal.as_ref().is_some_and(InventoryModel::second_bag_unlocked) {self.personal_page=0;}
        if !c.active || !c.input_enabled || !c.presentation.is_some_and(HeroPresentation::fits) {self.cancel();}
        let rows=m.hero.info.as_ref().map_or(0,|i|i.magics.len());self.ui.skill_start=self.ui.skill_start.min(rows.saturating_sub(1)/7*7);
    }
    fn item<'a>(m:&'a HeroUiReadModel,cell:HeroCell)->Option<&'a ItemModel> {
        Self::item_at(m,cell).filter(|i|i.unique_id.is_some_and(|v|v!=0))
    }
    fn item_at<'a>(m:&'a HeroUiReadModel,cell:HeroCell)->Option<&'a ItemModel> {
        let (inventory,container)=match cell.grid {HeroGrid::Inventory=>(m.personal.as_ref()?,0),HeroGrid::HeroInventory=>(&m.hero.inventory_view,0),HeroGrid::HeroEquipment=>(&m.hero.inventory_view,2)};
        inventory.items.iter().find(|i|i.container==container && i.slot==cell.slot && i.quantity>0)
    }
    fn hit(&self,m:&HeroUiReadModel,p:[f32;2])->Option<Hit> {
        let (window,action)=hero_dialog::geometry::hit(&self.ui,p,self.ui.front);
        if self.ui.modal() {return Some(Hit::Hero(window,action));}
        if self.ui.cross.player_front {if let Some(hit)=self.personal_hit(m,p){return Some(hit);}}
        if window.is_some() {return Some(Hit::Hero(window,action));}
        self.personal_hit(m,p)
    }
    fn personal_hit(&self,m:&HeroUiReadModel,p:[f32;2])->Option<Hit> {
        if self.personal_open && self.ui.inventory_open && m.personal.is_some() {
            let x=p[0]-self.personal_position[0] as f32;let y=p[1]-self.personal_position[1] as f32;
            if CrystalRect::new(0.,0.,316.,236.).contains(x,y) {
                if CrystalRect::new(289.,3.,24.,21.).contains(x,y) {return Some(Hit::PersonalClose);}
                for (page,left) in [(0,6.),(1,76.)] {if CrystalRect::new(left,7.,72.,23.).contains(x,y) {return Some(Hit::PersonalPage(page));}}
                let origin=crate::crystal_ui::panel_layouts::INVENTORY_GRID_ORIGIN;
                for local in 0..40 {let rect=bag_paint::bag_cell_rect(local).unwrap();let slot=u32::from(self.personal_page)*40+local as u32;
                    if slot<u32::from(m.personal.as_ref().unwrap().bag_slot_capacity()) && CrystalRect::new(origin.x as f32+rect.left,origin.y as f32+rect.top,rect.width,rect.height).contains(x,y) {return Some(Hit::PersonalCell(slot));}}
                return Some(Hit::PersonalBackground);
            }
        } None
    }
    fn cell(hit:Hit)->Option<(HeroCell,HeroOrigin)> {match hit {
        Hit::PersonalCell(slot)=>Some((HeroCell{grid:HeroGrid::Inventory,slot},HeroOrigin::Inventory)),
        Hit::Hero(window,Some(HeroAction::InventoryCell(slot)))=>Some((HeroCell{grid:HeroGrid::HeroInventory,slot:u32::from(slot)},if window==Some(HeroWindow::Belt){HeroOrigin::Belt}else{HeroOrigin::Inventory})),
        Hit::Hero(_,Some(HeroAction::EquipmentCell(slot)))=>Some((HeroCell{grid:HeroGrid::HeroEquipment,slot:u32::from(slot)},HeroOrigin::Character)),_=>None}}
    fn emit(&mut self,c:&HeroUiContext,q:&mut HeroIntentQueue,body:HeroIntentBody) {
        if q.0.len()>=16 || self.intent_sequence>=MAX_SAFE {self.cancel();return;}
        self.intent_sequence+=1;q.0.push(HeroUiIntent{stamp:c.stamp.clone(),intent_sequence:self.intent_sequence,body});
    }
    fn action(&mut self,c:&HeroUiContext,m:&HeroUiReadModel,q:&mut HeroIntentQueue,origin:HeroOrigin,action:HeroSemanticAction,old_key:Option<u8>) {
        if c.pending || q.0.iter().any(|i|matches!(i.body,HeroIntentBody::Action{..})) {return;}
        let Some(basis)=crate::hero_action_basis::capture_action_basis(m,c.windows,&action,old_key) else{return;};
        self.emit(c,q,HeroIntentBody::Action{origin,action,old_key,basis});
    }
    fn window_action(&mut self,c:&HeroUiContext,q:&mut HeroIntentQueue) {let windows=HeroWindows{inventory_open:self.ui.inventory_open,character_open:self.ui.character_open,
        character_page:self.ui.page.into(),belt_visible:self.ui.belt_visible,belt_vertical:self.ui.belt_vertical}; self.emit(c,q,HeroIntentBody::Windows{windows});}
    fn use_cell(&mut self,c:&HeroUiContext,m:&HeroUiReadModel,q:&mut HeroIntentQueue,cell:HeroCell,origin:HeroOrigin,confirmed:bool) {
        if c.pending || c.now_ms<self.next_use_ms || Self::item(m,cell).is_none() {return;}
        if cell.grid==HeroGrid::HeroEquipment {if hero_dialog::item_use::remove_plan(&m.hero,cell.slot as u8).is_some() {self.action(c,m,q,origin,HeroSemanticAction::Remove{from:cell.slot,to:None},None);}return;}
        if cell.grid!=HeroGrid::HeroInventory {return;}
        let plan=hero_dialog::item_use::plan(&m.hero,cell.slot as u8);
        match plan {
            hero_dialog::item_use::HeroUsePlan::ConfirmPotion(packet) if !confirmed=> {
                if let Some(item)=Self::item(m,cell) {self.confirmation=Some(Selected{cell,item:item.clone(),origin});self.ui.use_confirmation=Some(packet);}
            }
            hero_dialog::item_use::HeroUsePlan::ConfirmPotion(_) | hero_dialog::item_use::HeroUsePlan::Packet(_)=> {
                self.action(c,m,q,origin,HeroSemanticAction::Use{slot:cell.slot,confirmed},None);
            } _=>{}
        }
    }
    fn click_cell(&mut self,c:&HeroUiContext,m:&HeroUiReadModel,q:&mut HeroIntentQueue,cell:HeroCell,origin:HeroOrigin) {
        if c.pending {return;}
        if origin==HeroOrigin::Belt {self.use_cell(c,m,q,cell,origin,false);return;}
        let item=Self::item(m,cell);
        let double=item.and_then(|i|i.unique_id).is_some_and(|id|self.last_click.is_some_and(|(old,uid,at)|old==cell && uid==id && c.now_ms>=at && c.now_ms-at<=500));
        self.last_click=item.and_then(|i|i.unique_id).map(|id|(cell,id,c.now_ms));
        if double {self.last_click=None;self.selected=None;self.ui.selected=None;self.use_cell(c,m,q,cell,origin,false);return;}
        if let Some(old)=self.selected.take() {
            self.ui.selected=None;
            if old.cell==cell {return;}
            if Self::item(m,old.cell)!=Some(&old.item) {return;}
            if let Some(action)=cell_action(m,old.cell,cell,&old.item) {self.action(c,m,q,old.origin,action,None);return;}
        }
        self.selected=item.cloned().map(|item|Selected{cell,item,origin});
        self.ui.selected=self.selected.as_ref().filter(|s|s.cell.grid!=HeroGrid::Inventory).and_then(|s|s.item.unique_id.map(|id|(s.cell.grid==HeroGrid::HeroEquipment,s.cell.slot as u8,id)));
    }
    fn activate(&mut self,c:&HeroUiContext,m:&HeroUiReadModel,q:&mut HeroIntentQueue,hit:Hit) {
        if let Some((cell,origin))=Self::cell(hit) {self.click_cell(c,m,q,cell,origin);return;}
        match hit {
            Hit::PersonalPage(page) if page==0 || m.personal.as_ref().is_some_and(InventoryModel::second_bag_unlocked)=>{self.personal_page=page;self.selected=None;self.last_click=None;self.ui.selected=None;}
            Hit::PersonalClose=>{self.personal_open=false;self.selected=None;self.ui.selected=None;}
            Hit::Hero(_,Some(action))=>match action {
                HeroAction::CloseInventory=>{self.ui.inventory_open=false;self.cancel();self.window_action(c,q);}
                HeroAction::CloseCharacter=>{self.ui.character_open=false;self.cancel();self.window_action(c,q);}
                HeroAction::Page(page)=>{self.ui.page=page;self.cancel();self.window_action(c,q);}
                HeroAction::BeltClose=>{self.ui.belt_visible=false;self.cancel();self.window_action(c,q);}
                HeroAction::BeltRotate=>{self.ui.belt_vertical=!self.ui.belt_vertical;self.ui.belt_position=None;self.cancel();self.window_action(c,q);}
                HeroAction::Previous=>self.ui.skill_start=self.ui.skill_start.saturating_sub(7),
                HeroAction::Next=>{if m.hero.info.as_ref().is_some_and(|i|self.ui.skill_start+7<i.magics.len()){self.ui.skill_start+=7;}}
                HeroAction::Skill(index) if !c.pending=>self.ui.assign.show(&m.hero,index),
                HeroAction::AssignKey(key) if !c.pending=>self.ui.assign.choose(key),
                HeroAction::AssignSave if !c.pending=> {
                    let a=&self.ui.assign;let valid=m.hero.info.as_ref().is_some_and(|i|i.object_id==a.actor && i.hp>0 && m.hero.session_epoch==a.epoch
                        && m.hero.hero_generation==a.generation && i.magics.iter().any(|m|Some(m.spell)==a.spell && m.key==a.old_key));
                    if valid && a.open {if a.key==0 && a.old_key==0 {self.ui.assign=Default::default();}else if let Some(spell)=a.spell {
                        self.action(c,m,q,HeroOrigin::Character,HeroSemanticAction::MagicKey{spell:format!("{spell:?}"),key:a.key},Some(a.old_key));}}
                }
                HeroAction::AutoHp|HeroAction::AutoMp if !c.pending=>{if m.hero.info.as_ref().is_some_and(|i|i.auto_pot) {self.ui.amount=Some((if action==HeroAction::AutoHp{12}else{13},CrystalAmountInput::new(99)));}}
                HeroAction::AmountCancel=>self.ui.amount=None,
                HeroAction::AmountConfirm if !c.pending=> {if let Some((stat,input))=self.ui.amount.as_ref(){if let Some(value)=input.amount(){self.action(c,m,q,HeroOrigin::Inventory,HeroSemanticAction::AutoPotValue{stat:*stat,value},None);}}}
                HeroAction::AutoPotItem(hp) if !c.pending=> {
                    let selected=self.selected.as_ref();let slot=selected.filter(|s|s.cell.grid==HeroGrid::HeroInventory && Self::item(m,s.cell)==Some(&s.item)
                        && s.item.tooltip_source.as_ref().is_some_and(|t|t.info.item_type==13 && t.info.shape<=1)).map(|s|s.cell.slot);
                    if selected.is_none() || slot.is_some() {self.action(c,m,q,HeroOrigin::Inventory,HeroSemanticAction::AutoPotItem{grid:if hp{HeroPotGrid::HeroHpItem}else{HeroPotGrid::HeroMpItem},slot},None);}
                }
                HeroAction::UseCancel=>{self.confirmation=None;self.ui.use_confirmation=None;}
                HeroAction::UseConfirm if !c.pending=> {if let Some(draft)=self.confirmation.clone(){if Self::item(m,draft.cell)==Some(&draft.item)
                    && matches!(hero_dialog::item_use::plan(&m.hero,draft.cell.slot as u8),hero_dialog::item_use::HeroUsePlan::ConfirmPotion(_)){self.use_cell(c,m,q,draft.cell,draft.origin,true);}else{self.confirmation=None;self.ui.use_confirmation=None;}}}
                _=>{}
            },_=>{}
        }
    }
    pub fn accepted(&mut self,intent:&HeroUiIntent,accepted:bool,now_ms:u64) {
        if self.stamp.as_ref()!=Some(&intent.stamp) {return;}
        if accepted {if let HeroIntentBody::Action{action:HeroSemanticAction::Use{confirmed,..},..}=intent.body {
            self.next_use_ms=now_ms.saturating_add(if confirmed {100}else{300});}}
        if accepted && matches!(intent.body,HeroIntentBody::Action{..}) {self.selected=None;self.ui.selected=None;self.last_click=None;self.ui.amount=None;self.ui.use_confirmation=None;self.confirmation=None;
            if matches!(intent.body,HeroIntentBody::Action{action:HeroSemanticAction::MagicKey{..},..}) {self.ui.assign=Default::default();}}
    }
    pub fn process(&mut self,c:&HeroUiContext,m:&HeroUiReadModel,edge:HeroInputEdge,q:&mut HeroIntentQueue)->bool {
        if !edge.valid() || edge.stamp!=c.stamp || self.stamp.as_ref()!=Some(&c.stamp) || edge.sequence<=self.sequence {return false;}
        self.sequence=edge.sequence;
        if edge.phase=="cancel" {if self.held.as_ref().is_some_and(|h|h.pointer!=edge.pointer_id){return false;}self.cancel();return true;}
        if !c.active || !c.ready || !c.input_enabled || !c.presentation.is_some_and(HeroPresentation::fits) {self.cancel();return false;}
        if edge.phase=="key" {
            if edge.key=="Escape" {let consumed=self.captures_escape();self.cancel();return consumed;}
            if self.ui.amount.is_some() {
                if edge.key=="Enter" {self.activate(c,m,q,Hit::Hero(None,Some(HeroAction::AmountConfirm)));}
                else if let Some((_,input))=self.ui.amount.as_mut(){if edge.key=="Backspace" {input.backspace();}else if edge.control && edge.key=="KeyA"{input.select_all=true;}else if !edge.control{input.push_text(&edge.text);}}
                return true;
            }
            if edge.key=="Enter" && self.ui.use_confirmation.is_some() {self.activate(c,m,q,Hit::Hero(None,Some(HeroAction::UseConfirm)));return true;}
            return self.ui.modal();
        }
        if self.held.as_ref().is_some_and(|h|h.pointer!=edge.pointer_id) {return false;}
        let hit=self.hit(m,[edge.x,edge.y]);self.ui.hovered=hit.and_then(|h|match h{Hit::Hero(_,a)=>a,_=>None});
        if edge.phase=="move" {
            if let Some(held)=self.held.as_mut().filter(|h|h.pointer==edge.pointer_id){
                if let Some((window,offset))=self.ui.dragging {if let Some(rect)=hero_dialog::geometry::window_rect(&self.ui,window){
                    let p=c.presentation.unwrap();let pos=[(edge.x-offset[0]).clamp(0.,p.logical_width-rect.width-1.) as i32,(edge.y-offset[1]).clamp(0.,p.logical_height-rect.height-1.) as i32];
                    match window{HeroWindow::Inventory=>self.ui.inventory_position=pos,HeroWindow::Character=>self.ui.character_position=Some(pos),HeroWindow::Belt=>self.ui.belt_position=Some(pos)}held.dragged=true;}}
                if let Some(offset)=self.personal_drag {if let Some(p)=c.presentation {
                    self.personal_position=[(edge.x-offset[0]).clamp(0.,p.logical_width-317.) as i32,(edge.y-offset[1]).clamp(0.,p.logical_height-237.) as i32];held.dragged=true;}}
            }return self.held.is_some() || hit.is_some();
        }
        if edge.phase=="down" {
            if self.held.is_some(){return false;}let Some(hit)=hit else{return false;};
            if let Hit::Hero(Some(window),_)=hit {self.ui.front=window;self.ui.cross.player_front=false;}
            else if !self.ui.modal() {self.ui.cross.player_front=true;}
            if edge.button==2 {
                if self.selected.take().is_some(){self.ui.selected=None;self.last_click=None;}else if !edge.control && !edge.shift && !self.ui.modal(){
                    if let Some((cell,origin))=Self::cell(hit){self.use_cell(c,m,q,cell,origin,false);}}
                self.held=Some(Held{pointer:edge.pointer_id,hit,button:2,dragged:true});return true;
            }
            self.held=Some(Held{pointer:edge.pointer_id,hit,button:0,dragged:false});
            self.ui.armed=match hit{Hit::Hero(_,action)=>action,_=>None};
            if hit==Hit::PersonalBackground {self.personal_drag=Some([edge.x-self.personal_position[0] as f32,edge.y-self.personal_position[1] as f32]);}
            if let Hit::Hero(Some(window),None)=hit{self.ui.front=window;if let Some(rect)=hero_dialog::geometry::window_rect(&self.ui,window){self.ui.dragging=Some((window,[edge.x-rect.left,edge.y-rect.top]));}}
            return true;
        }
        if edge.phase=="up" {
            if !self.held.as_ref().is_some_and(|h|h.pointer==edge.pointer_id && h.button==edge.button){return false;}
            let held=self.held.take().unwrap();self.ui.dragging=None;self.personal_drag=None;self.ui.armed=None;
            if held.dragged || held.button==2 {return true;}
            if let Some(to)=hit {
                if held.hit==to {self.activate(c,m,q,to);}else if let (Some((from,origin)),Some((dest,_)))=(Self::cell(held.hit),Self::cell(to)){
                    if origin!=HeroOrigin::Belt {self.ui.selected=None;self.last_click=None;self.selected=Self::item(m,from).cloned().map(|item|Selected{cell:from,item,origin});
                        if let Some(source)=self.selected.take(){if let Some(action)=cell_action(m,source.cell,dest,&source.item){self.action(c,m,q,origin,action,None);}}}
                }
            }return true;
        } false
    }
}
pub fn cell_action(m:&HeroUiReadModel,from:HeroCell,to:HeroCell,source:&ItemModel)->Option<HeroSemanticAction>{
    if from==to || HeroUiState::item(m,from)!=Some(source){return None;}
    let target=HeroUiState::item_at(m,to);
    if target.is_some_and(|item|!item.unique_id.is_some_and(|uid|uid!=0)) {return None;}
    let grid=|g|match g{HeroGrid::HeroInventory=>MirGridType::HeroInventory,HeroGrid::HeroEquipment=>MirGridType::HeroEquipment,HeroGrid::Inventory=>MirGridType::Inventory};
    if target.is_some_and(|t|hero_dialog::item_use::merge_packet(source,t,grid(from.grid),grid(to.grid)).is_some())
        && !(from.grid==HeroGrid::Inventory && to.grid==HeroGrid::Inventory) && !([from.grid,to.grid].contains(&HeroGrid::Inventory)&&[from.grid,to.grid].contains(&HeroGrid::HeroEquipment)){return Some(HeroSemanticAction::Merge{from,to});}
    let limit=match to.grid{HeroGrid::HeroInventory=>u32::from(m.hero.inventory_view.capacity),HeroGrid::HeroEquipment=>14,HeroGrid::Inventory=>u32::from(m.personal.as_ref()?.bag_slot_capacity())};if to.slot>=limit{return None;}
    match (from.grid,to.grid){
        (HeroGrid::HeroInventory,HeroGrid::HeroInventory)=>Some(HeroSemanticAction::Move{from:from.slot,to:to.slot}),
        (HeroGrid::HeroInventory,HeroGrid::HeroEquipment)=>Some(HeroSemanticAction::Equip{from:from.slot,to:to.slot}),
        (HeroGrid::HeroEquipment,HeroGrid::HeroInventory) if target.is_none()=>Some(HeroSemanticAction::Remove{from:from.slot,to:Some(to.slot)}),
        (HeroGrid::Inventory,HeroGrid::HeroInventory)|(HeroGrid::HeroInventory,HeroGrid::Inventory)=>{
            let cell=|c:HeroCell|match c.grid{HeroGrid::Inventory=>hero_dialog::cross::Cell::Player(c.slot),_=>hero_dialog::cross::Cell::Hero(c.slot as u8)};
            let packet=hero_dialog::cross::transfer_packet(cell(from),cell(to),source,target)?;match packet{
                ClientPacket::TransferHeroItem{..}=>Some(HeroSemanticAction::Transfer{from:from.slot,to:to.slot}),
                ClientPacket::TakeBackHeroItem{..}=>Some(HeroSemanticAction::TakeBack{from:from.slot,to:to.slot}),_=>None}}
        _=>None}
}
pub fn input_regions(c:&HeroUiContext,state:&HeroUiState,m:&HeroUiReadModel)->Vec<CrystalRect>{
    if state.ui.modal() {return c.presentation.map(|p|vec![CrystalRect::new(0.,0.,p.logical_width,p.logical_height)]).unwrap_or_default();}
    let mut regions=[HeroWindow::Inventory,HeroWindow::Character,HeroWindow::Belt].into_iter().filter_map(|w|hero_dialog::geometry::window_rect(&state.ui,w)).collect::<Vec<_>>();
    if state.ui.inventory_open && state.personal_open && m.personal.is_some(){regions.push(CrystalRect::new(state.personal_position[0] as f32,state.personal_position[1] as f32,316.,236.));}regions
}
/// Preload all Hero controls and real item/appearance variants before takeover.
pub fn required_assets(m: &HeroUiReadModel) -> Vec<String> {
    let mut paths = Vec::new();
    for (library, indices) in [
        ("Prguse", vec![238,340,341,360,396,397,398,399,710,1422,1423,1428,1429,1656,1657,1658,1921,1923,1924,1925,1926,1927,1928,1934,1935,1936,1937,1938,1939,1940,1943,1946]),
        ("Title", vec![156,157,158,200,201,202,203,204,205,206,207,208,210,211,212,287,288,289,500,501,502,503,504,506,507,508,516,517,560,561,562,563,564,565]),
        ("Prguse2", (1290..=1324).chain(360..=362).collect::<Vec<u16>>()),
        ("Items", vec![116]),
    ] { paths.extend(indices.into_iter().map(|index|format!("original-ui/{library}/{index}.png"))); }
    if let Some(info)=m.hero.info.as_ref() {
        paths.push(format!("original-ui/Prguse/{}.png",100+info.class as u16));
        for magic in &info.magics {for index in [u16::from(magic.icon)*2,u16::from(magic.icon)*2+1] {paths.push(format!("original-ui/MagIcon2/{index}.png"));}}
    }
    for inventory in [&m.hero.inventory_view,&m.hero.auto_pot_view].into_iter().chain(m.personal.iter()) {
        paths.extend(inventory.items.iter().filter(|item|matches!(item.container,0|2)).filter_map(ItemModel::user_item_image_index).map(|index|format!("original-ui/Items/{index}.png")));
    }
    if m.personal.is_some() {paths.extend(bag_paint::PORTABLE_BAG_REQUIRED_SKINS.iter().map(|path|(*path).to_owned()));}
    if let Some(actor)=hero_dialog::render::actor_view(&m.hero) {
        paths.extend(crate::crystal_ui::character_page::crystal_character_paper_doll_layers(&m.hero.inventory_view,&actor).into_iter().map(|layer|layer.frame.asset_path()));
    }
    paths.sort();paths.dedup();paths
}
/// DrawBlend wings require their actual material, in addition to loaded PNGs.
pub fn appearance_materials_ready(m:&HeroUiReadModel,materials:Option<&CrystalCharacterWingMaterials>)->bool {
    let Some(actor)=hero_dialog::render::actor_view(&m.hero) else{return false;};
    crate::crystal_ui::character_page::crystal_character_paper_doll_layers(&m.hero.inventory_view,&actor).into_iter().all(|layer|
        layer.blend!=crate::crystal_ui::character_page::CrystalCharacterBlend::DrawBlend || materials.is_some_and(|materials|materials.get(layer.frame.index).is_some()))
}
#[derive(Component,Debug,Clone)]
pub struct SharedHeroRoot { pub stamp:HeroStamp,pub revision:u64,pub frame_sequence:u64,pub regions:Vec<CrystalRect> }
#[derive(Component)]
pub struct HeroPersonalRoot;
#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub struct HeroPaintSet;
fn reduce(c:Res<HeroUiContext>,m:Res<HeroUiReadModel>,mut state:ResMut<HeroUiState>,mut edges:ResMut<HeroInputQueue>,mut q:ResMut<HeroIntentQueue>){
    state.reconcile(&c,&m);for edge in std::mem::take(&mut edges.0){state.process(&c,&m,edge,&mut q);}
}
fn render(mut commands:Commands,c:Res<HeroUiContext>,m:Res<HeroUiReadModel>,state:Res<HeroUiState>,old:Query<Entity,With<SharedHeroRoot>>,
    server:Option<Res<AssetServer>>,surface:Option<Res<crate::crystal_ui::hud::SharedHudSurface>>,wings:Option<Res<CrystalCharacterWingMaterials>>){
    for entity in &old{commands.entity(entity).despawn();}
    let (Some(server),Some(surface))=(server,surface) else{return;};if !c.active{return;}let Some(p)=c.presentation.filter(|p|p.fits()) else{return;};
    let font=TextFont{font:FontSource::Handle(surface.font.clone()),font_size:FontSize::Px(32./3.),..default()};
    let root=commands.spawn((SharedHeroRoot{stamp:c.stamp.clone(),revision:c.revision,frame_sequence:c.frame_sequence,regions:input_regions(&c,&state,&m)},absolute_node(CrystalRect::new(0.,0.,p.logical_width,p.logical_height)),
        UiTargetCamera(surface.camera),FocusPolicy::Pass,Visibility::Hidden)).id();
    hero_dialog::render::paint(&mut commands,root,&state.ui,&m.hero,&server,wings.as_deref(),&font,c.now_ms);
    if c.pending {commands.entity(root).with_children(|parent|{parent.spawn((
        absolute_node(CrystalRect::new(12.,8.,216.,20.)),GlobalZIndex(1002),FocusPolicy::Pass,
        Text::new("英雄操作处理中…"),font.clone(),TextColor(Color::srgb(1.,0.85,0.2))));});}
    if state.personal_open && state.ui.inventory_open {if let Some(personal)=m.personal.as_ref(){
        commands.entity(root).with_children(|parent|{parent.spawn((HeroPersonalRoot,absolute_node(CrystalRect::new(state.personal_position[0] as f32,state.personal_position[1] as f32,316.,236.)),
            GlobalZIndex(if state.ui.cross.player_front {984}else{979}),FocusPolicy::Block)).with_children(|body|{
            let options=BagPaintOptions{page:state.personal_page,delete_mode:false,weight_ratio:None,free_slots:None,font:font.clone(),stack_split_hint:Default::default()};
            bag_paint::paint_crystal_hero_transfer_inventory(body,&server,personal,&m.player,&options,|_:BagPaintAction|(),|cell|BagCellPolicy::new(true,cell.container==0,!c.input_enabled),|body,cell|{
                if state.selected.as_ref().is_some_and(|s|s.cell.grid==HeroGrid::Inventory && s.cell.slot==cell.slot){body.spawn((Node{border:UiRect::all(Val::Px(1.)),..absolute_node(cell.rect)},BorderColor::all(Color::srgb(1.,0.85,0.2))));}
            });
        });});}}
}
fn button_visuals(state:Res<HeroUiState>,assets:Option<Res<AssetServer>>,mut buttons:Query<(&hero_dialog::render::HeroButtonVisual,&mut ImageNode)>){
    if let Some(assets)=assets{for(button,mut image)in &mut buttons{image.image=assets.load(button.image_path(&state.ui));}}
}
pub struct Mir2PortableHeroUiPlugin;
impl Plugin for Mir2PortableHeroUiPlugin{fn build(&self,app:&mut App){app.init_resource::<HeroUiContext>().init_resource::<HeroUiReadModel>().init_resource::<HeroUiState>()
    .init_resource::<HeroInputQueue>().init_resource::<HeroIntentQueue>().add_systems(Update,(reduce,render,button_visuals).chain().in_set(HeroPaintSet)
        .after(crate::pending_operations::PendingLifecycleSet::Ingest).before(layout_original_item_images));}}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_capture_uses_actual_private_selection_held_and_modal_state() {
        let c=context();let m=model();let mut state=HeroUiState::default();let mut intents=HeroIntentQueue::default();
        state.reconcile(&c,&m);assert!(!state.captures_escape());
        assert!(state.process(&c,&m,edge(&c,1,7,"down",2.,2.,0),&mut intents));
        assert!(state.captures_escape());state.cancel();assert!(!state.captures_escape());
        state.selected=Some(Selected {cell:HeroCell {grid:HeroGrid::Inventory,slot:0},
            item:ItemModel::default(),origin:HeroOrigin::Inventory});
        assert!(state.ui.selected.is_none());assert!(state.captures_escape());
        let mut escape=edge(&c,2,7,"key",0.,0.,0);escape.key="Escape".into();
        assert!(state.process(&c,&m,escape.clone(),&mut intents));assert!(!state.captures_escape());
        escape.sequence=3;assert!(!state.process(&c,&m,escape.clone(),&mut intents));assert!(intents.0.is_empty());
        state.ui.amount=Some((12,CrystalAmountInput::new(99)));
        assert!(state.ui.modal());assert!(state.captures_escape());
        escape.sequence=4;assert!(state.process(&c,&m,escape,&mut intents));
        assert!(!state.captures_escape());assert!(state.ui.amount.is_none());assert!(intents.0.is_empty());
    }

    fn context() -> HeroUiContext {
        HeroUiContext { stamp:HeroStamp {scope:HeroScope {run_generation:1,connection_generation:1,session_generation:1,
            scene_revision:1,player_object_id:42,map_file_name:"TestMap".into()},hero_object_id:12,hero_generation:1,
            hud_generation:1,model_revision:1,presentation_revision:1,window_epochs:[1;3]},revision:1,frame_sequence:2,
            windows:HeroWindows {inventory_open:true,..Default::default()},presentation:Some(HeroPresentation {
            logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false}),active:true,ready:true,input_enabled:true,
            now_ms:100,..Default::default() }
    }
    fn model() -> HeroUiReadModel {HeroUiReadModel {personal:Some(InventoryModel::default()),..Default::default()}}
    fn edge(c:&HeroUiContext,sequence:u64,pointer_id:u64,phase:&str,x:f32,y:f32,button:u8)->HeroInputEdge {
        HeroInputEdge {stamp:c.stamp.clone(),sequence,pointer_id,phase:phase.into(),x,y,button,key:String::new(),text:String::new(),control:false,shift:false}
    }
    fn test_basis(c:&HeroUiContext,action:&HeroSemanticAction,old_key:Option<u8>)->crate::hero_action_basis::HeroActionBasis {
        let mut read=crate::hero_action_basis::tests::read_model();
        read.hero.info.as_mut().unwrap().object_id=c.stamp.hero_object_id;
        if let HeroSemanticAction::Use{slot,..}=action {
            crate::hero_action_basis::tests::add(&mut read,HeroGrid::HeroInventory,*slot,7,1,1,13);
        } else {
            read.hero.info.as_mut().unwrap().magics.push(serde_json::from_value(serde_json::json!({
                "name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,"level1":1,
                "level2":2,"level3":3,"need1":1,"need2":2,"need3":3,"level":1,"key":17,
                "experience":0,"delay":100,"range":8,"cast_time":0})).unwrap());
        }
        crate::hero_action_basis::capture_action_basis(&read,c.windows,action,old_key).unwrap()
    }
    fn intent(c:&HeroUiContext,confirmed:bool)->HeroUiIntent {HeroUiIntent {stamp:c.stamp.clone(),intent_sequence:1,
        body:HeroIntentBody::Action {origin:HeroOrigin::Inventory,action:HeroSemanticAction::Use {slot:2,confirmed},old_key:None,
            basis:test_basis(c,&HeroSemanticAction::Use {slot:2,confirmed},None)}}}

    #[test]
    fn close_hero_inventory_removes_personal_hit_region_in_the_same_reducer_frame() {
        let c = context(); let m = model();
        let mut state = HeroUiState::default(); let mut q = HeroIntentQueue::default();
        state.reconcile(&c, &m);
        assert!(input_regions(&c, &state, &m).iter().any(|r| r.width == 316.));
        assert!(state.process(&c, &m, edge(&c, 1, 7, "down", 304., 8., 0), &mut q));
        assert!(state.process(&c, &m, edge(&c, 2, 7, "up", 304., 8., 0), &mut q));
        assert!(!state.ui.inventory_open);
        assert!(c.windows.inventory_open);
        assert!(input_regions(&c, &state, &m).is_empty());
        assert_eq!(q.0.len(), 1);
        assert!(matches!(&q.0[0].body, HeroIntentBody::Windows { windows } if !windows.inventory_open));
        state.reconcile(&c, &m);
        assert!(input_regions(&c, &state, &m).iter().any(|r| r.width == 316.));
    }

    #[test]
    fn secondary_pointer_does_not_consume_primary_button_or_drag() {
        let c=context();let m=model();let mut state=HeroUiState::default();let mut q=HeroIntentQueue::default();state.reconcile(&c,&m);
        assert!(state.process(&c,&m,edge(&c,1,7,"down",304.,8.,0),&mut q));
        assert!(!state.process(&c,&m,edge(&c,2,8,"up",304.,8.,0),&mut q));
        assert!(!state.process(&c,&m,edge(&c,3,8,"cancel",304.,8.,0),&mut q));
        assert!(!state.process(&c,&m,edge(&c,4,7,"up",304.,8.,2),&mut q));
        assert_eq!(state.held.as_ref().unwrap().pointer,7);assert_eq!(state.ui.armed,Some(HeroAction::CloseInventory));
        assert!(state.process(&c,&m,edge(&c,5,7,"up",304.,8.,0),&mut q));
        assert!(state.held.is_none());assert_eq!(q.0.len(),1);assert!(!state.ui.inventory_open);
        state.reconcile(&c,&m);assert!(state.process(&c,&m,edge(&c,6,7,"down",2.,2.,0),&mut q));
        assert!(!state.process(&c,&m,edge(&c,7,8,"move",500.,400.,0),&mut q));
        assert!(state.ui.dragging.is_some());assert!(state.process(&c,&m,edge(&c,8,7,"up",2.,2.,0),&mut q));
        assert!(state.ui.dragging.is_none());
    }
    #[test]
    fn new_source_stamp_cancels_held_action() {
        let mut c=context();let m=model();let mut state=HeroUiState::default();let mut q=HeroIntentQueue::default();state.reconcile(&c,&m);
        let old=edge(&c,2,7,"up",304.,8.,0);assert!(state.process(&c,&m,edge(&c,1,7,"down",304.,8.,0),&mut q));
        c.stamp.model_revision+=1;state.reconcile(&c,&m);assert!(state.held.is_none());assert!(state.ui.armed.is_none());
        assert!(!state.process(&c,&m,old,&mut q));assert!(q.0.is_empty());assert!(state.ui.inventory_open);
    }
    #[test]
    fn hero_reopen_restores_personal_transfer_window() {
        let mut c=context();let m=model();let mut state=HeroUiState::default();let mut q=HeroIntentQueue::default();state.reconcile(&c,&m);
        assert!(state.process(&c,&m,edge(&c,1,7,"down",300.,478.,0),&mut q));
        assert!(state.process(&c,&m,edge(&c,2,7,"up",300.,478.,0),&mut q));assert!(!state.personal_open);
        c.windows.inventory_open=false;c.stamp.window_epochs[0]+=1;state.reconcile(&c,&m);
        c.windows.inventory_open=true;c.stamp.window_epochs[0]+=1;state.reconcile(&c,&m);assert!(state.personal_open);
    }
    #[test]
    fn locked_personal_page_stays_in_first_bag_and_shrink_resets_it() {
        let c=context();let mut m=model();m.personal.as_mut().unwrap().capacity=46;
        let mut state=HeroUiState::default();let mut q=HeroIntentQueue::default();state.reconcile(&c,&m);
        state.activate(&c,&m,&mut q,Hit::PersonalPage(1));assert_eq!(state.personal_page,0);
        m.personal.as_mut().unwrap().capacity=54;state.activate(&c,&m,&mut q,Hit::PersonalPage(1));assert_eq!(state.personal_page,1);
        m.personal.as_mut().unwrap().capacity=46;state.reconcile(&c,&m);assert_eq!(state.personal_page,0);
    }
    #[test]
    fn modal_blocks_stage_and_pending_does_not_settle_drafts() {
        let mut c=context();c.pending=true;let m=model();let mut state=HeroUiState::default();let mut q=HeroIntentQueue::default();state.reconcile(&c,&m);
        let mut amount=CrystalAmountInput::new(99);amount.push_text("35");state.ui.amount=Some((12,amount));
        assert_eq!(input_regions(&c,&state,&m),vec![CrystalRect::new(0.,0.,1024.,768.)]);
        assert!(state.process(&c,&m,edge(&c,1,7,"down",1000.,730.,0),&mut q));
        assert!(state.process(&c,&m,edge(&c,2,7,"up",1000.,730.,0),&mut q));
        state.activate(&c,&m,&mut q,Hit::Hero(None,Some(HeroAction::AmountConfirm)));assert!(q.0.is_empty());
        state.reconcile(&c,&m);assert!(state.ui.amount.is_some());assert!(state.ui.pending.is_none());assert!(state.ui.config_pending.is_none());
    }
    #[test]
    fn rejected_and_stale_use_feedback_never_advances_cadence() {
        let mut c=context();let m=model();let mut state=HeroUiState::default();state.reconcile(&c,&m);let old=intent(&c,false);
        state.accepted(&old,false,100);assert_eq!(state.next_use_ms,0);
        state.accepted(&old,true,100);assert_eq!(state.next_use_ms,400);
        c.stamp.model_revision+=1;state.reconcile(&c,&m);state.ui.amount=Some((12,CrystalAmountInput::new(99)));
        state.accepted(&old,true,1000);assert_eq!(state.next_use_ms,400);assert!(state.ui.amount.is_some());
        let current=intent(&c,true);state.accepted(&current,true,1000);assert_eq!(state.next_use_ms,1100);assert!(state.ui.amount.is_none());
        assert!(state.ui.pending.is_none());
    }
    #[test]
    fn hero_intent_encodes_host_old_key_without_native_pending() {
        let c=context();let m=model();let mut state=HeroUiState::default();state.reconcile(&c,&m);
        let intent=HeroUiIntent {stamp:c.stamp.clone(),intent_sequence:1,body:HeroIntentBody::Action {origin:HeroOrigin::Character,
            action:HeroSemanticAction::MagicKey {spell:"FireBall".into(),key:18},old_key:Some(17),
            basis:test_basis(&c,&HeroSemanticAction::MagicKey {spell:"FireBall".into(),key:18},Some(17))}};
        let encoded=serde_json::to_value(&intent).unwrap();assert_eq!(encoded["oldKey"],17);assert!(encoded.get("old_key").is_none());
        state.accepted(&intent,true,100);assert!(state.ui.assign.pending.is_none());assert!(state.ui.pending.is_none());
    }
    #[test]
    fn personal_front_matches_hit_order_and_full_stage_modal_wins() {
        let c=context();let m=model();let mut state=HeroUiState::default();state.reconcile(&c,&m);state.personal_position=[0,0];
        state.ui.cross.player_front=false;assert_eq!(state.hit(&m,[300.,8.]),Some(Hit::Hero(Some(HeroWindow::Inventory),Some(HeroAction::CloseInventory))));
        state.ui.cross.player_front=true;assert_eq!(state.hit(&m,[300.,8.]),Some(Hit::PersonalClose));
        state.ui.amount=Some((12,CrystalAmountInput::new(99)));assert_eq!(state.hit(&m,[300.,8.]),Some(Hit::Hero(None,None)));
    }
    #[test]
    fn anonymous_and_stale_items_have_no_transfer_custody() {
        let c=context();let mut m=model();let mut state=HeroUiState::default();state.reconcile(&c,&m);let mut q=HeroIntentQueue::default();
        let item=ItemModel {unique_id:Some(0),container:0,slot:0,quantity:1,..Default::default()};m.personal.as_mut().unwrap().items.push(item.clone());
        let from=HeroCell {grid:HeroGrid::Inventory,slot:0};let to=HeroCell {grid:HeroGrid::HeroInventory,slot:2};assert!(cell_action(&m,from,to,&item).is_none());
        m.personal.as_mut().unwrap().items[0].unique_id=Some(17);assert!(cell_action(&m,from,to,&item).is_none());
        let actual=m.personal.as_ref().unwrap().items[0].clone();
        m.hero.inventory_view.items.push(ItemModel {unique_id:Some(0),container:0,slot:2,quantity:1,..Default::default()});
        assert!(cell_action(&m,from,to,&actual).is_none());
        let anonymous:mir2_protocol::UserItem=serde_json::from_value(serde_json::to_value(crate::inventory::CrystalUserItemModel {
            unique_id:0,item_index:1,count:1,..Default::default()}).unwrap()).unwrap();
        let mut equipment=vec![None;14];equipment[1]=Some(anonymous);
        m.hero.info=Some(mir2_protocol::HeroUserInformation {object_id:12,name:"Hero".into(),class:mir2_protocol::MirClass::Warrior,
            gender:mir2_protocol::MirGender::Male,level:1,hair:0,hp:10,mp:5,experience:0,max_experience:100,inventory:Some(vec![None;10]),
            equipment:Some(equipment),magics:vec![],auto_pot:false,auto_hp_percent:30,auto_mp_percent:30,hp_item_index:0,mp_item_index:0});
        m.hero.inventory_view.items.push(ItemModel {unique_id:Some(0),container:2,slot:1,quantity:1,..Default::default()});
        assert!(hero_dialog::item_use::remove_plan(&m.hero,1).is_some());
        state.use_cell(&c,&m,&mut q,HeroCell {grid:HeroGrid::HeroEquipment,slot:1},HeroOrigin::Character,false);assert!(q.0.is_empty());
    }
    #[test]
    fn checkpoint_from_other_hero_never_reuses_packet_hair() {
        let mut model=HeroModel::default();let info=mir2_protocol::HeroUserInformation {object_id:12,name:"First".into(),class:mir2_protocol::MirClass::Warrior,
            gender:mir2_protocol::MirGender::Male,level:1,hair:7,hp:10,mp:5,experience:0,max_experience:100,inventory:Some(vec![None;10]),
            equipment:Some(vec![None;14]),magics:vec![],auto_pot:false,auto_hp_percent:30,auto_mp_percent:30,hp_item_index:0,mp_item_index:0};
        model.apply_packet_at("HeroInformation",&serde_json::json!({"info":info}),0);
        assert_eq!(hero_dialog::render::actor_view(&model).unwrap().player.hair,Some(7));
        assert!(model.observe_snapshot(&serde_json::json!({"heroMaxExperience":200,"heroVitals":{"hp":20,"maxHp":30,"mp":10,"maxMp":15},
            "heroStats":[],"heroWeights":{"bag":1,"wear":2,"hand":3},"stage5Systems":{"heroLearnedMagics":[],
                "hero":{"name":"Second","class":"Wizard","gender":"Female","level":2,"experience":77,"behaviour":0,"spawned":true,
                    "autoPot":false,"autoHpPercent":30,"autoMpPercent":40,"hpItemIndex":0,"mpItemIndex":0}}})));
        let view=hero_dialog::render::actor_view(&model).unwrap();assert_eq!(view.player.hair,None);assert_eq!(view.player.name.as_deref(),Some("Second"));assert_eq!(view.player.experience,77);
    }
    #[test]
    fn absent_presentation_cancels_drag_without_panic_and_idle_escape_passes() {
        let mut c=context();let m=model();let mut state=HeroUiState::default();let mut q=HeroIntentQueue::default();state.reconcile(&c,&m);
        let mut escape=edge(&c,1,0,"key",0.,0.,0);escape.key="Escape".into();assert!(!state.process(&c,&m,escape,&mut q));
        assert!(state.process(&c,&m,edge(&c,2,7,"down",2.,2.,0),&mut q));assert!(state.ui.dragging.is_some());
        c.presentation=None;assert!(!state.process(&c,&m,edge(&c,3,7,"move",400.,400.,0),&mut q));
        assert!(state.held.is_none());assert!(state.ui.dragging.is_none());assert!(q.0.is_empty());
    }

    #[test]
    fn hero_reducer_real_actions_include_basis_and_window_intents_have_no_basis() {
        let c=context();let mut read=crate::hero_action_basis::tests::read_model();
        crate::hero_action_basis::tests::add(&mut read,HeroGrid::HeroInventory,2,u64::MAX,1,1,13);
        let mut state=HeroUiState::default();state.reconcile(&c,&read);let mut q=HeroIntentQueue::default();
        state.use_cell(&c,&read,&mut q,HeroCell{grid:HeroGrid::HeroInventory,slot:2},HeroOrigin::Inventory,false);
        assert_eq!(q.0.len(),1);
        let HeroIntentBody::Action{basis,action,..}=&q.0[0].body else{panic!("actual action");};
        assert_eq!(*action,HeroSemanticAction::Use{slot:2,confirmed:false});
        assert_eq!(basis.version,1);assert_eq!(basis.resolved_wire["uniqueId"],u64::MAX.to_string());
        assert!(serde_json::to_vec(basis).unwrap().len()<=crate::hero_action_basis::MAX_ACTION_BASIS_BYTES);
        q.0.clear();read.hero.inventory_view.items[0].tooltip_source=None;
        state.use_cell(&c,&read,&mut q,HeroCell{grid:HeroGrid::HeroInventory,slot:2},HeroOrigin::Inventory,false);
        assert!(q.0.is_empty());
        state.window_action(&c,&mut q);
        let json=serde_json::to_value(&q.0[0]).unwrap();assert_eq!(json["type"],"windows");assert!(json.get("basis").is_none());
    }

}
