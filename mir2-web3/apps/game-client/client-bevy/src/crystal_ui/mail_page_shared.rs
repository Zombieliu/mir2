//! One actual mailbox/letter/parcel-reader painter shared by Native and portable.
//! Model changes, selection, pointer ownership and transport belong to the host.
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use crate::mail::{mail_claim_enabled,mail_delete_enabled,mail_date_label,MailMessage,MailModel};
use super::{assets::CrystalButtonAssetSet,spec::{CrystalButtonSpec,CrystalRect},widget::spawn_crystal_image_button,item_image::spawn_original_item_image,bag_paint::format_crystal_gold};
const TEXT: Color=Color::srgb(0.95,0.92,0.82);
#[derive(Debug,Clone,Copy,PartialEq,Eq,Component)]
pub enum MailPaintAction {Close,Prev,Next,Select(u64),Compose,Reply(u64),Read(u64),Delete(u64),ReaderClose,ReaderDelete,ReaderLock,ReaderClaim}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum MailReaderKind {Letter,Parcel}
fn mail_font(font: Option<&TextFont>, size: f32) -> TextFont {
    let mut font=font.cloned().unwrap_or_else(||super::typography::crystal_text_font(size));
    font.font_size=FontSize::Px(size);font
}
fn mail_label(parent: &mut ChildSpawnerCommands, font: Option<&TextFont>, text: &str, rect: CrystalRect, font_size: f32, color: Color) {
    parent.spawn((Node{position_type:PositionType::Absolute,left:Val::Px(rect.left),top:Val::Px(rect.top),width:Val::Px(rect.width),height:Val::Px(rect.height),overflow:Overflow::clip(),..default()},Text::new(text),mail_font(font,font_size),TextColor(color),TextLayout::new(Justify::Left,LineBreak::NoWrap)));
}
fn spawn_static_overlay_sprite(parent: &mut ChildSpawnerCommands, assets: &AssetServer, path: String, rect: CrystalRect) {
    parent.spawn((Node{position_type:PositionType::Absolute,left:Val::Px(rect.left),top:Val::Px(rect.top),width:Val::Px(rect.width),height:Val::Px(rect.height),..default()},ImageNode::new(assets.load(path))));
}
fn spawn_overlay_frame(parent: &mut ChildSpawnerCommands, assets: &AssetServer, path: &'static str, width: f32, height: f32) {
    spawn_static_overlay_sprite(parent,assets,path.into(),CrystalRect::new(0.,0.,width,height));
}
fn spawn_invisible_overlay_button<B: Bundle>(parent: &mut ChildSpawnerCommands, rect: CrystalRect, action: B) {
    parent.spawn((Button,action,Node{position_type:PositionType::Absolute,left:Val::Px(rect.left),top:Val::Px(rect.top),width:Val::Px(rect.width),height:Val::Px(rect.height),..default()},BackgroundColor(Color::NONE)));
}
fn spawn_overlay_crystal_button<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: &AssetServer, library: &'static str, normal: u16, hover: u16, pressed: u16, rect: CrystalRect, action: B) {
    spawn_overlay_crystal_button_enabled(parent,assets,library,normal,hover,pressed,rect,action,true);
}
fn spawn_overlay_crystal_button_enabled<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: &AssetServer, library: &'static str, normal: u16, hover: u16, pressed: u16, rect: CrystalRect, action: B, enabled: bool) {
    spawn_overlay_crystal_button_enabled_with_disabled(parent,assets,library,normal,hover,pressed,None,rect,action,enabled);
}
fn spawn_overlay_crystal_button_enabled_with_disabled<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: &AssetServer, library: &'static str, normal: u16, hover: u16, pressed: u16, disabled: Option<u16>, rect: CrystalRect, action: B, enabled: bool) {
    let spec=CrystalButtonSpec::new(library,normal,hover,pressed,rect,rect.width,rect.height);
    let mut states=CrystalButtonAssetSet::from_spec(spec);if let Some(index)=disabled{states=states.with_disabled(spec.asset_path(index));}
    spawn_crystal_image_button(parent,assets,spec,states,action,false,enabled);
}
/// Both hosts paint the same reader; hosts retain identity, drag and action guards.
pub fn paint_reader<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: Option<&AssetServer>, message: &MailMessage, kind: MailReaderKind, font: Option<&TextFont>, mut map: impl FnMut(MailPaintAction)->B) {
    match kind {MailReaderKind::Letter=>paint_letter(parent,assets,message,font,&mut map),MailReaderKind::Parcel=>paint_parcel(parent,assets,message,font,&mut map)}
}
#[derive(Component)]
pub struct MailRowIcon;

