//! Native owner-only extension. All displayed growth comes from correlated server receipts.
use super::text_editor::{EditResult, FriendTextEditor};
use crate::companion::{
    format_companion_micros, CompanionControls, CompanionEpisode, CompanionOperation as Op,
};
use bevy::input::mouse::MouseWheel;
use bevy::input::{
    keyboard::{Key, KeyboardInput},
    ButtonState,
};
use bevy::prelude::*;
use bevy::window::{Ime, PrimaryWindow};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "companion/learning_supply.rs"]
mod learning_supply;
#[path = "companion/life.rs"]
mod life;

use super::overlays::{NativePlayerUiSet, NativePlayerUiState};
use crate::native_shell::{NativeShellModel, NativeShellScreen};

const GOLD: Color = Color::srgb(0.82, 0.69, 0.44);
const INK: Color = Color::srgb(0.92, 0.87, 0.76);
const MUTED: Color = Color::srgb(0.65, 0.62, 0.54);
#[derive(Component)]
pub struct CompanionPointerCapture;
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
    SaveChoice,
    TeachingCheck(bool),
    PurchaseIntent,
    FocusInstruction,
    SendInstruction,
    RefreshTask,
    ModelSettings,
    OpenTeaching,
    CloseTeaching,
    GenerateTeaching,
    ConfirmTeaching(String),
    CancelTeaching(String),
    PrepareSupply,
    AuthorizeSupply { resident_id: String, archive_id: String, policy_version: u64 },
    CancelSupply,
    LifeDuration(u16),
    PrepareLife,
    AuthorizeLife { resident_id: String, archive_id: String, policy_version: u64, duration_seconds: u16 },
    PrepareResumeLife,
    ResumeLife { resident_id: String, archive_id: String, policy_version: u64, life_permit_id: String },
    CancelLife,
}
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompanionInputSet;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CompanionClipboardLease(u64);
static INPUT_REVISION: AtomicU64 = AtomicU64::new(1);
fn input_revision() -> u64 {
    INPUT_REVISION.fetch_add(1, Ordering::Relaxed)
}
struct InstructionInput {
    editor: FriendTextEditor,
    focused: bool,
    composition: Option<super::overlays::text_input::Composition>,
    focus_revision: u64,
    revision: u64,
    modifiers: [bool; 4],
    ime_consumed: bool,
    notice: String,
}
impl Default for InstructionInput {
    fn default() -> Self {
        Self {
            editor: FriendTextEditor::new(String::new(), 2000, false),
            focused: false,
            composition: None,
            focus_revision: input_revision(),
            revision: input_revision(),
            modifiers: [false; 4],
            ime_consumed: false,
            notice: String::new(),
        }
    }
}
#[derive(Resource, Default)]
pub struct CompanionUi {
    pub invite_requested: bool,
    pub open_export_requested: bool,
    pub model_settings_requested: bool,
    instruction: InstructionInput,
    instruction_owner: Option<(String, String)>,
    instruction_for_teaching: bool,
    teaching_open: bool,
    life_duration_seconds: u16,
    submit_instruction: bool,
    last_task_refresh_ms: u64,
    tab: u8,
    subpage: bool,
    page: usize,
    confirm: Option<Control>,
    entry_source: Option<u8>,
    reminder_count: u8,
    choice_reason: Option<u8>,
    focus: usize,
    focus_actions: Vec<Control>,
    rendered_view: Option<(String, String)>,
    revision: u64,
}
impl CompanionUi {
    fn text_entry_page(&self) -> bool {
        !self.subpage && (self.tab == 3 || (self.tab == 0 && self.teaching_open))
    }
    fn changed_input(&mut self) {
        self.instruction.revision = input_revision();
        self.revision += 1;
    }
    fn blur_instruction(&mut self) {
        if self.instruction.focused || self.instruction.composition.is_some() {
            self.instruction.focused = false;
            self.instruction.composition = None;
            self.instruction.modifiers = [false; 4];
            self.instruction.focus_revision = input_revision();
            self.changed_input();
        }
    }
    pub fn instruction_clipboard_lease(&self) -> Option<CompanionClipboardLease> {
        (self.text_entry_page()
            && self.confirm.is_none()
            && self.instruction.focused
            && self.instruction.composition.is_none()
            && !self.instruction.ime_consumed)
            .then_some(CompanionClipboardLease(self.instruction.revision))
    }
    pub fn selected_instruction(&self, lease: CompanionClipboardLease) -> Option<String> {
        (self.instruction_clipboard_lease() == Some(lease))
            .then(|| self.instruction.editor.selected_text().to_owned())
    }
    pub fn paste_instruction(&mut self, lease: CompanionClipboardLease, text: &str) -> bool {
        self.instruction_clipboard_lease() == Some(lease) && self.insert_instruction(text)
    }
    pub fn cut_instruction(&mut self, lease: CompanionClipboardLease) {
        if self.instruction_clipboard_lease() == Some(lease)
            && !self.instruction.editor.selection().is_empty()
        {
            self.instruction.editor.delete(false);
            self.changed_input();
        }
    }
    pub fn instruction_clipboard_failed(&mut self) {
        self.instruction.notice = "无法读取系统剪贴板；可以直接输入。".into();
        self.revision += 1;
    }
    fn insert_instruction(&mut self, value: &str) -> bool {
        if value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        {
            self.instruction.notice = "私人委托只接受一行文字，未粘贴换行或控制字符。".into();
            self.revision += 1;
            return false;
        }
        let mut next = self.instruction.editor.clone();
        let result = next.insert(value);
        if result == EditResult::OverBudget
            || next.text().chars().count() > 1000
            || next.text().len() > 6000
        {
            self.instruction.notice = "最多输入 1000 个字符；超出部分没有写入。".into();
            self.revision += 1;
            return false;
        }
        if result == EditResult::Changed {
            self.instruction.editor = next;
            self.instruction.notice.clear();
            self.changed_input();
        }
        true
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
pub struct Mir2CompanionUiPlugin;
impl Plugin for Mir2CompanionUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CompanionControls>()
            .init_resource::<CompanionUi>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<MouseWheel>()
            .add_message::<KeyboardInput>()
            .add_message::<Ime>()
            .add_systems(Startup, spawn_root)
            .add_systems(
                Update,
                instruction_input
                    .in_set(CompanionInputSet)
                    .after(super::overlays::text_input::process_ime)
                    .before(NativePlayerUiSet::Mutate),
            )
            .add_systems(
                PostUpdate,
                position_instruction_ime
                    .after(super::overlays::text_input::position_ime)
                    .after(bevy::ui::UiSystems::Layout),
            )
            .add_systems(
                Update,
                interact
                    .in_set(NativePlayerUiSet::Mutate)
                    .before(super::overlays::process_overlay_keyboard),
            )
            .add_systems(Update, render.in_set(NativePlayerUiSet::Read));
    }
}
#[derive(Component)]
struct InstructionField;
#[derive(Default)]
struct InstructionImeLease {
    target: Option<(Entity, u64)>,
    enabled: bool,
}
fn instruction_input(
    shell: Res<NativeShellModel>,
    mut player: ResMut<NativePlayerUiState>,
    controls: Res<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
    mut typed: MessageReader<KeyboardInput>,
    mut ime: MessageReader<Ime>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut lease: Local<InstructionImeLease>,
) {
    let owner = controls.view.as_ref().map(|view| {
        (
            view.archive.resident_id.clone(),
            view.archive.archive_id.clone(),
        )
    });
    let for_teaching = ui.tab == 0 && ui.teaching_open;
    if ui.instruction_owner != owner || ui.instruction_for_teaching != for_teaching {
        ui.instruction = InstructionInput::default();
        ui.instruction_owner = owner;
        ui.instruction_for_teaching = for_teaching;
        ui.submit_instruction = false;
        ui.revision += 1;
    }
    ui.instruction.ime_consumed = false;
    let events: Vec<_> = typed.read().cloned().collect();
    let ime_events: Vec<_> = ime.read().cloned().collect();
    let active = shell.screen == NativeShellScreen::InGame
        && player.companion_open()
        && ui.text_entry_page()
        && ui.confirm.is_none()
        && controls.view.is_some()
        && ui.instruction.focused
        && super::overlays::text_input::editor_owner(&player).is_none();
    let Ok((window_id, mut window)) = windows.single_mut() else {
        return;
    };
    if !active || !window.focused {
        if lease.target.take().is_some() {
            window.ime_enabled = false;
            lease.enabled = false;
        }
        ui.blur_instruction();
        return;
    }
    let target = Some((window_id, ui.instruction.focus_revision));
    let new_lease = lease.target != target;
    if new_lease {
        lease.target = target;
        lease.enabled = false;
        window.ime_enabled = false; // One frame drops late commits from an earlier owner.
    } else {
        window.ime_enabled = true;
        for event in ime_events {
            match event {
                Ime::Enabled { window } if window == window_id => lease.enabled = true,
                Ime::Disabled { window } if window == window_id => {
                    lease.enabled = false;
                    ui.instruction.composition = None;
                }
                Ime::Preedit {
                    window,
                    value,
                    cursor,
                } if window == window_id && lease.enabled => {
                    ui.instruction.composition =
                        (!value.is_empty() && value.chars().count() <= 1000 && value.len() <= 6000)
                            .then_some(super::overlays::text_input::Composition { value, cursor });
                    ui.instruction.ime_consumed = true;
                    ui.revision += 1;
                }
                Ime::Commit { window, value } if window == window_id && lease.enabled => {
                    ui.instruction.composition = None;
                    ui.insert_instruction(&value);
                    ui.instruction.ime_consumed = true;
                }
                _ => {}
            }
        }
    }
    let composing = ui.instruction.composition.is_some();
    let consumed = ui.instruction.ime_consumed;
    for event in events {
        if event.window != window_id {
            continue;
        }
        let pressed = event.state == ButtonState::Pressed;
        match event.key_code {
            KeyCode::ControlLeft => ui.instruction.modifiers[0] = pressed,
            KeyCode::ControlRight => ui.instruction.modifiers[1] = pressed,
            KeyCode::ShiftLeft => ui.instruction.modifiers[2] = pressed,
            KeyCode::ShiftRight => ui.instruction.modifiers[3] = pressed,
            _ => {}
        }
        if !pressed {
            continue;
        }
        if event.key_code == KeyCode::Escape {
            if composing || consumed {
                ui.instruction.composition = None;
                ui.instruction.ime_consumed = true;
                ui.revision += 1;
            }
            continue;
        }
        if consumed || composing {
            continue;
        }
        let ctrl = ui.instruction.modifiers[0] || ui.instruction.modifiers[1];
        let shift = ui.instruction.modifiers[2] || ui.instruction.modifiers[3];
        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter if !event.repeat => ui.submit_instruction = true,
            KeyCode::Tab => ui.blur_instruction(),
            KeyCode::KeyA if ctrl => {
                ui.instruction.editor.select_all();
                ui.changed_input();
            }
            KeyCode::KeyC | KeyCode::KeyV | KeyCode::KeyX if ctrl => {}
            KeyCode::Backspace | KeyCode::Delete => {
                ui.instruction
                    .editor
                    .delete(event.key_code == KeyCode::Backspace);
                ui.changed_input();
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                ui.instruction
                    .editor
                    .horizontal(event.key_code == KeyCode::ArrowRight, shift);
                ui.changed_input();
            }
            KeyCode::Home | KeyCode::End => {
                ui.instruction
                    .editor
                    .home_end(event.key_code == KeyCode::End, true, shift);
                ui.changed_input();
            }
            _ if !ctrl => {
                if let Key::Character(text) = &event.logical_key {
                    ui.insert_instruction(text);
                }
            }
            _ => {}
        }
    }
    player.ime_frame_consumed |= composing || ui.instruction.ime_consumed;
}
fn position_instruction_ime(
    ui: Res<CompanionUi>,
    player: Res<NativePlayerUiState>,
    fields: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<InstructionField>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !player.companion_open() || !ui.instruction.focused {
        return;
    }
    if let (Some((node, transform)), Ok(mut window)) = (fields.iter().next(), windows.single_mut())
    {
        let point = transform.transform_point2(Vec2::new(
            -node.size().x * 0.5 + 12.,
            node.size().y * 0.5 - 8.,
        ));
        window.ime_position = point / window.scale_factor();
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
        bevy::ui::FocusPolicy::Pass,
    ));
}
fn now_ms(time: &Time<Real>) -> u64 {
    (time.elapsed_secs_f64() * 1000.) as u64
}
fn interact(
    shell: Res<NativeShellModel>,
    mut player: ResMut<NativePlayerUiState>,
    mut controls: ResMut<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
    time: Res<Time<Real>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &Control), Changed<Interaction>>,
    mut wheel: MessageReader<MouseWheel>,
) {
    if shell.screen != NativeShellScreen::InGame {
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
        if keys.just_pressed(KeyCode::Escape) && !ui.instruction.ime_consumed {
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
            ui.revision += 1;
        }
        if !ui.instruction.focused
            && !ui.instruction.ime_consumed
            && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space))
        {
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
        wheel.clear();
    }
    if open && ui.submit_instruction {
        ui.submit_instruction = false;
        actions.push(if ui.tab == 0 && ui.teaching_open {
            Control::GenerateTeaching
        } else {
            Control::SendInstruction
        });
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
        if control_needs_view(&action)
            && !controls.view.as_ref().is_some_and(|view| {
                ui.rendered_view.as_ref()
                    == Some(&(
                        view.archive.resident_id.clone(),
                        view.archive.archive_id.clone(),
                    ))
            })
        {
            // A same-frame receipt/owner change cannot retarget a button from
            // the previous rendered companion to the newly selected one.
            continue;
        }
        if !matches!(action, Control::FocusInstruction | Control::SendInstruction | Control::GenerateTeaching) {
            ui.blur_instruction();
        }
        let now = now_ms(&time);
        ui.revision += 1;
        match action {
            Control::Open => {
                if player.blocks_gameplay_keys() || player.blocks_world_action(false, false) {
                    continue;
                }
                player.apply(mir2_ui_core::UiAction::OpenCompanion);
                ui.focus = 0;
                controls.begin(Op::List, json!({}), now);
            }
            Control::Close => {
                player.apply(mir2_ui_core::UiAction::ClosePanel);
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
                    ui.choice_reason = None;
                }
            }
            Control::Tab(tab) => {
                ui.tab = tab;
                ui.page = 0;
                ui.subpage = false;
                ui.focus = 0;
                ui.teaching_open = false;
            }
            Control::OpenTeaching => {
                ui.teaching_open = true;
                ui.subpage = false;
                ui.page = 0;
                ui.focus = 0;
            }
            Control::CloseTeaching => {
                ui.teaching_open = false;
                ui.page = 0;
                ui.focus = 0;
            }
            Control::Subpage => {
                ui.subpage = !ui.subpage;
                ui.page = 0;
            }
            Control::PreviousPage => {
                ui.page = ui.page.saturating_sub(1);
            }
            Control::NextPage => {
                let count = controls.view.as_ref().map_or(0, |view| {
                    if ui.tab >= 4 || (ui.tab == 0 && ui.teaching_open) {
                        0
                    } else if ui.tab == 3 {
                        task_detail_lines(&view.runtime).len().div_ceil(10) * 4
                    } else if ui.tab == 1 {
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
            Control::FocusInstruction => {
                ui.instruction.focused = true;
                ui.instruction.focus_revision = input_revision();
                ui.instruction.modifiers = [false; 4];
                ui.changed_input();
            }
            Control::SendInstruction => {
                if ui.instruction.composition.is_some() || ui.instruction.ime_consumed {
                    continue;
                }
                if !controls
                    .view
                    .as_ref()
                    .is_some_and(|view| model_ready(&view.runtime))
                {
                    controls.notice = "真实模型尚未启用或 API 尚未配置，请先打开模型设置。".into();
                    controls.revision += 1;
                    continue;
                }
                let draft = ui.instruction.editor.text().to_owned();
                if controls.instruct(&draft, now) {
                    ui.blur_instruction();
                    ui.last_task_refresh_ms = now;
                }
            }
            Control::GenerateTeaching => {
                if ui.instruction.composition.is_some() || ui.instruction.ime_consumed {
                    continue;
                }
                if !controls.view.as_ref().is_some_and(|v| model_ready(&v.runtime)) {
                    controls.notice = "自然语言教学需要先配置并启用真实模型。".into();
                    controls.revision += 1;
                    continue;
                }
                let draft = ui.instruction.editor.text().to_owned();
                if controls.propose_teaching(&draft, now) {
                    ui.blur_instruction();
                    ui.last_task_refresh_ms = now;
                }
            }
            Control::ConfirmTeaching(id) => { controls.confirm_teaching(&id, now); }
            Control::CancelTeaching(id) => { controls.cancel_teaching(&id, now); }
            Control::PrepareSupply => {
                if let Some(view) = &controls.view {
                    ui.confirm = Some(Control::AuthorizeSupply {
                        resident_id: view.archive.resident_id.clone(),
                        archive_id: view.archive.archive_id.clone(),
                        policy_version: view.profile.policy_version,
                    });
                    ui.focus = 0;
                }
            }
            Control::AuthorizeSupply { .. } => {}
            Control::CancelSupply => { controls.begin(Op::CancelSupply, json!({}), now); }
            Control::LifeDuration(seconds) => {
                if matches!(seconds, 1200 | 1800) { ui.life_duration_seconds = seconds; }
            }
            Control::PrepareLife => {
                if let Some(view) = controls.view.as_ref().filter(|view| life::can_authorize(view)) {
                    ui.confirm = Some(Control::AuthorizeLife {
                        resident_id: view.archive.resident_id.clone(), archive_id: view.archive.archive_id.clone(),
                        policy_version: view.profile.policy_version, duration_seconds: life::duration(&ui),
                    });
                    ui.focus = 0;
                }
            }
            Control::PrepareResumeLife => {
                if let Some(view) = &controls.view {
                    if let Some(id) = life::resume_id(view) {
                        ui.confirm = Some(Control::ResumeLife {
                            resident_id: view.archive.resident_id.clone(), archive_id: view.archive.archive_id.clone(),
                            policy_version: view.profile.policy_version, life_permit_id: id.to_owned(),
                        });
                        ui.focus = 0;
                    }
                }
            }
            Control::AuthorizeLife { .. } | Control::ResumeLife { .. } => {}
            Control::CancelLife => { controls.begin(Op::CancelLife, json!({}), now); }
            Control::RefreshTask => {
                if !controls.busy() {
                    controls.begin(Op::View, json!({"limit":20}), now);
                }
            }
            Control::ModelSettings => ui.model_settings_requested = true,
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
                            Control::AuthorizeSupply { resident_id, archive_id, policy_version } => {
                                let valid = controls.view.as_ref().is_some_and(|v| {
                                    v.archive.resident_id == resident_id
                                        && v.archive.archive_id == archive_id
                                        && v.profile.policy_version == policy_version
                                });
                                if !valid {
                                    controls.notice = "伙伴或培养版本已变化，请重新查看补给授权。".into();
                                    controls.revision += 1;
                                    continue;
                                }
                                payload["budgetGold"] = json!(50);
                                payload["quantity"] = json!(1);
                                payload["expiresInSeconds"] = json!(300);
                                Op::AuthorizeSupply
                            }
                            Control::AuthorizeLife { resident_id, archive_id, policy_version, duration_seconds } => {
                                let valid = controls.view.as_ref().is_some_and(|v| {
                                    v.archive.resident_id == resident_id && v.archive.archive_id == archive_id
                                        && v.profile.policy_version == policy_version && life::can_authorize(v)
                                });
                                if !valid {
                                    controls.notice = "伙伴、培养版本或生活状态已变化，请重新查看授权。".into();
                                    controls.revision += 1;
                                    continue;
                                }
                                controls.authorize_life(duration_seconds, now);
                                ui.page = 0;
                                continue;
                            }
                            Control::ResumeLife { resident_id, archive_id, policy_version, life_permit_id } => {
                                let valid = controls.view.as_ref().is_some_and(|v| {
                                    v.archive.resident_id == resident_id && v.archive.archive_id == archive_id
                                        && v.profile.policy_version == policy_version && life::resume_id(v) == Some(life_permit_id.as_str())
                                });
                                if !valid {
                                    controls.notice = "原生活授权、培养版本或时效已变化，请刷新后重新确认。".into();
                                    controls.revision += 1;
                                    continue;
                                }
                                controls.resume_life(&life_permit_id, now);
                                ui.page = 0;
                                continue;
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
                controls.begin(Op::TrialEvent, payload, now);
            }
            Control::Choice(reason) => ui.choice_reason = Some(reason),
        }
    }
    let now = now_ms(&time);
    if open
        && (ui.tab == 3 || (ui.tab == 0 && ui.teaching_open) || ui.tab == 4 || ui.tab == 5)
        && !controls.busy()
        && now.saturating_sub(ui.last_task_refresh_ms) >= 2500
        && controls.view.as_ref().is_some_and(|view| {
            ui.tab == 5 || matches!(
                view.runtime
                    .pointer("/modelTask/status")
                    .and_then(Value::as_str),
                Some("planning" | "running")
            ) || view.runtime.pointer("/teachingProposal/status").and_then(Value::as_str) == Some("planning")
                || matches!(view.runtime.pointer("/supplyTask/status").and_then(Value::as_str),
                    Some("authorized" | "running" | "unknown" | "returning"))
        })
    {
        // Read-only status polling never repeats the instruction mutation.
        controls.begin(Op::View, json!({"limit":20}), now);
        ui.last_task_refresh_ms = now;
    }
    if open {
        // These edges belong to this window, including the edge that closes
        // it. Later HUD handlers must not open chat with that same Enter.
        for key in [
            KeyCode::Escape,
            KeyCode::Tab,
            KeyCode::Enter,
            KeyCode::Space,
            KeyCode::PageDown,
            KeyCode::PageUp,
        ] {
            keys.clear_just_pressed(key);
        }
    }
}

fn control_needs_view(action: &Control) -> bool {
    matches!(
        action,
        Control::FocusInstruction
            | Control::SendInstruction
            | Control::RefreshTask
            | Control::Teach
            | Control::Recall
            | Control::Resume
            | Control::Invite
            | Control::Export
            | Control::OpenExportFolder
            | Control::Revoke(_)
            | Control::DeleteFeedback(_)
            | Control::DeleteEpisode(_)
            | Control::Clear
            | Control::Confirm
            | Control::StartVisit
            | Control::EndVisit
            | Control::SaveChoice
            | Control::TeachingCheck(_)
            | Control::PurchaseIntent
            | Control::OpenTeaching
            | Control::CloseTeaching
            | Control::GenerateTeaching
            | Control::ConfirmTeaching(_)
            | Control::CancelTeaching(_)
            | Control::PrepareSupply
            | Control::AuthorizeSupply { .. }
            | Control::CancelSupply
            | Control::LifeDuration(_)
            | Control::PrepareLife
            | Control::AuthorizeLife { .. }
            | Control::PrepareResumeLife
            | Control::ResumeLife { .. }
            | Control::CancelLife
    )
}

fn text(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
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
            font: font.clone(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    ));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
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
    let entry = action == Control::Open;
    if enabled {
        entity.insert((Button, action));
    }
    if entry {
        entity.insert(CompanionPointerCapture);
    }
    entity.with_children(|p| {
        p.spawn((
            Text::new(label.to_owned()),
            TextFont {
                font: font.clone(),
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
    roots: Query<Entity, With<CompanionRoot>>,
    social: Option<Res<crate::social::SocialModel>>,
    mut previous: Local<Option<(u64, u64, bool, bool, bool, u64)>>,
) {
    let in_game = shell.screen == NativeShellScreen::InGame;
    let open = in_game && player.companion_open();
    let entry_available =
        !player.blocks_gameplay_keys() && !player.blocks_world_action(false, false);
    let member = controls.view.as_ref().is_some_and(|view| {
        social.as_ref().is_some_and(|s| {
            s.group
                .members
                .iter()
                .any(|m| m.name == view.archive.character_name)
        })
    });
    let fingerprint = (
        controls.revision,
        ui.revision,
        open,
        member,
        entry_available,
        crate::native_i18n::revision(),
    );
    if *previous == Some(fingerprint) && !shell.is_changed() {
        return;
    }
    *previous = Some(fingerprint);
    let Some(root) = roots.iter().next() else {
        return;
    };
    commands.entity(root).despawn_children();
    ui.focus_actions.clear();
    ui.rendered_view = controls.view.as_ref().map(|view| {
        (
            view.archive.resident_id.clone(),
            view.archive.archive_id.clone(),
        )
    });
    if !in_game {
        return;
    }
    let font_value = super::typography::crystal_text_font(14.).font;
    let font = &font_value;
    commands.entity(root).with_children(|root|{
        if !open{
            button(root,font,&mut ui,"伙伴 Companion",872.,174.,140.,Control::Open,entry_available);return;
        }
        root.spawn((Node{position_type:PositionType::Absolute,left:px(0.),top:px(0.),width:px(1024.),height:px(768.),..default()},
            BackgroundColor(Color::srgba(0.,0.,0.,0.54)),bevy::ui::FocusPolicy::Block));
        root.spawn((Name::new("伙伴 Companion window"),Node{position_type:PositionType::Absolute,left:px(140.),top:px(44.),width:px(744.),height:px(644.),
            border:UiRect::all(px(2.)),overflow:Overflow::clip(),..default()},BackgroundColor(Color::srgb(0.055,0.043,0.029)),BorderColor::all(GOLD)))
            .with_children(|panel|{
                text(panel,font,"伙伴 · 身份与成长",18.,12.,470.,30.,20.,GOLD);
                button(panel,font,&mut ui,"关闭 Esc",638.,10.,88.,Control::Close,true);
                button(panel,font,&mut ui,"模型设置",506.,10.,118.,Control::ModelSettings,true);
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
                if ui.tab==5 {
                    let resumable=controls.view.as_ref().is_some_and(|v|life::resume_id(v).is_some());
                    button(panel,font,&mut ui,"恢复生活",370.,87.,112.,Control::PrepareResumeLife,mutable&&resumable);
                }else{button(panel,font,&mut ui,"恢复行动",370.,87.,112.,Control::Resume,mutable);}
                text(panel,font,"组队以服务器名单为准",496.,91.,225.,26.,12.,MUTED);
                for(index,label)in["培养与体验","记忆管理","行动与成本","私人委托","独立补给","自主生活"].iter().enumerate(){
                    button(panel,font,&mut ui,label,18.+index as f32*120.,132.,110.,Control::Tab(index as u8),true);
                }
                if let Some(confirm)=ui.confirm.clone(){
                    // Confirmation stores only opaque IDs, never another copy of private text.
                    text(panel,font,"请确认这次操作",24.,205.,680.,32.,19.,GOLD);
                    let message=life::confirmation_message(&confirm).unwrap_or_else(|| match confirm{Control::Clear=>"清除全部教学和经历，并重新计算习惯。保留伙伴身份及服务器游戏角色。此操作不能撤销。",
                        Control::Revoke(_)=>"撤销这条教学并重新计算习惯。记录仍可查看；撤销后不会自动恢复。",
                        Control::DeleteEpisode(_)=>"删除这条经历及文字。由它产生的教学也将失效。此操作不能撤销。",
                        Control::AuthorizeSupply{..}=>"授权本次独立补给：去比奇杂货商鲁本处购买 1 瓶小型生命药水，最多花费 50 游戏金币，权限 5 分钟有效，买完回到授权点旁。仅本次购买；禁止拾取、出售、交易和换图。购买已提交后，撤销不会虚构退款。",
                        _=>"删除这条教学及文字，并重新计算习惯。此操作不能撤销。"}.into());
                    text(panel,font,&message,24.,252.,680.,130.,16.,INK);
                    ui.focus_actions.clear();
                    button(panel,font,&mut ui,"取消",130.,390.,210.,Control::CancelConfirm,true);
                    button(panel,font,&mut ui,"确认执行",370.,390.,210.,Control::Confirm,mutable);
                }else if let Some(view)=&controls.view{
                    match ui.tab{
                        0 if ui.teaching_open=>learning_supply::render_teaching(panel,font,&mut ui,view,mutable),
                        0 if !ui.subpage=>{
                            text(panel,font,&format!("{} · 培养 v{} · 成长修订 {}",view.archive.display_name,view.profile.policy_version,view.growth_revision),22.,181.,690.,28.,17.,GOLD);
                            let habits=&view.profile.party;
                            text(panel,font,&format!("组队先支援：{}\n战斗后再拾取：{}\n离队先询问：{}",yes(habits.assist_before_loot),yes(habits.loot_only_after_combat),yes(habits.ask_before_leaving)),22.,222.,690.,92.,16.,INK);
                            button(panel,font,&mut ui,"教它：组队先支援，战后再拾取，离队先问",22.,326.,690.,Control::Teach,mutable);
                            text(panel,font,"这是明确的习惯教学。保存成功后显示新版本；伙伴实际行动仍受血量、距离和游戏规则约束。",22.,371.,690.,62.,14.,MUTED);
                            button(panel,font,&mut ui,"用自己的话教阿洛",22.,435.,334.,Control::OpenTeaching,mutable);
                            button(panel,font,&mut ui,"体验自述与选择原因",368.,435.,344.,Control::Subpage,!busy);
                            text(panel,font,&format!("固定身份：{}\n{}",view.archive.archive_id,view.archive.base_setting),22.,478.,690.,61.,13.,MUTED);
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
                        3=>render_instruction(panel,font,&mut ui,view,mutable),
                        4=>learning_supply::render_supply(panel,font,&mut ui,view,mutable),
                        5=>life::render(panel,font,&mut ui,view,mutable),
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
fn model_ready(runtime: &Value) -> bool {
    runtime.get("provider").and_then(Value::as_str) == Some("openai")
        && runtime.get("modelEnabled").and_then(Value::as_bool) == Some(true)
        && runtime.get("configured").and_then(Value::as_bool) == Some(true)
}
fn bounded_display(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect()
}
fn model_configuration_label(runtime: &Value) -> String {
    let model = bounded_display(
        runtime
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("未指定"),
        70,
    );
    match runtime.get("provider").and_then(Value::as_str) {
        Some("rules") if runtime.get("configured").and_then(Value::as_bool) == Some(false) => {
            "API 尚未配置，当前为规则模式。模型设置保存后需重启本地试用。".into()
        }
        Some("rules") => "当前为规则模式，未启用真实模型。模型设置保存后需重启本地试用。".into(),
        Some("openai") if runtime.get("modelEnabled").and_then(Value::as_bool) != Some(true) => {
            "OpenAI 模型已关闭，当前不会发起模型调用。".into()
        }
        Some("openai") if runtime.get("configured").and_then(Value::as_bool) != Some(true) => {
            "OpenAI API 尚未配置。模型设置保存后需重启本地试用。".into()
        }
        Some("openai") => format!("OpenAI 模型：{model} · 已配置；实际调用以回执为准。"),
        _ => "尚未收到模型配置状态，请刷新或打开模型设置。".into(),
    }
}
fn task_status_label(runtime: &Value) -> &'static str {
    match runtime.pointer("/modelTask/status").and_then(Value::as_str) {
        Some("planning") => "模型正在制定计划；尚未证明任何击杀。",
        Some("running") => "正在执行计划；真实进度由服务器确认。",
        Some("completed") => "委托记录已结束；请核对下方服务器进度。",
        Some("paused") => "委托已暂停；不会把暂停当作完成。",
        Some("failed") => "委托未完成，请查看模型设置或刷新状态。",
        Some("idle") | None => "暂无执行中的模型委托。",
        _ => "委托状态暂未分类，请刷新记录。",
    }
}
fn task_progress(runtime: &Value) -> String {
    let task = &runtime["modelTask"];
    match (
        task.get("completedKills").and_then(Value::as_u64),
        task.get("requestedKills").and_then(Value::as_u64),
    ) {
        (Some(actual), Some(requested)) if requested > 0 => {
            format!("实际参战击杀：{actual} / {requested}（按服务器死亡回执记录）")
        }
        (Some(actual), _) => format!("实际参战击杀：{actual}；尚未提供击杀目标。"),
        _ => "实际参战击杀：尚未提供计数，不按模型文字推算。".into(),
    }
}
fn task_step_label(step: &str) -> &'static str {
    match step {
        "planning" => "制定计划",
        "seeking-monster" => "寻找可交战的怪物",
        "approaching-monster" => "接近已观察到的怪物",
        "fighting" => "交战中",
        "returning-to-owner" => "返回主人附近",
        "returning-home" => "返回住处",
        "waiting" => "等候",
        "following" => "跟随主人",
        "completed" => "本次计划已结束",
        "paused" => "已暂停",
        "failed" => "未完成",
        "awaiting-new-instruction" => "等待新的私人委托",
        _ => "暂未分类的步骤",
    }
}
fn task_detail_lines(runtime: &Value) -> Vec<String> {
    let task = &runtime["modelTask"];
    let mut sections = vec![
        task_progress(runtime),
        "模型计划与回复（这些文字不代表游戏行动已完成）：".into(),
    ];
    if let Some(instruction) = task.get("instruction").and_then(Value::as_str) {
        sections.push(format!("私人委托：{}", bounded_display(instruction, 1000)));
    }
    if let Some(reply) = task.get("reply").and_then(Value::as_str) {
        sections.push(format!("模型回复：{}", bounded_display(reply, 4000)));
    }
    if let Some(step) = task.get("step").filter(|v| v.is_string() || v.is_u64()) {
        sections.push(format!(
            "计划步骤（不是完成回执）：{}",
            step.as_str()
                .map(|s| task_step_label(s).to_owned())
                .unwrap_or_else(|| step.to_string())
        ));
    }
    if let Some(rows) = task.get("references").and_then(Value::as_array) {
        for row in rows.iter().take(10) {
            sections.push(format!(
                "引用经历 {}：{}",
                bounded_display(
                    row.get("episodeId")
                        .and_then(Value::as_str)
                        .unwrap_or("未提供编号"),
                    100
                ),
                bounded_display(
                    row.get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or("未提供摘要"),
                    300
                )
            ));
        }
    }
    let call = &task["lastCall"];
    if call.is_object() {
        let status = match call.get("status").and_then(Value::as_str) {
            Some("running" | "pending" | "planning") => "调用中",
            Some("completed" | "ok" | "success" | "succeeded") => "已有调用回执",
            Some("failed" | "error") => "调用失败",
            _ => "尚未分类",
        };
        let tokens = |key| {
            call.get(key)
                .and_then(Value::as_u64)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未计量".into())
        };
        sections.push(format!(
            "最近模型调用：{} · {status}；输入 {} / 输出 {} tokens",
            bounded_display(
                call.get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("未提供型号"),
                70
            ),
            tokens("inputTokens"),
            tokens("outputTokens")
        ));
        if let Some(request) = call.get("requestId").and_then(Value::as_str) {
            sections.push(format!("调用编号：{}", bounded_display(request, 100)));
        }
    }
    if task.get("error").is_some_and(|v| !v.is_null()) {
        sections.push(
            "最近调用或执行失败；请检查本地模型设置并刷新。错误详情不会写入公共聊天。".into(),
        );
    }
    let mut lines = Vec::new();
    for section in sections {
        let chars: Vec<_> = section.chars().collect();
        for chunk in chars.chunks(44) {
            lines.push(chunk.iter().collect());
        }
    }
    lines
}
fn instruction_preview(ui: &CompanionUi) -> String {
    let editor = if let Some(c) = &ui.instruction.composition {
        ui.instruction
            .editor
            .composition_preview(&c.value, c.cursor)
    } else {
        ui.instruction.editor.clone()
    };
    if editor.text().is_empty() {
        if ui.tab == 0 && ui.teaching_open {
            return "点击这里输入，例如：以后我战斗时先帮我，打完再捡东西。".into();
        }
        return "点击这里输入，例如：陪我打三只稻草人，先照顾队友。".into();
    }
    // Keep a long one-line draft's caret visible without estimating glyph widths.
    let before: Vec<_> = editor.text()[..editor.caret()]
        .chars()
        .rev()
        .take(75)
        .collect();
    let mut value: String = before.into_iter().rev().collect();
    if ui.instruction.focused {
        value.push('|');
    }
    value.extend(editor.text()[editor.caret()..].chars().take(45));
    if !editor.selection().is_empty() {
        value.push_str("  [已选择文字]");
    }
    value
}
fn render_instruction(
    p: &mut ChildSpawnerCommands,
    f: &FontSource,
    ui: &mut CompanionUi,
    v: &crate::companion::CompanionOwnerView,
    enabled: bool,
) {
    if ui.subpage {
        text(
            p,
            f,
            task_status_label(&v.runtime),
            22.,
            180.,
            690.,
            28.,
            14.,
            GOLD,
        );
        let lines = task_detail_lines(&v.runtime);
        ui.page = ui.page.min(lines.len().saturating_sub(1) / 10);
        for (index, line) in lines.iter().skip(ui.page * 10).take(10).enumerate() {
            text(
                p,
                f,
                line,
                22.,
                216. + index as f32 * 25.,
                690.,
                25.,
                14.,
                INK,
            );
        }
        page_buttons(p, f, ui, lines.len().div_ceil(10) * 4, false, false);
        button(
            p,
            f,
            ui,
            "返回私人委托输入",
            22.,
            513.,
            690.,
            Control::Subpage,
            true,
        );
        return;
    }
    text(
        p,
        f,
        &model_configuration_label(&v.runtime),
        22.,
        180.,
        690.,
        44.,
        14.,
        GOLD,
    );
    text(
        p,
        f,
        "私人委托 · 仅发送给你有权管理的伙伴，最多 1000 字符",
        22.,
        226.,
        690.,
        24.,
        13.,
        MUTED,
    );
    ui.focus_actions.push(Control::FocusInstruction);
    p.spawn((
        Name::new("私人委托输入"),
        InstructionField,
        Button,
        Control::FocusInstruction,
        Node {
            position_type: PositionType::Absolute,
            left: px(22.),
            top: px(255.),
            width: px(690.),
            height: px(84.),
            padding: UiRect::all(px(8.)),
            border: UiRect::all(px(1.)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.075, 0.052)),
        BorderColor::all(if ui.instruction.focused { GOLD } else { MUTED }),
    ))
    .with_children(|p| {
        p.spawn((
            Text::new(instruction_preview(ui)),
            TextFont {
                font: f.clone(),
                font_size: FontSize::Px(15.),
                ..default()
            },
            TextColor(INK),
        ));
    });
    button(
        p,
        f,
        ui,
        "发送私人委托",
        22.,
        349.,
        218.,
        Control::SendInstruction,
        enabled && model_ready(&v.runtime) && !ui.instruction.editor.text().trim().is_empty(),
    );
    button(
        p,
        f,
        ui,
        "刷新模型状态",
        254.,
        349.,
        218.,
        Control::RefreshTask,
        true,
    );
    text(
        p,
        f,
        &format!(
            "{} / 1000 字符 · Enter 发送
中文候选词确认后再按 Enter。Ctrl+V 粘贴。",
            ui.instruction.editor.text().chars().count()
        ),
        488.,
        347.,
        220.,
        42.,
        12.,
        MUTED,
    );
    text(
        p,
        f,
        task_status_label(&v.runtime),
        22.,
        393.,
        690.,
        44.,
        14.,
        GOLD,
    );
    text(
        p,
        f,
        &task_progress(&v.runtime),
        22.,
        438.,
        690.,
        44.,
        14.,
        INK,
    );
    text(
        p,
        f,
        &ui.instruction.notice,
        22.,
        479.,
        690.,
        28.,
        12.,
        MUTED,
    );
    button(
        p,
        f,
        ui,
        "查看模型计划、引用记忆与调用用量",
        22.,
        513.,
        690.,
        Control::Subpage,
        true,
    );
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
            "continue-unsafe-withdraw" => "继续撤离，直到抵达安全处。",
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
    f: &FontSource,
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
    f: &FontSource,
    ui: &mut CompanionUi,
    v: &crate::companion::CompanionOwnerView,
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
    text(
        p,
        f,
        "选择与理解情况均由你主动记录；本轮不收集自由文本。",
        22.,
        401.,
        690.,
        30.,
        13.,
        MUTED,
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
    f: &FontSource,
    ui: &mut CompanionUi,
    v: &crate::companion::CompanionOwnerView,
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
        controls.enrollments = vec![crate::companion::CompanionEnrollment {
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
    fn instruction_app() -> (App, Entity) {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.world_mut()
            .resource_mut::<CompanionControls>()
            .view
            .as_mut()
            .unwrap()
            .runtime = json!({"provider":"openai","model":"gpt-6-luna","modelEnabled":true,"configured":true,"modelTask":{"status":"idle"}});
        app.world_mut().resource_mut::<CompanionUi>().tab = 3;
        let window = app
            .world_mut()
            .spawn((
                Window {
                    focused: true,
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        app.update();
        press(&mut app, Control::FocusInstruction);
        app.update();
        app.update();
        (app, window)
    }
    fn teaching_app() -> (App, Entity) {
        let (mut app, window) = instruction_app();
        press(&mut app, Control::Tab(0));
        press(&mut app, Control::OpenTeaching);
        app.update();
        press(&mut app, Control::FocusInstruction);
        app.update();
        (app, window)
    }
    fn displayed_text(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect()
    }
    fn key(app: &mut App, window: Entity, key_code: KeyCode, value: &str) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key_code);
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: Key::Character(value.into()),
            text: (!value.is_empty()).then(|| value.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window,
        });
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key_code);
        keys.clear();
    }
    #[test]
    fn chinese_ime_preedit_and_commit_enter_never_send_until_a_separate_submit() {
        let (mut app, window) = instruction_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "陪我".into(),
            cursor: Some((6, 6)),
        });
        app.update();
        key(&mut app, window, KeyCode::Enter, "");
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<CompanionUi>()
                .instruction
                .editor
                .text(),
            ""
        );
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "陪我打三只稻草人".into(),
        });
        key(&mut app, window, KeyCode::Enter, "");
        assert_eq!(
            app.world()
                .resource::<CompanionUi>()
                .instruction
                .editor
                .text(),
            "陪我打三只稻草人"
        );
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
        key(&mut app, window, KeyCode::Enter, "");
        let requests = app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::Instruct);
        assert_eq!(
            requests[0].payload,
            json!({"text":"陪我打三只稻草人","expectedPolicyVersion":2})
        );
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
    }
    #[test]
    fn teaching_ime_is_private_and_separate_enter_only_proposes_without_changing_habits() {
        let (mut app, window) = teaching_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.world_mut().write_message(Ime::Preedit { window,
            value: "以后先帮我".into(), cursor: Some((0, "以后先帮我".len())) });
        key(&mut app, window, KeyCode::Enter, "");
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        app.world_mut().write_message(Ime::Commit { window, value: "以后先帮我，打完再捡东西。".into() });
        key(&mut app, window, KeyCode::Enter, "");
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        key(&mut app, window, KeyCode::Enter, "");
        let requests = app.world_mut().resource_mut::<CompanionControls>().drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::ProposeTeaching);
        assert_eq!(requests[0].payload, json!({"text":"以后先帮我，打完再捡东西。","expectedPolicyVersion":2}));
        assert_eq!(app.world().resource::<CompanionControls>().view.as_ref().unwrap().profile.policy_version, 2);
        assert!(app.world().resource::<NativePlayerUiState>().blocks_gameplay_keys());
    }
    #[test]
    fn switching_between_objective_and_teaching_discards_stale_clipboard_and_draft() {
        let (mut app, _) = instruction_app();
        let old = {
            let mut ui = app.world_mut().resource_mut::<CompanionUi>();
            assert!(ui.insert_instruction("旧的冒险委托"));
            ui.instruction_clipboard_lease().unwrap()
        };
        press(&mut app, Control::Tab(0));
        press(&mut app, Control::OpenTeaching);
        app.update();
        press(&mut app, Control::FocusInstruction);
        let mut ui = app.world_mut().resource_mut::<CompanionUi>();
        assert_eq!(ui.instruction.editor.text(), "");
        assert!(!ui.paste_instruction(old, "旧页面迟到的粘贴"));
    }
    #[test]
    fn teaching_preview_shows_false_patch_and_confirmation_submits_no_client_changes() {
        let (mut app, _) = teaching_app();
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().runtime["teachingProposal"] = json!({
                "proposalId":"proposal-2","status":"proposed","sourceVersion":2,
                "changes":{"assistBeforeLoot":false},"reply":"改成先收集物品。"});
            controls.revision += 1;
        }
        app.update();
        assert!(displayed_text(&mut app).iter().any(|s| s == "组队先支援：是 → 否"));
        assert!(app.world().resource::<CompanionControls>().view.as_ref().unwrap().profile.party.assist_before_loot);
        press(&mut app, Control::ConfirmTeaching("proposal-2".into()));
        let requests = app.world_mut().resource_mut::<CompanionControls>().drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::ConfirmTeaching);
        assert_eq!(requests[0].payload, json!({"proposalId":"proposal-2","expectedPolicyVersion":2}));
    }
    #[test]
    fn supply_requires_explicit_context_bound_confirmation_and_never_creates_local_success() {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.world_mut().resource_mut::<CompanionUi>().tab = 4;
        app.update();
        press(&mut app, Control::PrepareSupply);
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        assert!(displayed_text(&mut app).iter().any(|s| s.contains("最多花费 50 游戏金币")));
        press(&mut app, Control::Confirm);
        let requests = app.world_mut().resource_mut::<CompanionControls>().drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operation, Op::AuthorizeSupply);
        assert_eq!(requests[0].payload, json!({"expectedPolicyVersion":2,"budgetGold":50,"quantity":1,"expiresInSeconds":300}));
        assert!(app.world().resource::<CompanionControls>().view.as_ref().unwrap().runtime["supplyTask"].is_null());
    }
    #[test]
    fn changed_cultivation_version_invalidates_an_open_supply_confirmation() {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.world_mut().resource_mut::<CompanionUi>().tab = 4;
        app.update();
        press(&mut app, Control::PrepareSupply);
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().profile.policy_version = 3;
            controls.revision += 1;
        }
        app.update();
        press(&mut app, Control::Confirm);
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        assert!(app.world().resource::<CompanionControls>().notice.contains("重新查看补给授权"));
    }
    #[test]
    fn unknown_supply_result_does_not_show_zero_spending_or_allow_another_order() {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().runtime["supplyTask"] = json!({
                "status":"unknown","step":"reconcile","budgetGold":50,
                "chargedGold":null,"acquiredQuantity":0,"receiptKnown":false,"returnedToAnchor":false});
            controls.revision += 1;
        }
        app.world_mut().resource_mut::<CompanionUi>().tab = 4;
        app.update();
        let lines = displayed_text(&mut app);
        assert!(lines.iter().any(|s| s.contains("购买结果未知")));
        assert!(lines.iter().any(|s| s.contains("实际扣款：尚未确认")));
        assert!(!lines.iter().any(|s| s.contains("补给完成：")));
        let world = app.world_mut();
        assert!(!world.query::<&Control>().iter(world).any(|c| *c == Control::PrepareSupply));
        assert!(world.query::<&Control>().iter(world).any(|c| *c == Control::CancelSupply));
    }
    fn life_app() -> App {
        let mut app=app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.update();
        press(&mut app,Control::Tab(5));
        app
    }
    #[test]
    fn life_is_the_sixth_tab_and_both_durations_need_explicit_confirmation() {
        for seconds in [1200,1800] {
            let mut app=life_app();
            assert_eq!(app.world().resource::<CompanionUi>().tab,5);
            let tabs={let world=app.world_mut();world.query::<&Control>().iter(world).filter(|c|matches!(c,Control::Tab(_))).count()};
            assert_eq!(tabs,6);
            press(&mut app,Control::LifeDuration(seconds));
            press(&mut app,Control::PrepareLife);
            assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
            let text=displayed_text(&mut app).join("\n");
            assert!(text.contains(&format!("独立生活 {} 分钟",seconds/60)));
            assert!(text.contains("累计最多花费 50 游戏金币"));
            assert!(text.contains("只能向鲁本购买 1 瓶"));
            assert!(text.contains("y595–645"));
            press(&mut app,Control::Confirm);
            let requests=app.world_mut().resource_mut::<CompanionControls>().drain_requests();
            assert_eq!(requests.len(),1);
            assert_eq!(requests[0].operation,Op::AuthorizeLife);
            assert_eq!(requests[0].payload,json!({"expectedPolicyVersion":2,"budgetGold":50,"durationSeconds":seconds}));
            assert!(app.world().resource::<CompanionControls>().view.as_ref().unwrap().runtime["lifeTask"].is_null());
        }
    }
    #[test]
    fn cancelling_life_confirmation_sends_no_authorization_and_changed_version_is_refused() {
        let mut app=life_app();
        press(&mut app,Control::PrepareLife);
        press(&mut app,Control::CancelConfirm);
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        press(&mut app,Control::PrepareLife);
        {
            let mut controls=app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().profile.policy_version=3;
            controls.revision+=1;
        }
        app.update();
        press(&mut app,Control::Confirm);
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        assert!(app.world().resource::<CompanionControls>().notice.contains("重新查看授权"));
    }
    #[test]
    fn life_resume_confirms_the_original_permit_without_time_or_budget_reset() {
        let mut app=life_app();
        {
            let mut controls=app.world_mut().resource_mut::<CompanionControls>();
            let expiry=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64+600_000;
            controls.view.as_mut().unwrap().runtime=json!({"lifeTask":{"lifePermitId":"life-one","status":"paused","budgetGold":50,"maxPurchasedQuantity":1,"chargedGold":40,"reservedGold":10,"expiresAtMs":expiry},"resources":{"medicineUnresolved":true}});
            controls.revision+=1;
        }
        app.update();
        press(&mut app,Control::PrepareResumeLife);
        assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
        assert!(displayed_text(&mut app).join("\n").contains("不会延长时间"));
        press(&mut app,Control::Confirm);
        let requests=app.world_mut().resource_mut::<CompanionControls>().drain_requests();
        assert_eq!(requests.len(),1);
        assert_eq!(requests[0].operation,Op::ResumeLife);
        assert_eq!(requests[0].payload,json!({"expectedPolicyVersion":2,"lifePermitId":"life-one"}));
        assert_eq!(app.world().resource::<CompanionControls>().view.as_ref().unwrap().runtime["lifeTask"]["chargedGold"],40);
    }
    #[test]
    fn unknown_life_purchase_keeps_resources_unknown_and_stop_only_sends_a_request() {
        let mut app=life_app();
        {
            let mut controls=app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().runtime=json!({"lifeTask":{"lifePermitId":"life-one","status":"unknown","phase":"purchase","budgetGold":50,"chargedGold":null,"reservedGold":50},"resources":{"known":false,"medicineUnresolved":true}});
            controls.revision+=1;
        }
        app.update();
        let text=displayed_text(&mut app).join("\n");
        assert!(text.contains("金币：待核对"));
        assert!(text.contains("实扣 待核对"));
        assert!(text.contains("尚未算作完成"));
        let can_start={let world=app.world_mut();world.query::<&Control>().iter(world).any(|c|*c==Control::PrepareLife)};
        assert!(!can_start);
        press(&mut app,Control::CancelLife);
        let requests=app.world_mut().resource_mut::<CompanionControls>().drain_requests();
        assert_eq!(requests.len(),1);
        assert_eq!(requests[0].operation,Op::CancelLife);
        assert_eq!(requests[0].payload,json!({}));
        assert_eq!(app.world().resource::<CompanionControls>().view.as_ref().unwrap().runtime["lifeTask"]["status"],"unknown");
    }
    #[test]
    fn leaving_private_teaching_for_life_invalidates_its_clipboard_lease() {
        let (mut app,_)=teaching_app();
        let lease=app.world().resource::<CompanionUi>().instruction_clipboard_lease().unwrap();
        press(&mut app,Control::Tab(5));
        let mut ui=app.world_mut().resource_mut::<CompanionUi>();
        assert!(!ui.text_entry_page());
        assert!(ui.instruction_clipboard_lease().is_none());
        assert!(!ui.paste_instruction(lease,"旧教学迟到文本"));
    }
    #[test]
    fn unresolved_gift_arriving_after_life_preview_blocks_both_start_and_original_resume() {
        for resume in [false, true] {
            let mut app = life_app();
            if resume {
                let expiry = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64 + 600_000;
                let mut controls = app.world_mut().resource_mut::<CompanionControls>();
                controls.view.as_mut().unwrap().runtime = json!({
                    "lifeTask":{"lifePermitId":"life:old","status":"paused","budgetGold":50,
                        "maxPurchasedQuantity":1,"chargedGold":40,"reservedGold":0,"expiresAtMs":expiry},
                    "resources":{"medicineUnresolved":false},"giftTask":{"status":"idle"}
                });
                controls.revision += 1;
            }
            app.update();
            press(&mut app, if resume { Control::PrepareResumeLife } else { Control::PrepareLife });
            {
                let mut controls = app.world_mut().resource_mut::<CompanionControls>();
                controls.view.as_mut().unwrap().runtime["giftTask"] = json!({"status":"unknown","receivedGold":null});
                controls.revision += 1;
            }
            app.update();
            press(&mut app, Control::Confirm);
            assert!(app.world_mut().resource_mut::<CompanionControls>().drain_requests().is_empty());
            assert!(app.world().resource::<CompanionControls>().notice.contains("重新"));
            let world = app.world_mut();
            assert!(!world.query::<&Control>().iter(world).any(|control| {
                matches!(control, Control::PrepareLife | Control::PrepareResumeLife)
            }));
        }
    }
    #[test]
    fn life_page_renders_actual_runtime_decision_mode_and_receipt_counts_without_model_story() {
        let mut app = life_app();
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().runtime = json!({
                "lifeTask":{"status":"running","phase":"supplying","decisionMode":"model",
                    "budgetGold":50,"remainingBudgetGold":10,"reservedGold":0,"chargedGold":40,
                    "purchasedQuantity":1,"maxPurchasedQuantity":1,"killsObserved":3,
                    "reason":"只执行本次已授权的一瓶药补给。","reply":"模型虚构已经卖出稀有装备赚取百万金币"},
                "resources":{"known":true,"observedAtMs":1,"gold":45,"hp":18,"maxHp":18,
                    "inventoryCurrent":true,"hpPotionCount":2,"usableHpPotionCount":2,"freeBagSlots":40,
                    "currentWeight":5,"maxWeight":100,"medicineUnresolved":false}
            });
            controls.revision += 1;
        }
        app.update();
        let text = displayed_text(&mut app).join("\n");
        assert!(text.contains("自主生活 · 补给"));
        assert!(text.contains("模型：已有模型计划"));
        assert!(text.contains("已观察击杀 3 · 实扣 40 游戏金币"));
        assert!(text.contains("阿洛自己的金币：45"));
        assert!(!text.contains("模型：待核对"));
        assert!(!text.contains("百万金币"));
    }
    #[test]
    fn instruction_editor_keeps_graphemes_atomic_and_rejects_stale_or_multiline_paste() {
        let (mut app, window) = instruction_app();
        {
            let mut ui = app.world_mut().resource_mut::<CompanionUi>();
            let lease = ui.instruction_clipboard_lease().unwrap();
            assert!(ui.paste_instruction(lease, "你好👨‍👩‍👦"));
            assert!(!ui.paste_instruction(lease, "迟到的粘贴"));
        }
        key(&mut app, window, KeyCode::Backspace, "");
        assert_eq!(
            app.world()
                .resource::<CompanionUi>()
                .instruction
                .editor
                .text(),
            "你好"
        );
        let old = {
            let mut ui = app.world_mut().resource_mut::<CompanionUi>();
            let lease = ui.instruction_clipboard_lease().unwrap();
            assert!(!ui.paste_instruction(lease, "第一行\n第二行"));
            assert_eq!(ui.instruction.editor.text(), "你好");
            lease
        };
        app.world_mut()
            .resource_mut::<CompanionControls>()
            .view
            .as_mut()
            .unwrap()
            .archive
            .archive_id = "archive-next".into();
        app.update();
        let mut ui = app.world_mut().resource_mut::<CompanionUi>();
        assert_eq!(ui.instruction.editor.text(), "");
        assert!(!ui.paste_instruction(old, "旧角色的粘贴"));
    }
    #[test]
    fn another_native_editor_cancels_the_private_ime_lease_before_a_late_commit() {
        let (mut app, window) = instruction_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        {
            let mut player = app.world_mut().resource_mut::<NativePlayerUiState>();
            player.friends.open = true;
            player.friends.modal = Some(
                crate::crystal_ui::overlays::friend_dialog::FriendModal::Add {
                    blocked: false,
                    text: String::new(),
                },
            );
            player.friends.sync_editor();
        }
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "另一输入框的文字".into(),
        });
        app.update();
        let ui = app.world().resource::<CompanionUi>();
        assert_eq!(ui.instruction.editor.text(), "");
        assert!(ui.instruction_clipboard_lease().is_none());
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
    }
    #[test]
    fn composing_escape_cancels_candidate_before_closing_private_window() {
        let (mut app, window) = instruction_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "候选".into(),
            cursor: None,
        });
        app.update();
        key(&mut app, window, KeyCode::Escape, "");
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .companion_open());
        assert!(app
            .world()
            .resource::<CompanionUi>()
            .instruction
            .composition
            .is_none());
        key(&mut app, window, KeyCode::Escape, "");
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .companion_open());
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .blocks_gameplay_keys());
    }
    #[test]
    fn model_reply_cannot_invent_progress_and_unconfigured_api_keeps_settings_available() {
        let runtime = json!({"provider":"openai","configured":true,"modelEnabled":true,"modelTask":{"status":"completed","completedKills":0,"requestedKills":3,"step":"returning-to-owner","reply":"我已经击杀一百只怪物","lastCall":{"model":"gpt-6-luna","status":"success"}}});
        assert!(task_progress(&runtime).contains("0 / 3"));
        assert!(!task_status_label(&runtime).contains("击杀完成"));
        let lines = task_detail_lines(&runtime).join("");
        assert!(lines.contains("返回主人附近"));
        assert!(!lines.contains("returning-to-owner"));
        assert!(lines.contains("未计量"));
        for step in [
            "planning",
            "seeking-monster",
            "approaching-monster",
            "fighting",
            "returning-to-owner",
            "returning-home",
            "waiting",
            "following",
            "completed",
            "paused",
            "failed",
            "awaiting-new-instruction",
        ] {
            assert_ne!(task_step_label(step), "暂未分类的步骤");
        }
        let (mut app, _) = instruction_app();
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            controls.view.as_mut().unwrap().runtime =
                json!({"provider":"rules","configured":false,"modelEnabled":true});
            controls.revision += 1;
        }
        app.update();
        let world = app.world_mut();
        assert!(!world
            .query::<&Control>()
            .iter(world)
            .any(|c| *c == Control::SendInstruction));
        let lines: Vec<_> = world
            .query::<&Text>()
            .iter(world)
            .map(|t| t.0.clone())
            .collect();
        assert!(lines.iter().any(|line| line.contains("API 尚未配置")));
        press(&mut app, Control::ModelSettings);
        assert!(
            app.world()
                .resource::<CompanionUi>()
                .model_settings_requested
        );
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
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
    fn escape_closes_and_releases_companion_input() {
        let mut app = app(NativeShellScreen::InGame);
        press(&mut app, Control::Open);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .companion_open());
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .blocks_gameplay_keys());
    }
    #[test]
    fn a_stale_entry_cannot_replace_the_current_chat_focus() {
        let mut app = app(NativeShellScreen::InGame);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .set_chat_focused(true);
        // The old entry is still rendered until Read, but the live state
        // already belongs to chat. The click must not change its focus.
        press(&mut app, Control::Open);
        let player = app.world().resource::<NativePlayerUiState>();
        assert!(player.is_chat_focused());
        assert!(!player.companion_open());
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
    }
    #[test]
    fn keyboard_close_consumes_enter_before_the_later_chat_shortcut() {
        let mut app = app(NativeShellScreen::InGame);
        press(&mut app, Control::Open);
        assert_eq!(
            app.world().resource::<CompanionUi>().focus_actions.first(),
            Some(&Control::Close)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .companion_open());
        assert!(!app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::Enter));
    }
    #[test]
    fn an_old_rendered_teach_button_cannot_retarget_a_new_companion() {
        let mut app = app(NativeShellScreen::InGame);
        authorized_view(&mut app);
        app.update();
        {
            let mut controls = app.world_mut().resource_mut::<CompanionControls>();
            let entry = &mut controls.enrollments[0];
            entry.resident_id = "alo-2".into();
            entry.archive_id = "archive-2".into();
            controls.selected = Some("alo-2".into());
            let view = controls.view.as_mut().unwrap();
            view.archive.resident_id = "alo-2".into();
            view.archive.archive_id = "archive-2".into();
            view.profile.policy_version = 7;
            controls.revision += 1;
        }
        press(&mut app, Control::Teach);
        assert!(app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests()
            .is_empty());
        // After rendering the new identity, an explicit new click can act.
        press(&mut app, Control::Teach);
        let requests = app
            .world_mut()
            .resource_mut::<CompanionControls>()
            .drain_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].resident_id.as_deref(), Some("alo-2"));
        assert_eq!(requests[0].payload["expectedPolicyVersion"], 7);
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
        let withdrawal = episode(
            "decision",
            "retreat",
            "selected",
            "continue-unsafe-withdraw",
        );
        assert_eq!(
            episode_detail(&withdrawal),
            "原因：继续撤离，直到抵达安全处。"
        );
        assert_eq!(
            episode_heading(&withdrawal),
            "行动计划 · 撤离 · 已选择，尚未确认完成"
        );
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
