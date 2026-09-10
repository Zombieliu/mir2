//! Crystal RankingDialog.cs: authoritative twenty-row ranking window.
//! Included as a child of overlays. Packet dispatch and session reset belong to the host.
use super::*;
use mir2_protocol::{ClientPacket, MirClass, RankCharacterInfo, ServerPacket};
#[path = "player_inspect.rs"]
pub mod player_inspect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum RankingAction {
    Close,
    Class(u8),
    OnlineOnly,
    Previous,
    Next,
    Inspect(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingQuery {
    pub rank_type: u8,
    pub row_offset: i32,
    pub online_only: bool,
}

/// Only one untagged ranking request is outstanding. This prevents a late response
/// for the same class but an old filter/page from being painted as the new page.
#[derive(Debug, Clone, Resource, PartialEq)]
pub struct RankingDialogUi {
    pub player_inspect: player_inspect::PlayerInspectUi,
    pub open: bool,
    pub position: Option<Vec2>,
    pub query: RankingQuery,
    pub count: i32,
    pub my_rank: i32,
    pub rows: Vec<RankCharacterInfo>,
    requested: Option<RankingQuery>,
    due_ms: Option<u64>,
    drag_offset: Option<Vec2>,
    hit_regions: Vec<(CrystalRect, RankingAction)>,
    thumb_rect: Option<CrystalRect>,
    size: Option<Vec2>,
    thumb_dragging: bool,
    inspect_ready_ms: u64,
}

impl Default for RankingDialogUi {
    fn default() -> Self {
        Self {
            player_inspect: Default::default(),
            open: false,
            position: None,
            query: RankingQuery {
                rank_type: 0,
                row_offset: 0,
                online_only: false,
            },
            count: 0,
            my_rank: 0,
            rows: Vec::new(),
            requested: None,
            due_ms: None,
            drag_offset: None,
            hit_regions: Vec::new(),
            thumb_rect: None,
            size: None,
            thumb_dragging: false,
            inspect_ready_ms: 0,
        }
    }
}

impl RankingDialogUi {
    pub fn covers_cursor(&self, cursor: Vec2) -> bool {
        (self.open
            && self
                .position
                .zip(self.size)
                .is_some_and(|(p, s)| contains(CrystalRect::new(p.x, p.y, s.x, s.y), cursor)))
            || self.player_inspect.covers_cursor(cursor)
    }
    pub fn show(&mut self, now_ms: u64) {
        if !self.open {
            self.open = true;
            self.due_ms = Some(now_ms);
        }
    }

    pub fn hide(&mut self) {
        self.open = false;
        self.due_ms = None;
        self.drag_offset = None;
        self.thumb_dragging = false;
    }

    /// Pass the shared inspect cooldown, not a separate per-panel cooldown.
    pub fn action(
        &mut self,
        action: RankingAction,
        now_ms: u64,
        inspect_ready_ms: &mut u64,
    ) -> Option<ClientPacket> {
        if !self.open {
            return None;
        }
        match action {
            RankingAction::Close => self.hide(),
            RankingAction::Class(kind) if kind < 6 => {
                self.query.rank_type = kind;
                self.query.row_offset = 0;
                self.count = 0;
                self.rows.clear();
                self.due_ms = Some(now_ms);
            }
            RankingAction::OnlineOnly => {
                self.query.online_only = !self.query.online_only;
                self.query.row_offset = 0;
                self.count = 0;
                self.rows.clear();
                self.due_ms = Some(now_ms);
            }
            RankingAction::Previous => self.move_rows(-1, now_ms),
            RankingAction::Next => self.move_rows(1, now_ms),
            RankingAction::Inspect(row) => {
                if self.due_ms.is_some() || self.requested.is_some() || now_ms <= *inspect_ready_ms
                {
                    return None;
                }
                let object_id = u32::try_from(self.rows.get(row)?.player_id).ok()?;
                self.player_inspect.request(self.rows[row].name.clone());
                *inspect_ready_ms = now_ms.saturating_add(500);
                return Some(ClientPacket::Inspect {
                    object_id,
                    ranking: true,
                    hero: false,
                });
            }
            _ => {}
        }
        None
    }

    /// Crystal moves one row in the indicated direction even for a multi-notch wheel event.
    pub fn move_rows(&mut self, direction: i32, now_ms: u64) {
        if !self.open || direction == 0 {
            return;
        }
        let next = (self.query.row_offset + direction.signum())
            .clamp(0, self.count.saturating_sub(20).max(0));
        if next != self.query.row_offset {
            self.query.row_offset = next;
            self.rows.clear();
            self.due_ms = Some(now_ms.saturating_add(500));
        }
    }

    pub fn thumb_y(&self) -> f32 {
        let extra = self.count.saturating_sub(20).max(0);
        if extra == 0 {
            113.0
        } else {
            113.0 + 254.0 * self.query.row_offset as f32 / extra as f32
        }
    }

    pub fn drag_thumb(&mut self, local_y: f32, now_ms: u64) {
        if !self.open || !local_y.is_finite() {
            return;
        }
        let extra = self.count.saturating_sub(20).max(0);
        let row = (((local_y.clamp(110.0, 368.0) - 113.0) / 254.0) * extra as f32).max(0.0) as i32;
        let row = row.min(extra);
        if row != self.query.row_offset {
            self.query.row_offset = row;
            self.rows.clear();
            self.due_ms = Some(now_ms.saturating_add(500));
        }
    }

    pub fn begin_drag(&mut self, cursor: Vec2) {
        if self.open {
            self.drag_offset = self.position.map(|position| cursor - position);
        }
    }

    pub fn drag_to(&mut self, cursor: Vec2) {
        if let Some(offset) = self.drag_offset {
            self.position = Some(cursor - offset);
        }
    }

    pub fn end_drag(&mut self) {
        self.drag_offset = None;
    }

    pub fn poll_request(&mut self, now_ms: u64) -> Option<ClientPacket> {
        if !self.open || self.requested.is_some() || self.due_ms.is_none_or(|due| now_ms < due) {
            return None;
        }
        self.due_ms = None;
        self.requested = Some(self.query);
        Some(ClientPacket::GetRanking {
            rank_type: self.query.rank_type,
            rank_index: self.query.row_offset,
            online_only: self.query.online_only,
        })
    }

    /// Invoke only when the host knows the request was never sent.
    pub fn release_unsent(&mut self, packet: &ClientPacket, now_ms: u64) {
        let Some(request) = self.requested else {
            return;
        };
        if matches!(packet, ClientPacket::GetRanking { rank_type, rank_index, online_only } if *rank_type == request.rank_type && *rank_index == request.row_offset && *online_only == request.online_only)
        {
            self.requested = None;
            if self.open {
                self.due_ms = Some(now_ms);
            }
        }
    }

    pub fn apply_packet(&mut self, packet: &ServerPacket) -> bool {
        let ServerPacket::Rankings {
            rank_type,
            my_rank,
            listing_details,
            count,
            ..
        } = packet
        else {
            return false;
        };
        let Some(request) = self.requested else {
            return false;
        };
        if request.rank_type != *rank_type {
            return false;
        }
        self.requested = None;
        if request != self.query {
            return true;
        }
        self.count = (*count).max(0);
        self.my_rank = (*my_rank).max(0);
        let available = self.count.saturating_sub(request.row_offset).max(0) as usize;
        self.rows = listing_details
            .iter()
            .take(20.min(available))
            .cloned()
            .collect();
        true
    }
}

#[derive(Component)]
pub struct RankingPanel;
#[derive(Component)]
pub struct RankingThumb;

/// All frame sizes come from the decoded original asset manifest. Missing assets
/// abort the panel, rather than silently substituting a fabricated window.
pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut RankingDialogUi,
    viewport: Vec2,
    owner_name: &str,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> bool {
    if !model.open {
        return false;
    }
    let controls = [
        ("Prguse2", 360, 361, 362, 300.0, 3.0, RankingAction::Close),
        ("Title", 751, 753, 752, 10.0, 38.0, RankingAction::Class(0)),
        ("Title", 760, 762, 761, 40.0, 38.0, RankingAction::Class(3)),
        ("Title", 754, 756, 755, 60.0, 38.0, RankingAction::Class(1)),
        ("Title", 763, 765, 764, 80.0, 38.0, RankingAction::Class(2)),
        ("Title", 757, 759, 758, 100.0, 38.0, RankingAction::Class(4)),
        ("Title", 766, 768, 767, 120.0, 38.0, RankingAction::Class(5)),
        (
            "Prguse2",
            197,
            198,
            199,
            299.0,
            100.0,
            RankingAction::Previous,
        ),
        ("Prguse2", 207, 208, 209, 299.0, 386.0, RankingAction::Next),
    ];
    let Some(size) = dimensions("Title", 728) else {
        return false;
    };
    for &(library, normal, hover, pressed, ..) in &controls {
        if [normal, hover, pressed]
            .into_iter()
            .any(|index| dimensions(library, index).is_none())
        {
            return false;
        }
    }
    if [
        ("Prguse2", 205),
        ("Prguse2", 206),
        ("Prguse", 2086),
        ("Prguse", 2087),
    ]
    .into_iter()
    .any(|(library, index)| dimensions(library, index).is_none())
    {
        return false;
    }
    let position = *model.position.get_or_insert((viewport - size) / 2.0);
    model.size = Some(size);
    model.hit_regions.clear();
    for &(library, normal, _, _, x, y, action) in &controls {
        let d = dimensions(library, normal).unwrap();
        model
            .hit_regions
            .push((CrystalRect::new(x, y, d.x, d.y), action));
    }
    model.hit_regions.push((
        CrystalRect::new(190.0, size.y - 20.0, size.x - 190.0, 20.0),
        RankingAction::OnlineOnly,
    ));
    for row in 0..model.rows.len() {
        model.hit_regions.push((
            CrystalRect::new(32.0, 98.0 + row as f32 * 15.0, 270.0, 15.0),
            RankingAction::Inspect(row),
        ));
    }
    let thumb_size = dimensions("Prguse2", 205).unwrap();
    model.thumb_rect = Some(CrystalRect::new(
        299.0,
        model.thumb_y(),
        thumb_size.x,
        thumb_size.y,
    ));
    parent
        .spawn((
            RankingPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(position.x),
                top: Val::Px(position.y),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(OVERLAY_HELP_SORTED_Z),
        ))
        .with_children(|panel| {
            spawn_overlay_frame(panel, assets, "original-ui/Title/728.png", size.x, size.y);
            for &(library, normal, hover, pressed, x, y, action) in &controls {
                let d = dimensions(library, normal).unwrap();
                let spec = CrystalButtonSpec::new(
                    library,
                    normal,
                    hover,
                    pressed,
                    CrystalRect::new(x, y, d.x, d.y),
                    d.x,
                    d.y,
                );
                spawn_crystal_image_button(
                    panel,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    action,
                    false,
                    true,
                );
            }
            let d = dimensions("Prguse2", 205).unwrap();
            let spec = CrystalButtonSpec::new(
                "Prguse2",
                205,
                206,
                206,
                CrystalRect::new(299.0, model.thumb_y(), d.x, d.y),
                d.x,
                d.y,
            );
            spawn_crystal_image_button(
                panel,
                assets,
                spec,
                CrystalButtonAssetSet::from_spec(spec),
                RankingThumb,
                false,
                true,
            );
            let index = if model.query.online_only { 2087 } else { 2086 };
            let d = dimensions("Prguse", index).unwrap();
            let spec = CrystalButtonSpec::new(
                "Prguse",
                index,
                index,
                index,
                CrystalRect::new(190.0, size.y - 20.0, d.x, d.y),
                d.x,
                d.y,
            );
            spawn_crystal_image_button(
                panel,
                assets,
                spec,
                CrystalButtonAssetSet::from_spec(spec),
                RankingAction::OnlineOnly,
                false,
                true,
            );
            overlay_text_at(
                panel,
                "Online Only",
                CrystalRect::new(190.0 + d.x + 2.0, size.y - 20.0, 100.0, 16.0),
                32.0 / 3.0,
                Color::WHITE,
            );
            let rank = if model.my_rank == 0 {
                "Not Listed".into()
            } else {
                format!("Ranked {}", model.my_rank)
            };
            overlay_centered_text_at(
                panel,
                &rank,
                CrystalRect::new(229.0, 36.0, 82.0, 22.0),
                40.0 / 3.0,
                Color::srgb_u8(222, 184, 135),
            );
            for (i, row) in model.rows.iter().enumerate() {
                let rank = model.query.row_offset + i as i32 + 1;
                let color = row_color(rank, &row.name, owner_name);
                panel
                    .spawn((
                        RankingAction::Inspect(i),
                        Interaction::None,
                        FocusPolicy::Block,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(32.0),
                            top: Val::Px(98.0 + i as f32 * 15.0),
                            width: Val::Px(270.0),
                            height: Val::Px(15.0),
                            ..default()
                        },
                    ))
                    .with_children(|p| {
                        let class = match row.class {
                            MirClass::Warrior => "Warrior",
                            MirClass::Wizard => "Wizard",
                            MirClass::Taoist => "Taoist",
                            MirClass::Assassin => "Assassin",
                            MirClass::Archer => "Archer",
                        };
                        for (x, width, text) in [
                            (0.0, 55.0, rank.to_string()),
                            (55.0, 95.0, row.name.clone()),
                            (150.0, 70.0, class.into()),
                            (220.0, 50.0, row.level.to_string()),
                        ] {
                            overlay_text_at(
                                p,
                                &text,
                                CrystalRect::new(x, 0.0, width, 15.0),
                                32.0 / 3.0,
                                color,
                            );
                        }
                    });
            }
        });
    true
}

fn row_color(rank: i32, name: &str, owner_name: &str) -> Color {
    // Original's final else-if makes the owner's first/second place green,
    // while third place remains RosyBrown.
    if rank == 3 {
        Color::srgb_u8(188, 143, 143)
    } else if name == owner_name {
        Color::srgb_u8(0, 128, 0)
    } else if rank == 1 {
        Color::srgb_u8(255, 215, 0)
    } else if rank == 2 {
        Color::srgb_u8(192, 192, 192)
    } else {
        Color::WHITE
    }
}

fn contains(rect: CrystalRect, point: Vec2) -> bool {
    point.x >= rect.left
        && point.y >= rect.top
        && point.x < rect.left + rect.width
        && point.y < rect.top + rect.height
}

pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    shell: Option<Res<NativeShellModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut wheel: MessageReader<MouseWheel>,
    time: Option<Res<Time>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut audio: Option<ResMut<crate::ui_audio::NativeUiAudioQueue>>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    let now = time
        .as_ref()
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        state.ranking = RankingDialogUi::default();
        wheel.clear();
        return;
    }
    let blocked = state.amount_modal_open() || state.trade_dialog.message.is_some();
    if !blocked {
        if let (Ok(window), Some(mouse)) = (windows.single(), mouse.as_ref()) {
            if window.focused {
                if let Some(cursor) = help_cursor_logical(window) {
                    if player_inspect::process(&mut state, cursor, mouse, &mut intents) {
                        if let Some(text) = state.ranking.player_inspect.notice.take() {
                            if let Some(chat) = chat.as_deref_mut() {
                                chat.push(crate::chat::ChatLine {
                                    text,
                                    channel: "system".into(),
                                });
                            }
                        }
                        if mouse.just_pressed(MouseButton::Left) {
                            if let Some(audio) = audio.as_deref_mut() {
                                audio.push(crate::ui_audio::NativeUiSound::ButtonA);
                            }
                        }
                        wheel.clear();
                        return;
                    }
                }
            }
        }
    }
    let model = &mut state.ranking;
    if let Some(packet) = model.poll_request(now) {
        enqueue(model, &mut intents, packet, now);
    }
    if !model.open || blocked {
        model.end_drag();
        model.player_inspect.end_drag();
        model.thumb_dragging = false;
        wheel.clear();
        return;
    }
    let Ok(window) = windows.single() else {
        wheel.clear();
        return;
    };
    if !window.focused {
        model.end_drag();
        model.player_inspect.end_drag();
        model.thumb_dragging = false;
        wheel.clear();
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        wheel.clear();
        return;
    };
    let Some(position) = model.position else {
        wheel.clear();
        return;
    };
    let local = cursor - position;
    let inside = model
        .size
        .is_some_and(|s| contains(CrystalRect::new(0.0, 0.0, s.x, s.y), local));
    for event in wheel.read() {
        if inside && event.y != 0.0 {
            model.move_rows(if event.y > 0.0 { -1 } else { 1 }, now);
        }
    }
    let Some(mouse) = mouse else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        if model.thumb_rect.is_some_and(|r| contains(r, local)) {
            model.thumb_dragging = true;
        } else if let Some((_, action)) = model
            .hit_regions
            .iter()
            .find(|(rect, _)| contains(*rect, local))
            .copied()
        {
            if let Some(audio) = audio.as_deref_mut() {
                audio.push(crate::ui_audio::NativeUiSound::ButtonA);
            }
            let mut cooldown = model.inspect_ready_ms;
            let packet = model.action(action, now, &mut cooldown);
            model.inspect_ready_ms = cooldown;
            if let Some(packet) = packet {
                enqueue(model, &mut intents, packet, now);
            }
        } else if inside {
            model.begin_drag(cursor);
        }
    }
    if mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
        if model.thumb_dragging {
            model.drag_thumb(local.y, now);
        } else {
            model.drag_to(cursor);
        }
    }
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        model.end_drag();
        model.thumb_dragging = false;
    }
}

