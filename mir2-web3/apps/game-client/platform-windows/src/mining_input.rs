//! Crystal mine-wall AutoHit and held Shift attacks share the current gesture
//! fence, never a FIFO of future swings.

use mir2_client_bevy::inventory::InventoryModel;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EquippedMiningTool {
    pub unique_id: u64,
    pub item_index: i32,
}

/// A name, icon, bag item or unknown durability cannot establish CanMine.
/// Weapon is Crystal equipment slot zero. Read the exported ItemInfo flag.
pub(crate) fn equipped_mining_tool(
    inventory: Option<&InventoryModel>,
) -> Option<EquippedMiningTool> {
    let mut weapons = inventory?
        .items
        .iter()
        .filter(|item| item.container == 2 && item.slot == 0);
    let weapon = weapons.next()?;
    if weapons.next().is_some()
        || weapon.quantity != 1
        || weapon
            .equip_slot
            .as_deref()
            .is_some_and(|slot| !slot.eq_ignore_ascii_case("weapon"))
        || weapon
            .durability_current
            .is_none_or(|durability| durability == 0)
    {
        return None;
    }
    let source = weapon.tooltip_source.as_ref()?;
    if source.info.item_type != 1 || !source.info.can_mine {
        return None;
    }
    Some(EquippedMiningTool {
        unique_id: weapon.unique_id.filter(|id| *id != 0)?,
        item_index: source.info.item_index,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectionalAttackGesture {
    pub id: u64,
    pub owner: String,
    pub owner_stamp: Option<crate::gateway::NativeCommandStamp>,
    pub direction: &'static str,
    pub origin: (i32, i32),
    pub map_file: String,
    pub mining_tool: Option<EquippedMiningTool>,
}

impl DirectionalAttackGesture {
    pub fn mining(&self) -> bool {
        self.mining_tool.is_some()
    }

    pub fn owner_is_current(
        &self,
        owner: &str,
        stamp: Option<crate::gateway::NativeCommandStamp>,
    ) -> bool {
        self.owner == owner && self.owner_stamp == stamp
    }

    pub fn remains_current(
        &self,
        map_file: Option<&str>,
        origin: (i32, i32),
        inventory: Option<&InventoryModel>,
    ) -> bool {
        map_file == Some(self.map_file.as_str())
            && self.origin == origin
            && self
                .mining_tool
                .as_ref()
                .is_none_or(|tool| equipped_mining_tool(inventory).as_ref() == Some(tool))
    }
}

pub(crate) fn mining_interval_ms(level: u32) -> f64 {
    // Crystal PlayerObject Mine deliberately ignores AttackSpeed bonuses.
    (1400 - (i64::from(level) * 14).min(370)) as f64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectionalAttackDecision {
    Unhandled,
    Consumed,
    Turn(&'static str),
}

#[cfg(test)]
impl DirectionalAttackDecision {
    fn handled(self) -> bool {
        self != Self::Unhandled
    }
}

pub(crate) struct DirectionalAttackInput<'a> {
    pub object_id: &'a str,
    pub origin: (i32, i32),
    pub owner_stamp: Option<crate::gateway::NativeCommandStamp>,
    pub owner_ready: bool,
    pub mining_ui_open: bool,
    pub press_grid: Option<(i32, i32)>,
    pub press_grid_empty: bool,
    pub inventory: Option<&'a InventoryModel>,
    pub read_model: Option<&'a mir2_client_bevy::read_model::UiReadModel>,
    pub click_state: Option<&'a crate::gameplay_bridge::NativeWorldClickState>,
    pub left_pressed: bool,
    pub left_held: bool,
    pub shift: bool,
    pub now_ms: f64,
    pub motion_now_ms: u64,
}

pub(crate) fn handle_directional_attack(
    movement: &mut super::WorldPointerMovementState,
    queue: Option<&mut mir2_client_bevy::quest_ui::QuestUiIntentQueue>,
    presentation: &mut crate::entity_presentation::NativeEntityPresentation,
    input: DirectionalAttackInput<'_>,
) -> DirectionalAttackDecision {
    handle_directional_attack_with_collision(
        movement,
        queue,
        presentation,
        input,
        crate::map_parser::map_cell_blocks_player_movement,
    )
}

fn handle_directional_attack_with_collision(
    movement: &mut super::WorldPointerMovementState,
    mut queue: Option<&mut mir2_client_bevy::quest_ui::QuestUiIntentQueue>,
    presentation: &mut crate::entity_presentation::NativeEntityPresentation,
    input: DirectionalAttackInput<'_>,
    cell_blocks: impl Fn(&str, i32, i32) -> Option<bool>,
) -> DirectionalAttackDecision {
    use mir2_client_bevy::quest_ui::QuestUiIntent;
    use DirectionalAttackDecision::{Consumed, Turn, Unhandled};
    let origin = movement.authoritative_position.unwrap_or(input.origin);
    let map_file = presentation.current_map_file_name().map(str::to_owned);
    let had_gesture = movement.directional_attack.is_some();
    let valid_actor = input.owner_ready
        && movement.self_object_id.as_deref() == Some(input.object_id)
        && input
            .read_model
            .is_some_and(|model| model.player.max_hp > 0 && model.player.hp > 0)
        && input.click_state.is_some_and(|state| {
            state.riding_mount == Some(false)
                && state.dazed == Some(false)
                && state.fishing == Some(false)
        });
    let current = movement.directional_attack.as_ref().is_none_or(|gesture| {
        gesture.owner_is_current(input.object_id, input.owner_stamp)
            && gesture.remains_current(map_file.as_deref(), origin, input.inventory)
            && if gesture.mining() {
                !input.mining_ui_open
                    && !input.shift
                    && movement
                        .authoritative_direction
                        .as_deref()
                        .is_some_and(|facing| facing.eq_ignore_ascii_case(gesture.direction))
            } else {
                input.left_held && input.shift
            }
    });
    if !valid_actor || !current {
        if had_gesture {
            cancel(movement, queue.as_deref_mut(), presentation, input.now_ms);
        }
        return if had_gesture { Consumed } else { Unhandled };
    }

    // An accepted down edge owns its real grid. Never borrow the final cursor
    // of a coalesced down/move/up batch to choose a mining direction.
    let hover = if input.left_pressed {
        input.press_grid
    } else {
        presentation.hovered_grid_position()
    };
    let continuing_mining = movement
        .directional_attack
        .as_ref()
        .is_some_and(|gesture| gesture.mining());
    // AutoHit belongs to the accepted wall, not to wherever the cursor travels
    // after that click. A fresh press still has to establish an empty target.
    let empty_hover = continuing_mining
        || (presentation.hovered_object_id().is_none()
            && (!input.left_pressed || input.press_grid_empty));
    let direction = movement
        .directional_attack
        .as_ref()
        .filter(|gesture| gesture.mining())
        .map(|gesture| gesture.direction)
        .or_else(|| super::movement_direction_toward(hover, origin));
    let tool = equipped_mining_tool(input.inventory);
    let blocked_wall = tool.is_some()
        && map_file
            .as_deref()
            .zip(direction)
            .is_some_and(|(map, direction)| {
                let wall = super::movement_target(origin, direction, 1);
                cell_blocks(map, wall.0, wall.1) == Some(true)
            });
    // Shift has Crystal's earlier forced-attack precedence even with a pickaxe.
    let mining = !input.shift
        && !input.mining_ui_open
        && empty_hover
        && (continuing_mining || hover.is_some())
        && blocked_wall
        && tool.is_some();
    let archer_bow = input.click_state.is_some_and(|state| {
        state.has_class_weapon == Some(true)
            && state
                .class
                .as_deref()
                .is_some_and(|class| class.eq_ignore_ascii_case("Archer"))
    });
    let direction_attack =
        empty_hover && (mining || (input.left_held && input.shift && !archer_bow));
    if !direction_attack || direction.is_none() || map_file.is_none() {
        if had_gesture {
            cancel(movement, queue.as_deref_mut(), presentation, input.now_ms);
            return Consumed;
        }
        return Unhandled;
    }
    // A canceled AutoHit cannot rearm on release, a stale queued request or a
    // still-held button. A current walk may reach a wall with its original hold.
    if !had_gesture
        && !input.left_pressed
        && !(input.left_held && movement.active == Some(super::WorldPointerMovementMode::Walk))
    {
        return Unhandled;
    }
    let direction = direction.unwrap();
    let map_file = map_file.unwrap();
    if mining && !had_gesture {
        movement.stop_hold(input.now_ms, "miningClick");
        movement.stop_auto_path(input.now_ms, "miningClick");
        movement.attack_target = None;
        movement.harvest_target = None;
        movement.harvest_direction = None;
        if !movement.pending.is_empty() {
            return Consumed;
        }
        let Some(facing) = movement.authoritative_direction.as_deref() else {
            return Consumed;
        };
        if !facing.eq_ignore_ascii_case(direction) {
            // Source CheckInput returns after Standing. Only another accepted
            // click, after the authoritative turn ACK, may arm AutoHit.
            return if movement.can_send(input.now_ms) {
                Turn(direction)
            } else {
                Consumed
            };
        }
    }
    let mining_tool = mining.then_some(tool).flatten();
    let replaces = movement
        .directional_attack
        .as_ref()
        .is_none_or(|gesture| gesture.direction != direction || gesture.mining_tool != mining_tool);
    if replaces {
        movement.stop_hold(input.now_ms, "directionalAttack");
        movement.stop_auto_path(input.now_ms, "directionalAttack");
        movement.attack_target = None;
        movement.harvest_target = None;
        movement.harvest_direction = None;
        movement.directional_attack_epoch = movement.directional_attack_epoch.saturating_add(1);
        movement.directional_attack = Some(DirectionalAttackGesture {
            id: movement.directional_attack_epoch,
            owner: input.object_id.to_owned(),
            owner_stamp: input.owner_stamp,
            direction,
            origin,
            map_file,
            mining_tool,
        });
        presentation.clear_local_mining_request();
        if let Some(queue) = queue.as_deref_mut() {
            queue.clear_attack_intents();
        }
    }
    if !movement.pending.is_empty()
        || !super::attack_request_ready(movement, presentation, input.now_ms, input.motion_now_ms)
    {
        return Consumed;
    }
    let gesture = movement.directional_attack.as_ref().unwrap();
    let id = gesture.id;
    if let Some(queue) = queue.as_deref_mut() {
        queue.clear_directional_attack_intents();
        if queue.push_intent(QuestUiIntent::AttackDirection {
            direction: direction.to_owned(),
            mining: gesture.mining(),
            gesture_id: id,
        }) {
            let interval = if gesture.mining() {
                mining_interval_ms(input.read_model.unwrap().player.level)
            } else {
                super::crystal_attack_request_interval_ms(input.read_model)
            };
            movement.next_attack_request_at_ms = input.now_ms + interval;
            crate::movement_trace::record(serde_json::json!({
                "type":"directionalAttackQueued", "atMs":input.now_ms,
                "objectId":input.object_id, "direction":direction,
                "mining":gesture.mining(), "gestureId":id,
            }));
        }
    }
    Consumed
}

fn cancel(
    movement: &mut super::WorldPointerMovementState,
    queue: Option<&mut mir2_client_bevy::quest_ui::QuestUiIntentQueue>,
    presentation: &mut crate::entity_presentation::NativeEntityPresentation,
    now_ms: f64,
) {
    movement.stop_hold(now_ms, "directionalAttackCanceled");
    if let Some(queue) = queue {
        queue.clear_directional_attack_intents();
    }
    presentation.clear_local_mining_request();
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::inventory::{
        CrystalItemInfoModel, CrystalItemTooltipSourceModel, ItemModel,
    };
    use mir2_client_bevy::{
        quest_ui::{QuestUiIntent, QuestUiIntentQueue},
        read_model::UiReadModel,
    };

    fn fixture() -> (
        super::super::WorldPointerMovementState,
        QuestUiIntentQueue,
        crate::entity_presentation::NativeEntityPresentation,
        InventoryModel,
        UiReadModel,
        crate::gameplay_bridge::NativeWorldClickState,
    ) {
        let mut presentation = crate::entity_presentation::NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((24, 182), (528., 352.));
        presentation.observe_packet_payload(
            serde_json::json!({
                "mapFileName": "D401", "sceneView": {"center": {"x": 24, "y": 182}}, "entities": []
            }),
            0,
        );
        let mut read_model = UiReadModel::default();
        read_model.player.hp = 100;
        read_model.player.max_hp = 100;
        read_model.player.level = 12;
        let mut movement = super::super::WorldPointerMovementState::default();
        movement.observe_identity("1000", (24, 182), "right");
        (
            movement,
            Default::default(),
            presentation,
            InventoryModel {
                items: vec![pickaxe()],
                ..Default::default()
            },
            read_model,
            crate::gameplay_bridge::NativeWorldClickState {
                class: Some("Warrior".into()),
                riding_mount: Some(false),
                dazed: Some(false),
                fishing: Some(false),
                ..Default::default()
            },
        )
    }

    fn input<'a>(
        inventory: &'a InventoryModel,
        read_model: &'a UiReadModel,
        click_state: &'a crate::gameplay_bridge::NativeWorldClickState,
        now_ms: f64,
    ) -> DirectionalAttackInput<'a> {
        DirectionalAttackInput {
            object_id: "1000",
            origin: (24, 182),
            owner_stamp: None,
            owner_ready: true,
            mining_ui_open: false,
            press_grid: Some((25, 182)),
            press_grid_empty: true,
            inventory: Some(inventory),
            read_model: Some(read_model),
            click_state: Some(click_state),
            left_pressed: now_ms == 0.,
            left_held: true,
            shift: false,
            now_ms,
            motion_now_ms: now_ms as u64,
        }
    }

    pub(super) fn pickaxe() -> ItemModel {
        ItemModel {
            key: "crystal-item:836".into(),
            name: "PickAxe".into(),
            unique_id: Some(700),
            quantity: 1,
            container: 2,
            slot: 0,
            equip_slot: Some("weapon".into()),
            durability_current: Some(10_000),
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 836,
                    item_type: 1,
                    can_mine: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn mining_requires_real_equipped_source_and_working_durability() {
        let valid = pickaxe();
        let inventory = |item| InventoryModel {
            items: vec![item],
            ..Default::default()
        };
        assert_eq!(
            equipped_mining_tool(Some(&inventory(valid.clone())))
                .unwrap()
                .unique_id,
            700
        );
        let mut renamed = valid.clone();
        renamed.name.clear();
        assert!(equipped_mining_tool(Some(&inventory(renamed))).is_some());
        let mut invalids = Vec::new();
        let mut item = valid.clone();
        item.container = 0;
        invalids.push(item);
        let mut item = valid.clone();
        item.slot = 1;
        invalids.push(item);
        let mut item = valid.clone();
        item.durability_current = Some(0);
        invalids.push(item);
        let mut item = valid.clone();
        item.durability_current = None;
        invalids.push(item);
        let mut item = valid.clone();
        item.tooltip_source = None;
        invalids.push(item);
        let mut item = valid.clone();
        item.tooltip_source.as_mut().unwrap().info.can_mine = false;
        invalids.push(item);
        let mut item = valid.clone();
        item.unique_id = None;
        invalids.push(item);
        for item in invalids {
            assert_eq!(equipped_mining_tool(Some(&inventory(item))), None);
        }
        assert_eq!(
            equipped_mining_tool(Some(&InventoryModel {
                items: vec![valid.clone(), valid],
                ..Default::default()
            })),
            None
        );
    }

    #[test]
    fn mining_gesture_survives_wear_but_not_weapon_or_map_changes() {
        let mut inventory = InventoryModel {
            items: vec![pickaxe()],
            ..Default::default()
        };
        let gesture = DirectionalAttackGesture {
            id: 1,
            owner: "1000".into(),
            owner_stamp: None,
            direction: "right",
            origin: (24, 182),
            map_file: "D401".into(),
            mining_tool: equipped_mining_tool(Some(&inventory)),
        };
        inventory.items[0].durability_current = Some(99);
        assert!(gesture.remains_current(Some("D401"), (24, 182), Some(&inventory)));
        assert!(!gesture.remains_current(Some("D402"), (24, 182), Some(&inventory)));
        assert!(!gesture.remains_current(Some("D401"), (25, 182), Some(&inventory)));
        inventory.items[0].unique_id = Some(701);
        assert!(!gesture.remains_current(Some("D401"), (24, 182), Some(&inventory)));
        inventory.items[0].unique_id = Some(700);
        inventory.items[0].durability_current = Some(0);
        assert!(!gesture.remains_current(Some("D401"), (24, 182), Some(&inventory)));
    }

    #[test]
    fn mining_auto_hit_paces_and_survives_release_without_accumulating_intents() {
        let (mut movement, mut queue, mut presentation, inventory, read, click) = fixture();
        let next = mining_interval_ms(12);
        for now in [0., 10., next - 1., next, next + 5., next * 2.] {
            assert!(handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                input(&inventory, &read, &click, now),
                |_, _, _| Some(true),
            )
            .handled());
            assert_eq!(queue.len(), 1, "pending swings must never accumulate");
            assert!(movement.active.is_none());
            assert!(movement.auto_path_destination.is_none());
            assert_eq!(
                movement.next_move_send_at_ms, 0.,
                "mining must not block the escape movement clock"
            );
        }
        let id = movement.directional_attack.as_ref().unwrap().id;
        assert!(matches!(queue.drain_intents().as_slice(),
            [QuestUiIntent::AttackDirection { direction, mining: true, .. }] if direction == "right"));
        let mut released = input(&inventory, &read, &click, next * 2. + 10.);
        released.left_held = false;
        assert!(handle_directional_attack_with_collision(
            &mut movement,
            Some(&mut queue),
            &mut presentation,
            released,
            |_, _, _| Some(true),
        )
        .handled());
        assert!(
            queue.is_empty(),
            "release cannot bypass the original cooldown"
        );
        assert_eq!(movement.directional_attack.as_ref().unwrap().id, id);
        presentation.set_hover_grid_context_for_test((24, 182), (-1., -1.));
        presentation.observe_packet_payload(
            serde_json::json!({
                "mapFileName": "D401", "sceneView": {"center": {"x": 24, "y": 182}}, "entities": []
            }),
            10,
        );
        presentation.set_hovered_object_id_for_test(Some("2001"));
        assert!(presentation.hovered_grid_position().is_none());
        for now in [next * 3., next * 4.] {
            let mut idle = input(&inventory, &read, &click, now);
            idle.left_held = false;
            assert!(handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                idle,
                |_, _, _| Some(true),
            )
            .handled());
            assert!(matches!(queue.drain_intents().as_slice(),
                [QuestUiIntent::AttackDirection { gesture_id, mining: true, .. }] if *gesture_id == id));
        }
    }

    #[test]
    fn mining_auto_hit_same_frame_pulse_turns_then_needs_an_aligned_new_click() {
        let (mut movement, mut queue, mut presentation, inventory, read, click) = fixture();
        movement.authoritative_direction = Some("up".into());
        let mut pulse = input(&inventory, &read, &click, 0.);
        pulse.left_held = false;
        assert_eq!(
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                pulse,
                |_, _, _| Some(true),
            ),
            DirectionalAttackDecision::Turn("right")
        );
        assert!(movement.directional_attack.is_none());
        assert!(queue.is_empty(), "turning may not mine in the same click");
        // A real UserLocation ACK updates this field in the host; do not let an
        // old release create AutoHit even after the turn has been confirmed.
        movement.authoritative_direction = Some("right".into());
        let mut idle = input(&inventory, &read, &click, 100.);
        idle.left_held = false;
        assert_eq!(
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                idle,
                |_, _, _| Some(true),
            ),
            DirectionalAttackDecision::Unhandled
        );
        assert!(queue.is_empty());
        let mut pulse = input(&inventory, &read, &click, 200.);
        pulse.left_pressed = true;
        pulse.left_held = false;
        assert_eq!(
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                pulse,
                |_, _, _| Some(true),
            ),
            DirectionalAttackDecision::Consumed
        );
        assert!(matches!(queue.drain_intents().as_slice(),
            [QuestUiIntent::AttackDirection { mining: true, direction, .. }] if direction == "right"));
    }

    #[test]
    fn mining_auto_hit_uses_real_down_grid_instead_of_the_release_cursor() {
        let (mut movement, mut queue, mut presentation, inventory, read, click) = fixture();
        movement.authoritative_direction = Some("left".into());
        let mut pulse = input(&inventory, &read, &click, 0.);
        pulse.left_held = false;
        pulse.press_grid = Some((23, 182));
        assert_eq!(
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                pulse,
                |map, x, y| {
                    assert_eq!((map, x, y), ("D401", 23, 182));
                    Some(true)
                },
            ),
            DirectionalAttackDecision::Consumed
        );
        assert!(matches!(queue.drain_intents().as_slice(),
            [QuestUiIntent::AttackDirection { mining: true, direction, .. }] if direction == "left"));
    }

    #[test]
    fn mining_auto_hit_cold_click_refuses_missing_authority_actor_tool_or_wall() {
        for case in [
            "transport",
            "wrongOwner",
            "missingRead",
            "dead",
            "missingHp",
            "ridingUnknown",
            "riding",
            "dazedUnknown",
            "dazed",
            "fishingUnknown",
            "fishing",
            "noTool",
            "noDown",
            "actorAtDown",
            "hoverActor",
            "unknownWall",
            "open",
            "ui",
        ] {
            let (mut movement, mut queue, mut presentation, mut inventory, mut read, mut click) =
                fixture();
            match case {
                "dead" => read.player.hp = 0,
                "missingHp" => read.player.max_hp = 0,
                "ridingUnknown" => click.riding_mount = None,
                "riding" => click.riding_mount = Some(true),
                "dazedUnknown" => click.dazed = None,
                "dazed" => click.dazed = Some(true),
                "fishingUnknown" => click.fishing = None,
                "fishing" => click.fishing = Some(true),
                "noTool" => inventory.items.clear(),
                "hoverActor" => presentation.set_hovered_object_id_for_test(Some("2001")),
                _ => {}
            }
            let mut pulse = input(&inventory, &read, &click, 0.);
            pulse.left_held = false;
            match case {
                "transport" => pulse.owner_ready = false,
                "wrongOwner" => pulse.object_id = "2000",
                "missingRead" => pulse.read_model = None,
                "noDown" => pulse.press_grid = None,
                "actorAtDown" => pulse.press_grid_empty = false,
                "ui" => pulse.mining_ui_open = true,
                _ => {}
            }
            assert_eq!(
                handle_directional_attack_with_collision(
                    &mut movement,
                    Some(&mut queue),
                    &mut presentation,
                    pulse,
                    |_, _, _| match case {
                        "unknownWall" => None,
                        "open" => Some(false),
                        _ => Some(true),
                    },
                ),
                DirectionalAttackDecision::Unhandled,
                "{case}"
            );
            assert!(movement.directional_attack.is_none(), "{case}");
            assert!(queue.is_empty(), "{case}");
        }
    }

    #[test]
    fn mining_auto_hit_shift_keeps_forced_attack_held_and_separate() {
        let (mut movement, mut queue, mut presentation, inventory, read, click) = fixture();
        let mut shifted = input(&inventory, &read, &click, 0.);
        shifted.shift = true;
        assert_eq!(
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                shifted,
                |_, _, _| Some(true),
            ),
            DirectionalAttackDecision::Consumed
        );
        assert!(matches!(
            queue.drain_intents().as_slice(),
            [QuestUiIntent::AttackDirection { mining: false, .. }]
        ));
        let mut released = input(&inventory, &read, &click, 2000.);
        released.shift = true;
        released.left_held = false;
        assert_eq!(
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                released,
                |_, _, _| Some(true),
            ),
            DirectionalAttackDecision::Consumed
        );
        assert!(movement.directional_attack.is_none());
        assert!(queue.is_empty());
    }

    #[test]
    fn mining_empty_shift_attacks_and_wall_requires_usable_tool() {
        let (mut movement, mut queue, mut presentation, mut inventory, read, click) = fixture();
        inventory.items.clear();
        assert!(!handle_directional_attack_with_collision(
            &mut movement,
            Some(&mut queue),
            &mut presentation,
            input(&inventory, &read, &click, 0.),
            |_, _, _| Some(true)
        )
        .handled());
        assert!(queue.is_empty());
        let mut shifted = input(&inventory, &read, &click, 0.);
        shifted.shift = true;
        assert!(handle_directional_attack_with_collision(
            &mut movement,
            Some(&mut queue),
            &mut presentation,
            shifted,
            |_, _, _| Some(false)
        )
        .handled());
        assert!(
            matches!(queue.drain_intents().as_slice(), [QuestUiIntent::AttackDirection {
            direction, mining: false, .. }] if direction == "right")
        );
        presentation.set_hovered_object_id_for_test(Some("2001"));
        let mut shifted = input(&inventory, &read, &click, 2_000.);
        shifted.shift = true;
        assert!(handle_directional_attack_with_collision(
            &mut movement,
            Some(&mut queue),
            &mut presentation,
            shifted,
            |_, _, _| Some(false)
        )
        .handled());
        assert!(
            movement.directional_attack.is_none(),
            "actor clicks retain ordinary combat ownership"
        );
    }

    #[test]
    fn mining_hold_cancels_on_death_swap_map_or_transform_and_never_rearms_old_press() {
        for case in [
            "death",
            "toolSwap",
            "mapChange",
            "locationChange",
            "depletedTool",
        ] {
            let (mut movement, mut queue, mut presentation, mut inventory, mut read, click) =
                fixture();
            handle_directional_attack_with_collision(
                &mut movement,
                Some(&mut queue),
                &mut presentation,
                input(&inventory, &read, &click, 0.),
                |_, _, _| Some(true),
            )
            .handled();
            match case {
                "death" => read.player.hp = 0,
                "toolSwap" => inventory.items[0].unique_id = Some(999),
                "depletedTool" => inventory.items[0].durability_current = Some(0),
                "mapChange" => presentation.observe_packet_payload(serde_json::json!({
                    "mapFileName": "D402", "sceneView": {"center": {"x": 24, "y": 182}}, "entities": []}), 10),
                "locationChange" => movement.authoritative_position = Some((23, 182)),
                _ => unreachable!(),
            }
            assert!(
                handle_directional_attack_with_collision(
                    &mut movement,
                    Some(&mut queue),
                    &mut presentation,
                    input(&inventory, &read, &click, 2_000.),
                    |_, _, _| Some(true)
                )
                .handled(),
                "{case}"
            );
            assert!(movement.directional_attack.is_none(), "{case}");
            assert!(queue.is_empty(), "{case}");
            // Returning to the exact original valid facts must not resurrect
            // the discarded click or its already-queued directional request.
            read.player.hp = 100;
            inventory.items[0].unique_id = Some(700);
            inventory.items[0].durability_current = Some(10_000);
            movement.authoritative_position = Some((24, 182));
            presentation.observe_packet_payload(serde_json::json!({
                "mapFileName": "D401", "sceneView": {"center": {"x": 24, "y": 182}}, "entities": []
            }), 20);
            assert!(
                !handle_directional_attack_with_collision(
                    &mut movement,
                    Some(&mut queue),
                    &mut presentation,
                    input(&inventory, &read, &click, 3_000.),
                    |_, _, _| Some(true)
                )
                .handled(),
                "{case}"
            );
            assert!(queue.is_empty(), "{case}");
        }
    }
}
