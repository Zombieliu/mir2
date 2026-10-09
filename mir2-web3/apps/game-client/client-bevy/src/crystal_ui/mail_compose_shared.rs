//! Common compose presentation and pure text adapter. No sender, parcel ledger,
//! platform clipboard, socket, Window, or second text editor lives here.
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use serde::{Deserialize, Serialize};
use super::{assets::CrystalButtonAssetSet, mail_editor::MailLetterEditor,
    spec::{CrystalButtonSpec, CrystalRect}, widget::spawn_crystal_image_button,
    item_image::spawn_original_item_image};
// Independent additive ownership DTO; old portable M13 MailIdentity is untouched.
pub const SAFE:u64=9_007_199_254_740_991;
#[derive(Debug,Default,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ComposeIdentity {pub run:u64,pub connection_generation:u64,pub session_generation:u64,pub owner_revision:u64,
    pub scene_revision:u64,pub hud_generation:u64,pub player_object_id:u32}
impl ComposeIdentity {pub fn valid(self)->bool {self.player_object_id>0&&[self.run,self.connection_generation,self.session_generation,self.scene_revision,self.hud_generation].into_iter().all(|n|n>0&&n<=SAFE)&&self.owner_revision<=SAFE}}
use mir2_ui_core::state::MailComposeDraft;

pub const COMPOSE_UI_ABI: u8 = 1;
pub const TEXT_ADAPTER_ABI: u8 = 1;
pub const TEXT: Color = Color::srgb(0.95, 0.92, 0.82);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ComposeKind { Letter, Parcel }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Component)]
#[serde(rename_all = "camelCase")]
pub enum ComposeAction { Close, Submit, Cancel, Body, Recipient, Gold, Stamp, Slot(u8), RecipientSubmit, RecipientCancel, Feedback, GoldConfirm, GoldCancel }
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComposePresentation { pub logical_width: f32, pub logical_height: f32, pub stage_css_scale: f32, pub touch: bool }
#[derive(Debug, Clone, PartialEq)]
pub struct ComposeLayout {
    pub root: CrystalRect, pub body: CrystalRect, pub recipient: CrystalRect,
    pub notice: CrystalRect, pub close: CrystalRect, pub send: CrystalRect,
    pub cancel: CrystalRect, pub stamp: CrystalRect, pub postage: CrystalRect,
    pub gold: CrystalRect, pub slots: [CrystalRect; 5], pub body_font_px: f32,
    pub compact: bool,
}
impl ComposeLayout {
    /// Native source geometry remains exact. Adaptive touch is an independent
    /// contract and does not change M13's 1024x768 eligibility gate.
    pub fn desktop(kind: ComposeKind) -> Self {
        let parcel = kind == ComposeKind::Parcel;
        Self { root: CrystalRect::new(0., 0., 236., if parcel {384.} else {300.}),
            body: CrystalRect::new(15., if parcel {98.} else {92.}, 202., 165.),
            recipient: CrystalRect::new(70., 35., 150., 15.),
            notice: CrystalRect::new(15., if parcel {78.} else {72.}, 202., 15.),
            close: CrystalRect::new(209., 3., 24., 21.),
            send: CrystalRect::new(30., if parcel {350.} else {265.}, 76., 25.),
            cancel: CrystalRect::new(135., if parcel {350.} else {265.}, 68., 25.),
            stamp: CrystalRect::new(73., 56., 20., 20.),
            postage: CrystalRect::new(63., 269., 143., 15.),
            gold: CrystalRect::new(63., 290., 143., 15.),
            slots: std::array::from_fn(|i| CrystalRect::new(27. + 36. * i as f32, 311., 35., 31.)),
            body_font_px: super::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX, compact: false }
    }
    pub fn for_presentation(p: ComposePresentation, kind: ComposeKind) -> Option<Self> {
        if ![p.logical_width, p.logical_height, p.stage_css_scale].into_iter().all(f32::is_finite)
            || p.stage_css_scale <= 0. || p.stage_css_scale > 16.
            || p.logical_width > 16384. || p.logical_height > 16384. { return None; }
        if !p.touch {
            return (p.logical_width >= 1024. && p.logical_height >= 768.).then(|| {
                let mut layout=Self::desktop(kind);
                layout.root.left=if kind==ComposeKind::Parcel{326.}else{100.};
                layout.root.top=if kind==ComposeKind::Parcel{0.}else{100.};layout
            });
        }
        let w = p.logical_width * p.stage_css_scale;
        let h = p.logical_height * p.stage_css_scale;
        if w < 600. || h < 320. || w <= h { return None; }
        let px = |v: f32| v / p.stage_css_scale;
        let rw = w - 24.; let rh = h - 24.;
        // The right column holds five parcel cells; every action has a 44px
        // CSS hit target. The left body retains a useful keyboard-sized viewport.
        let rail = 168.; let gap = 12.; let bw = rw - rail - gap - 24.;
        let right = rw - rail - 12.;
        let r = |x, y, width, height| CrystalRect::new(px(x), px(y), px(width), px(height));
        let layout = Self { root: r(12., 12., rw, rh), body: r(12., 100., bw, rh - 112.),
            recipient: r(12., 0., bw, 44.), notice: r(12., 48., bw, 44.),
            close: r(rw - 56., 0., 44., 44.), send: r(right, rh - 44., 80., 44.),
            cancel: r(right + 88., rh - 44., 80., 44.), stamp: r(right, 48., 44., 44.),
            postage: r(right + 52., 48., 116., 44.), gold: r(right, 96., 168., 44.),
            slots: std::array::from_fn(|i| r(right + (i % 3) as f32 * 56., 148. + (i / 3) as f32 * 48., 44., 44.)),
            body_font_px: px(14.), compact: true };
        layout.valid(kind, 5).then_some(layout)
    }
    pub fn controls(&self, kind: ComposeKind, slots: usize) -> Vec<(ComposeAction, CrystalRect)> {
        let mut v = vec![(ComposeAction::Close, self.close), (ComposeAction::Submit, self.send),
            (ComposeAction::Cancel, self.cancel), (ComposeAction::Body, self.body)];
        if self.compact { v.push((ComposeAction::Recipient, self.recipient)); }
        if kind == ComposeKind::Parcel {
            v.push((ComposeAction::Stamp, self.stamp)); v.push((ComposeAction::Gold, self.gold));
            v.extend(self.slots.iter().take(slots.min(5)).enumerate().map(|(i,r)| (ComposeAction::Slot(i as u8), *r)));
        } v
    }
    pub fn valid(&self, kind: ComposeKind, slots: usize) -> bool {
        let controls = self.controls(kind, slots);
        let inside = |r: CrystalRect| r.is_valid_hit_target() && r.left >= 0. && r.top >= 0.
            && r.left + r.width <= self.root.width && r.top + r.height <= self.root.height;
        let overlap = |a: CrystalRect, b: CrystalRect| a.left < b.left+b.width && b.left<a.left+a.width
            && a.top < b.top+b.height && b.top<a.top+a.height;
        self.root.is_valid_hit_target() && self.body_font_px.is_finite() && self.body_font_px > 0.
            && controls.iter().all(|(_,r)| inside(*r))
            && controls.iter().enumerate().all(|(i,(_,r))| controls[..i].iter().all(|(_,a)| !overlap(*r,*a)))
    }
}

