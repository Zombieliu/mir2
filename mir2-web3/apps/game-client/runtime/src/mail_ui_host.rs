//! Independent Mail ABI. Raw authoritative mail is never changed by a UI action.
use mir2_client_bevy::{mail::{MailModel,MailMessage,MailAttachment},portable_mail_ui::{MailIdentity,SAFE,resolve_mail_attachment}};
use serde::{Deserialize,Serialize};
use std::collections::HashSet;
#[derive(Debug,Clone,Copy,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MailPresentation {pub logical_width:f32,pub logical_height:f32,pub stage_css_scale:f32,pub touch:bool}
impl MailPresentation {pub fn fits(self)->bool{[self.logical_width,self.logical_height,self.stage_css_scale].into_iter().all(f32::is_finite)
    &&self.logical_width>=1024.&&self.logical_width<=16384.&&self.logical_height>=768.&&self.logical_height<=16384.
    &&self.stage_css_scale>0.&&self.stage_css_scale<=16.&&(!self.touch||self.stage_css_scale*16.>=44.)}}
#[derive(Debug,Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MailAttachmentDto {pub unique_id:Option<u64>,pub item_index:Option<i32>,pub name:Option<String>,pub key:Option<String>,pub count:u16,
    pub current_dura:u16,pub max_dura:u16,pub soul_bound_id:i32,pub gem_count:u16,pub identified:Option<bool>,pub cursed:bool,#[serde(skip)]resolved_image:Option<u32>}
impl MailAttachmentDto {
    fn model(&self)->MailAttachment {MailAttachment{unique_id:self.unique_id,item_index:self.item_index,name:self.name.clone(),key:self.key.clone(),count:self.count,
        current_dura:self.current_dura,max_dura:self.max_dura,soul_bound_id:self.soul_bound_id,gem_count:self.gem_count,cursed:self.cursed,
        identified:self.identified.unwrap_or(false),image:self.resolved_image,..Default::default()}}
    fn resolve(&mut self)->Result<(),&'static str>{let mut item=self.model();resolve_mail_attachment(&mut item,self.identified)?;
        self.item_index=item.item_index;if self.unique_id.is_some(){self.key=item.item_index.map(|n|format!("crystal-item-{n}"));}self.identified=Some(item.identified);self.resolved_image=item.image;Ok(())}
}
#[derive(Debug,Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MailRow {pub mail_id:u64,pub sender_name:String,pub message:String,pub subject:String,pub opened:bool,pub locked:bool,
    pub can_reply:bool,pub collected:bool,pub gold:u32,pub items:Vec<MailAttachmentDto>,pub item_count:usize,
    pub date_sent_binary_datetime:Option<String>,pub metadata_known:bool}
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MailSnapshot {#[serde(flatten)]pub identity:MailIdentity,pub revision:u64,pub model_revision:u64,pub presentation_revision:u64,
    pub open:bool,pub input_enabled:bool,pub presentation:Option<MailPresentation>,pub mail:Vec<MailRow>}
impl MailSnapshot {
    pub fn parse(json:&str)->Result<Self,&'static str>{if json.len()>2*1024*1024{return Err("Mail snapshot bound");}
        let raw:serde_json::Value=serde_json::from_str(json).map_err(|_|"Mail schema")?;
        if let Some(rows)=raw.get("mail").and_then(serde_json::Value::as_array){for row in rows{if let Some(items)=row.get("items").and_then(serde_json::Value::as_array){for item in items{
            let keys=["uniqueId","itemIndex","name","key","count","currentDura","maxDura","soulBoundId","gemCount","identified","cursed"];
            if !item.as_object().is_some_and(|m|m.len()==keys.len()&&keys.iter().all(|k|m.contains_key(*k))){return Err("Mail attachment schema");}
        }}}}
        let mut s=mail_wire::snapshot(&raw).ok_or("Mail schema")?;s.validate()?;resolve_rows(&mut s.mail)?;Ok(s)}
    pub fn validate(&self)->Result<(),&'static str>{
        if !self.identity.valid()||[self.revision,self.model_revision,self.presentation_revision].into_iter().any(|n|n==0||n>SAFE)
            ||self.presentation.is_some_and(|p|!p.fits())||self.open&&self.presentation.is_none()||self.mail.len()>256{return Err("Mail owner/presentation bound");}
        validate_rows(&self.mail)
    }
    pub fn model(&self)->MailModel {MailModel{mails:self.mail.iter().map(|m|{
        let items=m.items.iter().map(MailAttachmentDto::model).collect();
        MailMessage{id:m.mail_id,sender:m.sender_name.clone(),subject:m.subject.clone(),body:m.message.clone(),can_reply:m.can_reply,
            date_sent_binary_datetime:m.date_sent_binary_datetime.as_ref().and_then(|d|d.parse().ok()).unwrap_or(0),metadata_known:m.metadata_known,
            read:m.opened,claimed:m.collected,locked:m.locked,gold:m.gold,items,operation:None}
        }).collect(),selected_id:None}}
}
fn validate_rows(rows:&[MailRow])->Result<(),&'static str>{
    if rows.len()>256{return Err("Mail row bound");}
        let mut ids=HashSet::new();
        for m in rows {if m.mail_id==0||m.mail_id>SAFE||!ids.insert(m.mail_id)||m.sender_name.len()>1024||m.message.len()>262144||m.subject.len()>16384
            ||m.items.len()>5||m.item_count!=m.items.len()||m.date_sent_binary_datetime.as_ref().is_some_and(|d|d.parse::<i64>().is_err())
            ||m.metadata_known&&m.date_sent_binary_datetime.is_none(){return Err("Mail row bound/identity");}
            for i in &m.items {if i.unique_id.is_some_and(|n|n==0||n>SAFE)||i.name.as_ref().is_some_and(|n|n.len()>2048)||i.key.as_ref().is_some_and(|n|n.len()>2048){return Err("Mail attachment identity");}}
        }Ok(())
}

