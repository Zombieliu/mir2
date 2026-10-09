//! Crystal MainDialogs CharacterDuraPanel and BuffDialog authoritative HUD state.
use super::*;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Instant;
fn source() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("status_hud_source.json"))
            .expect("validated Crystal HUD metadata")
    })
}
pub fn buff_icon(name: &str) -> u16 {
    source()["buffIcons"][name].as_u64().unwrap_or(0) as u16
}
pub fn frame_size(library: &str, index: u16) -> Option<Vec2> {
    let f = &source()["frames"][format!("{library}/{index}")];
    Some(Vec2::new(
        f["width"].as_u64()? as f32,
        f["height"].as_u64()? as f32,
    ))
}
#[derive(Debug, Clone, PartialEq)]
pub struct Buff {
    pub kind: String,
    pub remaining: i64,
    pub received: Instant,
    pub infinite: bool,
    pub paused: bool,
    pub stats: Vec<(u8, String, i64)>,
    pub caster: Option<String>,
    pub values: Vec<i64>,
}
impl Buff {
    pub fn remaining(&self, now: Instant) -> i64 {
        if self.paused {
            self.remaining
        } else {
            self.remaining.saturating_sub(
                now.saturating_duration_since(self.received)
                    .as_millis()
                    .min(i64::MAX as u128) as i64,
            )
        }
    }
    pub fn blink_hidden(&self, now: Instant) -> bool {
        let ms = self.remaining(now);
        !self.paused
            && !self.infinite
            && (ms as f64 / 1000.).round_ties_even() <= 5.
            && (ms as f64 / 100.).round_ties_even() % 10. < 5.
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum BuffEvent {
    Add {
        owner: u32,
        buff: Buff,
    },
    Remove {
        owner: u32,
        kind: String,
    },
    Pause {
        owner: u32,
        kind: String,
        paused: bool,
        received: Instant,
    },
}
impl BuffEvent {
    pub fn parse(packet: &str, p: &Value, now: Instant) -> Option<Self> {
        let owner = u32::try_from(p.get("objectId")?.as_u64()?).ok()?;
        let kind_value = p.get("buffType").or_else(|| p.get("type"))?;
        let kind = if let Some(id) = kind_value.as_u64().filter(|id| *id <= 255) {
            source()["buffNames"][id.to_string()]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| id.to_string())
        } else {
            kind_value.as_str()?.to_owned()
        };
        Some(match packet {
            "AddBuff" => Self::Add {
                owner,
                buff: Buff {
                    kind,
                    remaining: p
                        .get("remainingMs")
                        .or_else(|| p.get("expireTime"))?
                        .as_i64()?,
                    received: now,
                    infinite: p.get("infinite")?.as_bool()?,
                    paused: p.get("paused")?.as_bool()?,
                    caster: None, // Caster is not serialized by original S.AddBuff.
                    stats: p
                        .get("stats")?
                        .as_array()?
                        .iter()
                        .map(|s| {
                            Some((
                                u8::try_from(s.get("stat")?.as_u64()?).ok()?,
                                s.get("label")?.as_str()?.to_string(),
                                s.get("value")?.as_i64()?,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?,
                    values: p
                        .get("values")?
                        .as_array()?
                        .iter()
                        .map(Value::as_i64)
                        .collect::<Option<Vec<_>>>()?,
                },
            },
            "RemoveBuff" => Self::Remove { owner, kind },
            "PauseBuff" => Self::Pause {
                owner,
                kind,
                paused: p.get("paused")?.as_bool()?,
                received: now,
            },
            _ => return None,
        })
    }
    fn owner(&self) -> u32 {
        match self {
            Self::Add { owner, .. } | Self::Remove { owner, .. } | Self::Pause { owner, .. } => {
                *owner
            }
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusHud {
    pub buffs: Vec<Buff>,
    pub dura_camera_override: bool,
    pub owner: Option<u32>,
    pub hovered: bool,
    pub pressed: Option<u8>,
    pub cursor: Option<Vec2>,
    pub opacity: f32,
    pub fade_at: Option<Instant>,
}
impl StatusHud {
    pub fn dura_visible(&self, preference: bool) -> bool {
        preference || self.dura_camera_override
    }
    pub fn restore_camera(&mut self) {
        self.dura_camera_override = true;
    }

    pub fn observe(&mut self, owner: Option<u32>, event: &BuffEvent, _now: Instant) {
        let Some(owner) = owner.filter(|id| *id != 0) else {
            return;
        };
        if self.owner != Some(owner) {
            self.buffs.clear();
            self.owner = Some(owner);
        }
        if owner != event.owner() {
            return;
        }
        match event {
            BuffEvent::Add { buff, .. } => {
                if let Some(old) = self.buffs.iter_mut().find(|b| b.kind == buff.kind) {
                    *old = buff.clone();
                } else {
                    self.buffs.push(buff.clone());
                }
            }
            BuffEvent::Remove { kind, .. } => self.buffs.retain(|b| b.kind != *kind),
            BuffEvent::Pause {
                kind,
                paused,
                received,
                ..
            } => {
                if let Some(buff) = self.buffs.iter_mut().find(|b| b.kind == *kind) {
                    if buff.paused != *paused {
                        buff.remaining = buff.remaining(*received);
                        buff.received = *received;
                        buff.paused = *paused;
                    }
                }
            }
        }
    }
}
/// Original source uses integer halves/fifths and hides exhausted equipment.
pub fn durability_frame(slot: u32, item: &ItemModel) -> Option<u16> {
    let src = item.tooltip_source.as_ref()?;
    let user = src.user_item.as_ref()?;
    let ty = src.info.item_type;
    if ty == 11 {
        return (user.current_dura == 0).then_some(2137);
    }
    let base = match (slot, ty) {
        (_, 1) => 2125,
        (_, 2) => 2149,
        (_, 4) => 2155,
        (_, 5) => 2122,
        (5 | 6, 6) => 2143,
        (7 | 8, 7) => 2131,
        (_, 8) => 2134,
        (_, 9) => 2158,
        (_, 10) => 2152,
        (_, 12) => 2146,
        (_, 19) => 2140,
        _ => return None,
    };
    let (current, max) = if ty == 8 {
        (item.quantity, u32::from(src.info.stack_size))
    } else {
        (u32::from(user.current_dura), u32::from(user.max_dura))
    };
    if current == 0 {
        return None;
    }
    Some(
        base + if current <= max / 5 {
            2
        } else if current <= max / 2 {
            1
        } else {
            0
        },
    )
}
const SLOTS: [(u32, f32, f32); 14] = [
    (0, 4., 5.),
    (1, 16., 11.),
    (2, 24., 3.),
    (3, 44., 5.),
    (4, 3., 67.),
    (5, 3., 43.),
    (6, 43., 43.),
    (7, 3., 54.),
    (8, 43., 54.),
    (9, 16., 54.),
    (10, 23., 23.),
    (11, 17., 43.),
    (12, 30., 54.),
    (13, 43., 68.),
];
#[derive(Component)]
pub struct StatusRoot;
pub(super) fn image(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    library: &str,
    index: u16,
    p: Vec2,
    alpha: f32,
) {
    let Some(size) = frame_size(library, index) else {
        return;
    };
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(p.x),
            top: Val::Px(p.y),
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            ..default()
        },
        ImageNode {
            image: assets.load(format!("original-ui/{library}/{index}.png")),
            color: Color::srgba(1., 1., 1., alpha),
            ..default()
        },
        bevy::ui::FocusPolicy::Pass,
    ));
}
pub(super) fn buff_geometry(count: usize, expanded: bool) -> (u16, Vec2) {
    // MirImageControl.AutoSize remains true: Size getter ignores explicit base.Size.
    let index = if count > 0 && expanded {
        20 + (count - 1).min(10) as u16
    } else {
        20
    };
    (index, {
        let f = &source()["frames"][format!("Prguse2/{index}")];
        Vec2::new(
            f["trueWidth"].as_u64().unwrap() as f32,
            f["trueHeight"].as_u64().unwrap() as f32,
        )
    })
}
fn buff_rect(state: &NativePlayerUiState) -> CrystalRect {
    let (_, s) = buff_geometry(
        state.status_hud.buffs.len(),
        state.core.options.expanded_buff_window,
    );
    CrystalRect::new(897. - s.x, 0., s.x, s.y)
}
fn button_at(state: &NativePlayerUiState, p: Vec2) -> Option<u8> {
    if CrystalRect::new(1004., 154., 19., 19.).contains(p.x, p.y) {
        return Some(0);
    }
    let rect = buff_rect(state);
    if !state.status_hud.buffs.is_empty()
        && CrystalRect::new(rect.left + rect.width - 15., 0., 15., 15.).contains(p.x, p.y)
    {
        return Some(1);
    }
    None
}
pub fn process(
    mut state: ResMut<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut effects: Option<ResMut<UiEffectQueue>>,
    mut audio: Option<ResMut<crate::audio::NativeUiAudioQueue>>,
    skills: Option<Res<SkillModel>>,
) {
    state.status_hud.hovered = false;
    // Packet-owned identity wins over a delayed/coalesced skill model.
    if let Some(owner) = state.status_hud.owner.or_else(|| {
        skills
            .as_deref()
            .map(|s| s.authority.player_object_id)
            .filter(|id| *id != 0)
    }) {
        state.status_hud.owner = Some(owner);
        let active = state.guild_panel.buffs.enabled.iter().any(|b| b.active);
        let now = Instant::now();
        if active && !state.status_hud.buffs.iter().any(|b| b.kind == "Guild") {
            state.status_hud.buffs.push(Buff {
                kind: "Guild".into(),
                remaining: 0,
                received: now,
                infinite: true,
                paused: false,
                stats: vec![],
                values: vec![],
                caster: Some("Guild".into()),
            });
        } else if !active {
            state
                .status_hud
                .buffs
                .retain(|b| b.kind != "Guild" || b.caster.is_none());
        }
    }

    if shell.screen != NativeShellScreen::InGame
        || state.local_keys.camera_hidden
        || !windows.single().is_ok_and(|w| w.focused)
    {
        state.status_hud.pressed = None;
        return;
    }
    let Some(p) = windows.single().ok().and_then(help_cursor_logical) else {
        return;
    };
    state.status_hud.cursor = Some(p);
    let buff = buff_rect(&state);
    let over_buff = !state.status_hud.buffs.is_empty() && buff.contains(p.x, p.y);
    let over_dura = state.status_hud.dura_visible(state.core.options.dura_view)
        && CrystalRect::new(963., 200., 61., 85.).contains(p.x, p.y);
    state.status_hud.hovered = over_buff || over_dura || button_at(&state, p).is_some();
    let now = Instant::now();
    if state.status_hud.fade_at.is_none_or(|t| now >= t) {
        state.status_hud.opacity =
            (state.status_hud.opacity + if over_buff { 0.2 } else { -0.2 }).clamp(0., 1.);
        state.status_hud.fade_at = Some(now + std::time::Duration::from_millis(55));
    }
    if state.blocks_gameplay_keys() {
        state.status_hud.pressed = None;
        return;
    }
    let Some(mouse) = mouse else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        state.status_hud.pressed = button_at(&state, p);
    }
    if mouse.just_released(MouseButton::Left) {
        if let Some(button) = state
            .status_hud
            .pressed
            .take()
            .filter(|b| Some(*b) == button_at(&state, p))
        {
            if let Some(audio) = audio.as_deref_mut() {
                audio.push(crate::audio::NativeUiSound::ButtonA);
            }
            if button == 0 {
                state.core.options.dura_view =
                    !state.status_hud.dura_visible(state.core.options.dura_view);
                state.status_hud.dura_camera_override = false;
            } else {
                state.core.options.expanded_buff_window =
                    state.status_hud.buffs.len() == 1 || !state.core.options.expanded_buff_window;
            }
            if let Some(e) = effects.as_deref_mut() {
                e.push(mir2_ui_core::effect::UiEffect::PersistOptions {
                    options: state.core.options.clone(),
                });
            }
        }
    }
}
pub fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<StatusRoot>>,
    state: Res<NativePlayerUiState>,
    inventory: Res<InventoryModel>,
    shell: Res<NativeShellModel>,
    assets: Option<Res<AssetServer>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let (Ok(root), Some(assets)) = (roots.single(), assets) else {
        return;
    };
    if shell.screen != NativeShellScreen::InGame || state.local_keys.camera_hidden {
        return;
    }
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                StatusRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(1024.),
                    height: Val::Px(768.),
                    ..default()
                },
                bevy::ui::FocusPolicy::Pass,
                GlobalZIndex(20),
            ))
            .with_children(|parent| {
                let hit = state.status_hud.cursor.and_then(|p| button_at(&state, p));
                let index = if hit == Some(0) {
                    if state.status_hud.pressed == Some(0) {
                        2112
                    } else {
                        2111
                    }
                } else if state.status_hud.dura_visible(state.core.options.dura_view) {
                    2110
                } else {
                    2113
                };
                image(parent, &assets, "Prguse", index, Vec2::new(1004., 154.), 1.);
                if state.status_hud.dura_visible(state.core.options.dura_view) {
                    image(parent, &assets, "Prguse", 2105, Vec2::new(963., 200.), 1.);
                    image(parent, &assets, "Prguse", 2161, Vec2::new(966., 203.), 0.4);
                    image(parent, &assets, "Prguse", 2162, Vec2::new(966., 203.), 1.);
                    for (slot, x, y) in SLOTS {
                        if let Some(item) = inventory
                            .items
                            .iter()
                            .find(|i| i.container == 2 && i.slot == slot)
                        {
                            if let Some(index) = durability_frame(slot, item) {
                                image(
                                    parent,
                                    &assets,
                                    "Prguse",
                                    index,
                                    Vec2::new(966. + x, 203. + y),
                                    1.,
                                );
                            }
                        }
                    }
                }
                let count = state.status_hud.buffs.len();
                if count == 0 {
                    return;
                }
                let (frame, size) = buff_geometry(count, state.core.options.expanded_buff_window);
                let rect = buff_rect(&state);
                image(
                    parent,
                    &assets,
                    "Prguse2",
                    frame,
                    Vec2::new(rect.left, 0.),
                    state.status_hud.opacity,
                );
                image(
                    parent,
                    &assets,
                    "Prguse2",
                    if hit == Some(1) {
                        if state.status_hud.pressed == Some(1) {
                            9
                        } else {
                            8
                        }
                    } else {
                        7
                    },
                    Vec2::new(rect.left + size.x - 15., 0.),
                    state.status_hud.opacity,
                );
                for (i, buff) in state.status_hud.buffs.iter().enumerate() {
                    if !state.core.options.expanded_buff_window && i > 0 {
                        break;
                    }
                    if buff.blink_hidden(Instant::now()) {
                        continue;
                    }
                    image(
                        parent,
                        &assets,
                        "BuffIcon",
                        buff_icon(&buff.kind),
                        Vec2::new(
                            rect.left + size.x - 33. - (i % 10) as f32 * 23.,
                            6. + (i / 10) as f32 * 24.,
                        ),
                        1.,
                    );
                }
                if !state.core.options.expanded_buff_window {
                    count_label(parent, count, CrystalRect::new(rect.left, 7., 43., 20.));
                }
            });
    });
}
#[cfg(test)]
#[path = "status_hud_tests.rs"]
mod tests;

