//! Exercise the original host followed by the guarded item action path.
//! Bevy lays out the real inspector painter and produces Interaction from
//! ordinary mouse messages; no interaction or use receipt is fabricated.
use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use bevy::camera::{ComputedCameraValues, RenderTarget, RenderTargetInfo, Viewport};
use bevy::ecs::system::RunSystemOnce;
use bevy::input::mouse::MouseButtonInput;
use bevy::ui::{UiGlobalTransform, UiScale, UiTargetCamera};

struct Fixture {
    app: App,
    window: Entity,
    camera: Entity,
    inspector: Entity,
    font: Handle<Font>,
    offset: UVec2,
    origin: Vec2,
    width: f32,
}

fn food() -> ItemModel {
    ItemModel {
        unique_id: Some(42),
        key: "crystal-item-958".into(),
        name: "Cherries".into(),
        quantity: 1,
        container: 0,
        slot: 13,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_index: 958,
                name: "Cherries".into(),
                item_type: 36,
                shape: 23,
                stack_size: 1,
                ..default()
            },
            user_item: Some(CrystalUserItemModel {
                unique_id: 42,
                item_index: 958,
                count: 1,
                ..default()
            }),
            ..default()
        }),
        ..default()
    }
}

fn baby_pig() -> mir2_protocol::ClientIntelligentCreature {
    mir2_protocol::ClientIntelligentCreature {
        pet_type: 0,
        icon: 540,
        custom_name: "BabyPig".into(),
        fullness: 5298,
        slot_index: 0,
        expire_binary_datetime: 0,
        blackstone_time: 0,
        pet_mode: 1,
        creature_rules: mir2_protocol::IntelligentCreatureRules {
            minimal_fullness: 4000,
            mouse_pickup_enabled: false,
            mouse_pickup_range: 0,
            auto_pickup_enabled: false,
            auto_pickup_range: 0,
            semi_auto_pickup_enabled: true,
            semi_auto_pickup_range: 3,
            can_produce_blackstone: false,
        },
        filter: mir2_protocol::IntelligentCreatureItemFilter {
            pet_pickup_all: true,
            pet_pickup_gold: false,
            pet_pickup_weapons: false,
            pet_pickup_armours: false,
            pet_pickup_helmets: false,
            pet_pickup_boots: false,
            pet_pickup_belts: false,
            pet_pickup_accessories: false,
            pet_pickup_others: false,
        },
        pickup_grade: 0,
        maintain_food_time: 0,
    }
}

