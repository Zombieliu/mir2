//! Strict, independently negotiated browser warehouse ABI.
//! Page owns Gateway transport and pending receipts; this ingress is read-only.
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use mir2_client_bevy::{
    inventory::InventoryModel, storage::StorageModel, read_model::PlayerStats,
    portable_storage_ui::{StorageIdentity,StoragePresentation,StorageCell,StoragePointerEdge,
        StoragePointerPhase,StorageUiContext,StorageUiReadModel,StorageUiIntent,StorageUiIntentKind,SAFE},
};
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct StorageSnapshot {
    #[serde(flatten)] identity:StorageIdentity,
    revision:u64,model_revision:u64,presentation_revision:u64,service_revision:u64,
    open:bool,input_enabled:bool,presentation:Option<StoragePresentation>,
    inventory:InventoryModel,storage:StorageModel,player:PlayerStats,
    blocked_unique_ids:Vec<u64>,pending_cells:Vec<StorageCell>,language:String,
}
/// Value parsing here preserves duplicate-member rejection before legacy model
/// deserializers/defaults can collapse or truncate authoritative data.
struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value=StrictValue;
            fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("bounded JSON without duplicate members")}
            fn visit_bool<E:serde::de::Error>(self,v:bool)->Result<Self::Value,E>{Ok(StrictValue(Value::Bool(v)))}
            fn visit_i64<E:serde::de::Error>(self,v:i64)->Result<Self::Value,E>{Ok(StrictValue(v.into()))}
            fn visit_u64<E:serde::de::Error>(self,v:u64)->Result<Self::Value,E>{Ok(StrictValue(v.into()))}
            fn visit_f64<E:serde::de::Error>(self,v:f64)->Result<Self::Value,E>{
                serde_json::Number::from_f64(v).map(|n|StrictValue(Value::Number(n))).ok_or_else(||E::custom("nonfinite number"))
            }
            fn visit_str<E:serde::de::Error>(self,v:&str)->Result<Self::Value,E>{Ok(StrictValue(Value::String(v.to_owned())))}
            fn visit_string<E:serde::de::Error>(self,v:String)->Result<Self::Value,E>{Ok(StrictValue(Value::String(v)))}
            fn visit_unit<E:serde::de::Error>(self)->Result<Self::Value,E>{Ok(StrictValue(Value::Null))}
            fn visit_none<E:serde::de::Error>(self)->Result<Self::Value,E>{Ok(StrictValue(Value::Null))}
            fn visit_seq<A:serde::de::SeqAccess<'de>>(self,mut a:A)->Result<Self::Value,A::Error>{
                let mut values=Vec::new();while let Some(StrictValue(v))=a.next_element()?{
                    if values.len()>=1024{return Err(serde::de::Error::custom("array too large"));}values.push(v);
                }Ok(StrictValue(Value::Array(values)))
            }
            fn visit_map<A:serde::de::MapAccess<'de>>(self,mut a:A)->Result<Self::Value,A::Error>{
                let mut values=serde_json::Map::new();
                while let Some(key)=a.next_key::<String>()?{
                    if key.len()>256||values.len()>=256||values.contains_key(&key){return Err(serde::de::Error::custom("duplicate or excessive member"));}
                    let StrictValue(v)=a.next_value()?;values.insert(key,v);
                }Ok(StrictValue(Value::Object(values)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
fn strict(json:&str)->Result<Value,&'static str>{
    if json.len()>1_048_576{return Err("storage JSON too large");}
    serde_json::from_str::<StrictValue>(json).map(|v|v.0).map_err(|_|"invalid storage JSON")
}
fn complete_items(model:&Value,max:usize)->Result<(),&'static str>{
    let items=model.get("items").and_then(Value::as_array).ok_or("missing storage items")?;
    if items.len()>max{return Err("storage item array too large");}
    for row in items{
        let object=row.as_object().ok_or("invalid storage item")?;
        for key in ["uniqueId","key","name","quantity","slot","container","icon","description"]{
            if !object.contains_key(key){return Err("incomplete storage item");}
        }
    }Ok(())
}
impl StorageSnapshot {
    fn parse(json:&str)->Result<Self,&'static str>{
        let value=strict(json)?;let root=value.as_object().ok_or("invalid storage snapshot")?;
        let inventory=root.get("inventory").ok_or("missing storage inventory")?;
        for key in ["capacity","gold","items"]{if inventory.get(key).is_none(){return Err("incomplete storage inventory");}}
        complete_items(inventory,160)?;
        let storage=root.get("storage").ok_or("missing storage model")?;
        // Keep StorageModel's original snake-case authority. Its expiry is an
        // i64 .NET DateTime value, deliberately exempt from UID safe bounds.
        for key in ["size","has_password","unlocked","has_expanded","expiry","items"]{
            if storage.get(key).is_none(){return Err("incomplete storage model");}
        }
        complete_items(storage,160)?;
        let player=root.get("player").and_then(Value::as_object).ok_or("missing storage player")?;
        for key in ["gold","name"]{if !player.contains_key(key){return Err("incomplete storage player");}}
        let snapshot:Self=serde_json::from_value(value).map_err(|_|"invalid storage snapshot fields")?;
        snapshot.validate()?;Ok(snapshot)
    }
    fn validate(&self)->Result<(),&'static str>{
        if !self.identity.valid()||[self.revision,self.model_revision,self.presentation_revision,self.service_revision]
            .into_iter().any(|v|!(1..=SAFE).contains(&v)){return Err("invalid storage identity/revisions");}
        if self.open&&!self.presentation.is_some_and(StoragePresentation::valid)||self.input_enabled&&!self.open{
            return Err("invalid storage presentation");
        }
        if self.inventory.capacity!=InventoryModel::canonical_capacity(self.inventory.capacity)||!(1..=160).contains(&self.storage.size){
            return Err("invalid storage capacity");
        }
        if !matches!(self.language.as_str(),"en"|"zh-CN"|"ja"){return Err("invalid storage language");}
        let mut cells=HashSet::new();let mut ids=HashSet::new();
        for item in self.inventory.items.iter().chain(self.storage.items.iter()){
            if item.quantity==0||item.key.len()>256||item.name.len()>256||item.description.len()>8192{
                return Err("invalid storage item content");
            }
            if let Some(id)=item.unique_id{if id>SAFE||!ids.insert(id){return Err("invalid/duplicate storage UID");}}
            if !cells.insert((item.container,item.slot)){return Err("duplicate storage cell");}
        }
        for item in &self.inventory.items{
            if !match item.container{0=>item.slot<u32::from(self.inventory.bag_slot_capacity()),1=>item.slot<6,2=>item.slot<14,3=>item.slot<40,_=>false}{
                return Err("invalid carried storage cell");
            }
        }
        for item in &self.storage.items{if item.container!=4||item.slot>=u32::from(self.storage.size){return Err("invalid warehouse cell");}}
        if self.blocked_unique_ids.len()>320||self.pending_cells.len()>320{return Err("storage locks too large");}
        let mut blocked=HashSet::new();
        for id in &self.blocked_unique_ids{if *id>SAFE||!blocked.insert(*id){return Err("invalid storage blocked UID");}}
        let mut pending=HashSet::new();
        for cell in &self.pending_cells{
            if !matches!(cell.container,0|4)||cell.slot>=(if cell.container==0{80}else{160})||!pending.insert(*cell){
                return Err("invalid storage pending cell");
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct StorageIngress {
    accepted:Option<(StorageIdentity,u64,u64,u64,u64)>,
    pending:Option<StorageSnapshot>,withdrawn:Option<StorageIdentity>,edges:Vec<StoragePointerEdge>,
    last_edge_sequence:u64,lease:Option<StoragePointerEdge>,cancel_pointer:bool,error:Option<String>,
}
impl StorageIngress {
    fn accept(&mut self,s:StorageSnapshot)->bool{
        if let Some((old,revision,model,presentation,service))=self.accepted{
            if !s.identity.can_follow(old)||s.identity.run_generation==old.run_generation
                &&(s.revision<=revision||s.model_revision<model||s.presentation_revision<presentation||s.service_revision<service){return false;}
        }
        if self.accepted.is_none_or(|(old,..)|old!=s.identity){self.last_edge_sequence=0;}
        self.accepted=Some((s.identity,s.revision,s.model_revision,s.presentation_revision,s.service_revision));
        self.pending=Some(s);self.withdrawn=None;self.edges.clear();self.lease=None;self.error=None;true
    }
    fn withdraw(&mut self,identity:StorageIdentity)->bool{
        if self.accepted.is_none_or(|(current,..)|current!=identity){return false;}
        self.pending=None;self.withdrawn=Some(identity);self.edges.clear();self.lease=None;self.error=None;true
    }
    fn pointer(&mut self,edge:StoragePointerEdge,current:bool)->bool{
        if self.pending.is_some()||self.withdrawn.is_some()||self.error.is_some(){return false;}
        if !edge.valid()||self.accepted.is_none_or(|(identity,_,model,presentation,service)|
            identity!=edge.identity||model!=edge.model_revision
                ||presentation!=edge.presentation_revision||service!=edge.service_revision)||edge.sequence<=self.last_edge_sequence{return false;}
        let cleanup=matches!(edge.phase,StoragePointerPhase::Up|StoragePointerPhase::Cancel|StoragePointerPhase::Blur);
        if !current&&!cleanup{return false;}
        if self.edges.len()>=256{return false;}
        if edge.phase==StoragePointerPhase::Down {
            if self.lease.is_some(){return false;}
        } else if !self.lease.is_some_and(|down|edge.matches_lease(down)){return false;}
        self.lease=if cleanup{None}else{Some(edge)};
        self.last_edge_sequence=edge.sequence;self.edges.push(edge);true
    }
}
#[derive(Debug,Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
struct InputRegion {left:f32,top:f32,width:f32,height:f32}
#[derive(Debug,Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
struct StorageStatus {
    #[serde(flatten)]identity:StorageIdentity,frame:u64,ready:bool,input_enabled:bool,
    applied_revision:u64,applied_model_revision:u64,applied_presentation_revision:u64,applied_service_revision:u64,
    input_regions:Vec<InputRegion>,error:Option<String>,
}
impl StorageStatus {
    fn disable(&mut self){self.ready=false;self.input_enabled=false;self.input_regions.clear();}
}
fn encode_intent(i:StorageUiIntent)->String {
    let mut value=serde_json::json!({"runGeneration":i.identity.run_generation,
        "connectionGeneration":i.identity.connection_generation,"sessionGeneration":i.identity.session_generation,
        "ownerRevision":i.identity.owner_revision,"intentSequence":i.intent_sequence,
        "modelRevision":i.model_revision,"presentationRevision":i.presentation_revision,"serviceRevision":i.service_revision});
    let object=value.as_object_mut().expect("literal object");
    match i.kind{
        StorageUiIntentKind::Transfer{kind,source,target}=>{
            object.insert("type".into(),kind.into());
            object.insert("source".into(),serde_json::to_value(source).expect("bounded selection"));
            object.insert("target".into(),serde_json::to_value(target).expect("bounded cell"));
            if kind=="moveItem"{object.insert("grid".into(),if source.container==4{"storage"}else{"inventory"}.into());}
        }
        StorageUiIntentKind::Merge{source,target,grid_from,grid_to}=>{
            object.insert("type".into(),"mergeItem".into());
            object.insert("source".into(),serde_json::to_value(source).expect("bounded selection"));
            object.insert("target".into(),serde_json::to_value(target).expect("bounded selection"));
            object.insert("gridFrom".into(),grid_from.wire_name().into());
            object.insert("gridTo".into(),grid_to.wire_name().into());
        }
        StorageUiIntentKind::Close=>{object.insert("type".into(),"close".into());}
        StorageUiIntentKind::Password=>{object.insert("type".into(),"password".into());}
        StorageUiIntentKind::Rent=>{object.insert("type".into(),"rent".into());}
    }value.to_string()
}

#[cfg(target_arch="wasm32")]
mod web {
    use super::*;
    use std::cell::RefCell;
    use bevy::prelude::*;
    use bevy::asset::LoadState;
    use bevy::ui::UiSystems;
    use js_sys::Function;
    use wasm_bindgen::prelude::*;
    use mir2_client_bevy::{
        portable_storage_ui::{Mir2PortableStorageUiPlugin,StorageUiState,StorageUiIntentQueue,
            StoragePointerEdges,StorageTreeObserver,required_skins},
        portable_quest_ui::QuestUiFont,crystal_ui::hud::SharedHudSurface,
    };
    const FONT:&str="original-ui/fonts/NotoSansCJKsc-Regular.otf";
    thread_local!{
        static INGRESS:RefCell<StorageIngress>=RefCell::new(StorageIngress::default());
        static STATUS:RefCell<StorageStatus>=RefCell::new(StorageStatus::default());
        static SINK:RefCell<Option<Function>>=const{RefCell::new(None)};
    }
    #[derive(Resource,Default)]
    struct Applied {snapshot:Option<StorageSnapshot>,font:Option<Handle<Font>>,
        skins:Vec<Handle<Image>>,icons:Vec<Handle<Image>>,paths:Vec<String>,asset_error:Option<String>}
    pub(crate) fn install(app:&mut App){
        app.init_resource::<Applied>().add_plugins(Mir2PortableStorageUiPlugin)
            .add_systems(Update,ingest.in_set(mir2_client_bevy::pending_operations::PendingLifecycleSet::Ingest)
                .after(crate::quest_ui_host::QuestHostIngestSet))
            .add_systems(PostUpdate,publish.after(UiSystems::Layout)
                .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate))
            .add_systems(Last,forward);
    }
    fn ingest(mut applied:ResMut<Applied>,mut c:ResMut<StorageUiContext>,mut read:ResMut<StorageUiReadModel>,
        mut state:ResMut<StorageUiState>,mut edges:ResMut<StoragePointerEdges>,mut queue:ResMut<StorageUiIntentQueue>,
        server:Res<AssetServer>,mut font:ResMut<QuestUiFont>){
        let (snapshot,withdrawn,error,cancel)=INGRESS.with(|i|{let mut i=i.borrow_mut();
            (i.pending.take(),i.withdrawn.take(),i.error.clone(),std::mem::take(&mut i.cancel_pointer))});
        if cancel{state.cancel_pointer();queue.clear();edges.0.clear();}
        if error.is_some()||withdrawn.is_some_and(|id|id==c.identity){
            c.open=false;c.input_enabled=false;c.ready=false;state.invalidate();queue.clear();edges.0.clear();applied.snapshot=None;
        }
        if let Some(s)=snapshot{
            c.ready=false;state.cancel_pointer();queue.clear();edges.0.clear();
            c.identity=s.identity;c.revision=s.revision;c.model_revision=s.model_revision;
            c.presentation_revision=s.presentation_revision;c.service_revision=s.service_revision;
            c.open=s.open;c.input_enabled=s.input_enabled;c.presentation=s.presentation;c.language=s.language.clone();
            *read=StorageUiReadModel{inventory:s.inventory.clone(),storage:s.storage.clone(),player:s.player.clone(),
                blocked_unique_ids:s.blocked_unique_ids.iter().copied().collect(),pending_cells:s.pending_cells.iter().copied().collect()};
            if s.open&&applied.font.is_none(){
                let handle=server.load::<Font>(FONT);font.0=handle.clone();applied.font=Some(handle);
                applied.skins=required_skins().into_iter().map(|p|server.load::<Image>(p)).collect();
            }
            let mut paths=Vec::new();let mut error=None;
            for item in s.inventory.items.iter().filter(|i|i.container==0)
                .chain(s.storage.items.iter().filter(|i|s.storage.is_valid_slot(i.slot))){
                match item.user_item_image_index(){
                    Some(index)=>paths.push(format!("original-ui/Items/{index}.png")),
                    None=>error=Some("storage item icon metadata unavailable".to_owned()),
                }
            }
            paths.sort();paths.dedup();
            if paths!=applied.paths{applied.icons=paths.iter().map(|p|server.load::<Image>(p.clone())).collect();applied.paths=paths;}
            applied.asset_error=error;applied.snapshot=Some(s);
        }
        edges.0.extend(INGRESS.with(|i|std::mem::take(&mut i.borrow_mut().edges)));
    }
    fn publish(mut c:ResMut<StorageUiContext>,applied:Res<Applied>,state:Res<StorageUiState>,
        read:Res<StorageUiReadModel>,surface:Res<SharedHudSurface>,server:Res<AssetServer>,
        windows:Query<&Window>,tree:StorageTreeObserver){
        let observation=tree.observe(&c,&state,&read);
        let window=c.presentation.zip(windows.get(surface.window).ok())
            .is_some_and(|(p,w)|w.visible&&p.matches_window(w.width(),w.height()));
        let mut loaded=applied.font.is_some();let mut failed=false;
        for load in applied.font.iter().map(|h|server.get_load_state(h.id()))
            .chain(applied.skins.iter().map(|h|server.get_load_state(h.id())))
            .chain(applied.icons.iter().map(|h|server.get_load_state(h.id()))){
            match load{Some(LoadState::Loaded)=>{},Some(LoadState::Failed(_))=>{failed=true;loaded=false;},_=>loaded=false}
        }
        let pending=INGRESS.with(|i|{let i=i.borrow();i.pending.is_some()||i.withdrawn.is_some()||i.error.is_some()});
        let sink=SINK.with(|s|s.borrow().is_some());
        let error=INGRESS.with(|i|i.borrow().error.clone()).or_else(||applied.asset_error.clone())
            .or_else(||state.error.map(str::to_owned)).or_else(||failed.then(||"storage assets failed to load".into()))
            .or_else(||(c.open&&!window).then(||"storage presentation does not match window".into()));
        c.ready=applied.snapshot.is_some()&&c.open&&window&&surface.active&&loaded&&!failed
            &&observation.complete&&sink&&!pending&&error.is_none();
        STATUS.with(|s|{
            let mut s=s.borrow_mut();s.frame=s.frame.saturating_add(1).min(SAFE);
            s.identity=c.identity;s.applied_revision=c.revision;s.applied_model_revision=c.model_revision;
            s.applied_presentation_revision=c.presentation_revision;s.applied_service_revision=c.service_revision;
            s.ready=c.ready;s.input_enabled=c.ready&&c.input_enabled;s.error=error;
            s.input_regions=if c.ready{observation.input_regions.into_iter().map(|r|InputRegion{
                left:r.min.x,top:r.min.y,width:r.width(),height:r.height()}).collect()}else{Vec::new()};
        });
    }
    fn forward(mut queue:ResMut<StorageUiIntentQueue>,mut state:ResMut<StorageUiState>,c:Res<StorageUiContext>){
        for intent in queue.drain(){
            // Recheck after every synchronous JS callback: it can retire this
            // owner or replace the snapshot/sink during the same Last schedule.
            let active=STATUS.with(|s|s.borrow().clone());
            let pending=INGRESS.with(|i|{let i=i.borrow();i.pending.is_some()||i.withdrawn.is_some()||i.error.is_some()});
            let sink=SINK.with(|s|s.borrow().clone());
            let current=active.input_enabled&&!pending&&intent.matches(&c)&&intent.identity==active.identity
                &&intent.model_revision==active.applied_model_revision
                &&intent.presentation_revision==active.applied_presentation_revision
                &&intent.service_revision==active.applied_service_revision;
            let accepted=current&&sink.as_ref().is_some_and(|sink|sink.call1(&JsValue::NULL,&JsValue::from_str(&encode_intent(intent)))
                .ok().and_then(|v|v.as_bool())==Some(true));
            state.feedback(intent,accepted,&c);
        }
    }
    #[wasm_bindgen(js_name=setMir2StorageUiSnapshot)]
    pub fn set_snapshot(json:String)->bool{
        let snapshot=match StorageSnapshot::parse(&json){Ok(s)=>s,Err(error)=>{
            INGRESS.with(|i|{let mut i=i.borrow_mut();i.error=Some(error.into());i.pending=None;i.edges.clear();i.lease=None;i.cancel_pointer=true;});
            STATUS.with(|s|{let mut s=s.borrow_mut();s.disable();s.error=Some(error.into());});return false;
        }};
        let accepted=INGRESS.with(|i|i.borrow_mut().accept(snapshot));
        if accepted{STATUS.with(|s|s.borrow_mut().disable());}
        accepted
    }
    #[wasm_bindgen(js_name=getMir2StorageUiStatus)]
    pub fn get_status()->String{STATUS.with(|s|serde_json::to_string(&*s.borrow()).expect("bounded storage status"))}
    #[wasm_bindgen(js_name=setMir2StorageUiIntentSink)]
    pub fn set_sink(sink:Function){SINK.with(|s|*s.borrow_mut()=Some(sink));
        INGRESS.with(|i|{let mut i=i.borrow_mut();i.edges.clear();i.lease=None;i.cancel_pointer=true;});
        STATUS.with(|s|s.borrow_mut().disable());}
    #[wasm_bindgen(js_name=clearMir2StorageUiIntentSink)]
    pub fn clear_sink(){SINK.with(|s|*s.borrow_mut()=None);
        INGRESS.with(|i|{let mut i=i.borrow_mut();i.edges.clear();i.lease=None;i.cancel_pointer=true;});
        STATUS.with(|s|s.borrow_mut().disable());}
    #[wasm_bindgen(js_name=withdrawMir2StorageUiSnapshot)]
    pub fn withdraw(json:String)->bool{
        let Ok(value)=strict(&json)else{return false;};
        let Ok(identity)=serde_json::from_value::<StorageIdentity>(value)else{return false;};
        if !identity.valid(){return false;}
        let retired=INGRESS.with(|i|i.borrow_mut().withdraw(identity));
        if retired{STATUS.with(|s|s.borrow_mut().disable());}retired
    }
    #[wasm_bindgen(js_name=setMir2StorageUiPointerEdge)]
    pub fn pointer(json:String)->bool{
        let Ok(value)=strict(&json)else{return false;};
        let Ok(edge)=serde_json::from_value::<StoragePointerEdge>(value)else{return false;};
        let active=STATUS.with(|s|{let s=s.borrow();s.input_enabled&&s.identity==edge.identity
            &&s.applied_model_revision==edge.model_revision
            &&s.applied_presentation_revision==edge.presentation_revision&&s.applied_service_revision==edge.service_revision});
        let settled=INGRESS.with(|i|{let i=i.borrow();i.pending.is_none()&&i.withdrawn.is_none()&&i.error.is_none()});
        INGRESS.with(|i|i.borrow_mut().pointer(edge,active&&settled))
    }
}
#[cfg(target_arch="wasm32")]
pub(crate) use web::install;

#[cfg(test)]
#[path="storage_ui_host_tests.rs"]
mod tests;
