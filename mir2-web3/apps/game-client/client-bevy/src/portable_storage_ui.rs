//! Personal warehouse presentation and instance-pinned intentions.
//! Items and pending transactions belong to the host; no optimistic movement.
use std::collections::HashSet;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiTargetCamera};
use serde::{Deserialize, Serialize};
use crate::inventory::{InventoryModel, ItemModel};
use crate::read_model::PlayerStats;
use crate::storage::{StorageModel, StorageItemSelection, storage_expand_enabled};
use crate::storage_interaction::{self, StorageTransferPlan, StorageGrid};
use crate::crystal_ui::bag_paint::{
    paint_crystal_inventory_with_layout, BagPaintAction, BagPaintOptions,
    BagPaintLayout, BagCellPolicy, BagPaintCell, PORTABLE_BAG_REQUIRED_SKINS,
};
use crate::crystal_ui::storage_paint::{
    paint_storage_with_layout, StoragePaintAction, StoragePaintOptions,
    StoragePaintLayout, StoragePaintLabels, StoragePaintCell, STORAGE_REQUIRED_SKINS,
};
use crate::crystal_ui::item_image::layout_original_item_images;
use crate::crystal_ui::panel_layouts::INVENTORY_PANEL_SIZE;
use crate::portable_quest_ui::{QuestUiFont, QuestUiTargetCamera};
use crate::pending_operations::PendingLifecycleSet;