fn text(key: &str) -> &'static str {
    source()["text"][key].as_str().unwrap_or("")
}
fn format_text(key: &str, args: &[String]) -> String {
    let mut s = text(key).to_string();
    for (i, v) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), v);
    }
    s
}
fn effect_line(id: u8, label: &str, value: i64) -> String {
    let name = source()["statNames"][id.to_string()].as_str().unwrap_or("");
    let suffix = if name.contains("Percent") {
        "%"
    } else if name.contains("Multiplier") {
        "x"
    } else {
        ""
    };
    format_text(
        "BuffEffect",
        &[
            text(if value < 0 { "Decreases" } else { "Increases" }).into(),
            label.into(),
            value.to_string(),
            suffix.into(),
        ],
    )
}
fn duration(seconds: i64) -> String {
    if seconds < 60 {
        format!("{}s", seconds % 60)
    } else if seconds < 3600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!(
            "{}h {:02}m {:02}s",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!(
            "{}d {:02}h {:02}m {:02}s",
            seconds / 86400,
            seconds / 3600 % 24,
            seconds / 60 % 60,
            seconds % 60
        )
    }
}
pub(super) fn buff_hint(buff: &Buff, now: Instant) -> String {
    let mut title = String::new();
    for (i, c) in buff.kind.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            title.push(' ');
        }
        title.push(c);
    }
    title.push('\n');
    let v = |id| {
        buff.stats
            .iter()
            .find(|(n, _, _)| *n == id)
            .map_or(0, |(_, _, v)| *v)
            .to_string()
    };
    let key = match buff.kind.as_str() {
        "Hiding" | "ClearRing" => Some("InvisibleToManyMonsters"),
        "MoonLight" => Some("InvisibleToPlayersAndMonstersAtDistance"),
        "DarkBody" => Some("InvisibleToManyMonstersAbleToMove"),
        "VampireShot" => Some("GivesVampiricAbility"),
        "PoisonShot" => Some("GivesPoisonAbility"),
        "Concentration" => Some("IncreaseElementExtractionChance"),
        "Transform" => Some("DisguisesYourAppearance"),
        "Mentee" => Some("LearnSkillPointsTwiceAsQuick"),
        "Blindness" => Some("ReducesVisibility"),
        "Newbie" => Some("GuildMemberBoost"),
        _ => None,
    };
    if let Some(key) = key {
        title.push_str(text(key));
    }
    match buff.kind.as_str() {
        "GameMaster" => {
            let flags = buff.values.first().copied().unwrap_or(0);
            for (flag, key) in [(1, "Invisible"), (4, "Superman"), (2, "Observer")] {
                if flags & flag != 0 {
                    title.push_str(text(key));
                }
            }
        }
        "MentalState" => {
            if let Some(key) = buff.values.first().and_then(|v| match v {
                0 => Some("AgressiveFullDamageCantShootOverWalls"),
                1 => Some("TrickShotMinimalDamage"),
                2 => Some("GroupModeMediumDamageDontStealAgro"),
                _ => None,
            }) {
                title.push_str(text(key));
            }
        }
        "EnergyShield" => {
            title.push_str(&format_text("ChanceGainHpWhenAttacked", &[v(125), v(126)]))
        }
        "MagicBooster" => title.push_str(&format_text(
            "IncreaseMcAndConsumption",
            &[v(6), v(7), v(127)],
        )),
        _ => {}
    }
    if !matches!(buff.kind.as_str(), "EnergyShield" | "MagicBooster") {
        for (id, label, value) in &buff.stats {
            title.push_str(&effect_line(*id, label, *value));
        }
    }
    if buff.paused {
        title.push_str(text("ExpirePaused"));
    } else if buff.infinite {
        title.push_str(text("ExpireNever"));
    } else {
        title.push_str(&format_text(
            "Expire",
            &[duration(
                (buff.remaining(now) as f64 / 1000.).round_ties_even() as i64,
            )],
        ));
    }
    if let Some(caster) = &buff.caster {
        title.push_str(&format_text("CasterName", &[caster.clone()]));
    }
    title
}
pub(super) fn combined_hint(buffs: &[Buff]) -> String {
    let mut stats = std::collections::BTreeMap::<u8, (String, i64)>::new();
    for b in buffs {
        for (id, label, value) in &b.stats {
            let entry = stats.entry(*id).or_insert((label.clone(), 0));
            entry.1 = entry.1.saturating_add(*value);
        }
    }
    let mut s = text("ActiveBuffs").to_string();
    for (id, (label, value)) in stats {
        if value != 0 {
            s.push_str(&effect_line(id, &label, value));
        }
    }
    s
}
#[derive(Component)]
pub struct StatusHint;
pub fn render_hint(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<(Entity, &ComputedNode), With<StatusHint>>,
    mut size: Local<Vec2>,
    state: Res<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
) {
    for (e, node) in &old {
        *size = node.size() * node.inverse_scale_factor();
        commands.entity(e).despawn();
    }
    if shell.screen != NativeShellScreen::InGame
        || state.local_keys.camera_hidden
        || state.blocks_gameplay_keys()
    {
        return;
    }
    let (Some(p), Ok(root)) = (state.status_hud.cursor, roots.single()) else {
        return;
    };
    let hint = if button_at(&state, p) == Some(0) {
        text("DuraPanel").to_string()
    } else {
        let rect = buff_rect(&state);
        let expanded = state.core.options.expanded_buff_window;
        let found = state.status_hud.buffs.iter().enumerate().find(|(i, b)| {
            if !expanded && *i > 0 {
                return false;
            }
            let sz = frame_size("BuffIcon", buff_icon(&b.kind)).unwrap_or(Vec2::ZERO);
            CrystalRect::new(
                rect.left + rect.width - 33. - (*i % 10) as f32 * 23.,
                6. + (*i / 10) as f32 * 24.,
                sz.x,
                sz.y,
            )
            .contains(p.x, p.y)
        });
        let Some((_, b)) = found else {
            return;
        };
        if expanded {
            let mut displayed = b.clone();
            if b.kind == "Guild" && b.caster.is_some() {
                let mut stats = std::collections::BTreeMap::<u8, i64>::new();
                for active in state.guild_panel.buffs.enabled.iter().filter(|b| b.active) {
                    if let Some(info) = state
                        .guild_panel
                        .buffs
                        .catalog
                        .iter()
                        .find(|i| i.id == active.id)
                    {
                        for stat in &info.stats {
                            *stats.entry(stat.stat).or_default() += i64::from(stat.value);
                        }
                    }
                }
                displayed.stats = stats
                    .into_iter()
                    .filter(|(_, v)| *v != 0)
                    .map(|(id, v)| (id, mir2_protocol::crystal_stat_label(id).to_string(), v))
                    .collect();
            }
            buff_hint(&displayed, Instant::now())
        } else {
            combined_hint(&state.status_hud.buffs)
        }
    };
    let point = crate::crystal_ui::widget::crystal_hint_position_for_bounds(
        crate::crystal_ui::widget::CrystalHintStyle::Control,
        p,
        *size,
        Vec2::new(1024., 768.),
    );
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                StatusHint,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(point.x),
                    top: Val::Px(point.y),
                    padding: UiRect::all(Val::Px(2.)),
                    border: UiRect::all(Val::Px(1.)),
                    ..default()
                },
                BackgroundColor(crate::crystal_ui::widget::CRYSTAL_HINT_BACKGROUND),
                BorderColor::all(crate::crystal_ui::widget::CRYSTAL_HINT_BORDER),
                GlobalZIndex(20000),
                bevy::ui::FocusPolicy::Pass,
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(hint),
                    crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                    TextColor(crate::crystal_ui::widget::CRYSTAL_HINT_TEXT),
                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                ));
            });
    });
}

/// Original collapsed BuffDialog count uses Arial 10pt Bold, centered.
pub(super) fn count_label(parent: &mut ChildSpawnerCommands, count: usize, rect: CrystalRect) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(count.to_string()),
                crate::crystal_ui::typography::crystal_text_font(40. / 3.)
                    .with_font_weight(bevy::text::FontWeight::BOLD),
                TextColor(Color::srgb(1., 1., 0.)),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ));
        });
}
