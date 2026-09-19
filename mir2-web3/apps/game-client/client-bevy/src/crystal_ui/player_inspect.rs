//! Crystal MainDialogs.cs InspectDialog: read-only authoritative player gear.
use super::super::*;
use mir2_protocol::PlayerInspectInfo;

#[derive(Debug, Clone)]
pub struct PlayerInspectReadback {
    pub info: PlayerInspectInfo,
    pub inventory: InventoryModel,
}
impl PartialEq for PlayerInspectReadback {
    fn eq(&self, other: &Self) -> bool {
        self.info == other.info
            && self.inventory.capacity == other.inventory.capacity
            && self.inventory.gold == other.inventory.gold
            && self.inventory.items == other.inventory.items
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerInspectUi {
    pub data: Option<PlayerInspectReadback>,
    expected_name: Option<String>,
    pub position: Vec2,
    drag: Option<Vec2>,
    size: Option<Vec2>,
    hits: Vec<(CrystalRect, InspectAction)>,
    pub notice: Option<String>,
}

impl Default for PlayerInspectUi {
    fn default() -> Self {
        Self {
            data: None,
            expected_name: None,
            position: Vec2::new(536.0, 0.0),
            drag: None,
            size: None,
            hits: Vec::new(),
            notice: None,
        }
    }
}

impl PlayerInspectUi {
    pub fn end_drag(&mut self) {
        self.drag = None;
    }
    pub fn covers_cursor(&self, cursor: Vec2) -> bool {
        self.is_open()
            && self.size.is_some_and(|size| {
                super::contains(
                    CrystalRect::new(self.position.x, self.position.y, size.x, size.y),
                    cursor,
                )
            })
    }
    pub fn request(&mut self, name: String) {
        self.expected_name = Some(name);
        self.data = None;
        self.notice = None;
    }
    pub fn receive(&mut self, data: PlayerInspectReadback) -> bool {
        if self.expected_name.as_deref() != Some(data.info.name.as_str()) {
            return false;
        }
        self.expected_name = None;
        self.data = Some(data);
        true
    }
    pub fn close(&mut self) {
        self.data = None;
        self.expected_name = None;
        self.drag = None;
        self.notice = None;
    }
    pub fn fail_unsent(&mut self) {
        self.expected_name = None;
    }
    pub fn is_open(&self) -> bool {
        self.data.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
enum InspectAction {
    Gear,
    Close,
    Group,
    Friend,
    Mail,
    Trade,
    Observe,
    Whisper,
}
#[derive(Component)]
pub struct PlayerInspectPanel;

pub(super) fn process(
    state: &mut NativePlayerUiState,
    cursor: Vec2,
    mouse: &ButtonInput<MouseButton>,
    intents: &mut NativePlayerUiIntentQueue,
) -> bool {
    let model = &mut state.ranking.player_inspect;
    if !model.is_open() {
        return false;
    }
    let local = cursor - model.position;
    let inside = model
        .size
        .is_some_and(|size| super::contains(CrystalRect::new(0.0, 0.0, size.x, size.y), local));
    if mouse.just_pressed(MouseButton::Left) {
        let action = model
            .hits
            .iter()
            .find(|(r, _)| super::contains(*r, local))
            .map(|(_, a)| *a);
        if let Some(action) = action {
            let info = model.data.as_ref().unwrap().info.clone();
            match action {
                InspectAction::Gear => {}
                InspectAction::Close => model.close(),
                InspectAction::Group => {
                    intents.push_intent(NativePlayerUiIntent::GroupAddMember { name: info.name });
                }
                InspectAction::Friend => {
                    intents.push_intent(NativePlayerUiIntent::AddFriend {
                        name: info.name,
                        blocked: false,
                    });
                }
                InspectAction::Trade => {
                    intents.push_intent(NativePlayerUiIntent::TradeRequest);
                }
                InspectAction::Observe => {
                    if info.allow_observe {
                        intents
                            .push_intent(NativePlayerUiIntent::ObservePlayer { name: info.name });
                    } else {
                        model.notice = Some("Player has disabled observation.".into());
                    }
                }
                InspectAction::Mail => {
                    state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);
                    state.apply(mir2_ui_core::action::UiAction::SetMailRecipient {
                        recipient: info.name,
                    });
                    state.ranking.player_inspect.close();
                }
                InspectAction::Whisper => {
                    state.chat_draft = format!("/{} ", info.name);
                    state.set_chat_focused(true);
                }
            }
            return true;
        } else if inside {
            model.drag = Some(local);
        }
    }
    let model = &mut state.ranking.player_inspect;
    if let Some(offset) = model.drag {
        model.position = cursor - offset;
    }
    if !mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
        model.drag = None;
    }
    inside || model.drag.is_some()
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut PlayerInspectUi,
    viewer: &UiReadModel,
    wing_materials: Option<&CrystalCharacterWingMaterials>,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) {
    let Some(data) = &model.data else {
        return;
    };
    let Some(size) = dimensions("Prguse", 430) else {
        return;
    };
    model.size = Some(size);
    model.hits.clear();
    let controls = [
        ("Prguse2", 360, 361, 362, 241.0, 3.0, InspectAction::Close),
        ("Prguse", 431, 432, 433, 55.0, 357.0, InspectAction::Group),
        ("Prguse", 434, 435, 436, 85.0, 357.0, InspectAction::Friend),
        ("Prguse", 437, 438, 439, 115.0, 357.0, InspectAction::Mail),
        ("Prguse", 523, 524, 525, 145.0, 357.0, InspectAction::Trade),
        ("Title", 854, 855, 856, 175.0, 357.0, InspectAction::Observe),
    ];
    if controls.iter().any(|(lib, n, h, p, ..)| {
        [*n, *h, *p]
            .into_iter()
            .any(|i| dimensions(lib, i).is_none())
    }) {
        return;
    }
    for &(lib, index, _, _, x, y, action) in &controls {
        if data.info.is_hero && action != InspectAction::Close {
            continue;
        }
        let d = dimensions(lib, index).unwrap();
        model.hits.push((CrystalRect::new(x, y, d.x, d.y), action));
    }
    model.hits.push((
        CrystalRect::new(50.0, 12.0, 190.0, 20.0),
        InspectAction::Whisper,
    ));
    for (_, mut rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
        rect.top -= 20.0;
        model.hits.push((rect, InspectAction::Gear));
    }
    let gender = match data.info.gender {
        mir2_protocol::MirGender::Male => "male",
        mir2_protocol::MirGender::Female => "female",
    };
    let class = match data.info.class {
        mir2_protocol::MirClass::Warrior => "Warrior",
        mir2_protocol::MirClass::Wizard => "Wizard",
        mir2_protocol::MirClass::Taoist => "Taoist",
        mir2_protocol::MirClass::Assassin => "Assassin",
        mir2_protocol::MirClass::Archer => "Archer",
    };
    let mut target_ui = viewer.clone();
    target_ui.player.name = Some(data.info.name.clone());
    target_ui.player.class_name = Some(class.into());
    target_ui.player.gender = Some(gender.into());
    target_ui.player.hair = Some(data.info.hair);
    target_ui.player.wing_effect = data
        .inventory
        .items
        .iter()
        .find(|i| i.slot == 1)
        .and_then(|i| i.tooltip_source.as_ref())
        .map(|s| s.real_info.as_ref().unwrap_or(&s.info).effect);
    parent
        .spawn((
            PlayerInspectPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(model.position.x),
                top: Val::Px(model.position.y),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            GlobalZIndex(OVERLAY_HELP_SORTED_Z + 1),
            FocusPolicy::Block,
        ))
        .with_children(|panel| {
            spawn_overlay_frame(panel, assets, "original-ui/Prguse/430.png", size.x, size.y);
            let page = if gender == "male" { 340 } else { 341 };
            if let Some(d) = dimensions("Prguse", page) {
                spawn_static_overlay_sprite(
                    panel,
                    assets,
                    format!("original-ui/Prguse/{page}.png"),
                    CrystalRect::new(8.0, 70.0, d.x, d.y),
                );
            }
            overlay_centered_text_at(
                panel,
                &data.info.name,
                CrystalRect::new(50.0, 12.0, 190.0, 20.0),
                32.0 / 3.0,
                TEXT,
            );
            overlay_centered_text_at(
                panel,
                &format!("{} {}", data.info.guild_name, data.info.guild_rank),
                CrystalRect::new(50.0, 33.0, 190.0, 30.0),
                32.0 / 3.0,
                TEXT,
            );
            let class_index = 100 + data.info.class as u16;
            if let Some(d) = dimensions("Prguse", class_index) {
                spawn_static_overlay_sprite(
                    panel,
                    assets,
                    format!("original-ui/Prguse/{class_index}.png"),
                    CrystalRect::new(15.0, 33.0, d.x, d.y),
                );
            }
            if !data.info.lover_name.is_empty() {
                if let Some(d) = dimensions("Prguse", 604) {
                    spawn_static_overlay_sprite(
                        panel,
                        assets,
                        "original-ui/Prguse/604.png".into(),
                        CrystalRect::new(17.0, 17.0, d.x, d.y),
                    );
                }
            }
            for &(library, normal, hover, pressed, x, y, action) in &controls {
                if data.info.is_hero && action != InspectAction::Close {
                    continue;
                }
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
            for (slot, mut rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
                rect.top -= 20.0;
                if let Some(item) = data.inventory.items.iter().find(|i| i.slot == slot) {
                    panel
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(rect.left),
                                top: Val::Px(rect.top),
                                width: Val::Px(rect.width),
                                height: Val::Px(rect.height),
                                ..default()
                            },
                            CrystalItemHint(crystal_item_tooltip_document(item, &viewer.player)),
                            Interaction::None,
                            FocusPolicy::Block,
                        ))
                        .with_children(|cell| {
                            if let Some(index) = item.user_item_image_index() {
                                let (marker, node, image) =
                                    original_item_image_bundle(assets, Some(index), 36, 32);
                                cell.spawn((marker, node, image));
                            }
                        });
                }
            }
            // Inspect AfterDraw's order is armour, wings, weapon, helmet or hair.
            let mut layers = crystal_character_paper_doll_layers(&data.inventory, &target_ui);
            if layers.len() > 1 && matches!(layers[0].blend, CrystalCharacterBlend::DrawBlend) {
                layers.swap(0, 1);
            }
            for mut layer in layers {
                if matches!(layer.blend, CrystalCharacterBlend::DrawBlend) {
                    if let Some(frame) = crystal_character_wing_frame(
                        target_ui.player.wing_effect,
                        viewer.player.gender.as_deref(),
                    ) {
                        layer.frame = frame;
                    }
                }
                layer.frame.rect.top -= 20.0;
                spawn_character_layer(panel, assets, wing_materials, layer);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(name: &str) -> PlayerInspectReadback {
        PlayerInspectReadback {
            info: PlayerInspectInfo {
                name: name.into(),
                guild_name: String::new(),
                guild_rank: String::new(),
                equipment: vec![None; 14],
                class: mir2_protocol::MirClass::Warrior,
                gender: mir2_protocol::MirGender::Male,
                hair: 0,
                level: 1,
                lover_name: String::new(),
                allow_observe: false,
                is_hero: false,
            },
            inventory: InventoryModel::default(),
        }
    }
    #[test]
    fn late_or_unsolicited_inspect_cannot_reopen_closed_or_different_player() {
        let mut model = PlayerInspectUi::default();
        assert!(!model.receive(data("A")));
        model.request("A".into());
        model.request("B".into());
        assert!(!model.receive(data("A")));
        assert!(model.receive(data("B")));
        model.close();
        assert!(!model.receive(data("B")));
        assert!(!model.is_open());
    }
    #[test]
    fn failed_dispatch_releases_only_pending_inspect_without_fabricating_equipment() {
        let mut model = PlayerInspectUi::default();
        model.request("A".into());
        model.fail_unsent();
        assert!(!model.receive(data("A")));
        assert!(model.data.is_none());
    }
    #[test]
    fn inspect_equipment_press_is_read_only_and_does_not_drag_parent() {
        let mut state = NativePlayerUiState::default();
        let model = &mut state.ranking.player_inspect;
        model.request("A".into());
        model.receive(data("A"));
        model.size = Some(Vec2::new(264.0, 380.0));
        model.hits.push((
            CrystalRect::new(131.0, 77.0, 36.0, 32.0),
            InspectAction::Gear,
        ));
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        let mut queue = NativePlayerUiIntentQueue::default();
        assert!(process(
            &mut state,
            Vec2::new(680.0, 85.0),
            &mouse,
            &mut queue
        ));
        assert!(queue.drain_intents().is_empty());
        assert!(state.ranking.player_inspect.drag.is_none());
    }
}