pub const SAFE: u64 = 9_007_199_254_740_991;
pub fn required_skins() -> Vec<&'static str> {
    let mut paths = PORTABLE_BAG_REQUIRED_SKINS.to_vec();
    paths.extend_from_slice(STORAGE_REQUIRED_SKINS);
    paths.sort_unstable(); paths.dedup(); paths
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct StorageIdentity {
    pub run_generation:u64, pub connection_generation:u64,
    pub session_generation:u64, pub owner_revision:u64,
}
impl StorageIdentity {
    pub fn valid(self)->bool {
        [self.run_generation,self.connection_generation,self.session_generation]
            .into_iter().all(|v|(1..=SAFE).contains(&v)) && self.owner_revision<=SAFE
    }
    pub fn can_follow(self, old:Self)->bool {
        self.run_generation>old.run_generation || self.run_generation==old.run_generation
            && (self.connection_generation>old.connection_generation
                || self.connection_generation==old.connection_generation
                    && (self.session_generation>old.session_generation
                        || self.session_generation==old.session_generation
                            && self.owner_revision>=old.owner_revision))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct StoragePresentation {
    pub logical_width:f32, pub logical_height:f32, pub stage_css_scale:f32, pub touch:bool,
}
impl StoragePresentation {
    pub fn valid(self)->bool {
        [self.logical_width,self.logical_height,self.stage_css_scale].into_iter().all(f32::is_finite)
            && self.logical_width>0. && self.logical_width<=16384. && self.logical_width.fract()==0.
            && self.logical_height>0. && self.logical_height<=16384. && self.logical_height.fract()==0.
            && self.stage_css_scale>=0.05 && self.stage_css_scale<=16.
            && if self.touch {
                self.logical_width*self.stage_css_scale>=320.
                    && self.logical_height*self.stage_css_scale>=240.
            } else {self.logical_width>=760. && self.logical_height>=430.}
    }
    pub fn scale(self)->f32 { if self.touch {1.28/self.stage_css_scale} else {1.} }
    pub fn matches_window(self,width:f32,height:f32)->bool {
        self.valid() && (width-self.logical_width).abs()<=1. && (height-self.logical_height).abs()<=1.
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct StorageCell {pub container:u8,pub slot:u32}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct StorageSelection {pub container:u8,pub slot:u32,pub unique_id:u64}
#[derive(Resource, Default, Clone)]
pub struct StorageUiReadModel {
    pub inventory:InventoryModel, pub storage:StorageModel, pub player:PlayerStats,
    pub blocked_unique_ids:HashSet<u64>, pub pending_cells:HashSet<StorageCell>,
}
impl StorageUiReadModel {
    pub fn item(&self, cell:StorageCell)->Option<&ItemModel> {
        let valid=match cell.container {
            0=>cell.slot<u32::from(self.inventory.bag_slot_capacity()),
            4=>self.storage.is_valid_slot(cell.slot), _=>false,
        };
        if !valid {return None;}
        let rows=if cell.container==0 {&self.inventory.items} else {&self.storage.items};
        let mut matches=rows.iter().filter(|i|i.container==cell.container&&i.slot==cell.slot);
        let row=matches.next()?; if matches.next().is_some(){return None;} Some(row)
    }
    pub fn selection(&self, cell:StorageCell)->Option<StorageSelection> {
        let id=self.item(cell)?.unique_id?;
        let result=StorageSelection{container:cell.container,slot:cell.slot,unique_id:id};
        self.current(result).then_some(result)
    }
    pub fn current(&self, source:StorageSelection)->bool {
        let cell=StorageCell{container:source.container,slot:source.slot};
        self.cell_idle(cell) && self.item(cell).is_some_and(|i|i.unique_id==Some(source.unique_id))
            && !self.blocked_unique_ids.contains(&source.unique_id)
            && self.inventory.items.iter().chain(self.storage.items.iter())
                .filter(|i|i.unique_id==Some(source.unique_id)).count()==1
    }
    pub fn cell_idle(&self,cell:StorageCell)->bool {
        !self.pending_cells.contains(&cell)
            && self.item(cell).is_none_or(|i|i.unique_id.is_none_or(|id|!self.blocked_unique_ids.contains(&id)))
    }
    /// All three warehouse directions use the exact Phase58 planner. The
    /// planner's fallback destination is rechecked, not just the clicked cell.
    pub fn plan(&self,source:StorageSelection,target:StorageCell)->Option<StorageUiIntentKind> {
        if !self.current(source)||!self.cell_idle(target){return None;}
        let selected=StorageItemSelection{slot:source.slot,unique_id:source.unique_id};
        let plan=match(source.container,target.container) {
            (0,4)=>storage_interaction::plan_bag_to_storage(&self.inventory,&self.storage,selected,target.slot),
            (4,0)=>storage_interaction::plan_storage_to_bag(&self.inventory,&self.storage,selected,target.slot),
            (4,4)=>storage_interaction::plan_storage_to_storage(&self.storage,selected,target.slot),
            (0,0)=>{
                if source.slot==target.slot ||target.slot>=u32::from(self.inventory.bag_slot_capacity()){return None;}
                if let Some(other)=self.item(target) {
                    if !self.selection(target).is_some(){return None;}
                    if storage_interaction::compatible_storage_stack(self.item(StorageCell{container:0,slot:source.slot})?,other){
                        return Some(StorageUiIntentKind::Merge{source,target:self.selection(target)?,grid_from:StorageGrid::Inventory,grid_to:StorageGrid::Inventory});
                    }
                }
                return Some(StorageUiIntentKind::Transfer{kind:"moveItem",source,target});
            }
            _=>None,
        }?;
        let (kind,to)=match plan {
            StorageTransferPlan::Store{to,..}=>("storeItem",StorageCell{container:4,slot:to}),
            StorageTransferPlan::TakeBack{to,..}=>("takeBackItem",StorageCell{container:0,slot:to}),
            StorageTransferPlan::Move{to,..}=>("moveItem",StorageCell{container:4,slot:to}),
            StorageTransferPlan::Merge{grid_from,grid_to,id_to,..}=>{
                let other=self.selection(target)?;
                if other.unique_id!=id_to||!self.current(other){return None;}
                return Some(StorageUiIntentKind::Merge{source,target:other,grid_from,grid_to});
            }
        };
        self.cell_idle(to).then_some(StorageUiIntentKind::Transfer{kind,source,target:to})
    }
}
#[derive(Resource, Debug, Clone, Default)]
pub struct StorageUiContext {
    pub identity:StorageIdentity,pub revision:u64,pub model_revision:u64,
    pub presentation_revision:u64,pub service_revision:u64,
    pub open:bool,pub input_enabled:bool,pub ready:bool,
    pub presentation:Option<StoragePresentation>,pub language:String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageUiIntentKind {
    Transfer{kind:&'static str,source:StorageSelection,target:StorageCell},
    Merge{source:StorageSelection,target:StorageSelection,grid_from:StorageGrid,grid_to:StorageGrid},
    Close,Password,Rent,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct StorageUiIntent {
    pub identity:StorageIdentity,pub intent_sequence:u64,pub model_revision:u64,
    pub presentation_revision:u64,pub service_revision:u64,pub kind:StorageUiIntentKind,
}
impl StorageUiIntent {
    pub fn matches(self,c:&StorageUiContext)->bool {
        self.identity==c.identity&&self.model_revision==c.model_revision
            &&self.presentation_revision==c.presentation_revision&&self.service_revision==c.service_revision
    }
}
#[derive(Resource,Default)]
pub struct StorageUiIntentQueue {next:u64,pub intents:Vec<StorageUiIntent>}
impl StorageUiIntentQueue {
    pub fn push(&mut self,c:&StorageUiContext,kind:StorageUiIntentKind)->Option<u64> {
        if self.next>=SAFE||self.intents.len()>=64{return None;}
        self.next+=1;
        self.intents.push(StorageUiIntent{identity:c.identity,intent_sequence:self.next,model_revision:c.model_revision,
            presentation_revision:c.presentation_revision,service_revision:c.service_revision,kind});
        Some(self.next)
    }
    pub fn drain(&mut self)->Vec<StorageUiIntent>{std::mem::take(&mut self.intents)}
    pub fn clear(&mut self){self.intents.clear();}
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum StoragePointerPhase {Down,Move,Up,Cancel,Blur}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum StoragePointerOrigin {Storage,World}
#[derive(Debug,Clone,Copy,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct StoragePointerEdge {
    #[serde(flatten)]pub identity:StorageIdentity,pub sequence:u64,pub model_revision:u64,
    pub presentation_revision:u64,pub service_revision:u64,
    pub pointer_id:u64,pub down_sequence:u64,pub phase:StoragePointerPhase,pub origin:StoragePointerOrigin,
    pub x:f32,pub y:f32,pub button:u8,
}
impl StoragePointerEdge {
    pub fn matches_lease(self,down:Self)->bool {
        self.identity==down.identity&&self.model_revision==down.model_revision
            &&self.presentation_revision==down.presentation_revision&&self.service_revision==down.service_revision
            &&self.pointer_id==down.pointer_id&&self.origin==down.origin&&self.button==down.button
            &&self.down_sequence==down.down_sequence
    }
    pub fn valid(self)->bool {
        self.identity.valid()&&(1..=SAFE).contains(&self.sequence)
            &&[self.model_revision,self.presentation_revision,self.service_revision]
                .into_iter().all(|v|(1..=SAFE).contains(&v))&&self.pointer_id<=SAFE
            &&(1..=SAFE).contains(&self.down_sequence)&&self.down_sequence<=self.sequence
            &&(self.phase!=StoragePointerPhase::Down||self.down_sequence==self.sequence)
            &&[self.x,self.y].into_iter().all(|v|v.is_finite()&&v.abs()<=16384.)&&matches!(self.button,0|2)
    }
}
#[derive(Resource,Default)]
pub struct StoragePointerEdges(pub Vec<StoragePointerEdge>);
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]
pub enum StoragePane {Bag,#[default] Warehouse}
#[derive(Debug,Clone,Copy)]
struct PointerLease {edge:StoragePointerEdge,start:Option<StorageUiAction>,last:Vec2,scrolling:bool}
#[derive(Resource,Default)]
pub struct StorageUiState {
    pub bag_page:u8,pub storage_page:usize,pub pane:StoragePane,
    pub selection:Option<StorageSelection>,pub scroll:Vec2,pub drag_mode:bool,
    pub awaiting_sequence:Option<u64>,pub error:Option<&'static str>,
    last_identity:Option<StorageIdentity>,last_model:u64,last_presentation:u64,last_service:u64,last_open:bool,
    lease:Option<PointerLease>,last_pointer_sequence:u64,
}
impl StorageUiState {
    pub fn cancel_pointer(&mut self){self.lease=None;self.awaiting_sequence=None;}
    pub fn invalidate(&mut self) {self.selection=None;self.awaiting_sequence=None;self.lease=None;self.drag_mode=false;}
    pub fn feedback(&mut self,intent:StorageUiIntent,accepted:bool,c:&StorageUiContext) {
        if !intent.matches(c)||self.awaiting_sequence!=Some(intent.intent_sequence){return;}
        self.awaiting_sequence=None;
        if accepted {self.selection=None;self.drag_mode=false;}
    }
    fn apply(&mut self,c:&StorageUiContext,r:&StorageUiReadModel) {
        let identity=self.last_identity!=Some(c.identity);
        if identity||self.last_open&&!c.open||!c.input_enabled
            ||self.last_presentation!=c.presentation_revision||self.last_service!=c.service_revision{
            self.invalidate();
        }
        if self.last_model!=c.model_revision {self.lease=None;self.awaiting_sequence=None;}
        if identity {self.last_pointer_sequence=0;self.scroll=Vec2::ZERO;self.pane=StoragePane::Warehouse;self.bag_page=0;self.storage_page=0;}
        if self.selection.is_some_and(|s|!r.current(s)){self.selection=None;self.drag_mode=false;}
        if self.bag_page==1&&!r.inventory.second_bag_unlocked(){self.bag_page=0;}
        self.storage_page=r.storage.clamp_page(self.storage_page);
        self.last_identity=Some(c.identity);self.last_model=c.model_revision;self.last_presentation=c.presentation_revision;
        self.last_service=c.service_revision;self.last_open=c.open;
    }
}
#[derive(Component)]pub struct StorageUiRoot;
#[derive(Component)]pub struct StorageUiViewport;
#[derive(Component,Debug,Clone,Copy,PartialEq,Eq)]
pub enum StorageUiAction {Bag(BagPaintAction),Warehouse(StoragePaintAction),SwitchPane,Drag,Cancel,Close}
#[derive(Component,Debug,Clone,Copy,PartialEq,Eq)]
pub struct StorageTreeStamp {
    pub identity:StorageIdentity,pub revision:u64,pub model_revision:u64,pub presentation_revision:u64,
    pub service_revision:u64,pub bag_page:u8,pub storage_page:usize,pub pane:StoragePane,
}
impl StorageTreeStamp {
    pub fn current(self,c:&StorageUiContext,s:&StorageUiState)->bool {
        self.identity==c.identity&&self.revision==c.revision&&self.model_revision==c.model_revision
            &&self.presentation_revision==c.presentation_revision&&self.service_revision==c.service_revision
            &&self.bag_page==s.bag_page&&self.storage_page==s.storage_page&&self.pane==s.pane
    }
}
pub fn logical_rect(node:&ComputedNode,transform:&bevy::ui::UiGlobalTransform)->Option<bevy::math::Rect> {
    let scale=node.inverse_scale_factor;let center=transform.affine().translation*scale;let size=node.size()*scale;
    if !center.is_finite()||!size.is_finite()||size.x<=0.||size.y<=0.{return None;}
    Some(bevy::math::Rect::from_center_size(center,size))
}
fn descendant(mut entity:Entity,root:Entity,parents:&Query<&ChildOf>)->bool {
    while let Ok(parent)=parents.get(entity){entity=parent.parent();if entity==root{return true;}}
    false
}
fn displayed(mut entity:Entity,parents:&Query<&ChildOf>,nodes:&Query<&Node>)->bool {
    loop {
        if nodes.get(entity).is_ok_and(|node|node.display==Display::None){return false;}
        let Ok(parent)=parents.get(entity) else{return true;};entity=parent.parent();
    }
}
pub fn clipped_rect(mut entity:Entity,mut rect:bevy::math::Rect,
    parents:&Query<&ChildOf>,viewports:&Query<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<StorageUiViewport>>,
)->Option<bevy::math::Rect>{
    while let Ok(parent)=parents.get(entity){
        entity=parent.parent();
        if let Ok((node,transform))=viewports.get(entity){rect=rect.intersect(logical_rect(node,transform)?);}
    }
    (rect.width()>0.&&rect.height()>0.).then_some(rect)
}
pub struct Mir2PortableStorageUiPlugin;
impl Plugin for Mir2PortableStorageUiPlugin {
    fn build(&self,app:&mut App){
        app.init_resource::<StorageUiContext>().init_resource::<StorageUiReadModel>()
            .init_resource::<StorageUiState>().init_resource::<StorageUiIntentQueue>()
            .init_resource::<StoragePointerEdges>()
            .add_systems(Startup,spawn_root)
            .add_systems(Update,(apply_context,process_pointer,render_storage,update_readonly_locks,layout_original_item_images)
                .chain().after(PendingLifecycleSet::Ingest));
    }
}
fn spawn_root(mut commands:Commands,camera:Option<Res<QuestUiTargetCamera>>){
    let Some(camera)=camera else{return;};
    commands.spawn((StorageUiRoot,UiTargetCamera(camera.0),Node{
        position_type:PositionType::Absolute,width:Val::Percent(100.),height:Val::Percent(100.),
        display:Display::None,..default()},FocusPolicy::Pass,GlobalZIndex(997)));
}
fn apply_context(c:Res<StorageUiContext>,r:Res<StorageUiReadModel>,mut s:ResMut<StorageUiState>,mut q:ResMut<StorageUiIntentQueue>){
    let old=s.last_identity;s.apply(&c,&r);if old!=Some(c.identity)||!c.open||!c.input_enabled{q.clear();}
}
fn hit(point:Vec2,root:Entity,actions:&Query<(Entity,&StorageUiAction,&ComputedNode,&bevy::ui::UiGlobalTransform,&InheritedVisibility)>,
    parents:&Query<&ChildOf>,viewports:&Query<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<StorageUiViewport>>,
)->Option<StorageUiAction>{
    actions.iter().find_map(|(entity,action,node,transform,visible)|{
        if !visible.get()||!descendant(entity,root,parents){return None;}
        let rect=clipped_rect(entity,logical_rect(node,transform)?,parents,viewports)?;
        rect.contains(point).then_some(*action)
    })
}
fn action_cell(a:StorageUiAction)->Option<StorageCell>{
    match a {
        StorageUiAction::Bag(BagPaintAction::InspectCell{container:0,slot,..})=>Some(StorageCell{container:0,slot}),
        StorageUiAction::Warehouse(StoragePaintAction::Cell{slot,..})=>Some(StorageCell{container:4,slot}),_=>None,
    }
}
fn action_source(action:StorageUiAction,r:&StorageUiReadModel)->Option<StorageSelection>{
    let (container,slot,unique_id)=match action {
        StorageUiAction::Bag(BagPaintAction::InspectCell{container:0,slot,unique_id:Some(id)})=>(0,slot,id),
        StorageUiAction::Warehouse(StoragePaintAction::Cell{slot,unique_id:Some(id)})=>(4,slot,id),
        _=>return None,
    };
    let source=StorageSelection{container,slot,unique_id};
    r.current(source).then_some(source)
}
fn submit(source:StorageSelection,target:StorageCell,c:&StorageUiContext,r:&StorageUiReadModel,s:&mut StorageUiState,q:&mut StorageUiIntentQueue){
    if s.awaiting_sequence.is_some(){return;}
    let Some(kind)=r.plan(source,target) else{return;};
    if let Some(sequence)=q.push(c,kind){s.awaiting_sequence=Some(sequence);}
    else{s.error=Some("storage intent sequence or queue exhausted");}
}
fn activate(a:StorageUiAction,c:&StorageUiContext,r:&StorageUiReadModel,s:&mut StorageUiState,q:&mut StorageUiIntentQueue){
    if let Some(cell)=action_cell(a){
        if let Some(source)=s.selection {
            if source.container!=cell.container||source.slot!=cell.slot {submit(source,cell,c,r,s,q);return;}
        }
        s.selection=r.selection(cell);return;
    }
    match a{
        StorageUiAction::SwitchPane=>{s.pane=if s.pane==StoragePane::Bag{StoragePane::Warehouse}else{StoragePane::Bag};s.scroll=Vec2::ZERO;s.lease=None;},
        StorageUiAction::Drag=>{s.drag_mode=s.selection.is_some()&&!s.drag_mode;},
        StorageUiAction::Cancel=>{if s.awaiting_sequence.is_none(){s.selection=None;s.drag_mode=false;}},
        StorageUiAction::Bag(BagPaintAction::SelectPage(page)) if page<2&& (page==0||r.inventory.second_bag_unlocked())=>{
            s.bag_page=page;s.scroll=Vec2::ZERO;s.lease=None;
        },
        StorageUiAction::Warehouse(StoragePaintAction::Page(page))=>{s.storage_page=r.storage.clamp_page(page);s.scroll=Vec2::ZERO;s.lease=None;},
        StorageUiAction::Close|StorageUiAction::Bag(BagPaintAction::Close)|StorageUiAction::Warehouse(StoragePaintAction::Close)=>{let _=q.push(c,StorageUiIntentKind::Close);},
        StorageUiAction::Warehouse(StoragePaintAction::Password)=>{let _=q.push(c,StorageUiIntentKind::Password);},
        StorageUiAction::Warehouse(StoragePaintAction::Rent) if storage_expand_enabled(&r.storage,r.player.gold)=>{let _=q.push(c,StorageUiIntentKind::Rent);},
        _=>{},
    }
}
fn process_pointer(c:Res<StorageUiContext>,r:Res<StorageUiReadModel>,mut s:ResMut<StorageUiState>,
    mut edges:ResMut<StoragePointerEdges>,mut q:ResMut<StorageUiIntentQueue>,
    roots:Query<(Entity,&StorageTreeStamp),With<StorageUiRoot>>,
    actions:Query<(Entity,&StorageUiAction,&ComputedNode,&bevy::ui::UiGlobalTransform,&InheritedVisibility)>,
    parents:Query<&ChildOf>,viewports:Query<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<StorageUiViewport>>,
){
    for edge in std::mem::take(&mut edges.0){
        // Old-owner cleanup can only cancel that old lease, never the new one.
        if !edge.valid(){continue;}
        if edge.identity!=c.identity||edge.model_revision!=c.model_revision
            ||edge.presentation_revision!=c.presentation_revision||edge.service_revision!=c.service_revision {
            if matches!(edge.phase,StoragePointerPhase::Up|StoragePointerPhase::Cancel|StoragePointerPhase::Blur)
                &&s.lease.is_some_and(|l|edge.matches_lease(l.edge)){s.lease=None;}
            continue;
        }
        if edge.sequence<=s.last_pointer_sequence{continue;}
        if edge.phase==StoragePointerPhase::Down {if s.lease.is_some(){continue;}}
        else if !s.lease.is_some_and(|l|edge.matches_lease(l.edge)){continue;}
        s.last_pointer_sequence=edge.sequence;
        if edge.phase==StoragePointerPhase::Blur{
            if s.lease.is_some_and(|l|edge.matches_lease(l.edge)){s.lease=None;}
            continue;
        }
        let current=c.ready&&c.open&&c.input_enabled&&s.error.is_none();
        let root=roots.single().ok().filter(|(_,stamp)|stamp.current(&c,&s)).map(|(e,_)|e);
        if !current||root.is_none(){s.lease=None;continue;}
        let root=root.unwrap();let point=Vec2::new(edge.x,edge.y);
        match edge.phase {
            StoragePointerPhase::Down=>{
                if s.lease.is_some(){continue;}
                s.lease=Some(PointerLease{edge,start:(edge.origin==StoragePointerOrigin::Storage&&edge.button==0)
                    .then(||hit(point,root,&actions,&parents,&viewports)).flatten(),last:point,scrolling:false});
            },
            StoragePointerPhase::Move=>{
                if let Some(mut lease)=s.lease.filter(|l|edge.matches_lease(l.edge)) {
                    if c.presentation.is_some_and(|p|p.touch)&&!s.drag_mode&&lease.edge.origin==StoragePointerOrigin::Storage {
                        let start=Vec2::new(lease.edge.x,lease.edge.y);
                        let scale=c.presentation.unwrap().stage_css_scale;
                        if lease.scrolling||(point-start).length()*scale>8. {
                            lease.scrolling=true;
                            let delta=lease.last-point;
                            if let Some(p)=c.presentation {let max=scroll_max(p,s.pane);
                                s.scroll=(s.scroll+delta).clamp(Vec2::ZERO,max);}
                        }
                    }
                    lease.last=point;s.lease=Some(lease);
                }
            },
            StoragePointerPhase::Cancel=>{if s.lease.is_some_and(|l|edge.matches_lease(l.edge)){s.lease=None;}},
            StoragePointerPhase::Up=>{
                let Some(lease)=s.lease.filter(|l|edge.matches_lease(l.edge)) else{continue;};s.lease=None;
                if lease.scrolling||lease.edge.origin!=StoragePointerOrigin::Storage||edge.button!=0{continue;}
                let Some(end)=hit(point,root,&actions,&parents,&viewports) else{continue;};
                let Some(start)=lease.start else{continue;};
                if let (Some(from),Some(to))=(action_cell(start),action_cell(end)){
                    if from!=to {if let Some(source)=action_source(start,&r){s.selection=Some(source);submit(source,to,&c,&r,&mut s,&mut q);}continue;}
                }
                if start==end{activate(end,&c,&r,&mut s,&mut q);}
            }
            StoragePointerPhase::Blur=>{},
        }
    }
}
fn scroll_max(p:StoragePresentation,pane:StoragePane)->Vec2{
    let scale=p.scale();let content=Vec2::new(if pane==StoragePane::Bag{INVENTORY_PANEL_SIZE.width as f32}else{388.},
        if pane==StoragePane::Bag{INVENTORY_PANEL_SIZE.height as f32}else{346.})*scale;
    let viewport=Vec2::new(p.logical_width-24.,p.logical_height-44./p.stage_css_scale-28.);
    (content-viewport).max(Vec2::ZERO)
}
fn labels(language:&str)->StoragePaintLabels {
    match language{
        "zh-CN"=>StoragePaintLabels{rental_locked:"扩展仓库未租用".into(),expiry_prefix:"扩展仓库到期".into(),password_locked:"仓库已锁定".into()},
        "ja"=>StoragePaintLabels{rental_locked:"拡張倉庫は未契約".into(),expiry_prefix:"拡張倉庫の期限".into(),password_locked:"倉庫ロック中".into()},
        _=>StoragePaintLabels::default(),
    }
}
fn control(parent:&mut ChildSpawnerCommands,label:&str,action:StorageUiAction,font:&TextFont,width:f32,height:f32){
    parent.spawn((Button,action,Node{width:Val::Px(width),height:Val::Px(height),
        align_items:AlignItems::Center,justify_content:JustifyContent::Center,..default()},
        BackgroundColor(Color::srgb(0.19,0.16,0.12))))
        .with_children(|p|{p.spawn((Text::new(label),font.clone(),TextColor(Color::WHITE)));});
}
fn render_storage(mut commands:Commands,c:Res<StorageUiContext>,r:Res<StorageUiReadModel>,s:Res<StorageUiState>,
    roots:Query<Entity,With<StorageUiRoot>>,mut nodes:Query<&mut Node,With<StorageUiRoot>>,
    font:Option<Res<QuestUiFont>>,server:Option<Res<AssetServer>>,
    mut last:Local<Option<(StorageTreeStamp,Option<StorageSelection>,Vec2,bool,bool,String)>>,
){
    let Ok(root)=roots.single()else{return;};let Ok(mut node)=nodes.get_mut(root)else{return;};
    let visible=c.open&&c.presentation.is_some_and(StoragePresentation::valid);
    node.display=if visible{Display::Flex}else{Display::None};if !visible{return;}
    let (Some(font),Some(server),Some(p))=(font,server,c.presentation)else{return;};
    let stamp=StorageTreeStamp{identity:c.identity,revision:c.revision,model_revision:c.model_revision,
        presentation_revision:c.presentation_revision,service_revision:c.service_revision,
        bag_page:s.bag_page,storage_page:s.storage_page,pane:s.pane};
    let key=(stamp,s.selection,s.scroll,s.drag_mode,c.input_enabled,c.language.clone());
    if last.as_ref()==Some(&key)&&!r.is_changed()&&!font.is_changed(){return;}*last=Some(key);
    let scale=p.scale();let text=TextFont{font:bevy::text::FontSource::Handle(font.0.clone()),font_size:bevy::text::FontSize::Px(13.*scale),..default()};
    let header_h=if p.touch{44./p.stage_css_scale}else{38.};
    let panel_y=if p.touch{header_h+16.}else{72.};
    let desktop_left=(p.logical_width-730.)*0.5;
    commands.entity(root).insert(stamp).despawn_children();
    commands.entity(root).with_children(|parent|{
        parent.spawn((Node{position_type:PositionType::Absolute,left:Val::Px(if p.touch{12.}else{desktop_left}),
            top:Val::Px(if p.touch{8.}else{24.}),height:Val::Px(header_h),
            width:Val::Px(if p.touch{p.logical_width-24.}else{730.}),column_gap:Val::Px(4.),..default()},FocusPolicy::Block))
            .with_children(|header|{
                let (pane,drag,cancel,close)=match c.language.as_str(){
                    "zh-CN"=>("切换栏","拖放","取消","关闭"),"ja"=>("切替","ドラッグ","取消","閉じる"),_=>("Switch","Drag","Cancel","Close")};
                let unit=if p.touch{1./p.stage_css_scale}else{1.};
                if p.touch{control(header,pane,StorageUiAction::SwitchPane,&text,64.*unit,header_h);}
                control(header,drag,StorageUiAction::Drag,&text,60.*unit,header_h);
                control(header,cancel,StorageUiAction::Cancel,&text,64.*unit,header_h);
                control(header,close,StorageUiAction::Close,&text,56.*unit,header_h);
            });
        for pane in [StoragePane::Bag,StoragePane::Warehouse]{
            if p.touch&&pane!=s.pane{continue;}
            let width=if p.touch{p.logical_width-24.}else if pane==StoragePane::Bag{318.}else{388.};
            let height=if p.touch{p.logical_height-panel_y-12.}else{346.};
            let left=if p.touch{12.}else{desktop_left+if pane==StoragePane::Bag{0.}else{342.}};
            parent.spawn((StorageUiViewport,Node{position_type:PositionType::Absolute,left:Val::Px(left),top:Val::Px(panel_y),
                width:Val::Px(width),height:Val::Px(height),overflow:Overflow::clip(),..default()},FocusPolicy::Block))
                .with_children(|viewport|{
                    viewport.spawn((Node{position_type:PositionType::Absolute,left:Val::Px(if p.touch{-s.scroll.x}else{0.}),
                        top:Val::Px(if p.touch{-s.scroll.y}else{0.}),width:Val::Px(if pane==StoragePane::Bag{318.*scale}else{388.*scale}),
                        height:Val::Px((if pane==StoragePane::Bag{INVENTORY_PANEL_SIZE.height as f32}else{346.})*scale),..default()},FocusPolicy::Block))
                        .with_children(|content|{
                            if pane==StoragePane::Bag{
                                paint_crystal_inventory_with_layout(content,&server,&r.inventory,&r.player,
                                    &BagPaintOptions{page:s.bag_page,delete_mode:false,weight_ratio:None,free_slots:None,
                                        font:text.clone(),stack_split_hint:Default::default()},
                                    BagPaintLayout{scale,touch:p.touch},StorageUiAction::Bag,
                                    |cell|BagCellPolicy::new(true,c.input_enabled&&cell.container==0
                                        &&cell.slot<u32::from(r.inventory.bag_slot_capacity())
                                        &&r.cell_idle(StorageCell{container:0,slot:cell.slot}),false),
                                    |decor,cell|{if s.selection.is_some_and(|selected|selected.container==0&&selected.slot==cell.slot){
                                        decor.spawn((Node{position_type:PositionType::Absolute,width:Val::Percent(100.),height:Val::Percent(100.),
                                            border:UiRect::all(Val::Px(2.)),..default()},BorderColor::all(Color::srgb(1.,0.8,0.2))));}});
                            }else{
                                paint_storage_with_layout(content,Some(&server),&r.storage,&r.player,
                                    &StoragePaintOptions{page:s.storage_page,selection:s.selection.filter(|sel|sel.container==4)
                                        .map(|sel|StorageItemSelection{slot:sel.slot,unique_id:sel.unique_id}),font:Some(TextFont{font_size:bevy::text::FontSize::Px(13.),..text.clone()}),labels:labels(&c.language)},
                                    StoragePaintLayout{scale},StorageUiAction::Warehouse);
                            }
                        });
                });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(container:u8,slot:u32,id:u64)->ItemModel {ItemModel{container,slot,unique_id:Some(id),quantity:1,icon:1,..default()}}
    fn read()->StorageUiReadModel {StorageUiReadModel{inventory:InventoryModel{capacity:54,items:vec![item(0,43,10)],..default()},
        storage:StorageModel{items:vec![item(4,3,20)],..StorageModel::new()},..default()}}
    fn context()->StorageUiContext{StorageUiContext{identity:StorageIdentity{run_generation:1,connection_generation:1,session_generation:1,owner_revision:0},
        revision:1,model_revision:1,presentation_revision:1,service_revision:1,open:true,input_enabled:true,ready:true,..default()}}
    #[test]fn portable_storage_plans_physical_bag2_both_directions(){
        let r=read();assert_eq!(r.plan(StorageSelection{container:0,slot:43,unique_id:10},StorageCell{container:4,slot:4}),
            Some(StorageUiIntentKind::Transfer{kind:"storeItem",source:StorageSelection{container:0,slot:43,unique_id:10},target:StorageCell{container:4,slot:4}}));
        assert!(matches!(r.plan(StorageSelection{container:4,slot:3,unique_id:20},StorageCell{container:0,slot:47}),
            Some(StorageUiIntentKind::Transfer{kind:"takeBackItem",target:StorageCell{slot:47,..},..})));
        assert!(r.plan(StorageSelection{container:4,slot:3,unique_id:20},StorageCell{container:0,slot:48}).is_none());
    }
    #[test]fn portable_storage_pending_fallback_and_owner_reentry_retain_selection(){
        let mut r=read();r.storage.items.push(item(4,4,30));r.pending_cells.insert(StorageCell{container:4,slot:0});
        assert!(r.plan(StorageSelection{container:0,slot:43,unique_id:10},StorageCell{container:4,slot:4}).is_none());
        let c=context();let mut s=StorageUiState::default();let mut q=StorageUiIntentQueue::default();
        r.pending_cells.clear();s.selection=Some(StorageSelection{container:0,slot:43,unique_id:10});
        submit(s.selection.unwrap(),StorageCell{container:4,slot:5},&c,&r,&mut s,&mut q);
        let old=q.drain()[0];s.feedback(old,false,&c);assert!(s.selection.is_some());
        submit(s.selection.unwrap(),StorageCell{container:4,slot:5},&c,&r,&mut s,&mut q);
        let current=q.drain()[0];s.feedback(old,true,&c);assert_eq!(s.awaiting_sequence,Some(current.intent_sequence));
        s.feedback(current,true,&c);assert_eq!(s.selection,None);
    }
    #[test]fn portable_storage_owner_service_capacity_and_locks_invalidate_selection(){
        let mut r=read();let mut c=context();let mut s=StorageUiState::default();s.apply(&c,&r);
        s.selection=r.selection(StorageCell{container:0,slot:43});s.pane=StoragePane::Bag;
        activate(StorageUiAction::SwitchPane,&c,&r,&mut s,&mut StorageUiIntentQueue::default());
        assert!(s.selection.is_some(),"pane change preserves source");
        c.service_revision=2;s.apply(&c,&r);assert_eq!(s.selection,None);
        s.selection=r.selection(StorageCell{container:0,slot:43});r.inventory.capacity=46;s.apply(&c,&r);assert_eq!(s.selection,None);
        r=read();r.storage.has_password=true;r.storage.unlocked=false;
        assert!(r.plan(StorageSelection{container:0,slot:43,unique_id:10},StorageCell{container:4,slot:4}).is_none());
        r.blocked_unique_ids.insert(10);assert!(r.selection(StorageCell{container:0,slot:43}).is_none());
    }
    #[test]fn portable_storage_same_grid_move_and_duplicate_source_refuse(){
        let mut r=read();let source=StorageSelection{container:4,slot:3,unique_id:20};
        assert!(matches!(r.plan(source,StorageCell{container:4,slot:4}),Some(StorageUiIntentKind::Transfer{kind:"moveItem",..})));
        r.storage.items.push(item(4,4,20));assert!(r.plan(source,StorageCell{container:4,slot:5}).is_none());
    }
    #[test]fn portable_storage_sequence_exhaustion_is_permanent(){
        let mut q=StorageUiIntentQueue{next:SAFE,..default()};assert!(q.push(&context(),StorageUiIntentKind::Close).is_none());
        q.clear();assert!(q.push(&context(),StorageUiIntentKind::Close).is_none());
    }
    #[test]fn portable_storage_touch_is_large_real_scrollable_nodes(){
        let p=StoragePresentation{logical_width:640.,logical_height:480.,stage_css_scale:0.5,touch:true};
        assert!(p.valid());assert!(scroll_max(p,StoragePane::Warehouse).x>0.);
        assert!(scroll_max(p,StoragePane::Warehouse).y>0.);
        assert!(36.*p.scale()*p.stage_css_scale>=40.);
    }


    #[test]
    fn portable_storage_merge_uses_full_current_metadata_and_captured_drag_uid() {
        use crate::inventory::{CrystalItemInfoModel,CrystalItemTooltipSourceModel,CrystalUserItemModel};
        let mut r=read();
        for row in [&mut r.inventory.items[0],&mut r.storage.items[0]] {
            let id=row.unique_id.unwrap();
            row.tooltip_source=Some(CrystalItemTooltipSourceModel{
                info:CrystalItemInfoModel{item_index:1,stack_size:20,..default()},
                user_item:Some(CrystalUserItemModel{unique_id:id,item_index:1,count:1,..default()}),
                ..default()});
        }
        let source=StorageSelection{container:0,slot:43,unique_id:10};
        assert!(matches!(r.plan(source,StorageCell{container:4,slot:3}),
            Some(StorageUiIntentKind::Merge{grid_from:StorageGrid::Inventory,grid_to:StorageGrid::Storage,
                target:StorageSelection{unique_id:20,..},..})));
        r.blocked_unique_ids.insert(20);assert!(r.plan(source,StorageCell{container:4,slot:3}).is_none());
        r.blocked_unique_ids.clear();
        let captured=StorageUiAction::Warehouse(StoragePaintAction::Cell{slot:3,unique_id:Some(20)});
        assert_eq!(action_source(captured,&r),Some(StorageSelection{container:4,slot:3,unique_id:20}));
        r.storage.items[0].unique_id=Some(99);
        assert!(action_source(captured,&r).is_none(),"down's UID is immutable even if a host fails to bump its clock");
    }
    fn pointer(c:&StorageUiContext,sequence:u64,phase:StoragePointerPhase,origin:StoragePointerOrigin,point:Vec2)->StoragePointerEdge {
        StoragePointerEdge{identity:c.identity,sequence,model_revision:c.model_revision,
            presentation_revision:c.presentation_revision,service_revision:c.service_revision,
            pointer_id:1,down_sequence:sequence,phase,origin,x:point.x,y:point.y,button:0}
    }
    fn pointer_fixture()->App {
        let mut app=App::new();let c=context();let r=read();let mut s=StorageUiState::default();s.apply(&c,&r);
        let stamp=StorageTreeStamp{identity:c.identity,revision:c.revision,model_revision:c.model_revision,
            presentation_revision:c.presentation_revision,service_revision:c.service_revision,
            bag_page:s.bag_page,storage_page:s.storage_page,pane:s.pane};
        let root=app.world_mut().spawn((StorageUiRoot,stamp)).id();
        for (slot,x) in [(3,100.),(4,160.)] {
            app.world_mut().spawn((ChildOf(root),StorageUiAction::Warehouse(StoragePaintAction::Cell{slot,
                unique_id:(slot==3).then_some(20)}),ComputedNode{size:Vec2::splat(40.),..default()},
                bevy::ui::UiGlobalTransform::from_xy(x,100.),InheritedVisibility::VISIBLE));
        }
        app.insert_resource(c).insert_resource(r).insert_resource(s)
            .init_resource::<StoragePointerEdges>().init_resource::<StorageUiIntentQueue>()
            .add_systems(Update,process_pointer);
        app
    }
    fn edges(app:&mut App,origin:StoragePointerOrigin,points:&[(StoragePointerPhase,Vec2)]) {
        let c=app.world().resource::<StorageUiContext>().clone();
        let next=app.world().resource::<StorageUiState>().last_pointer_sequence+1;
        let down=if points.first().is_some_and(|(phase,_)|*phase==StoragePointerPhase::Down){next}
            else{app.world().resource::<StorageUiState>().lease.map_or(next,|l|l.edge.down_sequence)};
        app.world_mut().resource_mut::<StoragePointerEdges>().0.extend(points.iter().enumerate()
            .map(|(i,(phase,point))|{let mut edge=pointer(&c,next+i as u64,*phase,origin,*point);edge.down_sequence=down;edge}));
        app.update();
    }
    #[test]
    fn portable_storage_actual_pointer_system_never_retargets_world_origin() {
        // Controlled measured nodes exercise the actual system; this fixture is
        // separate from the actual painter/layout cases below.
        let mut app=pointer_fixture();
        edges(&mut app,StoragePointerOrigin::World,&[(StoragePointerPhase::Down,Vec2::new(100.,100.)),
            (StoragePointerPhase::Move,Vec2::new(160.,100.)),(StoragePointerPhase::Up,Vec2::new(160.,100.))]);
        assert!(app.world().resource::<StorageUiIntentQueue>().intents.is_empty());
        assert!(app.world().resource::<StorageUiState>().selection.is_none());
        assert!(app.world().resource::<StorageUiState>().lease.is_none());
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,Vec2::new(100.,100.)),
            (StoragePointerPhase::Up,Vec2::new(160.,100.))]);
        assert!(matches!(app.world().resource::<StorageUiIntentQueue>().intents[0].kind,
            StorageUiIntentKind::Transfer{kind:"moveItem",source:StorageSelection{unique_id:20,..},..}));
    }
    #[test]
    fn portable_storage_old_terminal_cannot_cancel_new_service_or_owner_lease() {
        let mut app=pointer_fixture();let old=app.world().resource::<StorageUiContext>().clone();
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,Vec2::new(100.,100.))]);
        {let world=app.world_mut();let mut c=world.resource_mut::<StorageUiContext>();c.service_revision+=1;c.identity.owner_revision+=1;}
        let now=app.world().resource::<StorageUiContext>().clone();
        app.world_mut().resource_mut::<StorageUiState>().apply(&now,&read());
        let stamp=StorageTreeStamp{identity:now.identity,revision:now.revision,model_revision:now.model_revision,
            presentation_revision:now.presentation_revision,service_revision:now.service_revision,
            bag_page:0,storage_page:0,pane:StoragePane::Warehouse};
        let root=app.world_mut().query_filtered::<Entity,With<StorageUiRoot>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(root).insert(stamp);
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,Vec2::new(100.,100.))]);
        let current_sequence=app.world().resource::<StorageUiState>().last_pointer_sequence;
        app.world_mut().resource_mut::<StoragePointerEdges>().0.push(pointer(&old,SAFE,StoragePointerPhase::Cancel,
            StoragePointerOrigin::Storage,Vec2::new(100.,100.)));
        app.update();
        assert!(app.world().resource::<StorageUiState>().lease.is_some());
        assert_eq!(app.world().resource::<StorageUiState>().last_pointer_sequence,current_sequence);
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Up,Vec2::new(160.,100.))]);
        assert_eq!(app.world().resource::<StorageUiIntentQueue>().intents.len(),1);
    }

    #[test]
    fn portable_storage_same_clock_old_nonce_does_not_retire_or_advance_new_lease() {
        let mut app=pointer_fixture();
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,Vec2::new(100.,100.))]);
        let old_down=app.world().resource::<StorageUiState>().lease.unwrap().edge;
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Up,Vec2::new(160.,100.))]);
        let first=app.world_mut().resource_mut::<StorageUiIntentQueue>().drain()[0];
        let c=app.world().resource::<StorageUiContext>().clone();
        app.world_mut().resource_mut::<StorageUiState>().feedback(first,false,&c);
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,Vec2::new(100.,100.))]);
        let new_down=app.world().resource::<StorageUiState>().lease.unwrap().edge;
        assert_ne!(new_down.down_sequence,old_down.down_sequence);
        for (phase,sequence) in [(StoragePointerPhase::Cancel,SAFE-1),(StoragePointerPhase::Up,SAFE)] {
            let mut late=old_down;late.phase=phase;late.sequence=sequence;
            app.world_mut().resource_mut::<StoragePointerEdges>().0.push(late);app.update();
            let state=app.world().resource::<StorageUiState>();
            assert_eq!(state.lease.unwrap().edge.down_sequence,new_down.down_sequence);
            assert_eq!(state.last_pointer_sequence,new_down.sequence,"stale terminal cannot advance new watermark");
            assert!(app.world().resource::<StorageUiIntentQueue>().intents.is_empty());
        }
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Up,Vec2::new(160.,100.))]);
        assert_eq!(app.world().resource::<StorageUiIntentQueue>().intents.len(),1);
    }
    #[test]
    fn portable_storage_model_change_cancels_down_before_target_up() {
        let mut app=pointer_fixture();
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,Vec2::new(100.,100.))]);
        app.world_mut().resource_mut::<StorageUiContext>().model_revision=2;
        let c=app.world().resource::<StorageUiContext>().clone();
        app.world_mut().resource_mut::<StorageUiState>().apply(&c,&read());
        assert!(app.world().resource::<StorageUiState>().lease.is_none());
        edges(&mut app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Up,Vec2::new(160.,100.))]);
        assert!(app.world().resource::<StorageUiIntentQueue>().intents.is_empty());
    }
    fn painted(p:StoragePresentation)->App {
        use bevy::asset::AssetApp;
        use bevy::camera::{ComputedCameraValues,RenderTargetInfo,Viewport};
        let mut app=App::new();
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default(),bevy::input::InputPlugin,
            bevy::image::ImagePlugin::default(),bevy::transform::TransformPlugin,
            bevy::camera::visibility::VisibilityPlugin,bevy::text::TextPlugin,bevy::ui::UiPlugin));
        app.init_asset::<Image>().init_asset::<Font>().init_asset::<bevy::image::TextureAtlasLayout>()
            .init_asset::<bevy::mesh::Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        let font=Font::from_bytes(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf")).expect("packaged font"));
        let font=app.world_mut().resource_mut::<Assets<Font>>().add(font);
        let size=UVec2::new(p.logical_width as u32,p.logical_height as u32);
        let camera=app.world_mut().spawn((Camera2d,Camera{computed:ComputedCameraValues{
            target_info:Some(RenderTargetInfo{physical_size:size,scale_factor:1.}),..default()},
            viewport:Some(Viewport{physical_size:size,..default()}),..default()})).id();
        app.insert_resource(QuestUiTargetCamera(camera)).insert_resource(QuestUiFont(font));
        let mut c=context();c.presentation=Some(p);app.insert_resource(c).insert_resource(read());
        app.add_plugins(Mir2PortableStorageUiPlugin);
        for _ in 0..4{app.update();}app
    }
    fn observe(app:&mut App)->StorageTreeObservation {
        let mut system=bevy::ecs::system::SystemState::<StorageTreeObserver>::new(app.world_mut());
        let tree=system.get(app.world()).expect("storage tree queries");
        tree.observe(app.world().resource::<StorageUiContext>(),app.world().resource::<StorageUiState>(),
            app.world().resource::<StorageUiReadModel>())
    }
    fn action_point(app:&mut App,wanted:StorageUiAction)->Vec2 {
        let world=app.world_mut();
        world.query::<(&StorageUiAction,&ComputedNode,&bevy::ui::UiGlobalTransform)>()
            .iter(world).find_map(|(action,node,transform)|(*action==wanted)
                .then(||logical_rect(node,transform).unwrap().center())).expect("actual painted action")
    }
    fn tap(app:&mut App,action:StorageUiAction) {
        let point=action_point(app,action);
        edges(app,StoragePointerOrigin::Storage,&[(StoragePointerPhase::Down,point),(StoragePointerPhase::Up,point)]);
        app.update();
    }
    #[test]
    fn portable_storage_actual_painted_two_panes_and_partial_bag2_are_measured() {
        let mut app=painted(StoragePresentation{logical_width:1024.,logical_height:768.,stage_css_scale:1.,touch:false});
        assert!(observe(&mut app).complete);
        assert_eq!(app.world_mut().query::<&StoragePaintCell>().iter(app.world()).count(),80);
        tap(&mut app,StorageUiAction::Bag(BagPaintAction::SelectPage(1)));
        assert_eq!(app.world_mut().query::<&BagPaintCell>().iter(app.world()).count(),8);
        assert!(observe(&mut app).complete,"partial Bag2 is ready with its eight actual cells");
        let before=app.world().resource::<StorageUiReadModel>().inventory.clone();
        tap(&mut app,StorageUiAction::Bag(BagPaintAction::InspectCell{container:0,slot:43,unique_id:Some(10)}));
        tap(&mut app,StorageUiAction::Warehouse(StoragePaintAction::Cell{slot:4,unique_id:None}));
        let intent=app.world_mut().resource_mut::<StorageUiIntentQueue>().drain()[0];
        assert!(matches!(intent.kind,StorageUiIntentKind::Transfer{kind:"storeItem",source:StorageSelection{slot:43,..},..}));
        let c=app.world().resource::<StorageUiContext>().clone();
        app.world_mut().resource_mut::<StorageUiState>().feedback(intent,false,&c);
        assert!(app.world().resource::<StorageUiState>().selection.is_some());
        assert_eq!(serde_json::to_value(&app.world().resource::<StorageUiReadModel>().inventory).unwrap(),
            serde_json::to_value(&before).unwrap(),"no optimistic item mutation");
        app.world_mut().resource_mut::<StorageUiContext>().revision+=1;
        assert!(!observe(&mut app).complete,"an old painted revision cannot establish readiness");
    }
    #[test]
    fn portable_storage_actual_touch_pane_switch_preserves_source_then_emits_target() {
        let mut app=painted(StoragePresentation{logical_width:640.,logical_height:480.,stage_css_scale:0.5,touch:true});
        assert!(observe(&mut app).complete);
        tap(&mut app,StorageUiAction::SwitchPane);
        tap(&mut app,StorageUiAction::Bag(BagPaintAction::SelectPage(1)));
        tap(&mut app,StorageUiAction::Bag(BagPaintAction::InspectCell{container:0,slot:43,unique_id:Some(10)}));
        let selected=app.world().resource::<StorageUiState>().selection;
        assert_eq!(selected,Some(StorageSelection{container:0,slot:43,unique_id:10}));
        tap(&mut app,StorageUiAction::SwitchPane);
        assert_eq!(app.world().resource::<StorageUiState>().selection,selected);
        assert_eq!(app.world_mut().query::<&BagPaintCell>().iter(app.world()).count(),0,"one actual pane, not eighty shrunk cells");
        assert!(observe(&mut app).complete);
        tap(&mut app,StorageUiAction::Warehouse(StoragePaintAction::Cell{slot:4,unique_id:None}));
        let intent=app.world_mut().resource_mut::<StorageUiIntentQueue>().drain()[0];
        assert!(matches!(intent.kind,StorageUiIntentKind::Transfer{kind:"storeItem",target:StorageCell{container:4,slot:4},..}));
        let c=app.world().resource::<StorageUiContext>().clone();
        app.world_mut().resource_mut::<StorageUiState>().feedback(intent,true,&c);
        assert_eq!(app.world().resource::<StorageUiState>().selection,None);
    }
    #[test]
    fn portable_storage_actual_touch_scroll_exposes_last_cell_and_pending_removes_action() {
        let mut app=painted(StoragePresentation{logical_width:640.,logical_height:480.,stage_css_scale:0.5,touch:true});
        let p=app.world().resource::<StorageUiContext>().presentation.unwrap();
        app.world_mut().resource_mut::<StorageUiState>().scroll=scroll_max(p,StoragePane::Warehouse);
        app.update();app.update();
        let point=action_point(&mut app,StorageUiAction::Warehouse(StoragePaintAction::Cell{slot:79,unique_id:None}));
        assert!(point.x>12.&&point.x<628.&&point.y>104.&&point.y<468.,"last actual cell can be reached by scroll");
        app.world_mut().resource_mut::<StorageUiReadModel>().pending_cells.insert(StorageCell{container:4,slot:79});
        app.update();app.update();
        assert!(!app.world_mut().query::<&StorageUiAction>().iter(app.world())
            .any(|action|*action==StorageUiAction::Warehouse(StoragePaintAction::Cell{slot:79,unique_id:None})));
        assert!(app.world_mut().query::<&StoragePaintCell>().iter(app.world()).any(|cell|cell.slot==79&&!cell.enabled));
        assert!(observe(&mut app).complete,"pending destination is read-only without disabling the whole panel");
    }
}


