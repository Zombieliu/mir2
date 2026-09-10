use super::*;
use guild_buff_dialog::*;
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuildPanelUi {
    pub buffs: GuildBuffDialog,
    pub right_buff: bool,
    pub position: Option<Vec2>,
    pub guild_name: Option<String>,
    pub error: Option<GuildBuffError>,
    pub consumed: bool,
    pub notice_editor: friend_dialog::FriendDialogUi,
    pub notice_scroll: usize,
    pub rank_editor: friend_dialog::FriendDialogUi,
    pub recruit_editor: friend_dialog::FriendDialogUi,
    pub rank_dropdown: bool,
    pub rank_scroll: usize,
    pub rank_name_ready_ms: u64,
    pub rank_option_ready_ms: u64,
    pub now_ms: u64,
    pub cursor: Option<Vec2>,
    pub left_down: bool,
    pub create_rank: bool,
    pub member_menu: Option<u8>,
    pub member_scroll: usize,
    pub hide_offline: bool,
    pub member_change: Option<(String, u8, String)>,
    pub kick_member: Option<String>,
    pub owner_name: String,
    pub invite: Option<(String, u64)>,
    pub answered_invite: Option<(String, u64)>,
    pub create_ready_ms: u64,
}
impl GuildPanelUi {
    pub fn sync_notice(&mut self, draft: &str) {
        self.notice_editor.modal = Some(friend_dialog::FriendModal::Memo {
            character_index: -1,
            text: draft.into(),
        });
        self.notice_editor.sync_editor();
    }
    pub fn notice_draft(&self) -> String {
        self.notice_editor
            .editor
            .as_ref()
            .map(|e| e.text().to_owned())
            .unwrap_or_default()
    }
    pub fn sync_rank(&mut self, draft: &str) {
        let revision = self.rank_editor.editor_revision;
        self.rank_editor.modal = Some(friend_dialog::FriendModal::Add {
            blocked: false,
            text: draft.into(),
        });
        self.rank_editor.sync_editor();
        if revision != self.rank_editor.editor_revision {
            self.rank_editor.editor = Some(friend_dialog::text_editor::FriendTextEditor::new(
                draft.into(),
                20,
                false,
            ));
        }
    }
    pub fn sync_recruit(&mut self, draft: &str) {
        let revision = self.recruit_editor.editor_revision;
        self.recruit_editor.modal = Some(friend_dialog::FriendModal::Add {
            blocked: false,
            text: draft.into(),
        });
        self.recruit_editor.sync_editor();
        if revision != self.recruit_editor.editor_revision {
            self.recruit_editor.editor = Some(friend_dialog::text_editor::FriendTextEditor::new(
                draft.into(),
                20,
                false,
            ));
        }
    }
    pub fn rect(&self) -> CrystalRect {
        let mut r = CRYSTAL_GUILD_PANEL_RECT;
        if let Some(p) = self.position {
            r.left = p.x;
            r.top = p.y;
        }
        r
    }
    pub fn request(&mut self, request: GuildBuffRequest, intents: &mut NativePlayerUiIntentQueue) {
        if !intents.push_intent(NativePlayerUiIntent::GuildBuffUpdate(request)) {
            self.buffs.send_failed(request);
        }
    }
    pub fn activate(
        &mut self,
        row: u8,
        guild: &crate::social::GuildModel,
        now: u64,
        intents: &mut NativePlayerUiIntentQueue,
    ) {
        let authority = GuildBuffAuthority {
            level: guild.level,
            spare_points: i32::from(guild.spare_points),
            gold: i64::from(guild.gold),
            may_activate: guild.my_options & 128 != 0,
        };
        match self.buffs.request_row(usize::from(row), authority, now) {
            Ok(Some(request)) => self.request(request, intents),
            Err(error) => self.error = Some(error),
            _ => {}
        }
    }
    pub fn member_page_key(&mut self, key: KeyCode, count: usize) -> bool {
        let max = count.saturating_sub(18);
        self.member_scroll = match key {
            KeyCode::ArrowUp => self.member_scroll.saturating_sub(1),
            KeyCode::ArrowDown => (self.member_scroll + 1).min(max),
            KeyCode::Home => 0,
            KeyCode::End => max,
            KeyCode::PageUp => self.member_scroll.saturating_sub(25),
            KeyCode::PageDown => (self.member_scroll + 25).min(max),
            _ => return false,
        };
        self.member_menu = None;
        true
    }
    pub fn blocks(&self) -> bool {
        self.error.is_some() || self.create_rank || self.member_change.is_some() || self.consumed
    }
    pub fn close_error(&mut self) {
        self.create_rank = false;
        self.member_change = None;
        self.kick_member = None;
        self.error = None;
        self.consumed = true;
    }
}
#[derive(Default, Resource)]
pub struct GuildHost {
    drag: Option<Vec2>,
    thumb: Option<f32>,
    selection: bool,
    notice_thumb: Option<f32>,
    member_thumb: Option<f32>,
    rank_thumb: Option<f32>,
    buff_hover: Option<(usize, Vec2)>,
    hover_size: Vec2,
}
#[derive(Component)]
pub(super) struct GuildErrorPanel;
pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<GuildHost>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    social: Res<crate::social::SocialModel>,
    time: Option<Res<Time>>,
    ui: Option<Res<crate::read_model::UiReadModel>>,
    shell: Option<Res<NativeShellModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    blocks: Query<(
        &friend_dialog::view::FriendEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
) {
    if let Some(ui) = ui {
        state.guild_panel.owner_name = ui.player.name.clone().unwrap_or_default();
    }
    state.guild_panel.now_ms = time.map_or(0, |t| t.elapsed().as_millis() as u64);
    host.buff_hover = None;
    state.guild_panel.cursor = None;
    state.guild_panel.left_down = false;
    let wheel = wheel.read().map(|e| e.y).sum::<f32>();
    state.guild_panel.consumed = state.guild_panel.error.is_some()
        || state.guild_panel.create_rank
        || state.guild_panel.member_change.is_some()
        || state.guild_panel.kick_member.is_some();
    if state.guild_panel.guild_name != social.guild.name {
        if state.guild_panel.guild_name.is_some() {
            let position = state.guild_panel.position;
            state.guild_panel = GuildPanelUi {
                position,
                ..Default::default()
            };
            state.guild_notice_editing = false;
            state.guild_notice_submission = None;
            state.guild_notice_draft.clear();
            state.guild_rank_name_focused = false;
            state.guild_rank_name_draft.clear();
            state.selected_guild_rank = None;
            state.guild_recruit_focused = false;
            state.guild_recruit_draft.clear();
        }
        state.guild_panel.guild_name = social.guild.name.clone();
    }
    if let Some(name) = social.guild.pending_invite_from.as_ref() {
        let identity = (name.clone(), social.guild.pending_invite_epoch);
        if state.guild_panel.answered_invite.as_ref() != Some(&identity) {
            state.guild_panel.invite = Some(identity);
        }
    } else {
        state.guild_panel.invite = None;
        state.guild_panel.answered_invite = None;
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        *host = Default::default();
        return;
    }
    if social.guild.name.is_some() {
        if let Some(request) = state.guild_panel.buffs.request_list() {
            state.guild_panel.request(request, &mut intents);
        }
    }
    if !state.guild_open() {
        *host = Default::default();
        return;
    }
    let (Some(mouse), Ok(window)) = (mouse, windows.single()) else {
        return;
    };
    if !window.focused || state.amount_modal_open() || state.keyboard.open {
        *host = Default::default();
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    state.guild_panel.cursor = Some(cursor);
    state.guild_panel.left_down = mouse.pressed(MouseButton::Left);
    let rect = state.guild_panel.rect();
    if !state.guild_panel.right_buff && social_has_permission(&social.guild, "recruit") {
        let draft = state.guild_recruit_draft.clone();
        state.guild_panel.sync_recruit(&draft);
        state.guild_panel.recruit_editor.editor_focused = state.guild_recruit_focused;
        for (tag, block, info) in &blocks {
            friend_dialog::host::capture_layout(
                &mut state.guild_panel.recruit_editor,
                tag,
                block,
                info,
            );
        }
        let local = cursor - Vec2::new(rect.left + 395., rect.top + 360.);
        if mouse.just_pressed(MouseButton::Left) {
            let focus = CrystalRect::new(0., 0., 130., 21.).contains(local.x, local.y);
            state.guild_recruit_focused = focus;
            state.guild_panel.recruit_editor.editor_focused = focus;
            if focus {
                friend_dialog::host::focus_editor(
                    &mut state.guild_panel.recruit_editor,
                    local,
                    false,
                );
            }
        }
    }
    if state.guild_left_page == GuildLeftPage::Notice {
        if state.guild_notice_editing {
            let draft = state.guild_notice_draft.clone();
            state.guild_panel.sync_notice(&draft);
            for (tag, block, info) in &blocks {
                friend_dialog::host::capture_layout(
                    &mut state.guild_panel.notice_editor,
                    tag,
                    block,
                    info,
                );
            }
            let local = cursor - Vec2::new(rect.left + 13., rect.top + 61.);
            if mouse.just_pressed(MouseButton::Left) {
                let focus = CrystalRect::new(0., 0., 322., 330.).contains(local.x, local.y);
                state.guild_panel.notice_editor.editor_focused = focus;
                host.selection = focus;
                if focus {
                    friend_dialog::host::focus_editor(
                        &mut state.guild_panel.notice_editor,
                        local,
                        false,
                    );
                }
            }
            if mouse.pressed(MouseButton::Left) && host.selection {
                friend_dialog::host::focus_editor(
                    &mut state.guild_panel.notice_editor,
                    local,
                    true,
                );
            }
            if !mouse.pressed(MouseButton::Left) {
                host.selection = false;
            }
        }
        let count = if state.guild_notice_editing {
            state.guild_notice_draft.split('\n').count()
        } else {
            social.guild.notice.len()
        };
        let interval = 289usize.checked_div(count.saturating_sub(25)).unwrap_or(0);
        let thumb_y = (16 + state.guild_panel.notice_scroll * interval).min(298) as f32;
        let local = cursor - Vec2::new(rect.left, rect.top + 60.);
        if mouse.just_pressed(MouseButton::Left)
            && CrystalRect::new(337., thumb_y, 12., 18.).contains(local.x, local.y)
        {
            host.notice_thumb = Some(local.y - thumb_y);
        }
        if mouse.pressed(MouseButton::Left) {
            if let Some(offset) = host.notice_thumb {
                state.guild_panel.notice_scroll = if interval > 0 {
                    (((local.y - offset).clamp(16., 298.) - 16.) / interval as f32) as usize
                } else {
                    0
                };
                state.guild_panel.notice_scroll = state
                    .guild_panel
                    .notice_scroll
                    .min(count.saturating_sub(25));
                state.guild_panel.notice_editor.text_scroll[1] =
                    state.guild_panel.notice_scroll as f32 * 13.;
            }
        } else {
            host.notice_thumb = None;
        }
        if CrystalRect::new(rect.left + 13., rect.top + 61., 340., 330.)
            .contains(cursor.x, cursor.y)
            && wheel != 0.
        {
            let count = if state.guild_notice_editing {
                state.guild_notice_draft.lines().count()
            } else {
                social.guild.notice.len()
            };
            state.guild_panel.notice_scroll = if wheel > 0. {
                state.guild_panel.notice_scroll.saturating_sub(1)
            } else {
                (state.guild_panel.notice_scroll + 1).min(count.saturating_sub(25))
            };
            state.guild_panel.notice_editor.text_scroll[1] =
                state.guild_panel.notice_scroll as f32 * 13.;
        }
    }
    if state.guild_left_page == GuildLeftPage::Members {
        let count = social
            .guild
            .members
            .iter()
            .filter(|m| !state.guild_panel.hide_offline || m.online)
            .count();
        let max = count.saturating_sub(18);
        state.guild_panel.member_scroll = state.guild_panel.member_scroll.min(max);
        let interval = 289usize.checked_div(max).unwrap_or(0);
        let thumb_y = (16 + state.guild_panel.member_scroll * interval).min(298) as f32;
        let local = cursor - Vec2::new(rect.left, rect.top + 60.);
        if mouse.just_pressed(MouseButton::Left)
            && CrystalRect::new(337., thumb_y, 12., 18.).contains(local.x, local.y)
        {
            host.member_thumb = Some(local.y - thumb_y);
        }
        if mouse.pressed(MouseButton::Left) {
            if let Some(offset) = host.member_thumb {
                state.guild_panel.member_scroll = if interval > 0 {
                    (((local.y - offset).clamp(16., 298.) - 16.) / interval as f32) as usize
                } else {
                    0
                };
                state.guild_panel.member_scroll = state.guild_panel.member_scroll.min(max);
                state.guild_panel.member_menu = None;
            }
        } else {
            host.member_thumb = None;
        }
        if wheel != 0. && CrystalRect::new(0., 0., 352., 332.).contains(local.x, local.y) {
            state.guild_panel.member_scroll = if wheel > 0. {
                state.guild_panel.member_scroll.saturating_sub(1)
            } else {
                (state.guild_panel.member_scroll + 1).min(max)
            };
            state.guild_panel.member_menu = None;
        }
    }
    if state.guild_left_page == GuildLeftPage::Ranks {
        let draft = state.guild_rank_name_draft.clone();
        state.guild_panel.sync_rank(&draft);
        state.guild_panel.rank_editor.editor_focused = state.guild_rank_name_focused;
        for (tag, block, info) in &blocks {
            friend_dialog::host::capture_layout(
                &mut state.guild_panel.rank_editor,
                tag,
                block,
                info,
            );
        }
        let local = cursor - Vec2::new(rect.left + 42., rect.top + 96.);
        if mouse.just_pressed(MouseButton::Left) {
            let focus = CrystalRect::new(0., 0., 130., 16.).contains(local.x, local.y)
                && state
                    .selected_guild_rank
                    .is_some_and(|r| i32::from(r) >= social.guild.my_rank_id)
                && social_has_permission(&social.guild, "changeRank")
                && state.guild_panel.now_ms >= state.guild_panel.rank_name_ready_ms;
            state.guild_rank_name_focused = focus;
            state.guild_panel.rank_editor.editor_focused = focus;
            host.selection = focus;
            if focus {
                friend_dialog::host::focus_editor(&mut state.guild_panel.rank_editor, local, false);
            }
        }
        if mouse.pressed(MouseButton::Left) && host.selection {
            friend_dialog::host::focus_editor(&mut state.guild_panel.rank_editor, local, true);
        }
        if !mouse.pressed(MouseButton::Left) {
            host.selection = false;
        }
        if state.guild_panel.rank_dropdown {
            let count = social.guild.ranks.len() + 1;
            let local = cursor - Vec2::new(rect.left + 198., rect.top + 96.);
            let thumb_y = 22.
                + state.guild_panel.rank_scroll as f32 * 52.
                    / count.saturating_sub(1).max(1) as f32;
            if mouse.just_pressed(MouseButton::Left)
                && CrystalRect::new(119., thumb_y, 8., 14.).contains(local.x, local.y)
            {
                host.rank_thumb = Some(local.y - thumb_y);
            }
            if mouse.pressed(MouseButton::Left) {
                if let Some(offset) = host.rank_thumb {
                    state.guild_panel.rank_scroll = (((local.y - offset).clamp(20., 60.) - 15.)
                        / (66. / count.saturating_sub(1).max(1) as f32))
                        as usize;
                    state.guild_panel.rank_scroll =
                        state.guild_panel.rank_scroll.min(count.saturating_sub(5));
                }
            } else {
                host.rank_thumb = None;
            }
        }
        if state.guild_panel.rank_dropdown
            && !CrystalRect::new(rect.left + 198., rect.top + 96., 130., 82.)
                .contains(cursor.x, cursor.y)
        {
            state.guild_panel.rank_dropdown = false;
        }
    }
    if mouse.just_pressed(MouseButton::Left)
        && CrystalRect::new(rect.left, rect.top, 565., 30.).contains(cursor.x, cursor.y)
    {
        host.drag = Some(cursor - Vec2::new(rect.left, rect.top));
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = host.drag {
            state.guild_panel.position = Some(cursor - offset);
        }
    } else {
        host.drag = None;
        host.thumb = None;
    }
    if !state.guild_panel.right_buff {
        return;
    }
    let local = cursor - Vec2::new(rect.left + 360., rect.top + 61.);
    if local.x >= 4. && local.x < 192. && local.y >= 27. {
        let row = ((local.y - 27.) / 38.).floor() as usize;
        if row < 8
            && (local.y - 27.) % 38. < 33.
            && state
                .guild_panel
                .buffs
                .catalog
                .get(state.guild_panel.buffs.start_index + row)
                .is_some()
        {
            host.buff_hover = Some((row, cursor));
        }
    }
    let y = state.guild_panel.buffs.thumb_y(18) as f32;
    if mouse.just_pressed(MouseButton::Left)
        && CrystalRect::new(203., y, 12., 18.).contains(local.x, local.y)
    {
        host.thumb = Some(local.y - y);
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = host.thumb {
            state
                .guild_panel
                .buffs
                .drag_thumb((local.y - offset) as i32, 18);
        }
    }
    if CrystalRect::new(0., 0., 216., 332.).contains(local.x, local.y) && wheel != 0. {
        state
            .guild_panel
            .buffs
            .wheel(if wheel > 0. { 120 } else { -120 });
    }
}
pub(super) fn render_buff(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    state: &NativePlayerUiState,
    guild: &crate::social::GuildModel,
    rect: CrystalRect,
) {
    let x = rect.left + 360.;
    let y = rect.top + 61.;
    spawn_static_overlay_sprite(
        parent,
        assets,
        "original-ui/Prguse/1853.png".into(),
        CrystalRect::new(x, y, 216., 332.),
    );
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x + 118.),
            top: Val::Px(y + 3.),
            width: Val::Px(100.),
            height: Val::Px(20.),
            ..default()
        },
        Text::new(guild.spare_points.to_string()),
        crate::crystal_ui::typography::crystal_text_font(32. / 3.),
        TextColor(Color::WHITE),
        TextLayout::justify(Justify::Center),
    ));
    for (index, row) in state.guild_panel.buffs.rows(guild.level).iter().enumerate() {
        let top = y + 27. + index as f32 * 38.;
        parent
            .spawn((
                Button,
                OverlayButton::GuildBuffActivate(index as u8),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x + 4.),
                    top: Val::Px(top),
                    width: Val::Px(188.),
                    height: Val::Px(33.),
                    ..default()
                },
            ))
            .with_children(|p| {
                if (0..48).contains(&row.icon) {
                    spawn_static_overlay_sprite(
                        p,
                        assets,
                        format!("original-ui/GuildSkill/{}.png", row.icon),
                        CrystalRect::new(1., 0., 36., 34.),
                    );
                }
                overlay_text_at(
                    p,
                    &row.name,
                    CrystalRect::new(35., 1., 153., 15.),
                    32. / 3.,
                    Color::WHITE,
                );
                let label = match row.status {
                    GuildBuffRowStatus::InsufficientLevel => "Insufficient Level",
                    GuildBuffRowStatus::Available => "Available",
                    GuildBuffRowStatus::CountingDown => "Counting Down",
                    GuildBuffRowStatus::Expired => "Expired",
                    GuildBuffRowStatus::Obtained => "Obtained",
                };
                overlay_text_at(
                    p,
                    label,
                    CrystalRect::new(35., 17., 112., 15.),
                    32. / 3.,
                    if row.warning_red {
                        Color::srgb(1., 0., 0.)
                    } else {
                        Color::WHITE
                    },
                );
                if let Some(active) = row.active {
                    overlay_text_at(
                        p,
                        if active { "Active" } else { "Inactive" },
                        CrystalRect::new(140., 17., 60., 15.),
                        32. / 3.,
                        Color::WHITE,
                    );
                }
            });
    }
    for (base, top, action) in [
        (197, 24., OverlayButton::GuildBuffScroll(-1)),
        (207, 317., OverlayButton::GuildBuffScroll(1)),
        (
            205,
            state.guild_panel.buffs.thumb_y(18) as f32,
            OverlayButton::GuildBuffThumb,
        ),
    ] {
        let spec = CrystalButtonSpec::new(
            "Prguse2",
            base,
            base + 1,
            if base == 205 { 206 } else { base + 2 },
            CrystalRect::new(x + 203., y + top, 12., if base == 205 { 18. } else { 12. }),
            12.,
            if base == 205 { 18. } else { 12. },
        );
        spawn_crystal_image_button(
            parent,
            assets,
            spec,
            CrystalButtonAssetSet::from_spec(spec),
            action,
            false,
            true,
        );
    }
}
fn buff_hint(model: &GuildBuffDialog, row: usize) -> String {
    let Some(info) = model.catalog.get(model.start_index + row) else {
        return String::new();
    };
    let mut lines = vec![info.name.clone()];
    if info.level_requirement > 0 {
        lines.push(format!("Minimum Guild Level: {}", info.level_requirement));
    }
    if info.points_requirement > 0 {
        lines.push(format!("Points Required: {}", info.points_requirement));
    }
    if info.activation_cost > 0 {
        lines.push(format!("Activation Cost: {} gold.", info.activation_cost));
    }
    if info.time_limit > 0 {
        lines.push(
            if let Some(active) = model.enabled.iter().find(|b| b.id == info.id && b.active) {
                format!("Time Remaining: {} minutes", active.active_time_remaining)
            } else {
                format!("Buff Length: {} minutes.", info.time_limit)
            },
        );
    }
    for stat in &info.stats {
        let name = mir2_protocol::crystal_stat_label(stat.stat);
        lines.push(format!(
            "{} {} by: {}{}.",
            if stat.value < 0 {
                "Decreases"
            } else {
                "Increases"
            },
            name,
            stat.value,
            if name.contains("Percent") { "%" } else { "" }
        ));
    }
    lines.join("\n")
}
pub(super) fn render_error(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<GuildErrorPanel>>,
    state: Res<NativePlayerUiState>,
    assets: Option<Res<AssetServer>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let (Some(assets), Ok(root)) = (assets, roots.single()) else {
        return;
    };
    let invite_text = state
        .guild_panel
        .invite
        .as_ref()
        .map(|(name, _)| format!("Do you want to join the {name} guild?"));
    let kick_text = state
        .guild_panel
        .kick_member
        .as_ref()
        .map(|name| format!("Are you sure you want to kick {name}?"));
    let member_text = state
        .guild_panel
        .member_change
        .as_ref()
        .map(|(name, _, rank)| {
            format!("Are you sure you want to change the rank of {name} to {rank}?")
        });
    let text = if let Some(text) = invite_text.as_deref() {
        text
    } else if let Some(text) = kick_text.as_deref() {
        text
    } else if let Some(text) = member_text.as_deref() {
        text
    } else if state.guild_panel.create_rank {
        "Are you sure you want to create a new rank?"
    } else {
        let Some(error) = state.guild_panel.error else {
            return;
        };
        match error {
            GuildBuffError::InsufficientPointsAvailable => "Insufficient points available.",
            GuildBuffError::GuildLevelTooLow => "Guild level too low.",
            GuildBuffError::GuildRankNoBuffActivation => {
                "Guild rank does not allow buff activation."
            }
            GuildBuffError::BuffIsActive => "Buff is still active.",
            GuildBuffError::GuildFundsInsufficient => "Insufficient guild funds.",
        }
    };
    commands.entity(root).with_children(|root| {
        root.spawn((
            GuildErrorPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(284.),
                top: Val::Px(289.),
                width: Val::Px(456.),
                height: Val::Px(190.),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(1098),
        ))
        .with_children(|p| {
            spawn_overlay_frame(p, &assets, "original-ui/Prguse/360.png", 456., 190.);
            friend_dialog::view::wrapped_text(
                p,
                text,
                CrystalRect::new(35., 35., 390., 110.),
                Color::WHITE,
            );
            let buttons = if state.guild_panel.invite.is_some() {
                vec![
                    (206, 260., OverlayButton::GuildInviteAccept),
                    (210, 360., OverlayButton::GuildInviteDecline),
                ]
            } else if state.guild_panel.kick_member.is_some() {
                vec![
                    (206, 260., OverlayButton::GuildKickConfirm),
                    (210, 360., OverlayButton::GuildBuffErrorClose),
                ]
            } else if state.guild_panel.member_change.is_some() {
                vec![
                    (206, 260., OverlayButton::GuildMemberRankConfirm),
                    (210, 360., OverlayButton::GuildBuffErrorClose),
                ]
            } else if state.guild_panel.create_rank {
                vec![
                    (206, 260., OverlayButton::GuildCreateRankConfirm),
                    (210, 360., OverlayButton::GuildBuffErrorClose),
                ]
            } else {
                vec![(200, 360., OverlayButton::GuildBuffErrorClose)]
            };
            for (frame, x, action) in buttons {
                let spec = CrystalButtonSpec::new(
                    "Title",
                    frame,
                    frame + 1,
                    frame + 2,
                    CrystalRect::new(x, 157., 76., 25.),
                    76.,
                    25.,
                );
                spawn_crystal_image_button(
                    p,
                    &assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    action,
                    false,
                    true,
                );
            }
        });
    });
}