impl Fixture {
    fn new(scale: f32, offset: UVec2, origin: Vec2, width: f32) -> Self {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::input::InputPlugin,
            bevy::image::ImagePlugin::default(),
            bevy::transform::TransformPlugin,
            bevy::camera::visibility::VisibilityPlugin,
            bevy::text::TextPlugin,
            bevy::ui::UiPlugin,
        ));
        app.init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<bevy::image::TextureAtlasLayout>()
            .init_asset::<bevy::mesh::Mesh>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<MenuHost>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(UiScale(scale))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            });
        super::super::tests::init_overlay_button_test_resources(&mut app);
        let font = Font::from_bytes(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../platform-windows/assets/fonts/NotoSansTC.ttf"
            ))
            .expect("packaged UI font"),
        );
        let font = app.world_mut().resource_mut::<Assets<Font>>().add(font);
        let size = UVec2::new((1024.0 * scale) as u32, (768.0 * scale) as u32);
        let mut window = Window::default();
        window.focused = true;
        window.resolution.set(
            (size.x + offset.x * 2) as f32,
            (size.y + offset.y * 2) as f32,
        );
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: size + offset * 2,
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    viewport: Some(Viewport {
                        physical_position: offset,
                        physical_size: size,
                        ..default()
                    }),
                    ..default()
                },
            ))
            .id();
        let item = food();
        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .push(item.clone());
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.inspect = Some(inspect_from_item(&item));
            state.creature.observe(
                &mir2_protocol::ServerPacket::UpdateIntelligentCreatureList {
                    creature_list: vec![baby_pig()],
                    creature_summoned: true,
                    summoned_creature_type: 0,
                    pearl_count: 0,
                },
            );
            state.creature.open = true;
            state.creature.position = Some(Vec2::new(100.0, 100.0));
        }
        // A lower ordinary creature background hit deliberately spans every
        // test point. Its rectangle is independent of the inspector layout.
        app.world_mut()
            .resource_mut::<MenuHost>()
            .pet_hits
            .push(CreatureHit {
                rect: CrystalRect::new(0.0, 0.0, 1024.0, 768.0),
                action: PetHit::Drag,
                hint: Some("covered background".into()),
            });
        app.add_systems(Update, (process, process_overlay_buttons).chain());
        let mut fixture = Self {
            app,
            window,
            camera,
            inspector: Entity::PLACEHOLDER,
            font,
            offset,
            origin,
            width,
        };
        fixture.rebuild();
        fixture
    }

    fn standard() -> Self {
        Self::new(1.0, UVec2::ZERO, Vec2::new(390.0, 170.0), 240.0)
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.app.update();
        }
    }

    fn rebuild(&mut self) {
        if self.inspector != Entity::PLACEHOLDER {
            self.app.world_mut().despawn(self.inspector);
        }
        let state = self.app.world().resource::<NativePlayerUiState>().clone();
        let inventory = self.app.world().resource::<InventoryModel>().clone();
        {
            let mut commands = self.app.world_mut().commands();
            self.inspector = commands
                .spawn((
                    OverlayInspect,
                    UiTargetCamera(self.camera),
                    GlobalZIndex(985),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(self.origin.x),
                        top: Val::Px(self.origin.y),
                        width: Val::Px(self.width),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(8.0)),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|parent| render_inspect(parent, &state, &inventory, None))
                .id();
        }
        self.app.world_mut().flush();
        // Keep the production painter/labels/flow; use the frozen native
        // packaged font rather than OS font discovery.
        let world = self.app.world_mut();
        let mut fonts = world.query::<&mut TextFont>();
        for mut text_font in fonts.iter_mut(world) {
            text_font.font = self.font.clone().into();
        }
        self.settle();
    }

    fn button_point(&mut self, wanted: OverlayButton) -> Vec2 {
        let world = self.app.world_mut();
        let mut buttons = world.query::<(&OverlayButton, &ComputedNode, &UiGlobalTransform)>();
        let (_, node, transform) = buttons
            .iter(world)
            .find(|(button, _, _)| **button == wanted)
            .expect("real rendered inspector button");
        assert!(
            node.size().x > 0.0 && node.size().y > 0.0,
            "actual layout must exist"
        );
        transform.affine().translation + self.offset.as_vec2()
    }

    fn pointer(&mut self, physical: Option<Vec2>) {
        self.app
            .world_mut()
            .entity_mut(self.window)
            .get_mut::<Window>()
            .unwrap()
            .set_physical_cursor_position(physical.map(|v| v.as_dvec2()));
    }

    fn mouse(&mut self, pressed: bool) {
        self.app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: if pressed {
                ButtonState::Pressed
            } else {
                ButtonState::Released
            },
            window: self.window,
        });
        self.app.update();
    }

    fn click(&mut self, button: OverlayButton) {
        let point = self.button_point(button);
        self.pointer(Some(point));
        self.mouse(true);
        self.mouse(false);
    }

    fn drain(&mut self) -> Vec<NativePlayerUiIntent> {
        self.app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
    }

    fn assert_yielded(&self) {
        let state = self.app.world().resource::<NativePlayerUiState>();
        let host = self.app.world().resource::<MenuHost>();
        assert!(!state.menu_pointer_consumed);
        assert!(host.drag.is_none() && host.pressed.is_none() && !host.text_selecting);
        assert!(host.hint.is_none());
        assert_eq!(state.creature.position, Some(Vec2::new(100.0, 100.0)));
    }
}

#[test]
fn creature_overlap_real_food_use_yields_without_drag_or_local_consumption() {
    let mut f = Fixture::standard();
    {
        let mut host = f.app.world_mut().resource_mut::<MenuHost>();
        host.drag = Some((3, Vec2::ZERO));
        host.pressed = Some(ItemCell::Bag(13));
        host.text_selecting = true;
    }
    let before = f.app.world().resource::<InventoryModel>().items.clone();
    let pets_before = f
        .app
        .world()
        .resource::<NativePlayerUiState>()
        .creature
        .creatures
        .clone();
    let point = f.button_point(OverlayButton::UseInspected);
    f.pointer(Some(point));
    f.mouse(true);
    // Multiple frames holding the same press must not produce another action.
    for _ in 0..3 {
        f.app.update();
    }
    f.mouse(false);
    f.assert_yielded();
    assert_eq!(
        f.drain(),
        vec![NativePlayerUiIntent::UseItem {
            key: Some("crystal-item-958".into()),
            unique_id: Some(42),
            slot: Some(13),
            grid: Some("inventory".into()),
        }]
    );
    assert_eq!(f.app.world().resource::<InventoryModel>().items, before);
    assert_eq!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .creatures,
        pets_before
    );
}