/// Display projection from the existing parcel owner. UID order and postage
/// never come from icons. A host must revalidate its real ledger on each action.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParcelPaint { pub stamped: bool, pub stamp_available: bool, pub quote_ready: bool,
    pub postage: Option<u32>, pub quote_error: Option<String>, pub cells: Vec<ParcelCell> }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParcelCell { pub unique_id: u64, pub image: Option<u16>, pub count_label: String }
impl ParcelPaint { pub fn slot_limit(&self) -> usize { if self.stamped {5} else {1} } }
#[derive(Component)] pub struct ComposeBody;
#[derive(Component)] pub struct ComposeSlot(pub u64);
#[derive(Component)] pub struct ComposeFrame(pub ComposeKind);
#[derive(Component)] pub struct ComposeFeedback;
#[derive(Component)] pub struct ComposeRecipient;
#[derive(Component)] pub struct ComposePromptField(pub &'static str);
#[derive(Component)] pub struct ComposePromptFrame;
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct GoldPaint {pub draft:String,pub max_amount:u32,pub amount:Option<u32>}
/// Full prompt replacement applies the existing Crystal uint textbox policy.
/// This is transient presentation state; the existing parcel owner accepts it.
pub fn replace_gold_prompt(max_amount:u32,text:&str)->GoldPaint {
    let mut input=super::amount_input::CrystalAmountInput::new(max_amount);
    input.draft.clear();input.select_all=false;input.push_text(text);
    GoldPaint{draft:input.draft.clone(),max_amount,amount:input.amount()}
}
pub fn gold_prompt_amount(gold:&GoldPaint)->Option<u32> {
    let mut input=super::amount_input::CrystalAmountInput::new(gold.max_amount);
    input.draft=gold.draft.clone();input.select_all=false;input.amount()
}

fn node(r: CrystalRect) -> Node { Node { position_type: PositionType::Absolute,
    left: Val::Px(r.left), top: Val::Px(r.top), width: Val::Px(r.width), height: Val::Px(r.height), ..default() } }