pub(super) fn save_rank_name(
    state: &mut NativePlayerUiState,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let Some(rank_index) = state.selected_guild_rank else {
        return;
    };
    if i32::from(rank_index) < social.guild.my_rank_id
        || !social_has_permission(&social.guild, "changeRank")
        || state.guild_panel.now_ms < state.guild_panel.rank_name_ready_ms
    {
        return;
    }
    let name = state.guild_rank_name_draft.clone();
    if name.is_empty() || name.encode_utf16().count() > 20 {
        return;
    }
    if intents.push_social_pending(
        social,
        NativePlayerUiIntent::GuildEditMember {
            change_type: 3,
            rank_index,
            name: String::new(),
            rank_name: name,
        },
    ) {
        state.guild_panel.rank_name_ready_ms = state.guild_panel.now_ms + 5000;
        state.guild_rank_name_focused = false;
        state.guild_panel.rank_editor.editor_focused = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_member_pages_advance_twenty_five_and_clamp_to_eighteen_visible_rows() {
        let mut panel = GuildPanelUi::default();
        assert!(panel.member_page_key(KeyCode::PageDown, 100));
        assert_eq!(panel.member_scroll, 25);
        panel.member_page_key(KeyCode::End, 100);
        assert_eq!(panel.member_scroll, 82);
        panel.member_page_key(KeyCode::PageUp, 100);
        assert_eq!(panel.member_scroll, 57);
        panel.member_page_key(KeyCode::End, 5);
        assert_eq!(panel.member_scroll, 0);
    }
    #[test]
    fn guild_invitation_response_is_bound_to_exact_inviter_epoch() {
        let mut state = NativePlayerUiState::default();
        let mut social = crate::social::SocialModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        social.guild.pending_invite_from = Some("Knights".into());
        social.guild.pending_invite_epoch = 2;
        state.guild_panel.invite = Some(("Knights".into(), 1));
        answer_invite(&mut state, &mut social, &mut queue, true);
        assert!(queue.drain_intents().is_empty());
        state.guild_panel.invite = Some(("Knights".into(), 2));
        answer_invite(&mut state, &mut social, &mut queue, false);
        assert!(matches!(
            &queue.drain_intents()[..],
            [NativePlayerUiIntent::GuildInvite {
                accept_invite: false
            }]
        ));
        assert!(state.guild_panel.invite.is_none());
        assert_eq!(social.guild.pending_invite_from.as_deref(), Some("Knights"));
    }
    #[test]
    fn rank_rename_uses_type_three_and_cannot_edit_a_senior_rank() {
        let mut state = NativePlayerUiState::default();
        let mut social = crate::social::SocialModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        social.guild.permissions = vec!["changeRank".into()];
        social.guild.my_rank_id = 1;
        state.selected_guild_rank = Some(0);
        state.guild_rank_name_draft = "Officer".into();
        save_rank_name(&mut state, &mut social, &mut queue);
        assert!(queue.drain_intents().is_empty());
        state.selected_guild_rank = Some(1);
        save_rank_name(&mut state, &mut social, &mut queue);
        assert!(
            matches!(&queue.drain_intents()[..],[NativePlayerUiIntent::GuildEditMember{change_type:3,rank_index:1,rank_name,..}] if rank_name=="Officer")
        );
        assert_eq!(state.guild_panel.rank_name_ready_ms, 5000);
    }
    #[test]
    fn member_rank_change_is_only_sent_after_confirmation_with_original_fields() {
        let mut state = NativePlayerUiState::default();
        let mut social = crate::social::SocialModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        confirm_member_rank(&mut state, &mut social, &mut queue);
        assert!(queue.drain_intents().is_empty());
        state.guild_panel.member_change = Some(("Alice".into(), 2, "Recruit".into()));
        assert!(state.guild_panel.blocks());
        confirm_member_rank(&mut state, &mut social, &mut queue);
        assert!(
            matches!(&queue.drain_intents()[..],[NativePlayerUiIntent::GuildEditMember{change_type:2,rank_index:2,name,rank_name}] if name=="Alice"&&rank_name=="Recruit")
        );
        assert!(state.guild_panel.member_change.is_none());
    }
    #[test]
    fn source_notice_edit_and_submission_have_separate_limits() {
        assert_eq!(guild_notice_lines(""), Vec::<String>::new());
        assert_eq!(guild_notice_lines("a\n"), vec!["a", ""]);
        let mut panel = GuildPanelUi::default();
        let text = vec!["long line over 32 characters must remain intact "; 201].join("\n");
        panel.sync_notice(&text);
        assert_eq!(panel.notice_draft(), text);
        assert_eq!(guild_notice_lines(&text).len(), 201);
        panel.notice_editor.editor.as_mut().unwrap().select_all();
        panel.notice_editor.paste("a\r\n\r\nb  ");
        assert_eq!(
            guild_notice_lines(&panel.notice_draft()),
            vec!["a", "", "b  "]
        );
    }
    #[test]
    fn source_rank_edit_budget_is_twenty_utf16_and_selection_survives_sync() {
        let mut panel = GuildPanelUi::default();
        panel.sync_rank("rank");
        panel.rank_editor.editor.as_mut().unwrap().select_all();
        panel.sync_rank("rank");
        panel.rank_editor.paste(&"x".repeat(30));
        assert_eq!(
            panel.rank_editor.editor.as_ref().unwrap().text(),
            "x".repeat(20)
        );
    }
    #[test]
    fn guild_buff_queue_failure_releases_list_retry_without_optimistic_unlock() {
        let mut panel = GuildPanelUi::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        for _ in 0..MAX_QUEUED {
            assert!(
                queue.push_intent(NativePlayerUiIntent::GuildBuffUpdate(GuildBuffRequest {
                    action: 1,
                    id: 9
                }))
            );
        }
        let request = panel.buffs.request_list().unwrap();
        panel.request(request, &mut queue);
        assert!(panel.buffs.request_list().is_some());
        assert!(panel.buffs.enabled.is_empty());
    }
}

pub(super) fn confirm_create_rank(
    state: &mut NativePlayerUiState,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
) {
    if !state.guild_panel.create_rank {
        return;
    }
    if intents.push_social_pending(
        social,
        NativePlayerUiIntent::GuildEditMember {
            change_type: 4,
            rank_index: 0,
            name: String::new(),
            rank_name: format!("Rank-{}", social.guild.ranks.len().saturating_sub(1)),
        },
    ) {
        state.guild_panel.create_ready_ms = state.guild_panel.now_ms + 5000;
        state.guild_panel.close_error();
    }
}

pub(super) fn confirm_member_rank(
    state: &mut NativePlayerUiState,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let Some((name, rank_index, rank_name)) = state.guild_panel.member_change.clone() else {
        return;
    };
    if intents.push_social_pending(
        social,
        NativePlayerUiIntent::GuildEditMember {
            change_type: 2,
            rank_index,
            name,
            rank_name,
        },
    ) {
        state.guild_panel.create_ready_ms = state.guild_panel.now_ms + 5000;
        state.guild_panel.close_error();
    }
}

pub(super) fn member_status(member: &crate::social::GuildMemberModel) -> String {
    if member.online {
        return "Online".into();
    }
    let ticks = (member.last_login_binary_datetime as u64 & 0x3fff_ffff_ffff_ffff) as i64;
    let seconds = ticks / 10_000_000 - 62_135_596_800;
    let Some(last) = chrono::DateTime::from_timestamp(seconds, 0) else {
        return String::new();
    };
    let days = (chrono::Local::now().naive_local()
        - last.with_timezone(&chrono::Local).naive_local())
    .num_days();
    match days {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        n => format!("{n} days ago"),
    }
}

pub(super) fn confirm_kick(
    state: &mut NativePlayerUiState,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let Some(name) = state.guild_panel.kick_member.clone() else {
        return;
    };
    if intents.push_social_pending(
        social,
        NativePlayerUiIntent::GuildEditMember {
            change_type: 1,
            rank_index: 0,
            name,
            rank_name: String::new(),
        },
    ) {
        state.guild_panel.create_ready_ms = state.guild_panel.now_ms + 5000;
        state.guild_panel.close_error();
    }
}

pub(super) fn answer_invite(
    state: &mut NativePlayerUiState,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
    accept: bool,
) {
    let Some(identity) = state.guild_panel.invite.clone() else {
        return;
    };
    if social.guild.pending_invite_from.as_ref() != Some(&identity.0)
        || social.guild.pending_invite_epoch != identity.1
    {
        return;
    }
    if intents.push_social_pending(
        social,
        NativePlayerUiIntent::GuildInvite {
            accept_invite: accept,
        },
    ) {
        state.guild_panel.answered_invite = Some(identity);
        state.guild_panel.invite = None;
        state.guild_panel.consumed = true;
    }
}

#[derive(Component)]
pub(super) struct GuildDropdownOption;
pub(super) fn dropdown_row(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    action: OverlayButton,
    pointer: (Option<Vec2>, bool),
) {
    parent
        .spawn((
            GuildDropdownOption,
            Button,
            action,
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                ..default()
            },
            BackgroundColor(
                if !pointer.1 && pointer.0.is_some_and(|p| rect.contains(p.x, p.y)) {
                    Color::srgb_u8(140, 70, 0)
                } else {
                    Color::srgb_u8(20, 20, 20)
                },
            ),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text),
                crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                TextColor(Color::WHITE),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
        });
}