fn resolve_rows(rows:&mut [MailRow])->Result<(),&'static str>{for row in rows {for item in &mut row.items {item.resolve()?;}}Ok(())}
/// Public presentation DTO only; no session, cache, or previous-row lookup.
/// The exact same once-per-carrier resolution feeds both this facade and snapshot ingestion.
pub fn resolve_mail_rows(json:&str)->Result<String,&'static str>{
    if json.len()>2*1024*1024{return Err("Mail rows bound");}
    let raw:serde_json::Value=serde_json::from_str(json).map_err(|_|"Mail rows schema")?;
    let source=raw.as_array().ok_or("Mail rows schema")?;
    for row in source {for item in row.get("items").and_then(serde_json::Value::as_array).ok_or("Mail rows schema")?{
        let keys=["uniqueId","itemIndex","name","key","count","currentDura","maxDura","soulBoundId","gemCount","identified","cursed"];
        if !item.as_object().is_some_and(|m|m.len()==keys.len()&&keys.iter().all(|k|m.contains_key(*k))){return Err("Mail attachment schema");}
    }}
    let mut rows=mail_wire::rows(&raw).ok_or("Mail rows schema")?;
    validate_rows(&rows)?;resolve_rows(&mut rows)?;
    serde_json::to_string(&rows).map_err(|_|"Mail rows serialize")
}
// Borrowed Value mapping keeps typed serde deserialization out of production Mail ingestion.
// Non-flatten structs also retain serde's exact-length field-sequence representation.
mod mail_wire {
    use super::*;
    use serde_json::Value;
    const SNAPSHOT: &[&str] = &["run", "connectionGeneration", "sessionGeneration", "ownerRevision", "sceneRevision", "hudGeneration", "playerObjectId", "revision", "modelRevision", "presentationRevision", "open", "inputEnabled", "presentation", "mail"];
    const PRESENTATION: &[&str] = &["logicalWidth", "logicalHeight", "stageCssScale", "touch"];
    const ROW: &[&str] = &["mailId", "senderName", "message", "subject", "opened", "locked", "canReply", "collected", "gold", "items", "itemCount", "dateSentBinaryDatetime", "metadataKnown"];
    const ATTACHMENT: &[&str] = &["uniqueId", "itemIndex", "name", "key", "count", "currentDura", "maxDura", "soulBoundId", "gemCount", "identified", "cursed"];

    struct Fields<'a> { value: &'a Value, keys: &'static [&'static str] }
    impl<'a> Fields<'a> {
        fn new(value: &'a Value, keys: &'static [&'static str], sequence: bool) -> Option<Self> {
            let valid = match value {
                Value::Object(map) => map.keys().all(|key| keys.contains(&key.as_str())),
                Value::Array(values) => sequence && values.len() == keys.len(),
                _ => false,
            };
            valid.then_some(Self { value, keys })
        }
        fn get(&self, index: usize) -> &Value {
            match self.value {
                Value::Array(values) => &values[index],
                _ => self.value.get(self.keys[index]).unwrap_or(&Value::Null),
            }
        }
    }
    fn optional<T>(value: &Value, read: fn(&Value) -> Option<T>) -> Option<Option<T>> {
        if value.is_null() { Some(None) } else { read(value).map(Some) }
    }
    fn string(value: &Value) -> Option<String> { value.as_str().map(str::to_owned) }
    fn u16_value(value: &Value) -> Option<u16> { u16::try_from(value.as_u64()?).ok() }
    fn u32_value(value: &Value) -> Option<u32> { u32::try_from(value.as_u64()?).ok() }
    fn i32_value(value: &Value) -> Option<i32> { i32::try_from(value.as_i64()?).ok() }
    fn f32_value(value: &Value) -> Option<f32> {
        // Serde casts integer visitors directly to f32, avoiding an intermediate f64 round.
        // Finite f64 overflow may produce infinity; presentation validation rejects it later.
        if let Some(n) = value.as_u64() { Some(n as f32) }
        else if let Some(n) = value.as_i64() { Some(n as f32) }
        else { value.as_f64().map(|n| n as f32) }
    }
    fn presentation(value: &Value) -> Option<MailPresentation> {
        let f = Fields::new(value, PRESENTATION, true)?;
        Some(MailPresentation { logical_width: f32_value(f.get(0))?, logical_height: f32_value(f.get(1))?, stage_css_scale: f32_value(f.get(2))?, touch: f.get(3).as_bool()? })
    }
    fn attachment(value: &Value) -> Option<MailAttachmentDto> {
        let f = Fields::new(value, ATTACHMENT, true)?;
        Some(MailAttachmentDto {
            unique_id: optional(f.get(0), Value::as_u64)?, item_index: optional(f.get(1), i32_value)?,
            name: optional(f.get(2), string)?, key: optional(f.get(3), string)?, count: u16_value(f.get(4))?,
            current_dura: u16_value(f.get(5))?, max_dura: u16_value(f.get(6))?, soul_bound_id: i32_value(f.get(7))?,
            gem_count: u16_value(f.get(8))?, identified: optional(f.get(9), Value::as_bool)?, cursed: f.get(10).as_bool()?, resolved_image: None,
        })
    }
    fn row(value: &Value) -> Option<MailRow> {
        let f = Fields::new(value, ROW, true)?;
        Some(MailRow {
            mail_id: f.get(0).as_u64()?, sender_name: string(f.get(1))?, message: string(f.get(2))?, subject: string(f.get(3))?,
            opened: f.get(4).as_bool()?, locked: f.get(5).as_bool()?, can_reply: f.get(6).as_bool()?, collected: f.get(7).as_bool()?,
            gold: u32_value(f.get(8))?, items: f.get(9).as_array()?.iter().map(attachment).collect::<Option<Vec<_>>>()?,
            item_count: usize::try_from(f.get(10).as_u64()?).ok()?, date_sent_binary_datetime: optional(f.get(11), string)?, metadata_known: f.get(12).as_bool()?,
        })
    }
    pub(super) fn rows(value: &Value) -> Option<Vec<MailRow>> { value.as_array()?.iter().map(row).collect() }
    pub(super) fn snapshot(value: &Value) -> Option<MailSnapshot> {
        // Flattened MailIdentity and deny_unknown_fields require one flat object.
        let f = Fields::new(value, SNAPSHOT, false)?;
        Some(MailSnapshot {
            identity: MailIdentity {
                run: f.get(0).as_u64()?, connection_generation: f.get(1).as_u64()?, session_generation: f.get(2).as_u64()?,
                owner_revision: f.get(3).as_u64()?, scene_revision: f.get(4).as_u64()?, hud_generation: f.get(5).as_u64()?, player_object_id: u32_value(f.get(6))?,
            },
            revision: f.get(7).as_u64()?, model_revision: f.get(8).as_u64()?, presentation_revision: f.get(9).as_u64()?,
            open: f.get(10).as_bool()?, input_enabled: f.get(11).as_bool()?, presentation: optional(f.get(12), presentation)?, mail: rows(f.get(13))?,
        })
    }
}

#[derive(Default)] pub struct MailMailbox {run:u64,revision:u64,pending:Option<MailSnapshot>}
impl MailMailbox {pub fn accept(&mut self,s:MailSnapshot)->bool {if s.identity.run<self.run||s.identity.run==self.run&&s.revision<=self.revision{return false;}
    self.run=s.identity.run;self.revision=s.revision;self.pending=Some(s);true}pub fn take(&mut self)->Option<MailSnapshot>{self.pending.take()}pub fn withdraw(&mut self){self.pending=None;}}

