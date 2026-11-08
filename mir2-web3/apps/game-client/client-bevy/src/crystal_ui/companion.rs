//! Native owner-only extension. All displayed growth comes from correlated server receipts.
use bevy::input::{keyboard::KeyboardInput, mouse::MouseWheel, ButtonState};
use bevy::prelude::*;
use mir2_ui_core::companion::{
    format_companion_micros, CompanionControls, CompanionEpisode, CompanionOperation as Op,
};
use serde_json::{json, Value};

use super::overlays::{NativePlayerUiSet, NativePlayerUiState};
use crate::native_shell::{NativeShellModel, NativeShellScreen};

const GOLD: Color = Color::srgb(0.82, 0.69, 0.44);
const INK: Color = Color::srgb(0.92, 0.87, 0.76);
const MUTED: Color = Color::srgb(0.65, 0.62, 0.54);
#[derive(Resource, Default)]
pub struct CompanionUiFont(pub Handle<Font>);
#[derive(Component)]
struct CompanionRoot;
#[derive(Component, Debug, Clone, PartialEq, Eq)]
enum Control {
    Open,
    Close,
    Refresh,
    PreviousResident,
    NextResident,
    Tab(u8),
    Subpage,
    PreviousPage,
    NextPage,
    Teach,
    Recall,
    Resume,
    Invite,
    Export,
    OpenExportFolder,
    Revoke(String),
    DeleteFeedback(String),
    DeleteEpisode(String),
    Clear,
    CancelConfirm,
    Confirm,
    EntrySource(u8),
    ReminderDown,
    ReminderUp,
    StartVisit,
    EndVisit,
    Choice(u8),
    ReasonInput,
    SaveChoice,
    TeachingCheck(bool),
    PurchaseIntent,
}
#[derive(Resource, Default)]
pub struct CompanionUi {
    pub invite_requested: bool,
    pub open_export_requested: bool,
    tab: u8,
    subpage: bool,
    page: usize,
    confirm: Option<Control>,
    entry_source: Option<u8>,
    reminder_count: u8,
    choice_reason: Option<u8>,
    reason_text: String,
    reason_focused: bool,
    focus: usize,
    focus_actions: Vec<Control>,
    revision: u64,
}
impl CompanionUi {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
pub struct Mir2CompanionUiPlugin;
impl Plugin for Mir2CompanionUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CompanionControls>()
            .init_resource::<CompanionUi>()
            .init_resource::<CompanionUiFont>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, fit_stage.before(NativePlayerUiSet::Read))
            .add_systems(
                Update,
                interact
                    .in_set(NativePlayerUiSet::Mutate)
                    .before(super::overlays::process_overlay_keyboard),
            )
            .add_systems(Update, render.in_set(NativePlayerUiSet::Read));
    }
}
fn spawn_root(mut commands: Commands) {
    commands.spawn((
        CompanionRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.),
            top: px(0.),
            width: px(1024.),
            height: px(768.),
            ..default()
        },
        GlobalZIndex(1_020),
        UiTransform::IDENTITY,
        bevy::ui::FocusPolicy::Pass,
    ));
}
fn fit_stage(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut roots: Query<&mut UiTransform, With<CompanionRoot>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let fit = super::metrics::CrystalStageTransform::fit(window.width(), window.height());
    for mut transform in &mut roots {
        transform.scale = Vec2::splat(fit.scale);
        transform.translation = bevy::ui::Val2::px(
            (window.width() - 1024.) * 0.5,
            (window.height() - 768.) * 0.5,
        );
    }
}
fn now_ms(time: &Time) -> u64 {
    (time.elapsed_secs_f64() * 1000.) as u64
}
fn interact(
    shell: Res<NativeShellModel>,
    mut player: ResMut<NativePlayerUiState>,
    mut controls: ResMut<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &Control), Changed<Interaction>>,
    mut typed: MessageReader<KeyboardInput>,
    mut wheel: MessageReader<MouseWheel>,
) {
    if shell.screen != NativeShellScreen::InGame {
        typed.clear();
        wheel.clear();
        return;
    }
    let open = player.companion_open();
    let mut actions: Vec<Control> = buttons
        .iter()
        .filter(|(state, _)| **state == Interaction::Pressed)
        .map(|(_, action)| action.clone())
        .collect();
    if open {
        if keys.just_pressed(KeyCode::Escape) {
            actions.push(if ui.confirm.is_some() {
                Control::CancelConfirm
            } else {
                Control::Close
            });
        }
        if keys.just_pressed(KeyCode::Tab) && !ui.focus_actions.is_empty() {
            let len = ui.focus_actions.len();
            ui.focus = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                (ui.focus + len - 1) % len
            } else {
                (ui.focus + 1) % len
            };
            ui.reason_focused = false;
            ui.revision += 1;
        }
        if ui.reason_focused {
            if keys.just_pressed(KeyCode::Backspace) {
                ui.reason_text.pop();
                ui.revision += 1;
            }
            for event in typed.read() {
                if event.state == ButtonState::Pressed {
                    if let Some(text) = &event.text {
                        for character in text.chars().filter(|c| !c.is_control()) {
                            if ui.reason_text.chars().count() < 300 {
                                ui.reason_text.push(character);
                                ui.revision += 1;
                            }
                        }
                    }
                }
            }
        } else {
            typed.clear();
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
                if let Some(action) = ui.focus_actions.get(ui.focus) {
                    actions.push(action.clone());
                }
            }
            if keys.just_pressed(KeyCode::PageDown) {
                actions.push(Control::NextPage);
            }
            if keys.just_pressed(KeyCode::PageUp) {
                actions.push(Control::PreviousPage);
            }
        }
        for event in wheel.read() {
            if ui.confirm.is_none() && event.y != 0. {
                actions.push(if event.y > 0. {
                    Control::PreviousPage
                } else {
                    Control::NextPage
                });
            }
        }
    } else {
        typed.clear();
        wheel.clear();
    }
    for action in actions {
        if !open && action != Control::Open {
            continue;
        }
        if ui.confirm.is_some()
            && !matches!(
                action,
                Control::Confirm | Control::CancelConfirm | Control::Close
            )
        {
            continue;
        }
        let now = now_ms(&time);
        ui.revision += 1;
        match action {
            Control::Open => {
                player.apply(mir2_ui_core::UiAction::OpenCompanion);
                ui.reason_focused = false;
                ui.focus = 0;
                controls.begin(Op::List, json!({}), now);
            }
            Control::Close => {
                player.apply(mir2_ui_core::UiAction::ClosePanel);
                ui.reason_text.clear();
                ui.reason_focused = false;
                ui.confirm = None;
            }
            Control::Refresh => {
                if !controls.busy() {
                    controls.begin(Op::List, json!({}), now);
                }
            }
            Control::PreviousResident | Control::NextResident => {
                if !controls.busy() && !controls.enrollments.is_empty() {
                    let len = controls.enrollments.len();
                    let current = controls
                        .enrollments
                        .iter()
                        .position(|e| Some(&e.resident_id) == controls.selected.as_ref())
                        .unwrap_or(0);
                    let index = if action == Control::NextResident {
                        (current + 1) % len
                    } else {
                        (current + len - 1) % len
                    };
                    let resident = controls.enrollments[index].resident_id.clone();
                    controls.choose(&resident, now);
                    ui.page = 0;
                    ui.reason_text.clear();
                    ui.choice_reason = None;
                }
            }
            Control::Tab(tab) => {
                ui.tab = tab;
                ui.page = 0;
                ui.subpage = false;
                ui.reason_focused = false;
                ui.focus = 0;
            }
            Control::Subpage => {
                ui.subpage = !ui.subpage;
                ui.page = 0;
                ui.reason_focused = false;
            }
            Control::PreviousPage => {
                ui.page = ui.page.saturating_sub(1);
            }
            Control::NextPage => {
                let count = controls.view.as_ref().map_or(0, |view| {
                    if ui.tab == 1 {
                        view.feedback.len()
                    } else {
                        view.episodes.len()
                    }
                });
                if (ui.page + 1) * 4 < count {
                    ui.page += 1;
                } else if (ui.tab == 1 || ui.tab == 2) && !ui.subpage && !controls.busy() {
                    if let Some(cursor) = controls
                        .view
                        .as_ref()
                        .and_then(|view| view.next_cursor.clone())
                    {
                        controls.begin(Op::View, json!({"cursor":cursor,"limit":20}), now);
                        ui.page = 0;
                    }
                }
            }
            Control::Teach => {
                if let Some(payload) = controls.version_payload() {
                    controls.begin(Op::Teach, payload, now);
                }
            }
            Control::Recall => {
                controls.begin(Op::Recall, json!({}), now);
            }
            Control::Resume => {
                controls.begin(Op::Resume, json!({}), now);
            }
            Control::Invite => {
                ui.invite_requested = true;
            }
            Control::Export => {
                controls.begin(Op::ExportBegin, json!({}), now);
            }
            Control::OpenExportFolder => {
                ui.open_export_requested = true;
            }
            Control::Revoke(_)
            | Control::DeleteFeedback(_)
            | Control::DeleteEpisode(_)
            | Control::Clear => {
                ui.confirm = Some(action);
                ui.focus = 0;
            }
            Control::CancelConfirm => {
                ui.confirm = None;
                ui.focus = 0;
            }
            Control::Confirm => {
                if let Some(confirm) = ui.confirm.take() {
                    if let Some(mut payload) = controls.version_payload() {
                        let operation = match confirm {
                            Control::Revoke(id) => {
                                payload["feedbackId"] = json!(id);
                                Op::RevokeFeedback
                            }
                            Control::DeleteFeedback(id) => {
                                payload["feedbackId"] = json!(id);
                                Op::DeleteFeedback
                            }
                            Control::DeleteEpisode(id) => {
                                payload["episodeId"] = json!(id);
                                Op::DeleteEpisode
                            }
                            Control::Clear => {
                                payload["confirmArchiveId"] =
                                    json!(controls.view.as_ref().unwrap().archive.archive_id);
                                Op::ClearGrowth
                            }
                            _ => continue,
                        };
                        controls.begin(operation, payload, now);
                        ui.page = 0;
                    }
                }
            }
            Control::EntrySource(source) => ui.entry_source = Some(source),
            Control::ReminderDown => ui.reminder_count = ui.reminder_count.saturating_sub(1),
            Control::ReminderUp => ui.reminder_count = ui.reminder_count.saturating_add(1).min(100),
            Control::StartVisit => {
                let visit_id = controls.next_visit_id();
                let source = ["unprompted", "reminder", "scheduled", "unknown"]
                    [usize::from(ui.entry_source.unwrap_or(3)).min(3)];
                controls.begin(Op::TrialEvent,json!({"kind":"visit","visitId":visit_id,"entrySource":source,"reminderCount":ui.reminder_count}),now);
            }
            Control::EndVisit
            | Control::SaveChoice
            | Control::TeachingCheck(_)
            | Control::PurchaseIntent => {
                let Some(visit_id) = controls
                    .view
                    .as_ref()
                    .and_then(|v| v.trial.get("activeVisitId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    continue;
                };
                let mut payload = json!({"visitId":visit_id});
                match action {
                    Control::EndVisit => payload["kind"] = json!("visitEnd"),
                    Control::SaveChoice => {
                        let Some(reason) = ui.choice_reason else {
                            continue;
                        };
                        payload["kind"] = json!("choice");
                        payload["reason"] = json!(
                            [
                                "shared-history",
                                "stronger",
                                "setup-ease",
                                "curiosity",
                                "other"
                            ][usize::from(reason).min(4)]
                        );
                    }
                    Control::TeachingCheck(understood) => {
                        payload["kind"] = json!("teachingCheck");
                        payload["understood"] = json!(understood);
                    }
                    Control::PurchaseIntent => {
                        let Some(offer_id) = controls
                            .view
                            .as_ref()
                            .and_then(|v| v.trial.pointer("/offer/offerId"))
                            .and_then(Value::as_str)
                        else {
                            continue;
                        };
                        payload["kind"] = json!("purchaseIntent");
                        payload["offerId"] = json!(offer_id);
                    }
                    _ => continue,
                }
                if matches!(action, Control::SaveChoice | Control::TeachingCheck(_))
                    && !ui.reason_text.trim().is_empty()
                {
                    payload["reasonText"] = json!(ui.reason_text.trim());
                }
                controls.begin(Op::TrialEvent, payload, now);
            }
            Control::Choice(reason) => ui.choice_reason = Some(reason),
            Control::ReasonInput => ui.reason_focused = true,
        }
    }
}

