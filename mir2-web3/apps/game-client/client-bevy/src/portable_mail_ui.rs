//! Portable mailbox interaction; both renderers call the common Native painter.
use bevy::prelude::*;
use bevy::ui::UiTargetCamera;
use serde::{Deserialize,Serialize};
use crate::{mail::{MailModel,MailMessage,mail_claim_enabled,mail_delete_enabled},portable_bag_ui::BagUiPresentation,
    crystal_ui::{mail_page_shared::{self,MailPaintAction,MailReaderKind},hud::SharedHudSurface,spec::CrystalRect}};
pub const SAFE:u64=9_007_199_254_740_991;

/// Additive composer ownership; the legacy list/reader DTO is unchanged.
#[derive(Resource, Default)]
pub struct PortableComposeSurface {
    pub owner:crate::crystal_ui::mail_compose_shared::ComposeIdentity, pub revision:u64, pub incarnation:u64, pub draft_epoch:u64,
    pub draft_generation:String, pub presentation_revision:u64, pub layout_revision:u64,
    pub active:bool, pub ready:bool, pub input_enabled:bool, pub awaiting_draft:bool,
    pub kind:Option<crate::crystal_ui::mail_compose_shared::ComposeKind>,
    pub layout:Option<crate::crystal_ui::mail_compose_shared::ComposeLayout>,
    pub presentation:Option<crate::crystal_ui::mail_compose_shared::ComposePresentation>,
    pub raw:Option<crate::crystal_ui::mail_compose_shared::ComposeFullRaw>,
    pub draft:mir2_ui_core::state::MailComposeDraft,
    pub parcel:crate::crystal_ui::mail_compose_shared::ParcelPaint,
    pub notice:Option<String>, pub recipient_prompt:Option<String>, pub feedback:Option<String>,
    pub gold_prompt:Option<crate::crystal_ui::mail_compose_shared::GoldPaint>,
    pub highwater:u64,
}
impl PortableComposeSurface {
    pub fn proof(&self, editor:&crate::crystal_ui::mail_editor::MailLetterEditor, sequence:u64)
        -> Option<crate::crystal_ui::mail_compose_shared::TextProof> {
        use crate::crystal_ui::mail_compose_shared::TextProof;
        let proof=TextProof{owner:self.owner,incarnation:self.incarnation,draft_epoch:self.draft_epoch,
            draft_generation:self.draft_generation.clone(),editor_revision:editor.revision(),
            focus_generation:editor.focus_generation(),presentation_revision:self.presentation_revision,
            layout_revision:self.layout_revision,sequence};
        (self.active&&self.raw.is_some()&&proof.valid()).then_some(proof)
    }
    pub fn withdraw(&mut self) {self.active=false;self.ready=false;self.awaiting_draft=false;}
}
#[derive(Component, Clone)]
pub struct PortableComposeRoot {pub owner:crate::crystal_ui::mail_compose_shared::ComposeIdentity,pub revision:u64,pub layout_revision:u64,
    pub editor_revision:u64,pub paint_revision:u64,pub font:Handle<Font>}