#[cfg(target_arch="wasm32")]
mod web {
    use super::*;
    use bevy::{prelude::*,asset::LoadState,ui::{UiGlobalTransform,UiSystems}};
    use mir2_client_bevy::{portable_mail_ui::*,crystal_ui::{hud::SharedHudSurface,mail_page_shared::MailPaintAction,spec::CrystalRect,widget::CrystalImageButton},read_model::UiReadModelIngress};
    use wasm_bindgen::prelude::*;use js_sys::Function;use std::cell::RefCell;
    #[derive(Resource,Default)] struct Applied {snapshot:Option<MailSnapshot>,paths:Vec<String>,images:Vec<Handle<Image>>,frame:u64}
    #[derive(Default,Serialize)] #[serde(rename_all="camelCase")]
    struct Status {#[serde(flatten)]identity:MailIdentity,version:u8,frame:u64,ready:bool,input_enabled:bool,modal:bool,
        render_revision:u64,applied_revision:u64,applied_model_revision:u64,applied_presentation_revision:u64,
        input_regions:Vec<mir2_client_bevy::crystal_ui::shared_hud::HudRect>,error:Option<String>}
    thread_local! {static MAILBOX:RefCell<MailMailbox>=RefCell::new(MailMailbox::default());static EDGES:RefCell<Vec<MailPointerEdge>>=const{RefCell::new(Vec::new())};
        static SINK:RefCell<Option<Function>>=const{RefCell::new(None)};static STATUS:RefCell<Status>=RefCell::new(Status{version:1,..Default::default()});static REJECTED:RefCell<bool>=const{RefCell::new(false)};}
    pub(crate) fn install(app:&mut App){super::compose_web::install(app);app.init_resource::<Applied>().add_plugins(Mir2PortableMailUiPlugin)
        .add_systems(Update,ingest.in_set(mir2_client_bevy::pending_operations::PendingLifecycleSet::Ingest).after(crate::quest_ui_host::QuestHostIngestSet))
        .add_systems(PostUpdate,publish.after(UiSystems::Layout).before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate));}
    fn required_assets(model:&MailModel)->Vec<String>{let mut p=Vec::new();
        for i in [7,670,672,675,540,541,542,686,687,688,193,194,195,680,681,682,683]{p.push(format!("original-ui/Title/{i}.png"));}
        for i in [540,541,545,550,551,552,563,564,565,569,570,571,572,573,574,557,558,559,520,521,522,523,524,525]{p.push(format!("original-ui/Prguse/{i}.png"));}
        for i in [360,361,362,257,258,259,240,241,242,243,244,245]{p.push(format!("original-ui/Prguse2/{i}.png"));}
        for m in &model.mails{for i in &m.items{if let Some(image)=i.image{p.push(format!("original-ui/Items/{image}.png"));}}}p.sort();p.dedup();p}
    fn ingest(mut a:ResMut<Applied>,mut c:ResMut<MailUiContext>,mut s:ResMut<MailUiState>,mut q:ResMut<MailPointerQueue>,mut intents:ResMut<MailIntentQueue>,server:Res<AssetServer>,surface:Res<SharedHudSurface>,ingress:Res<UiReadModelIngress>){
        if REJECTED.with(|r|r.replace(false)){a.snapshot=None;c.active=false;c.ready=false;s.close();q.0.clear();intents.0.clear();}
        if let Some(snapshot)=MAILBOX.with(|m|m.borrow_mut().take()) {let model=snapshot.model();c.ready=false;c.identity=snapshot.identity;c.revision=snapshot.revision;c.model_revision=snapshot.model_revision;c.presentation_revision=snapshot.presentation_revision;
            c.input_enabled=snapshot.input_enabled;c.presentation=snapshot.presentation.map(|p|mir2_client_bevy::portable_bag_ui::BagUiPresentation{logical_width:p.logical_width,logical_height:p.logical_height,stage_css_scale:p.stage_css_scale,touch:p.touch});
            s.ingest(&c,model.clone());q.0.clear();intents.0.clear();let paths=required_assets(&model);if a.paths!=paths{a.images=paths.iter().map(|p|server.load::<Image>(p.clone())).collect();a.paths=paths;}a.snapshot=Some(snapshot);}
        c.active=a.snapshot.as_ref().is_some_and(|v|v.open&&surface.active&&surface.generation==v.identity.hud_generation&&ingress.has_complete_generation(v.identity.hud_generation));
        if !c.active{c.ready=false;s.close();intents.0.clear();}q.0.extend(EDGES.with(|e|std::mem::take(&mut *e.borrow_mut())));
    }
    fn publish(mut a:ResMut<Applied>,mut c:ResMut<MailUiContext>,mut s:ResMut<MailUiState>,surface:Res<SharedHudSurface>,server:Res<AssetServer>,images:Res<Assets<Image>>,windows:Query<&Window>,
        mut roots:Query<(&MailRoot,&ComputedNode,&mut Visibility)>,buttons:Query<(&PortableMailAction,&ComputedNode,&UiGlobalTransform,Option<&CrystalImageButton>)>,mut intents:ResMut<MailIntentQueue>){
        let assets=matches!(server.get_load_state(surface.font.id()),Some(LoadState::Loaded))&&a.images.iter().all(|h|matches!(server.get_load_state(h.id()),Some(LoadState::Loaded))&&images.get(h).is_some_and(|i|i.width()>0&&i.height()>0));
        let window=c.presentation.zip(windows.get(surface.window).ok()).is_some_and(|(p,w)|w.visible&&(w.width()-p.logical_width).abs()<=1.&&(w.height()-p.logical_height).abs()<=1.);
        // An input proof belongs to the last actually published layout. Dispatch before replacing its status.
        for i in std::mem::take(&mut intents.0){let valid=STATUS.with(|v|{let v=v.borrow();v.ready&&v.input_enabled&&v.identity==i.identity&&v.render_revision==i.render_revision&&v.applied_model_revision==i.model_revision&&v.applied_presentation_revision==i.presentation_revision})
            &&assets&&window&&surface.active&&surface.generation==c.identity.hud_generation&&c.active&&c.input_enabled&&i.identity==c.identity&&i.model_revision==c.model_revision&&i.presentation_revision==c.presentation_revision;
            if valid {let sink=SINK.with(|sink|sink.borrow().clone());if let Some(f)=sink{if let Ok(json)=serde_json::to_string(&i){let _=f.call1(&JsValue::NULL,&JsValue::from_str(&json));}}}}
        a.frame=a.frame.saturating_add(1).min(SAFE);
        let expected=1+usize::from(s.reader.is_some());let layout=roots.iter().count()==expected&&roots.iter().all(|(r,n,_)|r.matches(&c,&s)&&(n.size().x-if r.reader{236.}else{312.}).abs()<=1.&&(n.size().y-if r.reader{if s.reader.as_ref().is_some_and(|m|m.has_attachment()){384.}else{300.}}else{444.}).abs()<=1.);
        let mut controls=Vec::new();let mut invalid=false;
        for (action,n,t,button)in &buttons {if button.is_some_and(|b|!b.enabled)||!s.allowed(action.0){continue;}
            if s.reader.is_some()&&!matches!(action.0,MailPaintAction::ReaderClose|MailPaintAction::ReaderDelete|MailPaintAction::ReaderLock|MailPaintAction::ReaderClaim){continue;}
            let size=n.size()*n.inverse_scale_factor;let center=t.affine().translation*n.inverse_scale_factor;
            if !size.is_finite()||!center.is_finite()||size.x<=0.||size.y<=0.{invalid=true;continue;}
            controls.push((action.0,CrystalRect::new(center.x-size.x*0.5,center.y-size.y*0.5,size.x,size.y)));}
        let ready=c.active&&assets&&window&&layout&&!invalid&&!controls.is_empty();c.ready=ready;s.controls=if ready&&c.input_enabled{controls}else{vec![]};if !ready||!c.input_enabled{s.withdraw();}
        for (_,_,mut v)in &mut roots{*v=if ready&&c.input_enabled{Visibility::Inherited}else{Visibility::Hidden};}
        let region=if s.reader.is_some(){CrystalRect::new(464.,90.,236.,if s.reader.as_ref().is_some_and(|m|m.has_attachment()){384.}else{300.})}else{CrystalRect::new(712.,70.,312.,444.)};
        STATUS.with(|v|*v.borrow_mut()=Status{identity:c.identity,version:1,frame:a.frame,ready,input_enabled:ready&&c.input_enabled,modal:s.reader.is_some(),render_revision:s.render_revision,
            applied_revision:c.revision,applied_model_revision:c.model_revision,applied_presentation_revision:c.presentation_revision,
            input_regions:if ready&&c.input_enabled{vec![mir2_client_bevy::crystal_ui::shared_hud::HudRect{left:region.left,top:region.top,width:region.width,height:region.height}]}else{vec![]},error:None});
    }
    #[wasm_bindgen(js_name=setMir2MailUiSnapshot)]pub fn set_snapshot(json:String)->bool {match MailSnapshot::parse(&json){
        Ok(snapshot)=>MAILBOX.with(|m|m.borrow_mut().accept(snapshot)),
        Err(_)=>{MAILBOX.with(|m|m.borrow_mut().withdraw());EDGES.with(|e|e.borrow_mut().clear());REJECTED.with(|r|*r.borrow_mut()=true);STATUS.with(|s|{let mut s=s.borrow_mut();s.ready=false;s.input_enabled=false;});false}
    }}
    #[wasm_bindgen(js_name=resolveMir2MailUiRows)]pub fn resolve(json:String)->Option<String>{resolve_mail_rows(&json).ok()}
    #[wasm_bindgen(js_name=getMir2MailUiStatus)]pub fn status()->String {STATUS.with(|s|serde_json::to_string(&*s.borrow()).unwrap())}
    #[wasm_bindgen(js_name=setMir2MailUiIntentSink)]pub fn set_sink(sink:Function){SINK.with(|s|*s.borrow_mut()=Some(sink));}
    #[wasm_bindgen(js_name=clearMir2MailUiIntentSink)]pub fn clear_sink(){SINK.with(|s|*s.borrow_mut()=None);MAILBOX.with(|m|m.borrow_mut().withdraw());EDGES.with(|e|e.borrow_mut().clear());REJECTED.with(|r|*r.borrow_mut()=true);STATUS.with(|s|{let mut s=s.borrow_mut();s.ready=false;s.input_enabled=false;});}
    #[wasm_bindgen(js_name=setMir2MailUiPointerEdge)]pub fn pointer(json:String)->bool {let Ok(e)=serde_json::from_str::<MailPointerEdge>(&json)else{return false;};
        if !e.identity.valid()||e.sequence==0||e.sequence>SAFE||e.pointer_id>SAFE||!e.x.is_finite()||!e.y.is_finite()||e.button!=0||!matches!(e.phase.as_str(),"down"|"move"|"up"|"cancel"){return false;}
        EDGES.with(|q|{let mut q=q.borrow_mut();if q.len()>=64{REJECTED.with(|r|*r.borrow_mut()=true);return false;}q.push(e);true})}
}
#[cfg(target_arch="wasm32")]pub(crate) use web::install;
#[cfg(test)]#[path="mail_ui_host_tests.rs"]mod tests;