#[test]
fn creature_overlap_measured_inspector_honors_scale_viewport_translation_and_width() {
    for scale in [1.0, 1.5, 2.0] {
        for (origin, width) in [
            (Vec2::new(390.0, 170.0), 240.0),
            (Vec2::new(120.0, 65.0), 320.0),
        ] {
            let mut f = Fixture::new(scale, UVec2::new(37, 29), origin, width);
            f.click(OverlayButton::UseInspected);
            f.assert_yielded();
            assert_eq!(f.drain().len(), 1, "scale={scale}, origin={origin:?}");
        }
    }
}

#[test]
fn creature_overlap_drop_confirmation_cancel_close_and_pending_remain_two_step() {
    let mut f = Fixture::standard();
    let before_height = f
        .app
        .world()
        .entity(f.inspector)
        .get::<ComputedNode>()
        .unwrap()
        .size()
        .y;
    f.click(OverlayButton::DropInspected);
    assert!(f.drain().is_empty());
    assert!(f
        .app
        .world()
        .resource::<NativePlayerUiState>()
        .drop_confirmation
        .is_some());
    f.rebuild();
    let confirmation_height = f
        .app
        .world()
        .entity(f.inspector)
        .get::<ComputedNode>()
        .unwrap()
        .size()
        .y;
    assert!(
        confirmation_height > before_height,
        "real confirmation changes the measured surface"
    );
    f.click(OverlayButton::CancelDropInspected);
    assert!(f
        .app
        .world()
        .resource::<NativePlayerUiState>()
        .drop_confirmation
        .is_none());
    assert!(f.drain().is_empty());
    f.rebuild();
    f.click(OverlayButton::DropInspected);
    f.rebuild();
    f.click(OverlayButton::ConfirmDropInspected);
    assert!(matches!(
        f.drain().as_slice(),
        [NativePlayerUiIntent::DropItem {
            unique_id: 42,
            count: 1,
            ..
        }]
    ));
    f.assert_yielded();
    // The same UID cannot be submitted twice while its real pending op remains.
    f.rebuild();
    f.click(OverlayButton::DropInspected);
    f.rebuild();
    f.click(OverlayButton::ConfirmDropInspected);
    assert!(f.drain().is_empty());
    f.click(OverlayButton::CloseInspect);
    assert!(f
        .app
        .world()
        .resource::<NativePlayerUiState>()
        .inspect
        .is_none());
    f.assert_yielded();
}

#[test]
fn creature_overlap_outside_surface_still_drags_and_entering_cancels_old_drag() {
    let mut f = Fixture::standard();
    f.pointer(Some(Vec2::new(20.0, 20.0)));
    f.mouse(true);
    assert!(f.app.world().resource::<MenuHost>().drag.is_some());
    assert!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .menu_pointer_consumed
    );
    let inside = f.button_point(OverlayButton::UseInspected);
    f.pointer(Some(inside));
    f.app.update();
    assert!(f.app.world().resource::<MenuHost>().drag.is_none());
    assert!(
        f.drain().is_empty(),
        "a held background drag cannot click Use"
    );
    f.pointer(Some(Vec2::new(25.0, 25.0)));
    f.app.update();
    assert!(
        f.app.world().resource::<MenuHost>().drag.is_none(),
        "yielded drag cannot resume without a new press"
    );
    f.mouse(false);
}