impl PortableComposeRoot {
    pub fn matches(&self,c:&PortableComposeSurface,e:&crate::crystal_ui::mail_editor::MailLetterEditor)->bool {
        self.owner==c.owner&&self.revision==c.revision&&self.layout_revision==c.layout_revision
            &&self.editor_revision==e.revision()&&self.paint_revision==e.paint_revision()
    }
}
#[derive(Component,Clone,Copy)] pub struct PortableComposeAction(pub crate::crystal_ui::mail_compose_shared::ComposeAction);
#[derive(Resource,Default)] pub struct PortableComposeEdges(pub Vec<crate::crystal_ui::mail_compose_shared::TextEdge>);
#[derive(Resource,Default)] pub struct PortableComposeIntents(pub Vec<crate::crystal_ui::mail_compose_shared::TextIntent>);
pub struct Mir2PortableMailComposeUiPlugin;
impl Plugin for Mir2PortableMailComposeUiPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<PortableComposeSurface>()
            .init_resource::<crate::crystal_ui::mail_editor::MailLetterEditor>()
            .init_resource::<crate::crystal_ui::mail_compose_shared::CommonMailTextAdapter>()
            .init_resource::<PortableComposeEdges>().init_resource::<PortableComposeIntents>()
            .add_systems(Update,(process_compose,paint_compose).chain().after(crate::pending_operations::PendingLifecycleSet::Ingest))
            .add_systems(PostUpdate,crate::crystal_ui::mail_editor::capture_layout_system
                .in_set(crate::crystal_ui::mail_editor::MailEditorLayoutSet).after(bevy::ui::UiSystems::PostLayout));
    }
}
pub fn process_compose(mut c:ResMut<PortableComposeSurface>,mut editor:ResMut<crate::crystal_ui::mail_editor::MailLetterEditor>,
    mut adapter:ResMut<crate::crystal_ui::mail_compose_shared::CommonMailTextAdapter>,
    mut edges:ResMut<PortableComposeEdges>,mut intents:ResMut<PortableComposeIntents>) {
    if !c.active {edges.0.clear();intents.0.clear();adapter.retire();editor.clear();return;}
    for edge in std::mem::take(&mut edges.0) {
        if let crate::crystal_ui::mail_compose_shared::TextOperation::PromptReplace{target,text}=&edge.operation {
            let Some(current)=c.proof(&editor,edge.proof.sequence)else{continue;};
            let active=(target=="recipient"&&c.recipient_prompt.is_some()&&text.len()<=1024)||(target=="gold"&&c.gold_prompt.is_some()&&text.len()<=256);
            if c.ready&&c.input_enabled&&current.same_capture(&edge.proof)&&edge.proof.sequence>c.highwater {
                c.highwater=edge.proof.sequence;
                if !active{continue;}
                let normalized=if target=="gold" {
                    c.gold_prompt.as_ref().map(|g|crate::crystal_ui::mail_compose_shared::replace_gold_prompt(g.max_amount,text))
                }else{None};
                let prompt_text=normalized.as_ref().map(|g|g.draft.clone()).unwrap_or_else(||text.clone());
                let prompt_value=normalized.as_ref().and_then(|g|g.amount);
                if let Some(raw)=c.raw.clone(){intents.0.push(crate::crystal_ui::mail_compose_shared::TextIntent{proof:edge.proof.clone(),action:"promptEdit".into(),base_raw:raw,
                    body_mutation:None,recipient_mutation:None,prompt_mutation:Some(crate::crystal_ui::mail_compose_shared::PromptMutation{target:target.clone(),text:prompt_text}),
                    prompt_value,request_id:None,clipboard_kind:None,selected_text:None,error:None});}
            }continue;
        }
        if !c.ready||!c.input_enabled||c.awaiting_draft||c.recipient_prompt.is_some()||c.feedback.is_some()||c.gold_prompt.is_some()||edge.proof.sequence<=c.highwater{continue;}
        let Some(current)=c.proof(&editor,edge.proof.sequence)else{continue;};
        let Some(raw)=c.raw.clone()else{continue;};
        let applied=adapter.apply(&current,edge,&mut editor,&mut c.draft,&raw);
        c.highwater=c.highwater.max(adapter.highwater());
        if let Some(intent)=applied {
            c.highwater=c.highwater.max(intent.proof.sequence);
            if intent.body_mutation.is_some()||intent.recipient_mutation.is_some(){c.awaiting_draft=true;c.ready=false;}
            intents.0.push(intent);
        }
    }
}
pub fn paint_compose(mut commands:Commands,c:Res<PortableComposeSurface>,editor:Res<crate::crystal_ui::mail_editor::MailLetterEditor>,
    surface:Res<SharedHudSurface>,server:Res<AssetServer>,roots:Query<(Entity,&PortableComposeRoot)>) {
    if c.active&&roots.iter().any(|(_,r)|r.matches(&c,&editor)&&r.font==surface.font){return;}
    for(e,_)in &roots{commands.entity(e).despawn();}
    if !c.active{return;}
    let(Some(kind),Some(layout))=(c.kind,c.layout.as_ref())else{return;};
    let font=TextFont{font:surface.font.clone().into(),..default()};
    let stamp=PortableComposeRoot{owner:c.owner,revision:c.revision,layout_revision:c.layout_revision,
        editor_revision:editor.revision(),paint_revision:editor.paint_revision(),font:surface.font.clone()};
    commands.spawn((stamp,UiTargetCamera(surface.camera),GlobalZIndex(1102),Visibility::Hidden,
        bevy::ui::LayoutConfig{use_rounding:false},Node{position_type:PositionType::Absolute,
            left:Val::Px(layout.root.left),top:Val::Px(layout.root.top),width:Val::Px(layout.root.width),height:Val::Px(layout.root.height),..default()}))
        .with_children(|p| {
            crate::crystal_ui::mail_compose_shared::paint_compose(p,Some(&server),kind,&c.draft,&c.parcel,
                c.notice.as_deref(),Some(&editor),layout,&font,PortableComposeAction);
            if let Some(recipient)=c.recipient_prompt.as_deref() {
                crate::crystal_ui::mail_compose_shared::paint_recipient(p,Some(&server),recipient,
                    if layout.compact{CrystalRect::new(0.,0.,layout.root.width,layout.root.height)}else{CrystalRect::new(368.-layout.root.left,306.-layout.root.top,288.,156.)},&font,PortableComposeAction);
            }
            if let Some(gold)=c.gold_prompt.as_ref() {
                crate::crystal_ui::mail_compose_shared::paint_gold(p,Some(&server),gold,
                    if layout.compact{CrystalRect::new(0.,0.,layout.root.width,layout.root.height)}else{CrystalRect::new(410.-layout.root.left,329.-layout.root.top,204.,109.)},
                    &font,(),PortableComposeAction);
            }
            if let Some(message)=c.feedback.as_deref() {
                crate::crystal_ui::mail_compose_shared::paint_feedback(p,Some(&server),message,
                    if layout.compact{CrystalRect::new(0.,0.,layout.root.width,layout.root.height)}else{CrystalRect::new(284.-layout.root.left,289.-layout.root.top,456.,190.)},&font,(),PortableComposeAction(crate::crystal_ui::mail_compose_shared::ComposeAction::Feedback));
            }
        });
}
/// Resolve one current carrier. Canonical keys are identity, display names are not.
/// Index-only ClientMail is authoritative; ItemState canonical key/index must agree.
pub fn resolve_mail_attachment(item:&mut crate::mail::MailAttachment,identified:Option<bool>)->Result<(),&'static str>{
    let canonical=match item.key.as_deref(){
        Some(key) if key.starts_with("crystal-item-")=>{
            let digits=key.strip_prefix("crystal-item-").unwrap();
            let index=digits.parse::<i32>().map_err(|_|"Mail canonical item key")?;
            if key!=format!("crystal-item-{index}"){return Err("Mail canonical item key");}Some(index)
        },
        Some(_) if item.unique_id.is_some()=>return Err("Mail unknown current item key"),
        _=>None,
    };
    if canonical.is_some()&&item.item_index.is_some()&&canonical!=item.item_index{return Err("Mail contradictory item template");}
    let index=canonical.or(item.item_index);
    let template=if let Some(index)=index {Some(mir2_game_data::crystal_item_by_index(index).ok_or("Mail unknown current item index")?)}
        else if item.unique_id.is_some(){return Err("Mail missing current item template");}
        else {item.name.as_deref().and_then(mir2_game_data::crystal_item_by_name)};
    if let Some(template)=template {item.item_index=Some(template.item_index);item.image=Some(u32::from(template.image));item.identified=identified.unwrap_or(!template.need_identify);}
    else {item.image=None;item.identified=identified.unwrap_or(false);}
    Ok(())
}
#[derive(Debug,Default,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MailIdentity {pub run:u64,pub connection_generation:u64,pub session_generation:u64,pub owner_revision:u64,
    pub scene_revision:u64,pub hud_generation:u64,pub player_object_id:u32}