fn enqueue(
    model: &mut RankingDialogUi,
    intents: &mut NativePlayerUiIntentQueue,
    packet: ClientPacket,
    now: u64,
) {
    let intent = match &packet {
        ClientPacket::GetRanking {
            rank_type,
            rank_index,
            online_only,
        } => NativePlayerUiIntent::GetRanking {
            rank_type: *rank_type,
            rank_index: *rank_index,
            online_only: *online_only,
        },
        ClientPacket::Inspect { object_id, .. } => NativePlayerUiIntent::InspectRanking {
            object_id: *object_id,
        },
        _ => return,
    };
    if !intents.push_intent(intent) {
        model.release_unsent(&packet, now);
        if matches!(packet, ClientPacket::Inspect { .. }) {
            model.player_inspect.fail_unsent();
        }
    }
}

pub(super) fn render_system(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    panels: Query<Entity, Or<(With<RankingPanel>, With<player_inspect::PlayerInspectPanel>)>>,
    mut state: ResMut<NativePlayerUiState>,
    shell: Option<Res<NativeShellModel>>,
    asset_server: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    wing_materials: Option<Res<CrystalCharacterWingMaterials>>,
    ui: Res<UiReadModel>,
    mut handles: Local<std::collections::HashMap<(String, u16), Handle<Image>>>,
) {
    for panel in &panels {
        commands.entity(panel).despawn();
    }
    if (!state.ranking.open && !state.ranking.player_inspect.is_open())
        || shell.is_none_or(|s| s.screen != NativeShellScreen::InGame)
    {
        return;
    }
    let (Some(assets), Some(images)) = (asset_server, images) else {
        return;
    };
    let Ok(root) = roots.single() else {
        return;
    };
    // Request the complete set together; no progressively half-painted page.
    for (library, indices) in [
        (
            "Title",
            vec![
                728, 751, 752, 753, 754, 755, 756, 757, 758, 759, 760, 761, 762, 763, 764, 765,
                766, 767, 768, 854, 855, 856,
            ],
        ),
        (
            "Prguse2",
            vec![360, 361, 362, 197, 198, 199, 207, 208, 209, 205, 206],
        ),
        (
            "Prguse",
            vec![
                2086, 2087, 430, 340, 341, 431, 432, 433, 434, 435, 436, 437, 438, 439, 523, 524,
                525, 604, 100, 101, 102, 103, 104,
            ],
        ),
    ] {
        for index in indices {
            handles
                .entry((library.to_owned(), index))
                .or_insert_with(|| assets.load(format!("original-ui/{library}/{index}.png")));
        }
    }
    commands.entity(root).with_children(|parent| {
        render(
            parent,
            &assets,
            &mut state.ranking,
            Vec2::new(1024.0, 768.0),
            ui.player.name.as_deref().unwrap_or(""),
            |library, index| {
                handles
                    .get(&(library.to_owned(), index))
                    .and_then(|handle| images.get(handle))
                    .map(|image| Vec2::new(image.width() as f32, image.height() as f32))
            },
        );
        player_inspect::render(
            parent,
            &assets,
            &mut state.ranking.player_inspect,
            &ui,
            wing_materials.as_deref(),
            |library, index| {
                handles
                    .get(&(library.to_owned(), index))
                    .and_then(|handle| images.get(handle))
                    .map(|image| Vec2::new(image.width() as f32, image.height() as f32))
            },
        );
    });
}

#[cfg(test)]
#[path = "ranking_dialog_tests.rs"]
mod tests;