// Independent M14c2 compose/text ABI. The old M13 DTOs above stay exact.
use mir2_client_bevy::crystal_ui::mail_compose_shared::{ComposeIdentity,ComposeFullRaw,ComposeKind,ComposePresentation,
    ComposeLayout,ParcelPaint,TextProof,TextEdge,TextIntent,ComposeAction,GoldPaint,ComposePointerEdge,ComposePointerRouter,PromptMutation};
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ComposeUiSnapshot {
    pub owner:ComposeIdentity,pub revision:u64,pub incarnation:u64,pub draft_epoch:u64,
    pub draft_generation:String,pub presentation_revision:u64,pub layout_revision:u64,
    pub open:bool,pub input_enabled:bool,pub kind:ComposeKind,pub presentation:ComposePresentation,
    pub raw:ComposeFullRaw,pub gold:u32,pub parcel:ParcelPaint,pub notice:Option<String>,
    pub recipient_prompt:Option<String>,pub feedback:Option<String>,pub gold_prompt:Option<GoldPaint>,
}
impl ComposeUiSnapshot {
    pub fn parse(json:&str)->Result<Self,&'static str> {
        if json.len()>32768{return Err("Compose snapshot bound");}
        // Deserialize directly: duplicate/unknown typed fields are not erased
        // by a Value-map round trip. Full raw is opaque and never normalized.
        let s:Self=serde_json::from_str(json).map_err(|_|"Compose snapshot schema")?;
        let v:serde_json::Value=serde_json::from_str(json).map_err(|_|"Compose snapshot schema")?;
        if !compose_exact(&v,&["owner","revision","incarnation","draftEpoch","draftGeneration","presentationRevision","layoutRevision","open","inputEnabled","kind","presentation","raw","gold","parcel","notice","recipientPrompt","feedback","goldPrompt"])
            ||!compose_exact(&v["parcel"],&["stamped","stampAvailable","quoteReady","postage","quoteError","cells"])
            ||v["parcel"]["cells"].as_array().is_some_and(|cells|cells.iter().any(|c|!compose_exact(c,&["uniqueId","image","countLabel"])))
            ||!v["goldPrompt"].is_null()&&!compose_exact(&v["goldPrompt"],&["draft","maxAmount","amount"]) {return Err("Compose exact nullable fields");}
        if !s.owner.valid()||!s.raw.valid()||[s.revision,s.incarnation,s.draft_epoch,s.presentation_revision,s.layout_revision]
            .into_iter().any(|n|n==0||n>SAFE)
            || !s.draft_generation.parse::<u64>().ok().is_some_and(|n|n>0&&n.to_string()==s.draft_generation)
            || ComposeLayout::for_presentation(s.presentation,s.kind).is_none()
            || s.parcel.cells.len()>5 || s.parcel.stamped!=s.raw.stamped
            || s.parcel.cells.iter().enumerate().any(|(i,c)|c.unique_id==0||c.unique_id>SAFE||c.count_label.len()>1024
                ||s.parcel.cells[..i].iter().any(|old|old.unique_id==c.unique_id)
                ||!s.raw.attachment_unique_ids.iter().any(|id|id==&c.unique_id.to_string()))
            || s.gold_prompt.as_ref().is_some_and(|g|g.draft.len()>256||g.amount.is_some()&&g.amount!=mir2_client_bevy::crystal_ui::mail_compose_shared::gold_prompt_amount(g))
            || s.recipient_prompt.as_ref().is_some_and(|s|s.len()>1024)
            || usize::from(s.recipient_prompt.is_some())+usize::from(s.feedback.is_some())+usize::from(s.gold_prompt.is_some())>1
            || [&s.notice,&s.recipient_prompt,&s.feedback,&s.parcel.quote_error].into_iter().any(|s|s.as_ref().is_some_and(|s|s.len()>8192))
        {return Err("Compose owner/raw/presentation bound");}
        Ok(s)
    }
}
fn compose_exact(v:&serde_json::Value,keys:&[&str])->bool {v.as_object().is_some_and(|m|m.len()==keys.len()&&keys.iter().all(|k|m.contains_key(*k)))}
#[derive(Default)]pub struct ComposeUiMailbox {last:Option<ComposeUiSnapshot>,pending:Option<ComposeUiSnapshot>}
impl ComposeUiMailbox {
    pub fn accept(&mut self,s:ComposeUiSnapshot)->bool {
        if self.last.as_ref().is_some_and(|old|s.owner.run<old.owner.run
            ||s.owner.run==old.owner.run&&s.revision<=old.revision
            ||s.owner==old.owner&&(s.draft_generation.parse::<u64>().ok()<old.draft_generation.parse::<u64>().ok()
                ||s.raw!=old.raw&&s.draft_generation==old.draft_generation)) {return false;}
        self.last=Some(s.clone());self.pending=Some(s);true
    }
    pub fn take(&mut self)->Option<ComposeUiSnapshot>{self.pending.take()}
    pub fn pending(&self)->bool{self.pending.is_some()}
    pub fn withdraw(&mut self){self.pending=None;}
}
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ComposeUiActionEdge {pub proof:TextProof,pub action:ComposeAction}