impl MailIdentity {pub fn valid(self)->bool {self.player_object_id>0&&[self.run,self.connection_generation,self.session_generation,self.scene_revision,self.hud_generation].into_iter().all(|n|n>0&&n<=SAFE)&&self.owner_revision<=SAFE}}
#[derive(Resource,Default,Clone)]
pub struct MailUiContext {pub identity:MailIdentity,pub revision:u64,pub model_revision:u64,pub presentation_revision:u64,
    pub active:bool,pub ready:bool,pub input_enabled:bool,pub presentation:Option<BagUiPresentation>}
#[derive(Component,Clone,Copy)]
pub struct MailRoot {pub identity:MailIdentity,pub revision:u64,pub model_revision:u64,pub presentation_revision:u64,pub render_revision:u64,pub reader:bool}
impl MailRoot {pub fn matches(&self,c:&MailUiContext,s:&MailUiState)->bool {self.identity==c.identity&&self.revision==c.revision&&self.model_revision==c.model_revision&&self.presentation_revision==c.presentation_revision&&self.render_revision==s.render_revision}}
#[derive(Component,Clone,Copy)] pub struct PortableMailAction(pub MailPaintAction);
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct MailPointerEdge {#[serde(flatten)]pub identity:MailIdentity,pub sequence:u64,pub model_revision:u64,pub presentation_revision:u64,
    pub render_revision:u64,pub pointer_id:u64,pub phase:String,pub x:f32,pub y:f32,pub button:u8}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct MailIntent {#[serde(flatten)]pub identity:MailIdentity,pub sequence:u64,pub model_revision:u64,pub presentation_revision:u64,
    pub render_revision:u64,pub action:String,pub mail_id:Option<u64>,pub lock:Option<bool>}