#[derive(Component)]
pub(super) struct GuildBuffHint;
pub(super) fn render_buff_hint(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<(Entity, &ComputedNode), With<GuildBuffHint>>,
    mut host: ResMut<GuildHost>,
    state: Res<NativePlayerUiState>,
) {
    for (entity, node) in &old {
        host.hover_size = node.size() * node.inverse_scale_factor();
        commands.entity(entity).despawn();
    }
    let (Some((row, cursor)), Ok(root)) = (host.buff_hover, roots.single()) else {
        return;
    };
    if state.guild_panel.blocks() {
        return;
    }
    let text = buff_hint(&state.guild_panel.buffs, row);
    let point = Vec2::new(
        (cursor.x + 15.).min(1024. - host.hover_size.x),
        cursor.y.min(768. - host.hover_size.y),
    );
    commands.entity(root).with_children(|p| {
        p.spawn((
            GuildBuffHint,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(point.x),
                top: Val::Px(point.y),
                padding: UiRect::all(Val::Px(4.)),
                border: UiRect::all(Val::Px(1.)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(50, 50, 50, 178)),
            BorderColor::all(Color::srgb_u8(128, 128, 128)),
            GlobalZIndex(1200),
            FocusPolicy::Pass,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text),
                crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                TextColor(Color::WHITE),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
        });
    });
}

pub(super) fn closed_dropdown(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    text: &str,
    rect: CrystalRect,
    action: OverlayButton,
    enabled: bool,
    shade: u8,
) {
    let mut row = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(shade, shade, shade)),
    ));
    if enabled {
        row.insert((Button, action, FocusPolicy::Block));
    }
    row.with_children(|p| {
        overlay_text_at(
            p,
            text,
            CrystalRect::new(0., 0., rect.width - 16., 15.),
            32. / 3.,
            Color::WHITE,
        );
        if enabled {
            spawn_overlay_crystal_button_enabled(
                p,
                assets,
                "Prguse2",
                207,
                208,
                209,
                CrystalRect::new(rect.width - 18., 2., 12., 12.),
                action,
                true,
            );
        }
    });
}
pub(super) fn member_text(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    color: Color,
    shade: u8,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(shade, shade, shade)),
        Text::new(text),
        crate::crystal_ui::typography::crystal_text_font(28. / 3.),
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    ));
}