pub fn layout_mail_row_icons(images: Option<Res<Assets<Image>>>, mut icons: Query<(&ImageNode, &mut Node), With<MailRowIcon>>) {
    let Some(images) = images else { return };
    for (image, mut node) in &mut icons {
        let Some(bitmap) = images.get(&image.image) else {
            node.display = Display::None;
            continue;
        };
        let size = bitmap.texture_descriptor.size;
        node.left = Val::Px(((34 - size.width as i32) / 2) as f32);
        node.top = Val::Px(((32 - size.height as i32) / 2) as f32);
        node.width = Val::Px(size.width as f32);
        node.height = Val::Px(size.height as f32);
        node.display = Display::Flex;
    }
}

pub fn preview(message: &MailMessage) -> String {
    format!("{}{}", if message.locked { "[*] " } else { "" }, message.body.replace("\r\n", " "))
}

fn paint_row<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: Option<&AssetServer>, message: &MailMessage, index: usize, selected: bool, font: Option<&TextFont>, map: &mut impl FnMut(MailPaintAction) -> B) {
    parent.spawn((Button, map(MailPaintAction::Select(message.id)), FocusPolicy::Block, Node {
        position_type: PositionType::Absolute,
        left: Val::Px(10.0), top: Val::Px(55.0 + index as f32 * 33.0),
        width: Val::Px(290.0), height: Val::Px(33.0),
        ..default()
    }, BackgroundColor(Color::NONE))).with_children(|row| {
        if let Some(assets) = assets {
            let path = if let Some(item) = message.items.first() {
                item.image.map(|image| format!("original-ui/Items/{image}.png"))
            } else {
                Some(format!("original-ui/Prguse/{}.png", if message.gold > 0 { 541 } else { 540 }))
            };
            if let Some(path) = path {
                row.spawn((MailRowIcon, Node { position_type: PositionType::Absolute, display: Display::None, ..default() }, ImageNode::new(assets.load(path))));
            }
            if !message.read {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/550.png".into(), CrystalRect::new(if !message.claimed || message.locked {20.0} else {5.0}, 17.0, 12.0, 9.0));
            }
            if message.locked {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/551.png".into(), CrystalRect::new(5.0, 17.0, 12.0, 10.0));
            }
            if !message.claimed {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/552.png".into(), CrystalRect::new(5.0, 17.0, 12.0, 11.0));
            }
            if selected {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/545.png".into(), CrystalRect::new(-5.0, -3.0, 296.0, 38.0));
            }
        }
        for (text, left, width) in [(message.sender.clone(), 35.0, 130.0), (preview(message), 170.0, 115.0)] {
            row.spawn(Node {
                position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(0.0),
                width: Val::Px(width), height: Val::Px(31.0), align_items: AlignItems::Center,
                overflow: Overflow::clip(), ..default()
            }).with_children(|label| {
                label.spawn((Text::new(text), mail_font(font, 10.0),
                    TextColor(TEXT), TextLayout::new(Justify::Left, LineBreak::NoWrap)));
            });
        }
    });
}

pub fn reader_text(message: &MailMessage) -> String {
    message.body.replace("\\r\\n", "\r\n")
}

fn wrapped_label(
    parent: &mut ChildSpawnerCommands,
    font: Option<&TextFont>,
    text: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        Text::new(text.to_owned()),
        mail_font(font, font_size),
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
    ));
}

fn spawn_mail_reader_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
    disabled: u16,
    rect: CrystalRect,
    action: B,
    enabled: bool,
) {
    if let Some(asset_server) = asset_server {
        spawn_overlay_crystal_button_enabled_with_disabled(
            parent,
            asset_server,
            library,
            normal,
            hover,
            pressed,
            Some(disabled),
            rect,
            action,
            enabled,
        );
    } else if enabled {
        spawn_invisible_overlay_button(parent, rect, action);
    }
}