fn text(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(x),
            top: px(y),
            width: px(width),
            height: px(height),
            overflow: Overflow::clip(),
            ..default()
        },
        Text::new(label.to_owned()),
        TextFont {
            font: FontSource::Handle(font.clone()),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    ));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    ui: &mut CompanionUi,
    label: &str,
    x: f32,
    y: f32,
    width: f32,
    action: Control,
    enabled: bool,
) {
    let focus = ui.focus_actions.len() == ui.focus;
    if enabled {
        ui.focus_actions.push(action.clone());
    }
    let mut entity = parent.spawn((
        Name::new(label.to_owned()),
        Node {
            position_type: PositionType::Absolute,
            left: px(x),
            top: px(y),
            width: px(width),
            height: px(29.),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1.)),
            ..default()
        },
        BorderColor::all(if focus && enabled {
            Color::srgb(1., 0.87, 0.45)
        } else {
            GOLD
        }),
        BackgroundColor(if enabled {
            Color::srgb(0.16, 0.11, 0.06)
        } else {
            Color::srgb(0.08, 0.07, 0.06)
        }),
    ));
    if enabled {
        entity.insert((Button, action));
    }
    entity.with_children(|p| {
        p.spawn((
            Text::new(label.to_owned()),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(13.),
                ..default()
            },
            TextColor(if enabled { INK } else { MUTED }),
        ));
    });
}