/// Inspects the same entities painted above, including descendants clipped by
/// touch viewports. Loaded handles alone never establish a usable input tree.
#[derive(Default)]
pub struct StorageTreeObservation {
    pub complete:bool,pub input_regions:Vec<bevy::math::Rect>,
}
#[derive(bevy::ecs::system::SystemParam)]
pub struct StorageTreeObserver<'w,'s> {
    roots:Query<'w,'s,(Entity,&'static StorageTreeStamp,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform),With<StorageUiRoot>>,
    viewports:Query<'w,'s,(Entity,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform),With<StorageUiViewport>>,
    bags:Query<'w,'s,(Entity,&'static BagPaintCell,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform,&'static InheritedVisibility)>,
    warehouses:Query<'w,'s,(Entity,&'static StoragePaintCell,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform,&'static InheritedVisibility)>,
    texts:Query<'w,'s,(Entity,&'static Text,Option<&'static bevy::text::TextLayoutInfo>)>,
    parents:Query<'w,'s,&'static ChildOf>,
    nodes:Query<'w,'s,&'static Node>,
    actions:Query<'w,'s,(Entity,&'static StorageUiAction,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform,&'static InheritedVisibility)>,
}
impl StorageTreeObserver<'_,'_> {
    pub fn observe(&self,c:&StorageUiContext,s:&StorageUiState,r:&StorageUiReadModel)->StorageTreeObservation {
        let Some(p)=c.presentation else{return StorageTreeObservation::default();};
        let Ok((root,stamp,node,transform))=self.roots.single()else{return StorageTreeObservation::default();};
        if !stamp.current(c,s)||logical_rect(node,transform).is_none(){return StorageTreeObservation::default();}
        let in_stage=|rect:bevy::math::Rect|rect.min.x>=-0.1&&rect.min.y>=-0.1
            &&rect.max.x<=p.logical_width+0.1&&rect.max.y<=p.logical_height+0.1;
        let regions:Vec<_>=self.viewports.iter().filter(|(e,_,_)|descendant(*e,root,&self.parents))
            .filter_map(|(_,node,transform)|logical_rect(node,transform)).collect();
        if regions.len()!=(if p.touch{1}else{2})||regions.iter().any(|r|!in_stage(*r)){return StorageTreeObservation::default();}
        let visible_bag=!p.touch||s.pane==StoragePane::Bag;
        let visible_storage=!p.touch||s.pane==StoragePane::Warehouse;
        let mut bag_cells=HashSet::new();let mut storage_cells=HashSet::new();
        for (entity,cell,node,transform,visible) in &self.bags {
            if !descendant(entity,root,&self.parents){continue;}
            if !visible_bag||!visible.get()||logical_rect(node,transform).is_none(){return StorageTreeObservation::default();}
            if !bag_cells.insert((cell.container,cell.slot)){return StorageTreeObservation::default();}
        }
        let expected_bag=if visible_bag {u32::from(r.inventory.bag_slot_capacity())
            .saturating_sub(u32::from(s.bag_page)*40).min(40) as usize}else{0};
        if bag_cells.len()!=expected_bag{return StorageTreeObservation::default();}
        let page=r.storage.page(s.storage_page);
        for (entity,cell,node,transform,visible) in &self.warehouses {
            if !descendant(entity,root,&self.parents){continue;}
            if !visible_storage||!storage_cells.insert(cell.slot){return StorageTreeObservation::default();}
            if !page.rental_locked&&(logical_rect(node,transform).is_none()||!visible.get()){return StorageTreeObservation::default();}
        }
        if storage_cells.len()!=(if visible_storage{80}else{0}){return StorageTreeObservation::default();}
        let mut text_count=0;
        for (entity,text,layout) in &self.texts {
            if !descendant(entity,root,&self.parents)||text.0.trim().is_empty()
                ||!displayed(entity,&self.parents,&self.nodes){continue;}
            if !layout.is_some_and(|l|!l.glyphs.is_empty()){return StorageTreeObservation::default();}
            text_count+=1;
        }
        if text_count==0{return StorageTreeObservation::default();}
        // Publish only actual measured regions. Header controls are individually
        // measured, so the world-owned gaps between panes stay world-owned.
        let mut input_regions=regions;
        let mut header_actions=0;
        for (entity,action,node,transform,visible) in &self.actions {
            if !descendant(entity,root,&self.parents)||!visible.get(){continue;}
            let Some(rect)=logical_rect(node,transform)else{return StorageTreeObservation::default();};
            if matches!(action,StorageUiAction::SwitchPane|StorageUiAction::Drag|StorageUiAction::Cancel|StorageUiAction::Close){
                if !in_stage(rect){return StorageTreeObservation::default();}
                if p.touch&&(rect.width()*p.stage_css_scale<40.||rect.height()*p.stage_css_scale<40.){return StorageTreeObservation::default();}
                input_regions.push(rect);header_actions+=1;
            }
        }
        if header_actions!=(if p.touch{4}else{3}){return StorageTreeObservation::default();}
        StorageTreeObservation{complete:true,input_regions}
    }
}
fn update_readonly_locks(mut commands:Commands,r:Res<StorageUiReadModel>,
    roots:Query<Entity,With<StorageUiRoot>>,parents:Query<&ChildOf>,
    mut cells:Query<(Entity,&mut StoragePaintCell,Option<&mut BackgroundColor>)>,
){
    let Ok(root)=roots.single()else{return;};
    for (entity,mut cell,color) in &mut cells {
        if !descendant(entity,root,&parents){continue;}
        if !r.cell_idle(StorageCell{container:4,slot:cell.slot}) {
            cell.enabled=false;
            commands.entity(entity).remove::<StorageUiAction>().remove::<StoragePaintAction>();
            if let Some(mut color)=color{color.0=Color::srgba(0.25,0.19,0.10,0.8);}
        }
    }
}