#[test]
fn creature_overlap_hidden_or_clipped_inspector_does_not_reserve_its_old_rectangle() {
    for hidden in 0..4 {
        let mut f = Fixture::standard();
        let point = f.button_point(OverlayButton::UseInspected);
        match hidden {
            0 => {
                f.app
                    .world_mut()
                    .entity_mut(f.inspector)
                    .get_mut::<Node>()
                    .unwrap()
                    .display = Display::None
            }
            1 => {
                f.app
                    .world_mut()
                    .entity_mut(f.inspector)
                    .insert(Visibility::Hidden);
            }
            2 => {
                let parent = f
                    .app
                    .world_mut()
                    .spawn((Node::default(), Visibility::Hidden))
                    .id();
                f.app
                    .world_mut()
                    .entity_mut(f.inspector)
                    .insert(ChildOf(parent));
            }
            _ => {
                let parent = f
                    .app
                    .world_mut()
                    .spawn(Node {
                        width: Val::Px(1024.0),
                        height: Val::Px(20.0),
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .id();
                f.app
                    .world_mut()
                    .entity_mut(f.inspector)
                    .insert(ChildOf(parent));
            }
        }
        f.settle();
        f.pointer(Some(point));
        f.mouse(true);
        assert!(
            f.app.world().resource::<MenuHost>().drag.is_some(),
            "hidden case {hidden}"
        );
        assert!(
            f.app
                .world()
                .resource::<NativePlayerUiState>()
                .menu_pointer_consumed
        );
        assert!(f.drain().is_empty());
    }
}

#[test]
fn creature_overlap_outside_inspector_keeps_lower_creature_and_equipment_buttons() {
    for creature in [false, true] {
        let mut f = Fixture::standard();
        {
            let mut host = f.app.world_mut().resource_mut::<MenuHost>();
            if creature {
                host.pet_hits[0].action = PetHit::Button(CreatureAction::Close);
            } else {
                host.pet_hits.clear();
                host.equip_hits.push(DialogHit {
                    rect: CrystalRect::new(0.0, 0.0, 40.0, 40.0),
                    action: EquipHit::Button(EquipmentAction::EscapeToggle),
                });
            }
        }
        if !creature {
            f.app
                .world_mut()
                .resource_mut::<NativePlayerUiState>()
                .equipment_dialogs
                .status_open = true;
        }
        f.pointer(Some(Vec2::new(20.0, 20.0)));
        f.mouse(true);
        f.mouse(false);
        let state = f.app.world().resource::<NativePlayerUiState>();
        assert!(state.menu_pointer_consumed);
        if creature {
            assert!(!state.creature.open);
        } else {
            assert!(state.equipment_dialogs.status_open);
            assert!(state.equipment_dialogs.escape_cancels);
        }
        assert!(f.app.world().resource::<MenuHost>().drag.is_none());
        assert!(!f
            .drain()
            .iter()
            .any(|i| matches!(i, NativePlayerUiIntent::UseItem { .. })));
    }
}

#[test]
fn creature_overlap_item_amount_modal_still_blocks_inspector_use() {
    let mut f = Fixture::standard();
    let target = ItemModel {
        unique_id: Some(84),
        key: "small-hp-drug".into(),
        name: "Small HP Drug".into(),
        quantity: 2,
        container: 0,
        slot: 14,
        ..default()
    };
    f.app
        .world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .push(target.clone());
    // Retain an otherwise valid pre-existing food action while a different
    // item's amount box owns input. The final modal guard must decide this.
    f.app
        .world_mut()
        .resource_mut::<NativePlayerUiState>()
        .inventory_delete_prompt = inventory_delete_prompt_for_item(&target);
    assert!(inspected_use_intent(
        f.app.world().resource::<NativePlayerUiState>(),
        f.app.world().resource::<InventoryModel>()
    )
    .is_some());
    assert!(matches!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_delete_prompt,
        Some(InventoryDeletePrompt::Amount { .. })
    ));
    f.click(OverlayButton::UseInspected);
    assert!(f
        .app
        .world()
        .resource::<NativePlayerUiState>()
        .inventory_delete_prompt
        .is_some());
    assert!(f.drain().is_empty());
    assert!(f.app.world().resource::<MenuHost>().drag.is_none());
}

#[test]
fn creature_overlap_higher_creature_notice_and_input_keep_modal_ownership() {
    for input in [false, true] {
        let mut f = Fixture::standard();
        {
            let mut state = f.app.world_mut().resource_mut::<NativePlayerUiState>();
            if input {
                state.creature.input = Some(creature_dialog::CreatureInput {
                    purpose: creature_dialog::InputPurpose::Rename,
                    pet_type: 0,
                    name: "BabyPig".into(),
                    text: "BabyPig".into(),
                });
            } else {
                state.creature.notice = Some("A higher modal owns the pointer".into());
            }
        }
        f.app.world_mut().resource_mut::<MenuHost>().pet_hits[0].action = PetHit::Block;
        f.click(OverlayButton::UseInspected);
        assert!(
            f.app
                .world()
                .resource::<NativePlayerUiState>()
                .menu_pointer_consumed
        );
        assert!(f.app.world().resource::<MenuHost>().drag.is_none());
        assert!(f.drain().is_empty());
    }
}

#[test]
fn creature_overlap_use_rechecks_stale_uid_and_mail_lock_after_paint() {
    for locked in [false, true] {
        let mut f = Fixture::standard();
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        if locked {
            let inventory = f.app.world().resource::<InventoryModel>().clone();
            let mut parcel = f
                .app
                .world_mut()
                .resource_mut::<mail_parcel::MailParcelUi>();
            assert!(parcel.attach(&mut draft, &inventory, 42));
            assert_eq!(draft.attachment_unique_ids, vec![42]);
            parcel.apply_lock_receipt(42, true);
            assert!(parcel.blocks_item(42));
        } else {
            f.app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(99);
        }
        f.click(OverlayButton::UseInspected);
        f.assert_yielded();
        assert!(f.drain().is_empty());
        assert_eq!(
            f.app.world().resource::<InventoryModel>().items[0].quantity,
            1
        );
    }
}

#[test]
fn creature_overlap_use_keeps_equipment_destination_pending_guard() {
    let mut f = Fixture::standard();
    {
        let mut inventory = f.app.world_mut().resource_mut::<InventoryModel>();
        inventory.items[0].equip_slot = Some("weapon".into());
        inventory.items[0]
            .tooltip_source
            .as_mut()
            .unwrap()
            .info
            .item_type = 1;
    }
    f.app
        .world_mut()
        .resource_scope(|world, mut intents: Mut<NativePlayerUiIntentQueue>| {
            assert!(intents.push_pending_intent(
                &mut world.resource_mut::<PendingOperations>(),
                NativePlayerUiIntent::EquipItem {
                    unique_id: 84,
                    grid: "inventory".into(),
                    to: 0,
                }
            ));
        });
    assert_eq!(f.drain().len(), 1);
    f.rebuild();
    f.click(OverlayButton::UseInspected);
    f.assert_yielded();
    assert!(f.drain().is_empty());
}

#[test]
fn creature_overlap_host_clears_gesture_on_blur_without_yielding_input() {
    let mut f = Fixture::standard();
    f.pointer(None);
    f.app
        .world_mut()
        .entity_mut(f.window)
        .get_mut::<Window>()
        .unwrap()
        .focused = false;
    {
        let mut host = f.app.world_mut().resource_mut::<MenuHost>();
        host.drag = Some((3, Vec2::ZERO));
        host.pressed = Some(ItemCell::Bag(13));
        host.text_selecting = true;
    }
    f.app.world_mut().run_system_once(process).unwrap();
    f.assert_yielded();
    assert!(f.drain().is_empty());
}

#[test]
fn creature_overlap_host_rejects_other_window_or_inactive_camera_geometry() {
    for other_window in [false, true] {
        let mut f = Fixture::standard();
        let point = f.button_point(OverlayButton::UseInspected);
        if other_window {
            let other = f.app.world_mut().spawn(Window::default()).id();
            f.app
                .world_mut()
                .entity_mut(f.camera)
                .insert(RenderTarget::Window(bevy::window::WindowRef::Entity(other)));
        } else {
            f.app
                .world_mut()
                .entity_mut(f.camera)
                .get_mut::<Camera>()
                .unwrap()
                .is_active = false;
        }
        f.pointer(Some(point));
        f.app
            .world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        f.app.world_mut().run_system_once(process).unwrap();
        assert!(
            f.app
                .world()
                .resource::<NativePlayerUiState>()
                .menu_pointer_consumed
        );
        assert!(f.app.world().resource::<MenuHost>().drag.is_some());
        assert!(f.drain().is_empty());
    }
}

fn cold_creature_fixture() -> Fixture {
    let mut f = Fixture::standard();
    f.pointer(None);
    f.app
        .world_mut()
        .resource_mut::<NativePlayerUiState>()
        .creature = Default::default();
    f
}

fn tick_creature_host(f: &mut Fixture) {
    f.app.world_mut().run_system_once(process).unwrap();
}

fn toggle_creature(f: &mut Fixture) {
    f.app
        .world_mut()
        .resource_scope(|world, mut state: Mut<NativePlayerUiState>| {
            toggle(
                &mut state,
                &mut world.resource_mut::<NativePlayerUiIntentQueue>(),
                2,
                1000,
            );
        });
}

fn allow_creature_bootstrap(f: &mut Fixture) {
    f.app
        .world_mut()
        .resource_mut::<NativePlayerUiState>()
        .creature
        .bootstrap_allowed = true;
}

fn observe_creature_list(
    f: &mut Fixture,
    creatures: Vec<mir2_protocol::ClientIntelligentCreature>,
) {
    let summoned = creatures.first().map(|pet| pet.pet_type).unwrap_or(99);
    let packet = mir2_protocol::ServerPacket::UpdateIntelligentCreatureList {
        creature_list: creatures.clone(),
        creature_summoned: summoned != 99,
        summoned_creature_type: summoned,
        pearl_count: 17,
    };
    let mut state = f.app.world_mut().resource_mut::<NativePlayerUiState>();
    assert!(
        state.creature.observe(&packet),
        "the production typed-packet consumer must accept the list"
    );
    assert_eq!(state.creature.creatures, creatures);
    assert_eq!(state.creature.pearls, 17);
    assert!(state.creature.data_received);
    assert!(!state.creature.bootstrap_pending);
}

fn assert_creature_request(f: &mut Fixture, update: bool) {
    assert_eq!(
        f.drain(),
        vec![NativePlayerUiIntent::EquipmentCreaturePacket(
            ClientPacket::RequestIntelligentCreatureUpdates { update }
        )]
    );
}

fn fill_creature_queue(f: &mut Fixture) {
    let mut intents = f
        .app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>();
    for _ in 0..MAX_QUEUED {
        assert!(
            intents.push_intent(NativePlayerUiIntent::EquipmentCreaturePacket(
                ClientPacket::RequestIntelligentCreatureUpdates { update: true }
            ))
        );
    }
}

#[test]
fn creature_bootstrap_fetches_unknown_list_only_after_allowed_ordinary_open() {
    let mut f = cold_creature_fixture();
    tick_creature_host(&mut f);
    assert!(f.drain().is_empty());
    allow_creature_bootstrap(&mut f);
    tick_creature_host(&mut f);
    assert!(
        f.drain().is_empty(),
        "a closed page must not silently subscribe"
    );
    toggle_creature(&mut f);
    assert!(
        f.drain().is_empty(),
        "unknown ownership is not a fabricated empty list"
    );
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    let state = f.app.world().resource::<NativePlayerUiState>();
    assert!(state.creature.requested_open && state.creature.bootstrap_pending);
    assert!(!state.creature.open && !state.creature.data_received);
    assert!(state.creature.notice.is_none());
    for _ in 0..3 {
        toggle_creature(&mut f);
        tick_creature_host(&mut f);
        assert!(
            f.drain().is_empty(),
            "repeated E while awaiting a real list cannot flood"
        );
    }
}

#[test]
fn creature_bootstrap_requested_open_waits_for_gameplay_admission() {
    let mut f = cold_creature_fixture();
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert!(f.drain().is_empty());
    assert!(
        !f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .bootstrap_pending
    );
    allow_creature_bootstrap(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
}

#[test]
fn creature_bootstrap_real_typed_list_opens_then_normal_toggle_close_can_repeat() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    let pet = baby_pig();
    observe_creature_list(&mut f, vec![pet.clone()]);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    {
        let state = f.app.world().resource::<NativePlayerUiState>();
        assert!(state.creature.open && !state.creature.requested_open);
        assert_eq!(state.creature.selected_slot, Some(pet.slot_index));
        assert_eq!(state.creature.summoned, Some(pet.pet_type));
        assert_eq!(state.creature.creatures, vec![pet.clone()]);
    }
    for _ in 0..3 {
        toggle_creature(&mut f);
        assert_creature_request(&mut f, false);
        tick_creature_host(&mut f);
        assert!(f.drain().is_empty());
        assert!(
            !f.app
                .world()
                .resource::<NativePlayerUiState>()
                .creature
                .open
        );
        toggle_creature(&mut f);
        assert_creature_request(&mut f, true);
        tick_creature_host(&mut f);
        assert!(f.drain().is_empty());
        assert!(
            f.app
                .world()
                .resource::<NativePlayerUiState>()
                .creature
                .open
        );
    }
}

#[test]
fn creature_bootstrap_real_empty_list_shows_notice_and_unsubscribes_once() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    observe_creature_list(&mut f, vec![]);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, false);
    {
        let state = f.app.world().resource::<NativePlayerUiState>();
        assert!(!state.creature.open && !state.creature.requested_open);
        assert_eq!(
            state.creature.notice.as_deref(),
            Some("You do not own any creatures.")
        );
    }
    for _ in 0..3 {
        tick_creature_host(&mut f);
        assert!(f.drain().is_empty());
    }
}