fn label(p: &mut ChildSpawnerCommands, text: &str, r: CrystalRect, font: &TextFont, size: f32, color: Color) {
    let mut f=font.clone(); f.font_size=FontSize::Px(size);
    let mut n=node(r); n.overflow=Overflow::clip();
    p.spawn((n, Text::new(text), f, TextColor(color), TextLayout::new(Justify::Left,LineBreak::WordOrCharacter)));
}
fn image(p: &mut ChildSpawnerCommands, assets: &AssetServer, path: String, r: CrystalRect) {
    p.spawn((node(r), ImageNode::new(assets.load(path)), FocusPolicy::Pass));
}
fn button<B: Bundle>(p: &mut ChildSpawnerCommands, assets: Option<&AssetServer>, library: &'static str,
    frames: [u16;3], r: CrystalRect, action: B, enabled: bool) {
    if let Some(assets)=assets {
        let spec=CrystalButtonSpec::new(library,frames[0],frames[1],frames[2],r,r.width,r.height);
        spawn_crystal_image_button(p,assets,spec,CrystalButtonAssetSet::from_spec(spec),action,false,enabled);
    } else if enabled { p.spawn((Button,action,node(r),BackgroundColor(Color::NONE),FocusPolicy::Block)); }
}

/// Both hosts invoke this function. Editor is the one canonical Resource;
/// adapters only map common actions to the existing parcel/Core send owner.
pub fn paint_compose<B: Bundle>(p: &mut ChildSpawnerCommands, assets: Option<&AssetServer>,
    kind: ComposeKind, draft: &MailComposeDraft, parcel: &ParcelPaint, notice: Option<&str>,
    editor: Option<&MailLetterEditor>, layout: &ComposeLayout, font: &TextFont, mut map: impl FnMut(ComposeAction)->B) {
    if !layout.valid(kind,parcel.slot_limit()) { return; }
    p.spawn((ComposeFrame(kind),node(CrystalRect::new(0.,0.,layout.root.width,layout.root.height)),FocusPolicy::Block));
    if let Some(assets)=assets { image(p,assets,format!("original-ui/Title/{}.png",if kind==ComposeKind::Parcel{674}else{671}),CrystalRect::new(0.,0.,layout.root.width,layout.root.height)); }
    button(p,assets,"Prguse2",[360,361,362],layout.close,map(ComposeAction::Close),true);
    let send=(!draft.recipient.trim().is_empty() && !draft.message.trim().is_empty())
        && (kind==ComposeKind::Letter || parcel.quote_ready);
    button(p,assets,"Title",[607,608,609],layout.send,map(ComposeAction::Submit),send);
    button(p,assets,"Title",[193,194,195],layout.cancel,map(ComposeAction::Cancel),true);
    if layout.compact { p.spawn((Button,map(ComposeAction::Recipient),node(layout.recipient),FocusPolicy::Block)); }
    label(p,&draft.recipient,layout.recipient,font,layout.body_font_px,TEXT);
    p.spawn((ComposeBody,map(ComposeAction::Body),node(layout.body),FocusPolicy::Pass));
    if let Some(editor)=editor { let mut f=font.clone(); f.font_size=FontSize::Px(layout.body_font_px); editor.render_with_style(p,layout.body,f,TEXT); }
    else { label(p,&draft.message,layout.body,font,layout.body_font_px,TEXT); }
    if let Some(notice)=notice { label(p,notice,layout.notice,font,if layout.compact{layout.body_font_px}else{8.},TEXT); }
    if kind!=ComposeKind::Parcel { return; }
    let frame=if parcel.stamped{204}else{203};
    button(p,assets,"Prguse2",[frame;3],layout.stamp,map(ComposeAction::Stamp),parcel.stamp_available);
    let size=if layout.compact{layout.body_font_px}else{9.};
    label(p,&format!("Postage: {}",parcel.postage.map_or("…".into(),|n|n.to_string())),layout.postage,font,size,TEXT);
    p.spawn((Button,map(ComposeAction::Gold),node(layout.gold),FocusPolicy::Block));
    label(p,&format!("Gold: {}",draft.gold),layout.gold,font,size,TEXT);
    if let Some(error)=parcel.quote_error.as_deref() { label(p,error,layout.notice,font,if layout.compact{size}else{8.},Color::srgb(0.95,0.34,0.28)); }
    if !parcel.stamped && !layout.compact { if let Some(assets)=assets { image(p,assets,"original-ui/Title/676.png".into(),CrystalRect::new(63.,310.,144.,33.)); } }
    for (slot,r) in layout.slots.iter().take(parcel.slot_limit()).enumerate() {
        p.spawn((Button,map(ComposeAction::Slot(slot as u8)),node(*r),FocusPolicy::Block));
        let Some(cell)=draft.attachment_unique_ids.get(slot).and_then(|id|parcel.cells.iter().find(|cell|cell.unique_id==*id)) else {continue;};
        p.spawn((ComposeSlot(cell.unique_id),node(*r),FocusPolicy::Pass)).with_children(|p| {
            if let (Some(assets),Some(image))=(assets,cell.image) { spawn_original_item_image(p,assets,image,r.width as i32,r.height as i32); }
            if !cell.count_label.is_empty() { label(p,&cell.count_label,CrystalRect::new(r.width-17.,r.height-14.,16.,12.),font,size,Color::srgb(1.,1.,0.)); }
        });
    }
}
pub fn paint_feedback<B: Bundle, C: Bundle>(p: &mut ChildSpawnerCommands, assets: Option<&AssetServer>, message: &str,
    rect: CrystalRect, font: &TextFont, marker: B, action: C) {
    let compact=rect.width!=456.||rect.height!=190.;let unit=if compact{rect.height/296.}else{1.};
    let text=if compact{CrystalRect::new(12.*unit,12.*unit,rect.width-24.*unit,rect.height-80.*unit)}else{CrystalRect::new(35.,35.,390.,110.)};
    let confirm=if compact{CrystalRect::new(rect.width-100.*unit,rect.height-56.*unit,88.*unit,44.*unit)}else{CrystalRect::new(360.,157.,76.,25.)};
    p.spawn((ComposeFeedback,ComposePromptFrame,marker,node(rect),FocusPolicy::Block)).with_children(|p| {
        if let Some(assets)=assets {image(p,assets,"original-ui/Prguse/360.png".into(),CrystalRect::new(0.,0.,rect.width,rect.height));}
        label(p,message,text,font,if compact{14.*unit}else{10.},TEXT);
        button(p,assets,"Title",[200,201,202],confirm,action,true);
    });
}
pub fn paint_recipient<B: Bundle>(p: &mut ChildSpawnerCommands, assets: Option<&AssetServer>, recipient: &str,
    rect: CrystalRect, font: &TextFont, mut map: impl FnMut(ComposeAction)->B) {
    let compact=rect.width!=288.||rect.height!=156.;let unit=if compact{rect.height/296.}else{1.};
    let field=if compact{CrystalRect::new(12.*unit,100.*unit,rect.width-24.*unit,44.*unit)}else{CrystalRect::new(23.,86.,240.,19.)};
    let caption=if compact{CrystalRect::new(12.*unit,12.*unit,rect.width-24.*unit,60.*unit)}else{CrystalRect::new(25.,25.,235.,40.)};
    let submit=if compact{CrystalRect::new(12.*unit,rect.height-56.*unit,(rect.width-36.*unit)*0.5,44.*unit)}else{CrystalRect::new(60.,123.,76.,25.)};
    let cancel=if compact{CrystalRect::new(submit.left+submit.width+12.*unit,submit.top,submit.width,submit.height)}else{CrystalRect::new(160.,123.,76.,25.)};
    let size=if compact{14.*unit}else{10.};
    p.spawn((ComposeRecipient,ComposePromptFrame,node(rect),FocusPolicy::Block)).with_children(|p| {
        if let Some(assets)=assets {image(p,assets,"original-ui/Prguse/660.png".into(),CrystalRect::new(0.,0.,rect.width,rect.height));}
        let mut caption_font=font.clone();caption_font.font_size=FontSize::Px(size);
        p.spawn((node(caption),Text::new("Enter mail recipient name"),caption_font,TextColor(TEXT),TextLayout::new(Justify::Center,LineBreak::WordOrCharacter)));
        let mut n=node(field);n.border=UiRect::all(Val::Px(1.));n.overflow=Overflow::clip();
        p.spawn((ComposePromptField("recipient"),n,BackgroundColor(Color::BLACK),BorderColor::all(Color::srgb(0.,1.,0.))))
            .with_children(|p|{let mut f=font.clone();f.font_size=FontSize::Px(size);
                p.spawn((node(CrystalRect::new(2.,0.,field.width-4.,field.height)),Text::new(recipient),f,TextColor(TEXT),TextLayout::new(Justify::Left,LineBreak::NoWrap)));});
        button(p,assets,"Title",[200,201,202],submit,map(ComposeAction::RecipientSubmit),true);
        button(p,assets,"Title",[203,204,205],cancel,map(ComposeAction::RecipientCancel),true);
    });
}
/// Exact opaque full-raw base from Core-slot. No UI projection reconstructs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ComposeFullRaw {
    pub to:String, pub subject:String, pub body:String, pub gold_text:String,
    pub items:Vec<String>, pub attachment_unique_ids:Vec<String>, pub stamped:bool,
    pub attachment_unique_ids_present:bool, pub stamped_present:bool,
}
impl ComposeFullRaw {
    pub fn valid(&self)->bool {
        self.to.len()<=1024 && self.subject.len()<=4096 && self.body.len()<=8192 && self.gold_text.len()<=256
            && self.items.len()<=5 && self.items.iter().all(|s|s.len()<=1024)
            && self.attachment_unique_ids.len()<=5
            && self.attachment_unique_ids.iter().enumerate().all(|(i,s)|s.parse::<u64>().ok().is_some_and(|n|n>0&&n<=SAFE&&n.to_string()==*s)
                && !self.attachment_unique_ids[..i].contains(s))
            && (self.attachment_unique_ids_present||self.attachment_unique_ids.is_empty())
            && (self.stamped_present||!self.stamped)
    }
}
/// All offsets identify Unicode scalar AND grapheme boundaries. Invalid DOM
/// surrogate-half offsets are rejected; adapters may explicitly clamp beforehand.
pub fn utf16_to_byte(text: &str, offset: usize) -> Option<usize> {
    let mut units=0;
    for (byte,c) in text.char_indices() { if units==offset {return Some(byte);} units+=c.len_utf16(); if units>offset{return None;} }
    (units==offset).then_some(text.len())
}
pub fn byte_to_utf16(text: &str, byte: usize) -> Option<usize> { text.get(..byte).map(|s|s.encode_utf16().count()) }
/// Nullable DOM range maps to byte endpoints without splitting a Unicode scalar.
/// Preview endpoints may be inside a grapheme; accepted body selection may not.
fn preview_cursor_bytes(text:&str,cursor:Option<[usize;2]>)->Option<Option<(usize,usize)>> {
    if text.len()>8192{return None;}
    match cursor {
        Some([start,end]) if start<=end && (end as u64)<=SAFE =>
            Some(Some((utf16_to_byte(text,start)?,utf16_to_byte(text,end)?))),
        Some(_)=>None,
        None=>Some(None),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextProof { pub owner: ComposeIdentity, pub incarnation: u64, pub draft_epoch: u64,
    pub draft_generation: String, pub editor_revision: u64, pub focus_generation: u64,
    pub presentation_revision: u64, pub layout_revision: u64, pub sequence: u64 }
impl TextProof {
    pub fn valid(&self) -> bool { self.owner.valid() && [self.incarnation,self.draft_epoch,self.editor_revision,
        self.focus_generation,self.presentation_revision,self.layout_revision,self.sequence].into_iter().all(|n|n>0&&n<=SAFE)
        && self.draft_generation.parse::<u64>().ok().is_some_and(|n|n>0&&n.to_string()==self.draft_generation) }
    pub fn same_capture(&self, other: &Self) -> bool { let mut a=self.clone();let mut b=other.clone();a.sequence=0;b.sequence=0;a==b }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag="op", rename_all="camelCase", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum TextOperation {
    Focus { focused: bool }, Selection { anchor_utf16: usize, caret_utf16: usize },
    Insert { text: String }, ReplaceBody { text: String }, Key { key: String, control: bool, shift: bool },
    Pointer { x: f32, y: f32, extend: bool }, Wheel { upward_pixels: f32 },
    ImeStart { composition_id: u64 }, ImePreview { composition_id: u64, text: String, cursor_utf16: Option<[usize;2]> },
    ImeCancel { composition_id: u64 }, ImeCommit { composition_id: u64, text: String },
    ClipboardRequest { request_id: u64, kind: String }, ClipboardResult { request_id: u64, success: bool, text: Option<String> },
    PromptReplace { target:String, text:String },
    Submit,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct TextEdge { pub proof: TextProof, pub operation: TextOperation }
#[derive(Debug,Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ComposePointerEdge {pub proof:TextProof,pub pointer_id:u64,pub phase:String,
    pub x:f32,pub y:f32,pub button:u8,pub shift:bool}
#[derive(Clone)]struct ComposeHeld {pointer:u64,proof:TextProof,action:ComposeAction}
#[derive(Resource,Default)]
pub struct ComposePointerRouter {held:Option<ComposeHeld>,highwater:u64,run:u64}
impl ComposePointerRouter {
    pub fn retire(&mut self){self.held=None;}
    pub fn highwater(&self)->u64{self.highwater}
    pub fn handle(&mut self,current:&TextProof,edge:ComposePointerEdge,controls:&[(ComposeAction,CrystalRect)],
        editor:&mut MailLetterEditor,draft:&MailComposeDraft,base:&ComposeFullRaw)->Option<TextIntent> {
        if self.run!=current.owner.run{self.run=current.owner.run;self.highwater=0;self.retire();}
        if !current.valid()||!edge.proof.valid()||!current.same_capture(&edge.proof)
            ||edge.proof.sequence<=self.highwater||edge.pointer_id>SAFE||edge.button!=0
            ||!edge.x.is_finite()||!edge.y.is_finite()
            ||!matches!(edge.phase.as_str(),"down"|"move"|"up"|"cancel") {self.retire();return None;}
        self.highwater=edge.proof.sequence;
        if edge.phase=="cancel" {self.retire();return None;}
        let hit=controls.iter().rev().find(|(_,r)|r.contains(edge.x,edge.y)).copied();
        let update_body=|editor:&mut MailLetterEditor,r:CrystalRect,extend:bool| {
            let p=(Vec2::new(edge.x-r.left,edge.y-r.top)-Vec2::splat(2.)).clamp(Vec2::ZERO,
                (editor.content_size()-Vec2::splat(0.001)).max(Vec2::ZERO));
            editor.common_pointer(p,extend);
        };
        match edge.phase.as_str() {
            "down"=> {
                if self.held.is_some(){self.retire();return None;}
                let(action,r)=hit?;let mut capture=edge.proof.clone();
                if action==ComposeAction::Body {update_body(editor,r,edge.shift);capture.editor_revision=editor.revision();capture.focus_generation=editor.focus_generation();}
                self.held=Some(ComposeHeld{pointer:edge.pointer_id,proof:capture,action});
                (action==ComposeAction::Body).then(||CommonMailTextAdapter::intent(edge.proof,draft,base,"focus"))
            },
            "move"=> {
                let Some(held)=self.held.as_ref()else{return None;};
                if held.pointer!=edge.pointer_id||!held.proof.same_capture(current)||hit.map(|(a,_)|a)!=Some(held.action){self.retire();return None;}
                if held.action==ComposeAction::Body {let(_,r)=hit?;update_body(editor,r,true);
                    if let Some(held)=self.held.as_mut(){held.proof.editor_revision=editor.revision();held.proof.focus_generation=editor.focus_generation();}}
                None
            },
            "up"=> {
                let held=self.held.take()?;
                if held.pointer!=edge.pointer_id||!held.proof.same_capture(current)||hit.map(|(a,_)|a)!=Some(held.action){return None;}
                if held.action==ComposeAction::Body{return None;}
                let action=match held.action {ComposeAction::Close=>"close",ComposeAction::Submit=>"submit",ComposeAction::Cancel=>"cancel",
                    ComposeAction::Recipient=>"recipient",ComposeAction::Gold=>"gold",ComposeAction::Stamp=>"stamp",ComposeAction::Slot(_)=>"slot",
                    ComposeAction::RecipientSubmit=>"recipientSubmit",ComposeAction::RecipientCancel=>"recipientCancel",ComposeAction::Feedback=>"feedback",
                    ComposeAction::GoldConfirm=>"goldConfirm",ComposeAction::GoldCancel=>"goldCancel",_=>return None};
                let mut out=CommonMailTextAdapter::intent(edge.proof,draft,base,action);
                if let ComposeAction::Slot(slot)=held.action{out.request_id=Some(u64::from(slot));}
                if held.action==ComposeAction::Submit {
                    if editor.composition().is_some(){return None;}
                    if let Err(error)=mir2_client_core::mail_compose::prepare_send(&draft.recipient,&draft.message,draft.gold,&draft.attachment_unique_ids){out.action="rejected".into();out.error=Some(error.message().into());}
                } Some(out)
            },_=>None,
        }
    }
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PromptMutation {pub target:String,pub text:String}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextIntent { pub proof: TextProof, pub action: String, pub base_raw: ComposeFullRaw,
    pub body_mutation: Option<String>, pub recipient_mutation: Option<String>,
    pub prompt_mutation:Option<PromptMutation>, pub prompt_value:Option<u32>,
    pub request_id: Option<u64>, pub clipboard_kind: Option<String>, pub selected_text: Option<String>, pub error: Option<String> }
#[derive(Debug, Clone)] struct ClipboardCapture { id:u64, kind:String, proof:TextProof, selection:std::ops::Range<usize>, caret:usize }
#[derive(Resource, Default)]
pub struct CommonMailTextAdapter { owner:Option<ComposeIdentity>, highwater:u64, composition:Option<(u64,TextProof)>,
    last_composition:u64, clipboard:Option<ClipboardCapture>, last_clipboard:u64 }
impl CommonMailTextAdapter {
    pub fn highwater(&self)->u64 {self.highwater}
    pub fn retire(&mut self) {self.composition=None;self.clipboard=None;}
    fn intent(proof:TextProof,draft:&MailComposeDraft,base:&ComposeFullRaw,action:&str) -> TextIntent { TextIntent {proof,action:action.into(),base_raw:base.clone(),
        body_mutation:(draft.message!=base.body).then(||draft.message.clone()),recipient_mutation:(draft.recipient!=base.to).then(||draft.recipient.clone()),prompt_mutation:None,prompt_value:None,request_id:None,clipboard_kind:None,selected_text:None,error:None} }
    /// UI proof is separate from socket/send-slot proof. Submit returns a full
    /// current draft; only the host's existing sender can reserve or transmit.
    pub fn apply(&mut self, current:&TextProof, edge:TextEdge, editor:&mut MailLetterEditor,
        draft:&mut MailComposeDraft, base:&ComposeFullRaw) -> Option<TextIntent> {
        if !base.valid() || !current.valid() || !edge.proof.valid() || !current.same_capture(&edge.proof)
            || current.editor_revision!=editor.revision() || current.focus_generation!=editor.focus_generation()
            || current.layout_revision!=editor.layout_revision() {return None;}
        if self.owner.is_some_and(|old|current.owner.run<old.run){return None;}
        if self.owner!=Some(current.owner) {
            // Local request IDs belong to a checked run. Withdrawal or another
            // capture owner in the same run must never reopen their namespace.
            if self.owner.is_none_or(|old|current.owner.run>old.run) {
                self.highwater=0;self.last_composition=0;self.last_clipboard=0;
            }
            self.retire();editor.clear_composition();self.owner=Some(current.owner);
        }
        if edge.proof.sequence<=self.highwater {return None;}self.highwater=edge.proof.sequence;
        let proof=edge.proof;
        match edge.operation {
            TextOperation::Focus{focused} => {editor.set_focused(focused);self.retire();return Some(Self::intent(proof,draft,base,"focus"));},
            _ if !editor.focused() => return None,
            TextOperation::Selection{anchor_utf16,caret_utf16} => {editor.set_dom_selection(anchor_utf16,caret_utf16)?;},
            TextOperation::Insert{text} => {if text.len()>8192{return None;}if self.composition.is_some(){return None;} editor.paste(draft,&text);},
            TextOperation::ReplaceBody{text} => {if text.len()>8192{return None;}self.retire();editor.replace_body_raw(draft,&text);},
            TextOperation::Key{key,control,shift} => {editor.common_key(&key,control,shift,draft)?;},
            TextOperation::Pointer{x,y,extend} => {if !x.is_finite()||!y.is_finite()||x<0.||y<0.||x>=editor.content_size().x||y>=editor.content_size().y{return None;}editor.pointer(Vec2::new(x,y),extend);},
            TextOperation::Wheel{upward_pixels} => {if !upward_pixels.is_finite(){return None;}editor.scroll_wheel_pixels(upward_pixels);},
            TextOperation::ImeStart{composition_id} => {if composition_id<=self.last_composition||composition_id>SAFE{return None;}self.last_composition=composition_id;self.composition=Some((composition_id,proof.clone()));},
            TextOperation::ImePreview{composition_id,text,cursor_utf16} => {
                if text.len()>8192||!self.composition.as_ref().is_some_and(|(id,c)|*id==composition_id&&c.same_capture(&proof)){return None;}
                let cursor=preview_cursor_bytes(&text,cursor_utf16)?;
                editor.set_composition(text,cursor);
            },
            TextOperation::ImeCancel{composition_id} => {if !self.composition.as_ref().is_some_and(|(id,_)|*id==composition_id){return None;}self.composition=None;editor.clear_composition();},
            TextOperation::ImeCommit{composition_id,text} => {
                if text.len()>8192||!self.composition.as_ref().is_some_and(|(id,c)|*id==composition_id&&c.same_capture(&proof)){return None;}
                self.composition=None;editor.clear_composition();editor.paste(draft,&text);
            },
            TextOperation::ClipboardRequest{request_id,kind} => {
                if request_id<=self.last_clipboard||request_id>SAFE||!matches!(kind.as_str(),"copy"|"cut"|"paste"){return None;}
                self.last_clipboard=request_id;
                let active=editor.active_editor()?;
                self.clipboard=Some(ClipboardCapture{id:request_id,kind:kind.clone(),proof:proof.clone(),selection:active.selection(),caret:active.caret()});
                let mut out=Self::intent(proof,draft,base,"clipboardRequest");out.request_id=Some(request_id);
                out.selected_text=(kind!="paste").then(||editor.selected_text().into());out.clipboard_kind=Some(kind);return Some(out);
            },
            TextOperation::ClipboardResult{request_id,success,text} => {
                let Some(c)=self.clipboard.as_ref()else{return None;};
                if c.id!=request_id {return None;}
                let c=self.clipboard.take()?;let active=editor.active_editor()?;
                if !success||!c.proof.same_capture(&proof)||c.selection!=active.selection()||c.caret!=active.caret(){return None;}
                match c.kind.as_str(){"cut"=>editor.cut_selection(draft),"paste"=>editor.paste(draft,text.as_deref()?),"copy"=>{},_=>return None}
            },
            TextOperation::PromptReplace{..} => return None,
            TextOperation::Submit => {
                if self.composition.is_some() {return None;}
                let mut out=Self::intent(proof,draft,base,"submit");
                if let Err(error)=mir2_client_core::mail_compose::prepare_send(&draft.recipient,&draft.message,draft.gold,&draft.attachment_unique_ids){out.action="rejected".into();out.error=Some(error.message().into());}
                return Some(out);
            },
        }
        Some(Self::intent(proof,draft,base,"edit"))
    }
}



/// The existing host amount owner supplies validity. This shared painter never
/// derives amounts, price, inventory custody, or send state from text/icons.
pub fn paint_gold<B:Bundle,C:Bundle>(p:&mut ChildSpawnerCommands,assets:Option<&AssetServer>,gold:&GoldPaint,
    rect:CrystalRect,font:&TextFont,field_marker:B,mut map:impl FnMut(ComposeAction)->C) {
    let compact=rect.width!=204.||rect.height!=109.;
    let unit=if compact{rect.height/296.}else{1.};let hit=44.*unit;let size=if compact{14.*unit}else{10.};
    p.spawn((ComposePromptFrame,node(rect),FocusPolicy::Block)).with_children(|p| {
        if let Some(assets)=assets{image(p,assets,"original-ui/Prguse/238.png".into(),CrystalRect::new(0.,0.,rect.width,rect.height));}
        let close=if compact{CrystalRect::new(rect.width-hit-12.*unit,12.*unit,hit,hit)}else{CrystalRect::new(180.,3.,24.,21.)};
        button(p,assets,"Prguse2",[360,361,362],close,map(ComposeAction::GoldCancel),true);
        label(p,"Gold Amount:",CrystalRect::new(19.*unit,8.*unit,rect.width-hit-40.*unit,if compact{hit}else{14.}),font,size,Color::WHITE);
        let field=if compact{CrystalRect::new(58.*unit,80.*unit,rect.width-82.*unit,hit)}else{CrystalRect::new(58.,43.,132.,19.)};
        let color=match gold.amount {None=>Color::srgb(1.,0.,0.),Some(n)if n==gold.max_amount=>Color::srgb(1.,0.647,0.),_=>Color::srgb(0.,1.,0.)};
        let mut n=node(field);n.border=UiRect::all(Val::Px(1.));n.overflow=Overflow::clip();
        p.spawn((ComposePromptField("gold"),field_marker,n,BackgroundColor(Color::BLACK),BorderColor::all(color)))
            .with_children(|p|label(p,&gold.draft,CrystalRect::new(2.,0.,field.width-4.,field.height),font,size,TEXT));
        if let Some(assets)=assets{p.spawn(node(CrystalRect::new(15.*unit,if compact{80.*unit}else{34.},38.*unit,34.*unit)))
            .with_children(|p|spawn_original_item_image(p,assets,116,(38.*unit)as i32,(34.*unit)as i32));}
        let ok=if compact{CrystalRect::new(12.*unit,rect.height-hit-12.*unit,(rect.width-36.*unit)*0.5,hit)}else{CrystalRect::new(23.,76.,76.,25.)};
        let cancel=if compact{CrystalRect::new(ok.left+ok.width+12.*unit,ok.top,ok.width,hit)}else{CrystalRect::new(110.,76.,76.,25.)};
        if gold.amount.is_some(){button(p,assets,"Title",[200,201,202],ok,map(ComposeAction::GoldConfirm),true);}
        button(p,assets,"Title",[203,204,205],cancel,map(ComposeAction::GoldCancel),true);
    });
}

/// Keep explicit nullable fields mandatory without erasing duplicate typed keys.
pub fn parse_text_edge(json:&str)->Option<TextEdge> {
    if json.len()>32768{return None;}
    let edge:TextEdge=serde_json::from_str(json).ok()?;
    let v:serde_json::Value=serde_json::from_str(json).ok()?;
    let exact=|v:&serde_json::Value,keys:&[&str]|v.as_object().is_some_and(|m|m.len()==keys.len()&&keys.iter().all(|k|m.contains_key(*k)));
    if !exact(&v,&["proof","operation"]){return None;}
    let keys:&[&str]=match v["operation"]["op"].as_str()? {
        "focus"=>&["op","focused"],"selection"=>&["op","anchorUtf16","caretUtf16"],
        "insert"|"replaceBody"=>&["op","text"],"key"=>&["op","key","control","shift"],
        "pointer"=>&["op","x","y","extend"],"wheel"=>&["op","upwardPixels"],
        "imeStart"|"imeCancel"=>&["op","compositionId"],"imePreview"=>&["op","compositionId","text","cursorUtf16"],
        "imeCommit"=>&["op","compositionId","text"],"clipboardRequest"=>&["op","requestId","kind"],
        "clipboardResult"=>&["op","requestId","success","text"],"promptReplace"=>&["op","target","text"],"submit"=>&["op"],_=>return None};
    let preview_valid=match &edge.operation {TextOperation::ImePreview{text,cursor_utf16,..}=>preview_cursor_bytes(text,*cursor_utf16).is_some(),_=>true};
    (exact(&v["operation"],keys)&&edge.proof.valid()&&preview_valid).then_some(edge)
}