fn render(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    player: Res<NativePlayerUiState>,
    controls: Res<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
    font: Res<CompanionUiFont>,
    roots: Query<Entity, With<CompanionRoot>>,
    social: Option<Res<crate::social::SocialModel>>,
    mut previous: Local<Option<(u64, u64, bool, bool)>>,
) {
    let in_game = shell.screen == NativeShellScreen::InGame;
    let open = in_game && player.companion_open();
    let member = controls.view.as_ref().is_some_and(|view| {
        social.as_ref().is_some_and(|s| {
            s.group
                .members
                .iter()
                .any(|m| m.name == view.archive.character_name)
        })
    });
    let fingerprint = (controls.revision, ui.revision, open, member);
    if *previous == Some(fingerprint) && !shell.is_changed() && !font.is_changed() {
        return;
    }
    *previous = Some(fingerprint);
    let Some(root) = roots.iter().next() else {
        return;
    };
    commands.entity(root).despawn_children();
    ui.focus_actions.clear();
    if !in_game {
        return;
    }
    let font = &font.0;
    commands.entity(root).with_children(|root|{
        if !open{
            button(root,font,&mut ui,"伙伴 Companion",872.,174.,140.,Control::Open,true);return;
        }
        root.spawn((Node{position_type:PositionType::Absolute,left:px(0.),top:px(0.),width:px(1024.),height:px(768.),..default()},
            BackgroundColor(Color::srgba(0.,0.,0.,0.54)),bevy::ui::FocusPolicy::Block));
        root.spawn((Name::new("伙伴 Companion window"),Node{position_type:PositionType::Absolute,left:px(140.),top:px(44.),width:px(744.),height:px(644.),
            border:UiRect::all(px(2.)),overflow:Overflow::clip(),..default()},BackgroundColor(Color::srgb(0.055,0.043,0.029)),BorderColor::all(GOLD)))
            .with_children(|panel|{
                text(panel,font,"伙伴 · 身份与成长",18.,12.,600.,30.,20.,GOLD);
                button(panel,font,&mut ui,"关闭 Esc",638.,10.,88.,Control::Close,true);
                let busy=controls.busy();let mutable=controls.view.is_some()&&!busy&&!controls.uncertain;
                if let Some(entry)=controls.selected_enrollment(){
                    text(panel,font,&format!("{} · {} · 已授权 {}/{}",entry.display_name,if entry.online{"在线"}else{"离线"},
                        controls.enrollments.iter().position(|e|e.resident_id==entry.resident_id).unwrap_or(0)+1,controls.enrollments.len()),18.,50.,540.,25.,15.,INK);
                }else{text(panel,font,"仅列出当前登录角色有权管理的伙伴",18.,50.,580.,25.,14.,MUTED);}
                button(panel,font,&mut ui,"上一位",558.,47.,78.,Control::PreviousResident,!busy&&controls.enrollments.len()>1);
                button(panel,font,&mut ui,"下一位",646.,47.,78.,Control::NextResident,!busy&&controls.enrollments.len()>1);
                button(panel,font,&mut ui,"刷新记录",18.,87.,112.,Control::Refresh,!busy);
                button(panel,font,&mut ui,if member{"已在队伍"}else{"邀请组队"},142.,87.,112.,Control::Invite,mutable&&!member);
                button(panel,font,&mut ui,"召回",266.,87.,92.,Control::Recall,mutable);
                button(panel,font,&mut ui,"恢复行动",370.,87.,112.,Control::Resume,mutable);
                text(panel,font,"组队以服务器名单为准",496.,91.,225.,26.,12.,MUTED);
                for(index,label)in["培养与体验","记忆管理","行动与成本"].iter().enumerate(){
                    button(panel,font,&mut ui,label,18.+index as f32*238.,132.,224.,Control::Tab(index as u8),true);
                }
                if let Some(confirm)=ui.confirm.clone(){
                    // Confirmation stores only opaque IDs, never another copy of private text.
                    text(panel,font,"请确认这次操作",24.,205.,680.,32.,19.,GOLD);
                    let message=match confirm{Control::Clear=>"清除全部教学和经历，并重新计算习惯。保留伙伴身份及服务器游戏角色。此操作不能撤销。",
                        Control::Revoke(_)=>"撤销这条教学并重新计算习惯。记录仍可查看；撤销后不会自动恢复。",
                        Control::DeleteEpisode(_)=>"删除这条经历及文字。由它产生的教学也将失效。此操作不能撤销。",
                        _=>"删除这条教学及文字，并重新计算习惯。此操作不能撤销。"};
                    text(panel,font,message,24.,252.,680.,110.,16.,INK);
                    ui.focus_actions.clear();
                    button(panel,font,&mut ui,"取消",130.,390.,210.,Control::CancelConfirm,true);
                    button(panel,font,&mut ui,"确认执行",370.,390.,210.,Control::Confirm,mutable);
                }else if let Some(view)=&controls.view{
                    match ui.tab{
                        0 if !ui.subpage=>{
                            text(panel,font,&format!("{} · 培养 v{} · 成长修订 {}",view.archive.display_name,view.profile.policy_version,view.growth_revision),22.,181.,690.,28.,17.,GOLD);
                            let habits=&view.profile.party;
                            text(panel,font,&format!("组队先支援：{}\n战斗后再拾取：{}\n离队先询问：{}",yes(habits.assist_before_loot),yes(habits.loot_only_after_combat),yes(habits.ask_before_leaving)),22.,222.,690.,92.,16.,INK);
                            button(panel,font,&mut ui,"教它：组队先支援，战后再拾取，离队先问",22.,326.,690.,Control::Teach,mutable);
                            text(panel,font,"这是明确的习惯教学。保存成功后显示新版本；伙伴实际行动仍受血量、距离和游戏规则约束。",22.,371.,690.,62.,14.,MUTED);
                            text(panel,font,&format!("固定身份：{}\n世界：{}\n{}",view.archive.archive_id,view.archive.realm_id,view.archive.base_setting),22.,435.,690.,78.,13.,MUTED);
                            button(panel,font,&mut ui,"打开体验自述与选择原因",22.,513.,690.,Control::Subpage,!busy);
                        }
                        0=>render_trial(panel,font,&mut ui,view,mutable),
                        1=>{
                            text(panel,font,&format!("教学记录 · 当前读取 {} 条 · 第 {} 页",view.feedback.len(),ui.page+1),22.,180.,690.,28.,15.,GOLD);
                            for(index,row)in view.feedback.iter().skip(ui.page*4).take(4).enumerate(){let y=218.+index as f32*65.;
                                text(panel,font,&format!("{} · {}",match row.status.as_str(){"active"=>"生效中","revoked"=>"已撤销","deleted"=>"已删除",_=>"记录"},short_time(row.recorded_at)),22.,y,470.,20.,12.,MUTED);
                                text(panel,font,if row.summary.is_empty(){"文字已删除"}else{&row.summary},22.,y+20.,462.,42.,14.,INK);
                                button(panel,font,&mut ui,"撤销",500.,y+10.,95.,Control::Revoke(row.feedback_id.clone()),mutable&&row.status=="active");
                                button(panel,font,&mut ui,"删除文字",605.,y+10.,108.,Control::DeleteFeedback(row.feedback_id.clone()),mutable&&row.status!="deleted");
                            }
                            if view.feedback.is_empty(){text(panel,font,"还没有教学记录。",22.,245.,690.,35.,16.,MUTED);}
                            page_buttons(panel,font,&mut ui,view.feedback.len(),view.next_cursor.is_some(),busy);
                            button(panel,font,&mut ui,"清除全部成长…",22.,513.,220.,Control::Clear,mutable);
                            text(panel,font,"完整历史可导出；不受购买服务限制。",258.,518.,450.,26.,13.,MUTED);
                        }
                        2 if !ui.subpage=>{
                            text(panel,font,"行动记录（选择只是计划，完成须服务器确认）",22.,180.,690.,28.,14.,GOLD);
                            for(index,row)in view.episodes.iter().skip(ui.page*4).take(4).enumerate(){let y=215.+index as f32*64.;
                                text(panel,font,&format!("{} · {}",short_time(row.recorded_at),episode_heading(row)),22.,y,560.,22.,12.,MUTED);
                                text(panel,font,&episode_detail(row),22.,y+22.,560.,40.,14.,INK);
                                button(panel,font,&mut ui,"删除",610.,y+10.,104.,Control::DeleteEpisode(row.episode_id.clone()),mutable);
                            }
                            if view.episodes.is_empty(){text(panel,font,"还没有已记录的行动经历。",22.,245.,690.,35.,16.,MUTED);}
                            page_buttons(panel,font,&mut ui,view.episodes.len(),view.next_cursor.is_some(),busy);
                            button(panel,font,&mut ui,"查看服务状态、费用与可选服务",22.,513.,690.,Control::Subpage,!busy);
                        }
                        _=>render_costs(panel,font,&mut ui,view,mutable),
                    }
                }else{text(panel,font,if busy{"正在读取已授权的伙伴资料…"}else{"请刷新伙伴列表。没有授权时不会显示私人记忆。"},22.,215.,690.,90.,16.,MUTED);}
                text(panel,font,&controls.notice,20.,556.,700.,45.,13.,GOLD);
                if ui.confirm.is_none(){
                    button(panel,font,&mut ui,"导出角色与完整成长",20.,602.,272.,Control::Export,controls.view.is_some()&&!busy);
                    button(panel,font,&mut ui,"打开导出目录",310.,602.,164.,Control::OpenExportFolder,controls.export_path.is_some());
                    text(panel,font,"查看与导出无需购买。\nTab 选控件，Enter 确认。",490.,601.,236.,38.,12.,MUTED);
                }
            });
    });
}
fn yes(value: bool) -> &'static str {
    if value {
        "是"
    } else {
        "否"
    }
}
fn selection_label(label: &str, selected: bool) -> String {
    format!("{}{label}", if selected { "已选：" } else { "" })
}