fn paint_reader_header<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    message: &MailMessage,
    frame: &'static str,
    height: f32,
    font: Option<&TextFont>,
    map: &mut impl FnMut(MailPaintAction) -> B,
) {
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(parent, asset_server, frame, 236.0, height);
    }
    spawn_mail_reader_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        362,
        CrystalRect::new(209.0, 3.0, 24.0, 21.0),
        map(MailPaintAction::ReaderClose),
        true,
    );
    mail_label(parent, font,
        &message.sender,
        CrystalRect::new(70.0, 35.0, 150.0, 15.0),
        10.0,
        TEXT,
    );
    mail_label(parent, font,
        &mail_date_label(message.date_sent_binary_datetime),
        CrystalRect::new(70.0, 56.0, 150.0, 15.0),
        10.0,
        TEXT,
    );
}

fn paint_letter<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    message: &MailMessage,
    font: Option<&TextFont>,
    map: &mut impl FnMut(MailPaintAction) -> B,
) {
    paint_reader_header(
        parent,
        asset_server,
        message,
        "original-ui/Title/672.png",
        300.0,
        font,
        map,
    );
    wrapped_label(parent, font,
        &reader_text(message),
        CrystalRect::new(15.0, 92.0, 202.0, 165.0),
        10.0,
        TEXT,
    );
    // Source handlers guard delete against Locked; retain the source button
    // but repeat that guard at the action boundary.
    spawn_mail_reader_button(
        parent,
        asset_server,
        "Title",
        540,
        541,
        542,
        542,
        CrystalRect::new(12.0, 265.0, 64.0, 25.0),
        map(MailPaintAction::ReaderDelete),
        true,
    );
    spawn_mail_reader_button(
        parent,
        asset_server,
        "Title",
        686,
        687,
        688,
        688,
        CrystalRect::new(81.0, 265.0, 64.0, 25.0),
        map(MailPaintAction::ReaderLock),
        true,
    );
    spawn_mail_reader_button(
        parent,
        asset_server,
        "Title",
        193,
        194,
        195,
        195,
        CrystalRect::new(154.0, 265.0, 68.0, 25.0),
        map(MailPaintAction::ReaderClose),
        true,
    );
}