#[test]
fn creature_bootstrap_full_queue_releases_pending_and_retries_requested_fetch() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    fill_creature_queue(&mut f);
    tick_creature_host(&mut f);
    assert!(
        !f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .bootstrap_pending
    );
    assert!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .requested_open
    );
    assert_eq!(f.drain().len(), MAX_QUEUED);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    assert!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .bootstrap_pending
    );
}

#[test]
fn creature_bootstrap_unsent_fetch_uses_original_release_and_retries_without_local_list() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    f.app
        .world_mut()
        .resource_mut::<NativePlayerUiState>()
        .creature
        .release_unsent(&ClientPacket::RequestIntelligentCreatureUpdates { update: true });
    {
        let state = f.app.world().resource::<NativePlayerUiState>();
        assert!(!state.creature.bootstrap_pending && !state.creature.data_received);
        assert!(state.creature.requested_open && state.creature.creatures.is_empty());
    }
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    tick_creature_host(&mut f);
    assert!(f.drain().is_empty());
}

#[test]
fn creature_bootstrap_empty_reply_unsubscribe_retries_queue_capacity_then_stops() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    observe_creature_list(&mut f, vec![]);
    fill_creature_queue(&mut f);
    tick_creature_host(&mut f);
    assert!(
        !f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .bootstrap_pending
    );
    assert!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .requested_open
    );
    assert_eq!(f.drain().len(), MAX_QUEUED);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, false);
    tick_creature_host(&mut f);
    assert!(f.drain().is_empty());
    assert!(
        !f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .requested_open
    );
}