fn episode_heading(episode: &CompanionEpisode) -> String {
    let kind = match episode.kind.as_str() {
        "decision" => "行动计划",
        "party" => "组队记录",
        "item-use" => "用药记录",
        "inventory-change" => "背包变化",
        "feedback" => "教学记录",
        _ => "未分类记录",
    };
    let action = match episode.action.as_str() {
        "wait" => "等候",
        "attack" => "攻击怪物",
        "follow" => "跟随队长",
        "retreat" => "撤离",
        "loot" | "pickup" => "拾取掉落",
        "usepotion" | "use-potion" => "使用血药",
        "acceptinvite" | "accept-invite" => "接受组队邀请",
        "requestapproval" | "request-approval" => "等待离队许可",
        "teach-support-before-loot" => "组队习惯教学",
        "" => "未记录行动",
        _ => "未分类行动",
    };
    // Acceptance of a request is not completion of its in-world effect.
    let outcome = match (episode.kind.as_str(), episode.outcome.as_str()) {
        (_, "selected") => "已选择，尚未确认完成",
        ("feedback", "accepted") => "教学已保存",
        (_, "accepted") => "请求已获接受",
        (_, "rejected") => "请求未获接受",
        (_, "uncertain") => "结果尚未确认",
        (_, "observed") => "已观察到变化",
        (_, "") => "未记录结果",
        _ => "结果暂未分类",
    };
    format!("{kind} · {action} · {outcome}")
}