#[cfg(target_arch="wasm32")]
mod compose_web {
    use super::*;
    use bevy::{prelude::*,asset::LoadState,ui::{UiGlobalTransform,UiSystems}};
    use mir2_client_bevy::{portable_mail_ui::{PortableComposeSurface,PortableComposeRoot,PortableComposeAction,
        PortableComposeEdges,PortableComposeIntents},crystal_ui::{hud::SharedHudSurface,
        mail_editor::{MailLetterEditor,MailLetterEditText,MailEditorCaret,MailEditorViewport},mail_compose_shared::TextOperation,
        widget::CrystalImageButton},read_model::UiReadModelIngress};
    use wasm_bindgen::prelude::*;
    use js_sys::Function;
    use std::cell::RefCell;
    #[derive(Debug,Clone,Serialize)]#[serde(rename_all="camelCase")]
    struct Rect {left:f32,top:f32,width:f32,height:f32}
    #[derive(Debug,Clone,Serialize)]#[serde(rename_all="camelCase")]
    struct Caret {left:f32,top:f32,width:f32,height:f32,anchor_utf16:usize,caret_utf16:usize,editor_revision:u64,layout_revision:u64}
    #[derive(Default,Clone,Serialize)]#[serde(rename_all="camelCase")]
    struct ComposeStatus {version:u8,owner:ComposeIdentity,ready:bool,input_enabled:bool,modal:bool,proof:Option<TextProof>,
        text_target:Option<String>,prompt_text:Option<String>,prompt_viewport:Option<Rect>,body_viewport:Option<Rect>,
        input_regions:Vec<Rect>,caret:Option<Caret>,scroll:[f32;2],error:Option<String>}
    #[derive(Resource,Default)]struct ComposeApplied {snapshot:Option<ComposeUiSnapshot>,images:Vec<Handle<Image>>,font:Option<Handle<Font>>}
    #[derive(Resource,Default)]struct AllowedActions(Vec<(ComposeAction,mir2_client_bevy::crystal_ui::spec::CrystalRect)>);
    thread_local! {
        static MAILBOX:RefCell<ComposeUiMailbox>=RefCell::new(ComposeUiMailbox::default());
        static EDGES:RefCell<Vec<TextEdge>>=const{RefCell::new(Vec::new())};
        static POINTERS:RefCell<Vec<ComposePointerEdge>>=const{RefCell::new(Vec::new())};
        static ACTIONS:RefCell<Vec<ComposeUiActionEdge>>=const{RefCell::new(Vec::new())};
        static ADMITTED:RefCell<Vec<TextProof>>=const{RefCell::new(Vec::new())};
        static SINK:RefCell<Option<Function>>=const{RefCell::new(None)};
        static STATUS:RefCell<ComposeStatus>=RefCell::new(ComposeStatus{version:1,..default()});
        static REJECTED:RefCell<bool>=const{RefCell::new(false)};
    }
    pub(crate) fn install(app:&mut App) {
        app.init_resource::<ComposeApplied>().init_resource::<AllowedActions>().init_resource::<ComposePointerRouter>()
            .add_systems(Update,(ingest,pointers,actions).chain().in_set(mir2_client_bevy::pending_operations::PendingLifecycleSet::Ingest)
                .after(crate::quest_ui_host::QuestHostIngestSet))
            .add_systems(PostUpdate,publish.after(UiSystems::PostLayout)
                .after(mir2_client_bevy::crystal_ui::mail_editor::MailEditorLayoutSet)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate));
    }
    fn assets(s:&ComposeUiSnapshot)->Vec<String> {
        let mut paths=vec![];
        for i in [671,674,676,607,608,609,193,194,195,200,201,202,203,204,205] {paths.push(format!("original-ui/Title/{i}.png"));}
        for i in [360,361,362,203,204] {paths.push(format!("original-ui/Prguse2/{i}.png"));}
        for i in [360,660,238,116] {paths.push(format!("original-ui/Prguse/{i}.png"));}
        for cell in &s.parcel.cells {if let Some(image)=cell.image {paths.push(format!("original-ui/Items/{image}.png"));}}
        paths
    }
    fn ingest(mut a:ResMut<ComposeApplied>,mut c:ResMut<PortableComposeSurface>,mut e:ResMut<MailLetterEditor>,
        mut adapter:ResMut<mir2_client_bevy::crystal_ui::mail_compose_shared::CommonMailTextAdapter>,
        mut edges:ResMut<PortableComposeEdges>,mut out:ResMut<PortableComposeIntents>,server:Res<AssetServer>,
        surface:Res<SharedHudSurface>,ingress:Res<UiReadModelIngress>,mut pointers:ResMut<ComposePointerRouter>) {
        if REJECTED.with(|v|v.replace(false)){a.snapshot=None;c.withdraw();e.retire_host();adapter.retire();edges.0.clear();out.0.clear();}
        if let Some(s)=MAILBOX.with(|m|m.borrow_mut().take()) {
            ADMITTED.with(|v|v.borrow_mut().clear());
            pointers.retire();
            let owner_changed=c.owner!=s.owner||c.incarnation!=s.incarnation||c.draft_epoch!=s.draft_epoch;
            if owner_changed{e.retire_host();adapter.retire();}
            let old_target=if c.feedback.is_some(){"feedback"}else if c.gold_prompt.is_some(){"gold"}else if c.recipient_prompt.is_some(){"recipient"}else{"body"};
            let new_target=if s.feedback.is_some(){"feedback"}else if s.gold_prompt.is_some(){"gold"}else if s.recipient_prompt.is_some(){"recipient"}else{"body"};
            if old_target!=new_target{e.retire_focus_capture();adapter.retire();}
            if c.recipient_prompt!=s.recipient_prompt
                ||c.gold_prompt.as_ref().map(|g|(&g.draft,g.max_amount))!=s.gold_prompt.as_ref().map(|g|(&g.draft,g.max_amount)) {
                e.retire_text_capture();adapter.retire();
            }
            let layout=ComposeLayout::for_presentation(s.presentation,s.kind);
            let font_changed=a.font.as_ref().is_some_and(|f|*f!=surface.font);
            let mounted=(!font_changed||owner_changed||s.layout_revision>c.layout_revision)&&layout.as_ref().is_some_and(|l|e.mount_external(&s.raw.body,s.draft_generation.parse().unwrap_or(0),l.body,s.layout_revision));
            if c.owner.run!=s.owner.run{c.highwater=0;}
            c.owner=s.owner;c.revision=s.revision;c.incarnation=s.incarnation;c.draft_epoch=s.draft_epoch;
            c.draft_generation=s.draft_generation.clone();c.presentation_revision=s.presentation_revision;c.layout_revision=s.layout_revision;
            c.kind=Some(s.kind);c.presentation=Some(s.presentation);c.layout=layout;c.ready=false;c.awaiting_draft=false;c.input_enabled=s.input_enabled;
            c.raw=Some(s.raw.clone());c.draft.recipient=s.raw.to.clone();c.draft.message=s.raw.body.clone();c.draft.gold=s.gold;
            c.draft.attachment_unique_ids=s.raw.attachment_unique_ids.iter().filter_map(|s|s.parse().ok()).collect();
            c.parcel=s.parcel.clone();c.notice=s.notice.clone();c.recipient_prompt=s.recipient_prompt.clone();c.feedback=s.feedback.clone();
            c.gold_prompt=s.gold_prompt.clone().map(|mut g|{g.amount=mir2_client_bevy::crystal_ui::mail_compose_shared::gold_prompt_amount(&g);g});
            e.set_focused(c.recipient_prompt.is_none()&&c.gold_prompt.is_none()&&c.feedback.is_none());
            edges.0.clear();out.0.clear();a.images=assets(&s).iter().map(|p|server.load(p.clone())).collect();
            a.font=Some(surface.font.clone());a.snapshot=mounted.then_some(s);
        }
        c.active=a.snapshot.as_ref().is_some_and(|s|s.open&&surface.active&&a.font.as_ref()==Some(&surface.font)&&surface.generation==s.owner.hud_generation&&ingress.has_complete_generation(s.owner.hud_generation));
        if !c.active{c.ready=false;edges.0.clear();out.0.clear();adapter.retire();}
        else {edges.0.extend(EDGES.with(|q|std::mem::take(&mut*q.borrow_mut())));}
    }
    fn pointers(mut c:ResMut<PortableComposeSurface>,mut e:ResMut<MailLetterEditor>,allowed:Res<AllowedActions>,
        mut router:ResMut<ComposePointerRouter>,mut out:ResMut<PortableComposeIntents>) {
        for edge in POINTERS.with(|q|std::mem::take(&mut*q.borrow_mut())) {
            let Some(current)=c.proof(&e,edge.proof.sequence)else{router.retire();continue;};
            if !c.ready||!c.input_enabled||c.awaiting_draft||edge.proof.sequence<=c.highwater {router.retire();continue;}
            let Some(raw)=c.raw.clone()else{continue;};
            let mut result=router.handle(&current,edge,&allowed.0,&mut e,&c.draft,&raw);
            c.highwater=c.highwater.max(router.highwater());
            if let Some(intent)=result.as_mut() {
                if intent.action=="goldConfirm"{intent.prompt_mutation=c.gold_prompt.as_ref().map(|p|PromptMutation{target:"gold".into(),text:p.draft.clone()});intent.prompt_value=c.gold_prompt.as_ref().and_then(mir2_client_bevy::crystal_ui::mail_compose_shared::gold_prompt_amount);}
                if intent.action=="recipientSubmit"{intent.prompt_mutation=c.recipient_prompt.as_ref().map(|s|PromptMutation{target:"recipient".into(),text:s.clone()});}
            }
            if let Some(intent)=result{out.0.push(intent);}
        }
    }
    fn actions(mut c:ResMut<PortableComposeSurface>,e:Res<MailLetterEditor>,allowed:Res<AllowedActions>,
        mut edges:ResMut<PortableComposeEdges>,mut out:ResMut<PortableComposeIntents>) {
        for edge in ACTIONS.with(|q|std::mem::take(&mut*q.borrow_mut())) {
            let Some(current)=c.proof(&e,edge.proof.sequence)else{continue;};
            if !c.ready||!c.input_enabled||c.awaiting_draft||!current.same_capture(&edge.proof)
                ||edge.proof.sequence<=c.highwater{continue;}
            if !allowed.0.iter().any(|(action,_)|*action==edge.action){c.highwater=edge.proof.sequence;continue;}
            if edge.action==ComposeAction::Submit {
                edges.0.push(TextEdge{proof:edge.proof,operation:TextOperation::Submit});continue;
            }
            c.highwater=edge.proof.sequence;
            let Some(raw)=c.raw.clone()else{continue;};
            let action=match edge.action {ComposeAction::Close=>"close",ComposeAction::Cancel=>"cancel",ComposeAction::Body=>"focus",
                ComposeAction::Recipient=>"recipient",ComposeAction::Gold=>"gold",ComposeAction::Stamp=>"stamp",ComposeAction::Slot(_)=>"slot",
                ComposeAction::RecipientSubmit=>"recipientSubmit",ComposeAction::RecipientCancel=>"recipientCancel",ComposeAction::Feedback=>"feedback",
                ComposeAction::GoldConfirm=>"goldConfirm",ComposeAction::GoldCancel=>"goldCancel",_=>continue};
            out.0.push(TextIntent{proof:edge.proof,action:action.into(),base_raw:raw,body_mutation:None,recipient_mutation:None,
                prompt_mutation:if edge.action==ComposeAction::GoldConfirm{c.gold_prompt.as_ref().map(|p|PromptMutation{target:"gold".into(),text:p.draft.clone()})}
                    else if edge.action==ComposeAction::RecipientSubmit{c.recipient_prompt.as_ref().map(|s|PromptMutation{target:"recipient".into(),text:s.clone()})}else{None},
                prompt_value:if edge.action==ComposeAction::GoldConfirm{c.gold_prompt.as_ref().and_then(mir2_client_bevy::crystal_ui::mail_compose_shared::gold_prompt_amount)}else{None},
                request_id:if let ComposeAction::Slot(slot)=edge.action{Some(u64::from(slot))}else{None},clipboard_kind:None,selected_text:None,error:None});
        }
    }
    fn rect(n:&ComputedNode,t:&UiGlobalTransform)->Option<Rect> {
        let size=n.size()*n.inverse_scale_factor;let center=t.affine().translation*n.inverse_scale_factor;
        (size.is_finite()&&center.is_finite()&&size.min_element()>0.).then_some(Rect{left:center.x-size.x*0.5,top:center.y-size.y*0.5,width:size.x,height:size.y})
    }
    fn publish(a:Res<ComposeApplied>,mut c:ResMut<PortableComposeSurface>,e:Res<MailLetterEditor>,
        surface:Res<SharedHudSurface>,server:Res<AssetServer>,images:Res<Assets<Image>>,windows:Query<&Window>,
        mut roots:Query<(&PortableComposeRoot,&ComputedNode,&UiGlobalTransform,&mut Visibility)>,
        buttons:Query<(&PortableComposeAction,&ComputedNode,&UiGlobalTransform,Option<&CrystalImageButton>)>,
        texts:Query<(&MailLetterEditText,&TextFont)>,viewports:Query<(&ComputedNode,&UiGlobalTransform),With<MailEditorViewport>>,
        prompts:Query<(&mir2_client_bevy::crystal_ui::mail_compose_shared::ComposePromptField,&ComputedNode,&UiGlobalTransform)>,carets:Query<(&ComputedNode,&UiGlobalTransform),With<MailEditorCaret>>,
        prompt_frames:Query<(&ComputedNode,&UiGlobalTransform),With<mir2_client_bevy::crystal_ui::mail_compose_shared::ComposePromptFrame>>,
        mut allowed:ResMut<AllowedActions>,mut intents:ResMut<PortableComposeIntents>) {
        let loaded=matches!(server.get_load_state(surface.font.id()),Some(LoadState::Loaded))&&a.images.iter().all(|h|
            matches!(server.get_load_state(h.id()),Some(LoadState::Loaded))&&images.get(h).is_some_and(|i|i.width()>0&&i.height()>0));
        let window=c.presentation.zip(windows.get(surface.window).ok()).is_some_and(|(p,w)|w.visible&&(w.width()-p.logical_width).abs()<=1.&&(w.height()-p.logical_height).abs()<=1.);
        let measured=roots.single().ok().and_then(|(r,n,t,_)|{
            let expected=c.layout.as_ref()?;let actual=rect(n,t)?;
            (r.matches(&c,&e)&&(actual.left-expected.root.left).abs()<=1.&&(actual.top-expected.root.top).abs()<=1.
                &&(actual.width-expected.root.width).abs()<=1.&&(actual.height-expected.root.height).abs()<=1.)
                .then_some(actual)
        });
        let shaped=(e.layout_ready()||e.active_editor().is_some_and(|e|e.text().is_empty()))&&texts.single().ok().is_some_and(|(t,f)|t.revision==e.revision()&&t.layout_revision==e.layout_revision()
            &&t.viewport==[e.content_size().x,e.content_size().y]&&f.font==bevy::text::FontSource::Handle(surface.font.clone())
            &&c.layout.as_ref().is_some_and(|l|f.font_size==FontSize::Px(l.body_font_px)));
        // Shape capture can change paint revision in this frame. Keep the exact
        // admitted proof/base until the actual root and shaped font settle.
        // Never hold a RefCell borrow across the reentrant JavaScript callback.
        for intent in std::mem::take(&mut intents.0) {
            let valid=ADMITTED.with(|v|v.borrow().iter().any(|p|p==&intent.proof))
                &&c.active&&surface.active&&surface.generation==c.owner.hud_generation&&intent.proof.owner==c.owner
                &&intent.proof.incarnation==c.incarnation&&intent.proof.draft_epoch==c.draft_epoch
                &&intent.proof.draft_generation==c.draft_generation&&intent.proof.presentation_revision==c.presentation_revision
                &&intent.proof.layout_revision==c.layout_revision&&c.raw.as_ref()==Some(&intent.base_raw);
            if !valid{continue;}
            if !loaded||!window||measured.is_none()||!shaped {intents.0.push(intent);continue;}
            let sink=SINK.with(|v|v.borrow().clone());
            if let Some(sink)=sink {
                ADMITTED.with(|v|v.borrow_mut().retain(|p|p!=&intent.proof));
                if let Ok(json)=serde_json::to_string(&intent){let _=sink.call1(&JsValue::NULL,&JsValue::from_str(&json));}
            }else{intents.0.push(intent);}
        }
        // A reentrant setter/clear must keep the old root/status withdrawn until
        // its new authoritative snapshot is ingested, never resurrect old raw.
        let retired=MAILBOX.with(|m|m.borrow().pending())||REJECTED.with(|r|*r.borrow());
        let ready=!retired&&c.active&&loaded&&window&&measured.is_some()&&shaped&&!c.awaiting_draft&&c.proof(&e,c.highwater.max(1)).is_some();
        allowed.0.clear();
        if ready {for(action,n,t,button)in &buttons {
            let modal=c.feedback.is_some()||c.recipient_prompt.is_some()||c.gold_prompt.is_some();
            let permitted=if c.feedback.is_some(){action.0==ComposeAction::Feedback}
                else if c.recipient_prompt.is_some(){matches!(action.0,ComposeAction::RecipientSubmit|ComposeAction::RecipientCancel)}
                else if c.gold_prompt.is_some(){matches!(action.0,ComposeAction::GoldConfirm|ComposeAction::GoldCancel)}else{!modal};
            if permitted&&!button.is_some_and(|b|!b.enabled){if let Some(r)=rect(n,t){allowed.0.push((action.0,mir2_client_bevy::crystal_ui::spec::CrystalRect::new(r.left,r.top,r.width,r.height)));}}
        }}
        c.ready=ready;
        if ready{ADMITTED.with(|v|v.borrow_mut().retain(|p|p.owner.run!=c.owner.run||p.sequence>c.highwater));}
        for(_,_,_,mut visibility)in &mut roots{*visibility=if ready{Visibility::Inherited}else{Visibility::Hidden};}
        let proof=ready.then(||c.proof(&e,c.highwater.max(1))).flatten();
        let text_target=if !ready||c.feedback.is_some(){None}else if c.gold_prompt.is_some(){Some("gold".to_owned())}else if c.recipient_prompt.is_some(){Some("recipient".to_owned())}else{Some("body".to_owned())};
        let prompt_text=if text_target.as_deref()==Some("gold"){c.gold_prompt.as_ref().map(|p|p.draft.clone())}else if text_target.as_deref()==Some("recipient"){c.recipient_prompt.clone()}else{None};
        let prompt_viewport=if ready{prompts.iter().find(|(p,_,_)|Some(p.0)==text_target.as_deref()).and_then(|(_,n,t)|rect(n,t))}else{None};
        let body_viewport=if ready{viewports.single().ok().and_then(|(n,t)|rect(n,t)).map(|r|Rect{left:r.left+2.,top:r.top+2.,width:r.width-4.,height:r.height-4.})}else{None};
        let caret=if ready&&text_target.as_deref()==Some("body"){carets.single().ok().and_then(|(n,t)|rect(n,t)).and_then(|r|{
            let clip=body_viewport.as_ref()?;
            if r.left<clip.left||r.top<clip.top||r.left+r.width>clip.left+clip.width||r.top+r.height>clip.top+clip.height{return None;}
            let editor=e.active_editor()?;let selection=editor.selection();let anchor=if editor.caret()==selection.start{selection.end}else{selection.start};
            Some(Caret{left:r.left,top:r.top,width:r.width,height:r.height,
                anchor_utf16:mir2_client_bevy::crystal_ui::mail_compose_shared::byte_to_utf16(editor.text(),anchor)?,
                caret_utf16:mir2_client_bevy::crystal_ui::mail_compose_shared::byte_to_utf16(editor.text(),editor.caret())?,
                editor_revision:e.revision(),layout_revision:e.layout_revision()})
        })}else{None};
        let mut input_regions=if ready{measured.into_iter().collect::<Vec<_>>()}else{vec![]};
        if ready{input_regions.extend(prompt_frames.iter().filter_map(|(n,t)|rect(n,t)));}
        STATUS.with(|v|*v.borrow_mut()=ComposeStatus{version:1,owner:c.owner,ready,input_enabled:ready&&c.input_enabled,
            modal:c.active,proof,text_target,prompt_text,prompt_viewport,body_viewport,input_regions,caret,scroll:e.scroll_offset().to_array(),error:None});
    }
    fn withdraw() {MAILBOX.with(|v|v.borrow_mut().withdraw());EDGES.with(|v|v.borrow_mut().clear());ACTIONS.with(|v|v.borrow_mut().clear());POINTERS.with(|v|v.borrow_mut().clear());ADMITTED.with(|v|v.borrow_mut().clear());
        REJECTED.with(|v|*v.borrow_mut()=true);STATUS.with(|v|{let mut v=v.borrow_mut();v.ready=false;v.input_enabled=false;v.proof=None;v.input_regions.clear();v.caret=None;v.text_target=None;v.prompt_text=None;v.prompt_viewport=None;v.body_viewport=None;v.scroll=[0.,0.];});}
    fn current(proof:&TextProof)->bool {proof.valid()&&STATUS.with(|v|{let v=v.borrow();v.ready&&v.input_enabled
        &&v.proof.as_ref().is_some_and(|p|p.same_capture(proof)&&proof.sequence>p.sequence)})}
    fn admit(proof:&TextProof)->bool {ADMITTED.with(|v|{let mut v=v.borrow_mut();
        if v.len()>=64||v.iter().any(|p|p.owner.run==proof.owner.run&&p.sequence==proof.sequence){return false;}
        v.push(proof.clone());true})}
    #[wasm_bindgen(js_name=setMir2MailComposeUiSnapshot)]pub fn snapshot(json:String)->bool {
        let result=ComposeUiSnapshot::parse(&json).ok().is_some_and(|s|MAILBOX.with(|v|v.borrow_mut().accept(s)));
        if result{ADMITTED.with(|v|v.borrow_mut().clear());EDGES.with(|v|v.borrow_mut().clear());ACTIONS.with(|v|v.borrow_mut().clear());POINTERS.with(|v|v.borrow_mut().clear());STATUS.with(|v|{let mut v=v.borrow_mut();v.ready=false;v.input_enabled=false;v.proof=None;});}else{withdraw();}result
    }
    #[wasm_bindgen(js_name=getMir2MailComposeUiStatus)]pub fn status()->String {STATUS.with(|v|serde_json::to_string(&*v.borrow()).unwrap_or_default())}
    #[wasm_bindgen(js_name=setMir2MailTextEdge)]pub fn text(json:String)->bool {
        if json.len()>32768{return false;}let Some(edge)=mir2_client_bevy::crystal_ui::mail_compose_shared::parse_text_edge(&json)else{return false;};if !current(&edge.proof){return false;}
        EDGES.with(|v|{let mut v=v.borrow_mut();if v.len()>=64||!admit(&edge.proof){return false;}v.push(edge);true})
    }
    #[wasm_bindgen(js_name=setMir2MailComposeUiActionEdge)]pub fn action(json:String)->bool {
        if json.len()>32768{return false;}let Ok(edge)=serde_json::from_str::<ComposeUiActionEdge>(&json)else{return false;};if !current(&edge.proof){return false;}
        if matches!(edge.action,ComposeAction::Slot(slot)if slot>=5){return false;}
        ACTIONS.with(|v|{let mut v=v.borrow_mut();if v.len()>=64||!admit(&edge.proof){return false;}v.push(edge);true})
    }
    #[wasm_bindgen(js_name=setMir2MailComposeUiPointerEdge)]pub fn pointer(json:String)->bool {
        if json.len()>32768{return false;}let Ok(edge)=serde_json::from_str::<ComposePointerEdge>(&json)else{return false;};
        if !current(&edge.proof)||edge.pointer_id>SAFE||edge.button!=0||!edge.x.is_finite()||!edge.y.is_finite()
            ||!matches!(edge.phase.as_str(),"down"|"move"|"up"|"cancel"){return false;}
        POINTERS.with(|v|{let mut v=v.borrow_mut();if v.len()>=64||!admit(&edge.proof){return false;}v.push(edge);true})
    }
    #[wasm_bindgen(js_name=setMir2MailComposeUiIntentSink)]pub fn sink(sink:Function){SINK.with(|v|*v.borrow_mut()=Some(sink));}
    #[wasm_bindgen(js_name=clearMir2MailComposeUiIntentSink)]pub fn clear(){SINK.with(|v|*v.borrow_mut()=None);withdraw();}
}