#[test]
fn creature_bootstrap_nonempty_reply_queue_rejection_does_not_strand_open_request() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    observe_creature_list(&mut f, vec![baby_pig()]);
    fill_creature_queue(&mut f);
    tick_creature_host(&mut f);
    assert!(
        !f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .open
    );
    assert!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .requested_open
    );
    assert_eq!(f.drain().len(), MAX_QUEUED);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    assert!(
        f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .open
    );
    assert!(
        !f.app
            .world()
            .resource::<NativePlayerUiState>()
            .creature
            .requested_open
    );
}

#[test]
fn creature_bootstrap_new_session_defaults_do_not_reuse_previous_owned_list_or_pending() {
    let mut f = cold_creature_fixture();
    allow_creature_bootstrap(&mut f);
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    observe_creature_list(&mut f, vec![baby_pig()]);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    // This is the same CreatureUi default replacement used by the Native
    // session reset. Ownership-fence rejection of old transport packets is
    // tested separately by the existing Native lifecycle path.
    f.app
        .world_mut()
        .resource_mut::<NativePlayerUiState>()
        .creature = Default::default();
    toggle_creature(&mut f);
    tick_creature_host(&mut f);
    assert!(f.drain().is_empty());
    {
        let state = f.app.world().resource::<NativePlayerUiState>();
        assert!(!state.creature.data_received && !state.creature.bootstrap_pending);
        assert!(state.creature.creatures.is_empty() && state.creature.requested_open);
        assert!(state.creature.notice.is_none());
    }
    allow_creature_bootstrap(&mut f);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, true);
    observe_creature_list(&mut f, vec![]);
    tick_creature_host(&mut f);
    assert_creature_request(&mut f, false);
    assert!(f
        .app
        .world()
        .resource::<NativePlayerUiState>()
        .creature
        .creatures
        .is_empty());
}