fn episode_detail(episode: &CompanionEpisode) -> String {
    // Owner-written teaching text is content, not a machine reason code.
    if episode.kind == "feedback" {
        return if episode.summary.is_empty() {
            "教学内容可在“记忆管理”中查看。".into()
        } else {
            episode.summary.clone()
        };
    }
    let reason = if episode.kind == "decision" {
        match episode.summary.as_str() {
            "previous-action-in-flight" => "上一项行动仍在处理中，先等候。",
            "clock-unknown" => "暂时无法确认行动时机，先等候。",
            "invalid-party-distance-config" => "跟随范围设置无效，暂缓行动。",
            "world-not-ready" => "还在等待游戏状态，暂不行动。",
            "dead" => "角色已经倒下，等待复活。",
            "health-unknown" => "还不能确认自身血量，先等候。",
            "map-not-authorized" => "当前地图不在允许活动的范围内。",
            "retreat-destination-unknown" => "还没有确认撤离方向，先等候。",
            "health-low-use-owned-potion" => "血量偏低，准备使用随身血药。",
            "owner-recall" => "主人请求召回，准备返回。",
            "unsafe-withdraw" => "当前风险较高，准备撤离。",
            "configured-leader-invited" => "收到指定队长的邀请，准备接受。",
            "leader-not-visible" => "暂时无法确认队长的位置，先等候。",
            "collect-observed-allowed-drop" => "看见可拾取的掉落，准备收集。",
            "assist-confirmed-party-engagement" => "观察到队友交战，准备优先支援。",
            "defend-confirmed-engagement" => "观察到自身受到威胁，准备自卫。",
            "loot-would-leave-party-leash" => "拾取会离队太远，先等主人同意。",
            "stay-with-party-leader" => "与队长拉开了距离，准备跟上。",
            "party-combat-still-active" => "观察到队伍仍在交战，先保持警戒。",
            "near-party-leader" => "已经在队长附近，先等候。",
            "no-authorized-party-activity" => "暂无可按当前约定进行的活动，先等候。",
            _ => "",
        }
    } else {
        ""
    };
    if !reason.is_empty() {
        return format!("原因：{reason}");
    }
    match (episode.kind.as_str(), episode.summary.as_str()) {
        ("party", "Invitation acceptance result unknown; the same invitation is not replayed.") => {
            "接受邀请的结果尚未确认，不会自动重复接受同一邀请。".into()
        }
        (
            "item-use",
            "Item use result unknown; the exact item is quarantined until observed reconciliation.",
        ) => "用药结果尚未确认；确认物品变化前，不会重复使用这份药。".into(),
        ("item-use", "Potion consumed; timed recovery remains server-owned.") => {
            "已消耗血药；恢复需要时间，实际血量以游戏状态为准。".into()
        }
        ("item-use", "Item use rejected.") => "这次用药未获接受。".into(),
        (
            "inventory-change",
            "Tile pickup result unknown; this observed drop is not blindly retried.",
        ) => "拾取结果尚未确认，不会自动重复拾取同一掉落。".into(),
        ("inventory-change", "Owned inventory or gold changed after ordinary tile pickup.") => {
            "发出拾取请求后，观察到自己的背包或金币发生变化。".into()
        }
        _ if episode.kind == "decision" => {
            "原因暂未分类；这只是行动计划，完成情况尚未确认。".into()
        }
        _ => "未分类记录；实际结果以服务器确认为准。".into(),
    }
}
fn short_time(at: Option<u64>) -> String {
    let Some(at) = at else {
        return "时间未记录".into();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if at > now + 60_000 {
        return "时间待校准".into();
    }
    let minutes = now.saturating_sub(at) / 60_000;
    if minutes < 1 {
        "刚刚".into()
    } else if minutes < 60 {
        format!("{minutes} 分钟前")
    } else if minutes < 1440 {
        format!("{} 小时前", minutes / 60)
    } else {
        format!("{} 天前", minutes / 1440)
    }
}
fn page_buttons(
    p: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    ui: &mut CompanionUi,
    count: usize,
    older: bool,
    busy: bool,
) {
    let previous = ui.page > 0;
    let next = (ui.page + 1) * 4 < count;
    button(
        p,
        f,
        ui,
        "上一页",
        360.,
        482.,
        166.,
        Control::PreviousPage,
        previous,
    );
    button(
        p,
        f,
        ui,
        if next {
            "下一页"
        } else {
            "读取更早记录"
        },
        540.,
        482.,
        174.,
        Control::NextPage,
        !busy && (next || older),
    );
}
fn render_trial(
    p: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    ui: &mut CompanionUi,
    v: &mir2_ui_core::companion::CompanionOwnerView,
    enabled: bool,
) {
    let active = v
        .trial
        .get("activeVisitId")
        .and_then(Value::as_str)
        .is_some();
    let configured = v
        .trial
        .get("experimentId")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    text(
        p,
        f,
        "本参与者试用累计 · 体验自述（不证明付款或完成）",
        22.,
        180.,
        690.,
        26.,
        14.,
        GOLD,
    );
    for (index, label) in ["自己想来", "收到提醒", "约好时间", "不确定"]
        .iter()
        .enumerate()
    {
        let label = selection_label(label, ui.entry_source == Some(index as u8));
        button(
            p,
            f,
            ui,
            &label,
            22. + index as f32 * 174.,
            216.,
            162.,
            Control::EntrySource(index as u8),
            enabled && !active,
        );
    }
    text(
        p,
        f,
        &format!("收到提醒次数（自述）：{}", ui.reminder_count),
        22.,
        256.,
        400.,
        26.,
        14.,
        INK,
    );
    let decrease = enabled && !active && ui.reminder_count > 0;
    let increase = enabled && !active && ui.reminder_count < 100;
    button(
        p,
        f,
        ui,
        "−",
        540.,
        252.,
        78.,
        Control::ReminderDown,
        decrease,
    );
    button(
        p,
        f,
        ui,
        "+",
        634.,
        252.,
        78.,
        Control::ReminderUp,
        increase,
    );
    button(
        p,
        f,
        ui,
        if active {
            "结束本次体验"
        } else {
            "开始本次体验"
        },
        22.,
        291.,
        690.,
        if active {
            Control::EndVisit
        } else {
            Control::StartVisit
        },
        enabled && configured,
    );
    text(
        p,
        f,
        "选择这位伙伴的原因（自述）",
        22.,
        332.,
        690.,
        23.,
        14.,
        INK,
    );
    for (index, label) in ["共同经历", "战力更强", "省去设置", "好奇", "其他"]
        .iter()
        .enumerate()
    {
        let label = selection_label(label, ui.choice_reason == Some(index as u8));
        button(
            p,
            f,
            ui,
            &label,
            22. + index as f32 * 140.,
            360.,
            130.,
            Control::Choice(index as u8),
            enabled && active,
        );
    }
    let draft = if ui.reason_text.is_empty() {
        "可选单行说明（最多300字；不填凭据）".into()
    } else {
        format!(
            "{}{}",
            ui.reason_text,
            if ui.reason_focused { " ▏" } else { "" }
        )
    };
    button(
        p,
        f,
        ui,
        &draft,
        22.,
        401.,
        690.,
        Control::ReasonInput,
        enabled && active,
    );
    let choice = enabled && active && ui.choice_reason.is_some();
    button(
        p,
        f,
        ui,
        "记录选择原因",
        22.,
        443.,
        690.,
        Control::SaveChoice,
        choice,
    );
    button(
        p,
        f,
        ui,
        "教学反馈：我理解了",
        22.,
        484.,
        335.,
        Control::TeachingCheck(true),
        enabled && active,
    );
    button(
        p,
        f,
        ui,
        "教学反馈：还没看懂",
        377.,
        484.,
        335.,
        Control::TeachingCheck(false),
        enabled && active,
    );
    button(
        p,
        f,
        ui,
        "返回培养习惯",
        22.,
        521.,
        690.,
        Control::Subpage,
        true,
    );
}
fn render_costs(
    p: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    ui: &mut CompanionUi,
    v: &mir2_ui_core::companion::CompanionOwnerView,
    enabled: bool,
) {
    let runtime = &v.runtime;
    let trial = &v.trial;
    let usage = &trial["usage"];
    let costs = &trial["costs"];
    let mode = match runtime.get("modelEnabled").and_then(Value::as_bool) {
        Some(false) => "规则执行器（未启用大模型）",
        Some(true) => "已启用模型服务",
        None => "运行服务未说明",
    };
    text(
        p,
        f,
        "本参与者试用累计（跨已授权伙伴）",
        22.,
        180.,
        690.,
        24.,
        16.,
        GOLD,
    );
    let value = |key: &str| {
        usage
            .get(key)
            .and_then(Value::as_u64)
            .map(|x| x.to_string())
            .unwrap_or_else(|| "未计量".into())
    };
    text(
        p,
        f,
        &format!(
            "运行秒数：{}　模型调用：{}　已确认冒险：{}",
            value("runtimeSeconds"),
            value("modelCalls"),
            value("confirmedAdventureCount")
        ),
        22.,
        212.,
        690.,
        32.,
        14.,
        INK,
    );
    let currency = costs.get("currency").and_then(Value::as_str);
    let money =
        |key: &str| format_companion_micros(costs.get(key).and_then(Value::as_str), currency);
    let contribution = if costs.get("complete").and_then(Value::as_bool) == Some(true) {
        money("contributionMicros")
    } else {
        "成本未齐，暂不可计算".into()
    };
    text(
        p,
        f,
        &format!(
            "模型金额记录：{}　托管：{}\n支付费用：{}　分成：{}\n贡献：{}　人工支持秒数：{}",
            money("modelMicros"),
            money("hostingMicros"),
            money("paymentMicros"),
            money("revenueShareMicros"),
            contribution,
            costs
                .get("supportSeconds")
                .and_then(Value::as_u64)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "未计量".into())
        ),
        22.,
        255.,
        690.,
        94.,
        14.,
        INK,
    );
    text(
        p,
        f,
        &format!("当前伙伴：{mode}；缺失成本未计量，费用记录不等于付款。"),
        22.,
        350.,
        690.,
        24.,
        12.,
        MUTED,
    );
    let offer = &trial["offer"];
    let price = format_companion_micros(
        offer.get("priceMicros").and_then(Value::as_str),
        offer.get("currency").and_then(Value::as_str),
    );
    let offered = offer.get("offerId").and_then(Value::as_str).is_some() && price != "未计量";
    let offer_label = if offered {
        format!(
            "可选未来服务：{price} · {}天 / {}次冒险",
            offer
                .get("durationDays")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            offer
                .get("adventureCount")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        )
    } else {
        "尚未配置服务价格；购买意向入口未启用。".into()
    };
    text(p, f, &offer_label, 22., 386., 690., 28., 14., GOLD);
    text(
        p,
        f,
        offer
            .get("paymentInstructions")
            .and_then(Value::as_str)
            .unwrap_or("不购买仍保留身份、成长、记忆管理与导出入口。"),
        22.,
        420.,
        690.,
        52.,
        13.,
        INK,
    );
    button(
        p,
        f,
        ui,
        "申请此服务（仅记录意向，不会扣款）",
        22.,
        477.,
        690.,
        Control::PurchaseIntent,
        enabled && offered && trial.get("activeVisitId").and_then(Value::as_str).is_some(),
    );
    button(
        p,
        f,
        ui,
        "返回行动记录",
        22.,
        518.,
        690.,
        Control::Subpage,
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn episode(kind: &str, action: &str, outcome: &str, summary: &str) -> CompanionEpisode {
        CompanionEpisode {
            episode_id: "episode-1".into(),
            recorded_at: None,
            kind: kind.into(),
            summary: summary.into(),
            action: action.into(),
            outcome: outcome.into(),
            policy_version: Some(2),
        }
    }
    fn app(screen: NativeShellScreen) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(NativeShellModel {
                screen,
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .configure_sets(
                Update,
                NativePlayerUiSet::Mutate.before(NativePlayerUiSet::Read),
            )
            .add_plugins(Mir2CompanionUiPlugin);
        app.update();
        app
    }
    fn press(app: &mut App, action: Control) {
        let world = app.world_mut();
        let entity = world
            .query::<(Entity, &Control)>()
            .iter(world)
            .find(|(_, control)| **control == action)
            .map(|(e, _)| e)
            .expect("enabled real button");
        world.entity_mut(entity).insert(Interaction::Pressed);
        app.update();
    }
    fn authorized_view(app: &mut App) {
        let mut controls = app.world_mut().resource_mut::<CompanionControls>();
        controls.enrollments = vec![mir2_ui_core::companion::CompanionEnrollment {
            resident_id: "alo-1".into(),
            archive_id: "archive-1".into(),
            display_name: "阿洛".into(),
            online: true,
        }];
        controls.selected = Some("alo-1".into());
        controls.view=Some(serde_json::from_value(json!({"archive":{"archiveId":"archive-1","residentId":"alo-1","displayName":"阿洛","characterName":"AloAI","realmId":"trial","worldEpoch":"world-1"},
            "profile":{"policyVersion":2,"style":"collection","party":{"assistBeforeLoot":true,"lootOnlyAfterCombat":true,"askBeforeLeaving":true}},"growthRevision":2,
            "feedback":[{"feedbackId":"feedback-opaque","status":"active","summary":"owned memory"}],"episodes":[],"trial":{"experimentId":"trial-1"}})).unwrap());
        controls.revision += 1;
        drop(controls);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .apply(mir2_ui_core::UiAction::OpenCompanion);
    }
    #[test]
    fn native_entry_exists_only_in_game_and_open_emits_owner_list() {
        let mut app = app(NativeShellScreen::Login);
        assert_eq!(
            app.world_mut()
                .query::<&Control>()
                .iter(app.world())
                .filter(|c| **c == Control::Open)
                .count(),
            0
        );
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
        app.update();
        press(&mut app, Control::Open);
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .companion_open());
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .blocks_gameplay_keys());
        let requests = app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::List);
        assert!(requests[0].resident_id.is_none());
    }
    #[test]
    fn without_authorized_view_no_teach_delete_clear_or_export_can_be_clicked() {
        let mut app = app(NativeShellScreen::InGame);
        press(&mut app, Control::Open);
        let world = app.world_mut();
        let actions: Vec<_> = world.query::<&Control>().iter(world).cloned().collect();
        assert!(!actions.contains(&Control::Teach));
        assert!(!actions.contains(&Control::Clear));
        assert!(!actions.contains(&Control::Export));
        assert!(!actions.iter().any(|action| matches!(
            action,
            Control::DeleteFeedback(_) | Control::DeleteEpisode(_)
        )));
    }
    #[test]
    fn escape_closes_and_clears_private_reason_draft() {
        let mut app = app(NativeShellScreen::InGame);
        press(&mut app, Control::Open);
        app.world_mut().resource_mut::<CompanionUi>().reason_text = "private reason".into();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .companion_open());
        assert!(app.world().resource::<CompanionUi>().reason_text.is_empty());
    }
    #[test]
    fn deletion_waits_for_explicit_confirmation_and_sends_the_selected_opaque_id() {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.world_mut().resource_mut::<CompanionUi>().tab = 1;
        app.update();
        press(&mut app, Control::DeleteFeedback("feedback-opaque".into()));
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
        press(&mut app, Control::CancelConfirm);
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
        press(&mut app, Control::DeleteFeedback("feedback-opaque".into()));
        press(&mut app, Control::Confirm);
        let requests = app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::DeleteFeedback);
        assert_eq!(
            requests[0].payload,
            json!({"feedbackId":"feedback-opaque","expectedPolicyVersion":2})
        );
        assert_eq!(
            app.world()
                .resource::<CompanionControls>()
                .view
                .as_ref()
                .unwrap()
                .feedback[0]
                .summary,
            "owned memory"
        );
    }
    #[test]
    fn starting_without_an_entry_source_choice_records_unknown_not_unprompted() {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.world_mut().resource_mut::<CompanionUi>().subpage = true;
        app.update();
        press(&mut app, Control::StartVisit);
        let requests = app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::TrialEvent);
        assert_eq!(requests[0].payload["entrySource"], "unknown");
        assert_eq!(requests[0].payload["kind"], "visit");
    }
    #[test]
    fn native_action_rows_show_a_readable_plan_without_claiming_completion() {
        let row = episode("decision", "wait", "selected", "near-party-leader");
        assert_eq!(
            episode_heading(&row),
            "行动计划 · 等候 · 已选择，尚未确认完成"
        );
        assert_eq!(episode_detail(&row), "原因：已经在队长附近，先等候。");
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().episodes.push(row);
            controls.revision += 1;
        }
        app.world_mut().resource_mut::<CompanionUi>().tab = 2;
        app.update();
        let world = app.world_mut();
        let lines: Vec<_> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(lines
            .iter()
            .any(|line| line.contains("行动计划 · 等候 · 已选择，尚未确认完成")));
        assert!(lines
            .iter()
            .any(|line| line == "原因：已经在队长附近，先等候。"));
        assert!(!lines
            .iter()
            .any(|line| line.contains("near-party-leader") || line.contains("selected")));
    }
    #[test]
    fn accepted_observed_and_uncertain_receipts_keep_their_actual_evidence_limits() {
        let potion = episode(
            "item-use",
            "use-potion",
            "accepted",
            "Potion consumed; timed recovery remains server-owned.",
        );
        assert_eq!(
            episode_heading(&potion),
            "用药记录 · 使用血药 · 请求已获接受"
        );
        assert_eq!(
            episode_detail(&potion),
            "已消耗血药；恢复需要时间，实际血量以游戏状态为准。"
        );
        let pickup = episode(
            "inventory-change",
            "pickup",
            "observed",
            "Owned inventory or gold changed after ordinary tile pickup.",
        );
        assert_eq!(
            episode_heading(&pickup),
            "背包变化 · 拾取掉落 · 已观察到变化"
        );
        assert!(episode_detail(&pickup).contains("观察到自己的背包或金币发生变化"));
        let uncertain = episode(
            "inventory-change",
            "pickup",
            "uncertain",
            "Tile pickup result unknown; this observed drop is not blindly retried.",
        );
        assert!(episode_heading(&uncertain).ends_with("结果尚未确认"));
        assert!(episode_detail(&uncertain).contains("不会自动重复拾取"));
        for row in [&potion, &pickup, &uncertain] {
            assert!(!episode_heading(row).contains("已完成"));
        }
    }
    #[test]
    fn unknown_codes_do_not_turn_into_success_and_owner_teaching_text_is_preserved() {
        let unknown = episode(
            "future-event",
            "future-action",
            "completed",
            "future-success-code",
        );
        assert_eq!(
            episode_heading(&unknown),
            "未分类记录 · 未分类行动 · 结果暂未分类"
        );
        assert_eq!(
            episode_detail(&unknown),
            "未分类记录；实际结果以服务器确认为准。"
        );
        let decision = episode(
            "decision",
            "requestapproval",
            "selected",
            "loot-would-leave-party-leash",
        );
        assert!(episode_heading(&decision).contains("等待离队许可"));
        assert_eq!(
            episode_detail(&decision),
            "原因：拾取会离队太远，先等主人同意。"
        );
        let owner_text = episode(
            "feedback",
            "teach-support-before-loot",
            "accepted",
            "near-party-leader",
        );
        assert_eq!(episode_detail(&owner_text), "near-party-leader");
        assert!(episode_heading(&owner_text).ends_with("教学已保存"));
    }
    #[test]
    fn selected_choices_use_common_chinese_text_instead_of_a_missing_check_glyph() {
        assert_eq!(selection_label("共同经历", true), "已选：共同经历");
        assert_eq!(selection_label("自己想来", false), "自己想来");
        assert!(!selection_label("收到提醒", true).contains('✓'));
    }
}