fn paint_parcel<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    message: &MailMessage,
    font: Option<&TextFont>,
    map: &mut impl FnMut(MailPaintAction) -> B,
) {
    paint_reader_header(
        parent,
        asset_server,
        message,
        "original-ui/Title/675.png",
        384.0,
        font,
        map,
    );
    wrapped_label(parent, font,
        &reader_text(message),
        CrystalRect::new(15.0, 98.0, 202.0, 165.0),
        10.0,
        TEXT,
    );
    mail_label(parent, font,
        &format_crystal_gold(message.gold),
        CrystalRect::new(63.0, 290.0, 143.0, 15.0),
        10.0,
        TEXT,
    );
    for (index, attachment) in message.items.iter().take(5).enumerate() {
        let left = 27.0 + index as f32 * 36.0;
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(311.0),
                width: Val::Px(35.0),
                height: Val::Px(31.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.0, 1.0, 0.0)),
            BackgroundColor(Color::NONE),
        ));
        if let (Some(asset_server), Some(image)) = (
            asset_server,
            attachment.image.and_then(|image| u16::try_from(image).ok()),
        ) {
            parent.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(311.0),
                width: Val::Px(35.0),
                height: Val::Px(31.0),
                ..default()
            }).with_children(|cell| {
                spawn_original_item_image(cell, asset_server, image, 35, 31);
            });
        }
        if attachment.count > 1 {
            mail_label(parent, font,
                &attachment.count.to_string(),
                CrystalRect::new(left + 18.0, 327.0, 16.0, 12.0),
                9.0,
                Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
    let collect_enabled = mail_claim_enabled(message);
    spawn_mail_reader_button(
        parent,
        asset_server,
        "Title",
        680,
        681,
        682,
        683,
        CrystalRect::new(30.0, 350.0, 72.0, 25.0),
        map(MailPaintAction::ReaderClaim),
        collect_enabled,
    );
    spawn_mail_reader_button(
        parent,
        asset_server,
        "Title",
        193,
        194,
        195,
        195,
        CrystalRect::new(135.0, 350.0, 68.0, 25.0),
        map(MailPaintAction::ReaderClose),
        true,
    );
}

pub fn paint_list<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    mail: &MailModel,
    page_index: usize,
    font: Option<&TextFont>,
    mut map: impl FnMut(MailPaintAction) -> B,
) {
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Title/670.png",
            312.0,
            444.0,
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(288.0, 3.0, 24.0, 21.0),
            map(MailPaintAction::Close),
        );
        // This is MailDialog's context-help control, not MenuDialog's global
        // HelpDialog button. Its source handler remains unimplemented here.
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            257,
            258,
            259,
            CrystalRect::new(262.0, 3.0, 24.0, 21.0),
            map(MailPaintAction::Close),
            false,
        );
    }
    let page = mail.page(page_index);
    if let Some(assets) = asset_server {
        spawn_static_overlay_sprite(parent, assets, "original-ui/Title/7.png".into(), CrystalRect::new(18.0, 9.0, 43.0, 14.0));
    }
    for (label, left, width) in [("Type", 8.0, 37.0), ("Sender", 47.0, 132.0), ("Message", 181.0, 122.0)] {
        mail_label(parent, font, label, CrystalRect::new(left, 34.0, width, 19.0), 10.0, TEXT);
    }
    for (row, msg) in page.entries.iter().enumerate() {
        paint_row(parent, asset_server, msg, row, mail.selected_id == Some(msg.id), font, &mut map);
    }

    if let Some(asset_server) = asset_server {
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            240,
            241,
            242,
            CrystalRect::new(102.0, 389.0, 16.0, 16.0),
            map(MailPaintAction::Prev),
            page.page > 0,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            243,
            244,
            245,
            CrystalRect::new(192.0, 389.0, 16.0, 16.0),
            map(MailPaintAction::Next),
            page.page + 1 < page.page_count,
        );
    }
    mail_label(parent, font,
        &format!("{}/{}", page.page + 1, page.page_count),
        CrystalRect::new(120.0, 389.0, 67.0, 15.0),
        9.0,
        TEXT,
    );

    let selected = mail.selected();
    let selected_id = selected.map(|message| message.id).unwrap_or_default();
    if let Some(asset_server) = asset_server {
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse",
            563,
            564,
            565,
            CrystalRect::new(75.0, 414.0, 28.0, 25.0),
            map(MailPaintAction::Compose),
        );
        if selected.is_some_and(|message| message.can_reply) { spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            569,
            570,
            571,
            CrystalRect::new(102.0, 414.0, 28.0, 25.0),
            map(MailPaintAction::Reply(selected_id)),
            selected.is_some_and(|message| message.can_reply),
        ); }
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            572,
            573,
            574,
            CrystalRect::new(129.0, 414.0, 28.0, 25.0),
            map(MailPaintAction::Read(selected_id)),
            selected.is_some(),
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            557,
            558,
            559,
            CrystalRect::new(156.0, 414.0, 28.0, 25.0),
            map(MailPaintAction::Delete(selected_id)),
            selected.is_some_and(mail_delete_enabled),
        );
        for (index, left) in [(520, 183.0), (523, 210.0)] {
            spawn_overlay_crystal_button_enabled(parent, asset_server, "Prguse", index, index + 1, index + 2,
                CrystalRect::new(left, 414.0, 28.0, 25.0), map(MailPaintAction::Close), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::mail::MailAttachment;
    use super::super::{widget::CrystalImageButton,item_image::{OriginalItemImage,layout_original_item_images}};

    #[derive(Component)]
    struct NativeActions(MailPaintAction);
    #[derive(Component)]
    struct PortableActions(MailPaintAction);

    fn fixture() -> MailModel {
        let mut mail=MailModel::default();
        mail.mails=(1..=11).map(|id|MailMessage {id,sender:format!("Sender{id}"),subject:"never the row preview".into(),body:"本地化长正文\r\nSecond line".into(),locked:id==1,can_reply:id==2,..default()}).collect();
        mail.selected_id=Some(1);
        mail.mails[0].items=(0..6).map(|i|MailAttachment {image:Some(71+i),count:2,..default()}).collect();
        mail.mails[0].gold=1280;mail
    }
    fn painted(portable: bool, page: usize) -> App {
        let mut app=App::new();
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default())).init_asset::<Image>();
        let mail=fixture();
        app.world_mut().run_system_once(move |mut commands:Commands,assets:Res<AssetServer>|{
            let font=TextFont {font:Handle::<Font>::default().into(),font_size:FontSize::Px(99.),..default()};
            commands.spawn(Node::default()).with_children(|parent|{
                if portable {paint_list(parent,Some(&assets),&mail,page,Some(&font),PortableActions)}
                else {paint_list(parent,Some(&assets),&mail,page,Some(&font),NativeActions)}
            });
            for kind in [MailReaderKind::Letter,MailReaderKind::Parcel] {
                commands.spawn(Node::default()).with_children(|parent|{
                    if portable {paint_reader(parent,Some(&assets),&mail.mails[0],kind,Some(&font),PortableActions)}
                    else {paint_reader(parent,Some(&assets),&mail.mails[0],kind,Some(&font),NativeActions)}
                });
            }
        }).unwrap();app
    }
    fn description(app: &mut App) -> Vec<String> {
        let w=app.world_mut();
        let mut rows=w.query::<(&Node,Option<&Text>,Option<&ImageNode>,Option<&CrystalImageButton>)>().iter(w).map(|(node,text,image,button)|format!("{node:?}|{:?}|{:?}|{button:?}",text.map(|t|t.0.as_str()),image.and_then(|i|i.image.path()).map(|p|p.to_string()))).collect::<Vec<_>>();
        rows.sort();rows
    }
    #[test]
    fn both_action_adapters_paint_same_actual_nodes_and_source_controls() {
        let mut native=painted(false,0);let mut portable=painted(true,0);
        assert_eq!(description(&mut native),description(&mut portable));
        let w=native.world_mut();
        let actions=w.query::<(&NativeActions,Option<&Button>,&Node)>().iter(w).collect::<Vec<_>>();
        assert_eq!(actions.iter().filter(|(a,_,_)|matches!(a.0,MailPaintAction::Select(_))).count(),10);
        assert!(!actions.iter().any(|(a,_,_)|matches!(a.0,MailPaintAction::Reply(_))));
        assert!(actions.iter().any(|(a,b,_)|a.0==MailPaintAction::Prev&&b.is_none()));
        assert!(actions.iter().any(|(a,b,n)|a.0==MailPaintAction::Next&&b.is_some()&&n.left==Val::Px(192.)));
        assert!(actions.iter().any(|(a,b,_)|a.0==MailPaintAction::Delete(1)&&b.is_none()));
        let (claim,button)=w.query::<(&NativeActions,&CrystalImageButton)>().iter(w).find(|(a,_)|a.0==MailPaintAction::ReaderClaim).unwrap();
        assert_eq!(claim.0,MailPaintAction::ReaderClaim);assert!(!button.enabled);
        assert_eq!(button.assets.disabled.as_deref(),Some("original-ui/Title/683.png"));
        assert!(w.query::<&ImageNode>().iter(w).any(|i|i.image.path().is_some_and(|p|p.to_string()=="original-ui/Title/683.png")));
        assert_eq!(w.query::<&OriginalItemImage>().iter(w).count(),5);
        assert!(w.query::<&Text>().iter(w).any(|t|t.0=="[*] 本地化长正文 Second line"));
        assert!(!w.query::<&Text>().iter(w).any(|t|t.0=="never the row preview"));
        let fonts=w.query::<(&Text,&TextFont,&TextLayout,Option<&Node>,&ChildOf)>().iter(w).collect::<Vec<_>>();
        let expected_font=TextFont{font:Handle::<Font>::default().into(),..default()};
        for (text,font,layout,node,parent) in fonts {
            assert_eq!(format!("{:?}",font.font),format!("{:?}",expected_font.font),"font missing on {}",text.0);
            let wrapped=text.0=="本地化长正文\r\nSecond line";
            assert_eq!(layout.justify,Justify::Left);assert_eq!(layout.linebreak,if wrapped{LineBreak::WordOrCharacter}else{LineBreak::NoWrap});
            let own=node.unwrap();
            if own.width==Val::Auto {assert_eq!(w.get::<Node>(parent.parent()).unwrap().overflow,Overflow::clip());}
            else {assert_eq!(own.overflow,Overflow::clip());}
            assert_ne!(font.font_size,FontSize::Px(99.));
        }
        let mut last=painted(true,usize::MAX);let w=last.world_mut();
        assert_eq!(w.query::<&PortableActions>().iter(w).filter(|a|matches!(a.0,MailPaintAction::Select(_))).map(|a|a.0).collect::<Vec<_>>(),vec![MailPaintAction::Select(11)]);
        assert!(w.query::<(&PortableActions,Option<&Button>)>().iter(w).any(|(a,b)|a.0==MailPaintAction::Next&&b.is_none()));
        assert!(w.query::<&Text>().iter(w).any(|t|t.0=="2/2"));
    }
    #[test]
    fn full_bitmap_rows_and_alpha_bound_parcels_use_distinct_actual_layouts() {
        let mut app=App::new();app.init_resource::<Assets<Image>>().add_systems(Update,(layout_mail_row_icons,layout_original_item_images).chain());
        let mut image=Image::new_fill(bevy::render::render_resource::Extent3d{width:20,height:12,depth_or_array_layers:1},bevy::render::render_resource::TextureDimension::D2,&[0,0,0,0],bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,bevy::asset::RenderAssetUsages::default());
        for (x,y) in [(17,9),(18,9),(17,10),(18,10)] {image.data.as_mut().unwrap()[(y*20+x)*4+3]=255;}
        let handle=app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let row=app.world_mut().spawn((MailRowIcon,ImageNode::new(handle.clone()),Node::default())).id();
        let parcel=app.world_mut().spawn((OriginalItemImage{cell_width:35,cell_height:31},ImageNode::new(handle.clone()),Node::default())).id();
        app.update();let w=app.world();
        let row_node=w.get::<Node>(row).unwrap();let parcel_node=w.get::<Node>(parcel).unwrap();
        assert_eq!((row_node.left,row_node.top,row_node.width,row_node.height),(Val::Px(7.),Val::Px(10.),Val::Px(20.),Val::Px(12.)));
        assert_eq!((parcel_node.left,parcel_node.top,parcel_node.width,parcel_node.height),(Val::Px(16.),Val::Px(14.),Val::Px(20.),Val::Px(12.)));
        app.world_mut().resource_mut::<Assets<Image>>().remove(handle.id());app.update();
        assert_eq!(app.world().get::<Node>(row).unwrap().display,Display::None);assert_eq!(app.world().get::<Node>(parcel).unwrap().display,Display::None);
    }
    #[test]
    fn reader_body_and_missing_attachment_metadata_remain_bounded() {
        let mut app=App::new();app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default())).init_asset::<Image>();
        app.world_mut().run_system_once(|mut commands:Commands,assets:Res<AssetServer>|{
            let message=MailMessage{body:"one\\r\\ntwo".into(),gold:1280,items:vec![MailAttachment{image:None,count:2,..default()},MailAttachment{image:Some(u32::MAX),count:1,..default()}],..default()};
            commands.spawn(Node::default()).with_children(|p|paint_reader(p,Some(&assets),&message,MailReaderKind::Parcel,None,PortableActions));
        }).unwrap();let w=app.world_mut();
        assert_eq!(w.query::<&OriginalItemImage>().iter(w).count(),0);
        let (_,node,layout)=w.query::<(&Text,&Node,&TextLayout)>().iter(w).find(|(t,_,_)|t.0=="one\r\ntwo").unwrap();
        assert_eq!((node.left,node.top,node.width,node.height),(Val::Px(15.),Val::Px(98.),Val::Px(202.),Val::Px(165.)));
        assert_eq!(node.overflow,Overflow::clip());assert_eq!(layout.linebreak,LineBreak::WordOrCharacter);
        assert!(w.query::<&Text>().iter(w).any(|t|t.0=="1,280"));
        assert!(w.query::<(&PortableActions,&CrystalImageButton)>().iter(w).any(|(a,b)|a.0==MailPaintAction::ReaderClaim&&b.enabled));
    }
}