#[derive(Resource,Default)] pub struct MailPointerQueue(pub Vec<MailPointerEdge>);
#[derive(Resource,Default)] pub struct MailIntentQueue(pub Vec<MailIntent>);
#[derive(Clone)] struct Held {pointer:u64,action:MailPaintAction,model:u64,presentation:u64,render:u64}
#[derive(Resource,Default)]
pub struct MailUiState {pub model:MailModel,pub page:usize,pub reader:Option<MailMessage>,pub render_revision:u64,
    pub controls:Vec<(MailPaintAction,CrystalRect)>,identity:MailIdentity,highwater:u64,held:Option<Held>,paint_key:Option<(MailModel,usize,Option<MailMessage>)>}
impl MailUiState {
    pub fn withdraw(&mut self){self.held=None;self.controls.clear();}
    pub fn close(&mut self){self.withdraw();self.reader=None;self.model.selected_id=None;self.page=0;}
    pub fn ingest(&mut self,c:&MailUiContext,mut model:MailModel){
        if self.identity!=c.identity {self.close();if self.identity.run!=c.identity.run {self.highwater=0;}self.identity=c.identity;}
        self.withdraw();
        self.reader=self.reader.as_ref().and_then(|old|model.mails.iter().find(|m|same_reader(old,m))).cloned();
        model.selected_id=self.model.selected().and_then(|old|model.mails.iter().find(|m|same_reader(old,m))).map(|m|m.id);
        self.model=model;self.page=self.model.page(self.page).page;
        self.update_render();
    }
    pub fn update_render(&mut self){
        // All model fields participate, including metadata that can change a held action.
        let unchanged=self.paint_key.as_ref().is_some_and(|(model,page,reader)|model==&self.model&&*page==self.page&&reader==&self.reader);
        if !unchanged {self.paint_key=Some((self.model.clone(),self.page,self.reader.clone()));
            self.render_revision=self.render_revision.saturating_add(1).min(SAFE);self.withdraw();}}
    pub fn allowed(&self,a:MailPaintAction)->bool {match a {
        MailPaintAction::Prev=>self.page>0,MailPaintAction::Next=>self.page+1<self.model.page(self.page).page_count,
        MailPaintAction::Select(id)=>self.model.mails.iter().any(|m|m.id==id),
        MailPaintAction::Read(id)=>self.model.mails.iter().any(|m|m.id==id),
        MailPaintAction::Reply(id)=>self.model.mails.iter().any(|m|m.id==id&&m.can_reply),
        MailPaintAction::Delete(id)=>self.model.mails.iter().any(|m|m.id==id&&mail_delete_enabled(m)),
        MailPaintAction::ReaderDelete=>self.reader.as_ref().is_some_and(|m|mail_delete_enabled(m)),
        MailPaintAction::ReaderClaim=>self.reader.as_ref().is_some_and(|m|mail_claim_enabled(m)),
        MailPaintAction::ReaderLock=>self.reader.is_some(),_=>true}}
    pub fn process(&mut self,c:&MailUiContext,e:MailPointerEdge,out:&mut MailIntentQueue){
        if e.identity!=c.identity||e.sequence<=self.highwater||e.sequence>SAFE{return;}self.highwater=e.sequence;
        if e.phase=="cancel" {self.withdraw();return;}
        if !c.active||!c.ready||!c.input_enabled||e.model_revision!=c.model_revision||e.presentation_revision!=c.presentation_revision||e.render_revision!=self.render_revision {self.withdraw();return;}
        let hit=self.controls.iter().rev().find(|(a,r)|self.allowed(*a)&&e.x>=r.left&&e.y>=r.top&&e.x<r.left+r.width&&e.y<r.top+r.height).map(|(a,_)|*a);
        match e.phase.as_str(){
            "down"=>{if self.held.is_some(){self.withdraw();return;}if let Some(action)=hit{self.held=Some(Held{pointer:e.pointer_id,action,model:c.model_revision,presentation:c.presentation_revision,render:self.render_revision});}},
            "move"=>{if self.held.as_ref().is_some_and(|h|h.pointer!=e.pointer_id||Some(h.action)!=hit){self.withdraw();}},
            "up"=>{let Some(h)=self.held.take()else{return;};if h.pointer!=e.pointer_id||Some(h.action)!=hit||h.model!=c.model_revision||h.presentation!=c.presentation_revision||h.render!=self.render_revision{return;}
                let mut action=None;let mut id=None;let mut lock=None;
                match h.action {
                    MailPaintAction::Prev=>{self.page=self.page.saturating_sub(1);self.model.selected_id=None;},MailPaintAction::Next=>{self.page+=1;self.model.selected_id=None;},
                    MailPaintAction::Select(i)=>{if self.model.selected_id==Some(i){if let Some(m)=self.model.mails.iter().find(|m|m.id==i){self.reader=Some(m.clone());id=Some(i);if !m.read{action=Some("read");}}}else{self.model.selected_id=Some(i);}},
                    MailPaintAction::Read(i)=>{if let Some(m)=self.model.mails.iter().find(|m|m.id==i){self.reader=Some(m.clone());id=Some(i);if !m.read{action=Some("read");}}},
                    MailPaintAction::Reply(i)=>{id=Some(i);action=Some("reply");},MailPaintAction::Delete(i)=>{id=Some(i);action=Some("delete");},
                    MailPaintAction::ReaderClose=>self.reader=None,
                    MailPaintAction::ReaderDelete=>{id=self.reader.as_ref().map(|m|m.id);action=Some("delete");},
                    MailPaintAction::ReaderClaim=>{id=self.reader.as_ref().map(|m|m.id);action=Some("claim");},
                    MailPaintAction::ReaderLock=>{if let Some(m)=self.reader.as_ref().filter(|m|!m.has_attachment()){id=Some(m.id);lock=Some(!m.locked);action=Some("lock");}},
                    MailPaintAction::Compose=>action=Some("compose"),MailPaintAction::Close=>action=Some("close"),
                }
                // The emitted proof names the rendered input frame, before local selection rebuild.
                if let Some(action)=action{out.0.push(MailIntent{identity:c.identity,sequence:e.sequence,model_revision:c.model_revision,
                    presentation_revision:c.presentation_revision,render_revision:e.render_revision,action:action.into(),mail_id:id,lock});}
                self.update_render();
            },_=>self.withdraw(),
        }
    }
}
pub fn same_reader(a:&MailMessage,b:&MailMessage)->bool {a.id==b.id&&a.sender==b.sender&&a.body==b.body&&a.gold==b.gold&&a.items.len()==b.items.len()&&a.items.iter().zip(&b.items).all(|(x,y)|{
    x.unique_id==y.unique_id&&(x.unique_id.is_some()||x.name==y.name&&x.key==y.key)&&x.item_index==y.item_index
    &&x.count==y.count&&x.current_dura==y.current_dura&&x.max_dura==y.max_dura&&x.soul_bound_id==y.soul_bound_id&&x.gem_count==y.gem_count&&x.identified==y.identified&&x.cursed==y.cursed
})&&a.has_attachment()==b.has_attachment()}
pub struct Mir2PortableMailUiPlugin;
impl Plugin for Mir2PortableMailUiPlugin {fn build(&self,app:&mut App){app.init_resource::<MailUiContext>().init_resource::<MailUiState>()
    .init_resource::<MailPointerQueue>().init_resource::<MailIntentQueue>().add_plugins(Mir2PortableMailComposeUiPlugin)
    .add_systems(Update,(process,paint).chain().after(crate::pending_operations::PendingLifecycleSet::Ingest))
    .add_systems(PostUpdate,mail_page_shared::layout_mail_row_icons.before(bevy::ui::UiSystems::Layout));}}
fn process(c:Res<MailUiContext>,mut s:ResMut<MailUiState>,mut q:ResMut<MailPointerQueue>,mut out:ResMut<MailIntentQueue>,compose:Option<Res<PortableComposeSurface>>){
    if !c.active||compose.as_deref().is_some_and(|v|v.active){s.close();q.0.clear();out.0.clear();return;}for e in std::mem::take(&mut q.0){s.process(&c,e,&mut out);}}
fn paint(mut commands:Commands,c:Res<MailUiContext>,s:Res<MailUiState>,surface:Res<SharedHudSurface>,server:Res<AssetServer>,roots:Query<(Entity,&MailRoot)>,compose:Option<Res<PortableComposeSurface>>){
    if compose.as_deref().is_some_and(|v|v.active){for(e,_)in &roots{commands.entity(e).despawn();}return;}
    if c.active&&roots.iter().any(|(_,r)|r.matches(&c,&s)){return;}
    for (e,_)in &roots{commands.entity(e).despawn();}if !c.active{return;}
    let font=TextFont{font:surface.font.clone().into(),..default()};
    let stamp=|reader|MailRoot{identity:c.identity,revision:c.revision,model_revision:c.model_revision,presentation_revision:c.presentation_revision,render_revision:s.render_revision,reader};
    commands.spawn((stamp(false),UiTargetCamera(surface.camera),GlobalZIndex(1100),Visibility::Hidden,Node{position_type:PositionType::Absolute,left:Val::Px(712.),top:Val::Px(70.),width:Val::Px(312.),height:Val::Px(444.),..default()}))
        .with_children(|p|mail_page_shared::paint_list(p,Some(&server),&s.model,s.page,Some(&font),PortableMailAction));
    if let Some(m)=&s.reader{let kind=if m.has_attachment(){MailReaderKind::Parcel}else{MailReaderKind::Letter};let height=if m.has_attachment(){384.}else{300.};
        commands.spawn((stamp(true),UiTargetCamera(surface.camera),GlobalZIndex(1101),Visibility::Hidden,Node{position_type:PositionType::Absolute,left:Val::Px(464.),top:Val::Px(90.),width:Val::Px(236.),height:Val::Px(height),..default()}))
            .with_children(|p|mail_page_shared::paint_reader(p,Some(&server),m,kind,Some(&font),PortableMailAction));}
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn cache_fixture()->(MailUiContext,MailUiState) {
        let identity=MailIdentity{run:1,connection_generation:2,session_generation:3,owner_revision:0,scene_revision:4,hud_generation:5,player_object_id:6};
        let context=MailUiContext{identity,revision:1,model_revision:1,presentation_revision:1,active:true,ready:true,input_enabled:true,presentation:None};
        let attachment=crate::mail::MailAttachment{image:Some(71),unique_id:Some(77),item_index:Some(1),key:Some("crystal-item-1".into()),
            name:Some("本地化\\r\\nitem".into()),count:2,current_dura:3,max_dura:4,soul_bound_id:5,identified:true,cursed:false,gem_count:6};
        let message=MailMessage{id:71,sender:"Sender".into(),can_reply:true,date_sent_binary_datetime:638962902000000001,metadata_known:true,
            subject:"Subject".into(),body:"本地化\r\n\\r\\n正文".into(),gold:100,items:vec![attachment],..default()};
        let model=MailModel{mails:vec![message],selected_id:Some(71)};
        let mut state=MailUiState::default();state.ingest(&context,model);
        state.model.selected_id=Some(71);state.update_render();(context,state)
    }
    fn cache_edge(context:&MailUiContext,revision:u64,sequence:u64,phase:&str)->MailPointerEdge {
        MailPointerEdge{identity:context.identity,sequence,model_revision:context.model_revision,presentation_revision:context.presentation_revision,
            render_revision:revision,pointer_id:1,phase:phase.into(),x:5.,y:5.,button:0}
    }
    fn hold_read(context:&MailUiContext,state:&mut MailUiState,out:&mut MailIntentQueue) {
        state.controls=vec![(MailPaintAction::Read(71),CrystalRect::new(0.,0.,20.,20.))];
        state.process(context,cache_edge(context,state.render_revision,1,"down"),out);
        assert!(state.held.is_some());assert!(out.0.is_empty());
    }
    #[test]
    fn mail_render_cache_first_update_and_unchanged_updates_preserve_current_hold() {
        let mut empty=MailUiState::default();empty.update_render();assert_eq!(empty.render_revision,1);
        empty.update_render();assert_eq!(empty.render_revision,1);
        let(context,mut state)=cache_fixture();let revision=state.render_revision;let mut out=MailIntentQueue::default();
        hold_read(&context,&mut state,&mut out);
        let before=serde_json::to_string(&(&state.model,state.page,&state.reader)).unwrap();
        state.model=state.model.clone();state.reader=state.reader.clone();
        assert_eq!(before,serde_json::to_string(&(&state.model,state.page,&state.reader)).unwrap());
        for _ in 0..3 {state.update_render();assert_eq!(state.render_revision,revision);assert!(state.held.is_some());assert_eq!(state.controls.len(),1);}
        state.process(&context,cache_edge(&context,revision,2,"up"),&mut out);
        assert_eq!(out.0.len(),1);assert_eq!(out.0[0].action,"read");assert_eq!(out.0[0].mail_id,Some(71));
        assert_eq!(out.0[0].render_revision,revision);assert!(!state.model.mails[0].read);assert!(!state.reader.as_ref().unwrap().read);
        assert_eq!(state.render_revision,revision+1);assert!(state.controls.is_empty());assert!(state.held.is_none());
    }
    #[test]
    fn mail_render_cache_all_serialized_fields_match_old_key_and_retire_held_input() {
        // The former serializer is an oracle only in tests, never a release cache.
        let changes: &[(&str,fn(&mut MailUiState))]=&[
            ("mail count",|s|s.model.mails.push(MailMessage{id:72,..default()})),
            ("selected id",|s|s.model.selected_id=None),
            ("page",|s|s.page+=1),
            ("reader entry",|s|s.reader=Some(s.model.mails[0].clone())),
            ("mail id",|s|s.model.mails[0].id=72),
            ("sender",|s|s.model.mails[0].sender.push('2')),
            ("reply policy",|s|s.model.mails[0].can_reply=false),
            ("date",|s|s.model.mails[0].date_sent_binary_datetime+=1),
            ("metadata",|s|s.model.mails[0].metadata_known=false),
            ("subject",|s|s.model.mails[0].subject.push('2')),
            ("body",|s|s.model.mails[0].body.push('2')),
            ("gold",|s|s.model.mails[0].gold+=1),
            ("attachment count",|s|s.model.mails[0].items.push(crate::mail::MailAttachment::default())),
            ("operation",|s|s.model.mails[0].operation=Some(crate::mail::MailOperationFeedback{kind:crate::mail::MailOperationKind::Read,success:true,mail_id:Some(71)})),
            ("claimed",|s|s.model.mails[0].claimed=true),
            ("locked",|s|s.model.mails[0].locked=true),
            ("read",|s|s.model.mails[0].read=true),
            ("image",|s|s.model.mails[0].items[0].image=Some(72)),
            ("unique id",|s|s.model.mails[0].items[0].unique_id=Some(78)),
            ("item index",|s|s.model.mails[0].items[0].item_index=Some(2)),
            ("key",|s|s.model.mails[0].items[0].key=Some("crystal-item-2".into())),
            ("display name",|s|s.model.mails[0].items[0].name=Some("当前标签".into())),
            ("stack count",|s|s.model.mails[0].items[0].count+=1),
            ("current dura",|s|s.model.mails[0].items[0].current_dura+=1),
            ("max dura",|s|s.model.mails[0].items[0].max_dura+=1),
            ("soul bound",|s|s.model.mails[0].items[0].soul_bound_id+=1),
            ("identified",|s|s.model.mails[0].items[0].identified=false),
            ("cursed",|s|s.model.mails[0].items[0].cursed=true),
            ("gem count",|s|s.model.mails[0].items[0].gem_count+=1),
        ];
        for (name,change) in changes {
            let(context,mut state)=cache_fixture();let revision=state.render_revision;let mut out=MailIntentQueue::default();
            let before=serde_json::to_string(&(&state.model,state.page,&state.reader)).unwrap();hold_read(&context,&mut state,&mut out);
            state.update_render();assert_eq!(state.render_revision,revision,"unchanged {name}");assert!(state.held.is_some());
            change(&mut state);
            let after=serde_json::to_string(&(&state.model,state.page,&state.reader)).unwrap();assert_ne!(before,after,"oracle {name}");
            state.update_render();assert_eq!(state.render_revision,revision+1,"changed {name}");assert!(state.controls.is_empty());assert!(state.held.is_none());
            state.process(&context,cache_edge(&context,revision,2,"up"),&mut out);assert!(out.0.is_empty(),"retired hold {name}");
            state.update_render();assert_eq!(state.render_revision,revision+1,"stable changed {name}");
        }
    }
    #[test]
    fn mail_render_cache_reader_changes_close_and_saturation_still_withdraw() {
        let(context,mut state)=cache_fixture();state.reader=Some(state.model.mails[0].clone());state.update_render();
        let revision=state.render_revision;let mut out=MailIntentQueue::default();hold_read(&context,&mut state,&mut out);
        let before=serde_json::to_string(&(&state.model,state.page,&state.reader)).unwrap();
        state.reader.as_mut().unwrap().date_sent_binary_datetime+=1;
        assert_ne!(before,serde_json::to_string(&(&state.model,state.page,&state.reader)).unwrap());
        state.update_render();assert_eq!(state.render_revision,revision+1);assert!(state.held.is_none());assert!(state.controls.is_empty());
        state.close();assert!(state.reader.is_none());assert!(state.model.selected_id.is_none());assert_eq!(state.render_revision,revision+1);
        state.update_render();assert_eq!(state.render_revision,revision+2);state.update_render();assert_eq!(state.render_revision,revision+2);
        for ceiling in [SAFE,u64::MAX] {
            let(context,mut state)=cache_fixture();let mut out=MailIntentQueue::default();hold_read(&context,&mut state,&mut out);
            state.render_revision=ceiling;state.model.mails[0].subject.push('2');state.update_render();
            assert_eq!(state.render_revision,SAFE);assert!(state.controls.is_empty());assert!(state.held.is_none());
        }
    }
    #[test]
    fn portable_mail_adapter_calls_shared_painter_with_injected_font_and_hidden_roots() {
        let mut app=App::new();app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default())).init_asset::<Image>();
        let camera=app.world_mut().spawn_empty().id();let window=app.world_mut().spawn(Window::default()).id();
        app.insert_resource(SharedHudSurface{camera,window,font:Handle::default(),active:true,generation:5,revision:1});
        let identity=MailIdentity{run:1,connection_generation:2,session_generation:3,owner_revision:0,scene_revision:4,hud_generation:5,player_object_id:6};
        let c=MailUiContext{identity,revision:1,model_revision:1,presentation_revision:1,active:true,ready:false,input_enabled:false,presentation:None};
        let model=MailModel{mails:vec![MailMessage{id:71,sender:"Sender".into(),body:"本地化正文".into(),gold:100,..default()}],selected_id:Some(71)};
        let mut state=MailUiState::default();state.ingest(&c,model);state.reader=Some(state.model.mails[0].clone());state.update_render();
        app.insert_resource(c);app.insert_resource(state);app.world_mut().run_system_once(paint).unwrap();
        let w=app.world_mut();let roots=w.query::<(&MailRoot,&Node,&Visibility)>().iter(w).collect::<Vec<_>>();assert_eq!(roots.len(),2);
        assert!(roots.iter().all(|(_,_,v)|**v==Visibility::Hidden));assert!(roots.iter().any(|(r,n,_)|r.reader&&n.height==Val::Px(384.)));
        assert!(w.query::<&PortableMailAction>().iter(w).any(|a|a.0==MailPaintAction::ReaderClaim));
        assert!(w.query::<&Text>().iter(w).any(|t|t.0=="本地化正文"));
        let font=TextFont{font:Handle::<Font>::default().into(),..default()};
        assert!(w.query::<&TextFont>().iter(w).all(|f|format!("{:?}",f.font)==format!("{:?}",font.font)));
        w.resource_mut::<MailUiContext>().active=false;w.run_system_once(paint).unwrap();assert_eq!(w.query::<&MailRoot>().iter(w).count(),0);
    }
}
