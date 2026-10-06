use super::*;
use bevy::{ecs::system::RunSystemOnce, input::keyboard::KeyboardInput, window::PrimaryWindow};
use mir2_client_bevy::{
    crystal_ui::overlays::{hero_dialog::{host, HeroPage}, NativePlayerUiIntentQueue},
    hero_model::{HeroModel, HeroModelReceipts},
};

fn seed_shared_hero_fixture(world: &mut World) -> Entity {
    let mut model = HeroModel::default();
    assert!(model.apply_packet("HeroInformation", &serde_json::json!({"info":crate::hero_ingress::tests::info()})));
    let mut state = NativePlayerUiState::default();
    state.core.screen = mir2_ui_core::state::UiScreen::InGame;
    state.hero.observe(&model);
    state.hero.toggle_page(HeroPage::Equipment);
    let mut shell = NativeShellModel::default();
    shell.screen = NativeShellScreen::InGame;
    world.insert_resource(state);
    world.insert_resource(shell);
    world.insert_resource(model);
    world.init_resource::<HeroModelReceipts>();
    world.init_resource::<NativePlayerUiIntentQueue>();
    world.init_resource::<ButtonInput<MouseButton>>();
    world.init_resource::<Messages<KeyboardInput>>();
    world.init_resource::<Messages<bevy::window::CursorMoved>>();
    let mut window = Window::default();
    window.focused = true;
    window.resolution.set_physical_resolution(2340, 1080);
    window.resolution.set_scale_factor_override(Some(2.0));
    world.spawn((window, PrimaryWindow)).id()
}

fn shared_hero_fixture() -> (World, Entity) {
    let mut world = World::new();
    let entity = seed_shared_hero_fixture(&mut world);
    (world, entity)
}

#[test]
fn android_touch_publication_reaches_original_shared_hero_tab_hit_in_same_frame() {
    let (mut world, entity) = shared_hero_fixture();
    let before = serde_json::to_value(world.resource::<HeroModel>()).unwrap();
    let window = world.get::<Window>(entity).unwrap();
    let stage = mir2_client_bevy::crystal_ui::metrics::CrystalStageTransform::fit_native(window.width(), window.height());
    let (x, y) = stage.logical_to_physical(862.0, 80.0);
    world.run_system_once(move |mut windows: Query<&mut Window>, writer: MessageWriter<bevy::window::CursorMoved>| {
        let mut window = windows.get_mut(entity).unwrap();
        publish_android_touch_cursor(entity, &mut window, Vec2::new(x, y), &mut Some(writer));
    }).unwrap();
    assert_eq!(world.resource::<Messages<bevy::window::CursorMoved>>().len(), 1,
        "The Android branch really published the notification");
    let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
    mouse.press(MouseButton::Left);
    mouse.release(MouseButton::Left); // same-frame tap is a supported touch edge
    drop(mouse);
    world.run_system_once(host::process).unwrap();
    assert_eq!(world.resource::<NativePlayerUiState>().hero.page, HeroPage::Status,
        "CursorMoved notification alone is not Window.cursor_position for shared Hero input");
    assert!(world.resource::<NativePlayerUiState>().hero.pending.is_none());
    assert!(world.resource_mut::<NativePlayerUiIntentQueue>().drain_intents().is_empty());
    assert_eq!(serde_json::to_value(world.resource::<HeroModel>()).unwrap(), before);
}

#[test]
fn android_touch_clear_removes_cursor_seen_by_original_shared_window_readers() {
    let mut window = Window::default();
    window.set_cursor_position(Some(Vec2::new(100.0, 200.0)));
    clear_android_touch_cursor(&mut window);
    assert!(window.cursor_position().is_none(), "Cancellation must not expose stale synthesized touch coordinates");
}

#[test]
fn native_cursor_frame_is_restored_after_original_shared_input_and_before_winit_last() {
    let mut app = App::new();
    let entity = seed_shared_hero_fixture(app.world_mut());
    app.init_resource::<TouchPointer>();
    let original = Vec2::new(20.0, 30.0);
    app.world_mut().get_mut::<Window>(entity).unwrap().set_physical_cursor_position(Some(original.as_dvec2()));
    install_android_touch_cursor_frame(&mut app); // exact Android registration, without a native OS backend
    app.add_systems(PreUpdate, move |mut pointer: ResMut<TouchPointer>, mut windows: Query<&mut Window>,
        mut mouse: ResMut<ButtonInput<MouseButton>>, writer: MessageWriter<bevy::window::CursorMoved>| {
        let mut window = windows.get_mut(entity).unwrap();
        pointer.native_cursor_frame.capture(entity, &window);
        let stage = mir2_client_bevy::crystal_ui::metrics::CrystalStageTransform::fit_native(window.width(), window.height());
        let (x,y) = stage.logical_to_physical(862.0,80.0);
        publish_android_touch_cursor(entity, &mut window, Vec2::new(x,y), &mut Some(writer));
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
    });
    app.add_systems(Update, host::process);
    app.add_systems(Last, move |windows: Query<&Window>, state: Res<NativePlayerUiState>| {
        assert_eq!(state.hero.page, HeroPage::Status, "Original handler must consume the touch before restoring the cursor");
        assert_eq!(windows.get(entity).unwrap().physical_cursor_position(), Some(original),
            "Native backend must see its original cursor; no unsupported Android cursor warp");
    });
    let before = serde_json::to_value(app.world().resource::<HeroModel>()).unwrap();
    app.update();
    assert!(app.world().resource::<TouchPointer>().native_cursor_frame.previous.is_none());
    assert!(app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents().is_empty());
    assert_eq!(serde_json::to_value(app.world().resource::<HeroModel>()).unwrap(),before);
}

#[test]
fn native_cursor_frame_keeps_first_original_and_restores_only_cursor_not_window_properties() {
    let mut world = World::new();
    let entity = world.spawn(Window::default()).id();
    world.init_resource::<TouchPointer>();
    {
        let mut window = world.get_mut::<Window>(entity).unwrap();
        window.set_cursor_position(Some(Vec2::new(10.0,20.0)));
    }
    let original = world.get::<Window>(entity).unwrap().physical_cursor_position();
    world.resource_scope(|world, mut pointer: Mut<TouchPointer>| {
        let mut window = world.get_mut::<Window>(entity).unwrap();
        pointer.native_cursor_frame.capture(entity,&window);
        publish_android_touch_cursor(entity,&mut window,Vec2::new(100.0,200.0),&mut None);
        pointer.native_cursor_frame.capture(entity,&window); // a later sample must not replace the original
        publish_android_touch_cursor(entity,&mut window,Vec2::new(300.0,400.0),&mut None);
        window.title="Another system's new title".into();
    });
    world.run_system_once(restore_android_touch_cursor_frame).unwrap();
    assert_eq!(world.get::<Window>(entity).unwrap().physical_cursor_position(),original);
    assert_eq!(world.get::<Window>(entity).unwrap().title,"Another system's new title");
    assert!(world.resource::<TouchPointer>().native_cursor_frame.previous.is_none());
    world.run_system_once(restore_android_touch_cursor_frame).unwrap();
    assert_eq!(world.get::<Window>(entity).unwrap().physical_cursor_position(),original);
}

#[test]
fn native_cursor_restore_does_not_leak_to_a_replacement_window() {
    let mut world=World::new();
    world.init_resource::<TouchPointer>();
    let old=world.spawn(Window::default()).id();
    let snapshot=world.get::<Window>(old).unwrap().clone();
    world.resource_mut::<TouchPointer>().native_cursor_frame.capture(old,&snapshot);
    world.despawn(old);
    let mut replacement=Window::default();replacement.set_cursor_position(Some(Vec2::new(20.0,30.0)));
    let new=world.spawn(replacement).id();
    world.run_system_once(restore_android_touch_cursor_frame).unwrap();
    assert_eq!(world.get::<Window>(new).unwrap().cursor_position(),Some(Vec2::new(20.0,30.0)));
    assert!(world.resource::<TouchPointer>().native_cursor_frame.previous.is_none());
}

#[test]
fn native_cursor_cancel_is_absent_for_shared_input_before_restoring_real_cursor() {
    let (mut world,entity)=shared_hero_fixture();
    world.init_resource::<TouchPointer>();
    let original=Vec2::new(20.0,30.0);
    world.get_mut::<Window>(entity).unwrap().set_cursor_position(Some(original));
    world.resource_scope(|world,mut pointer:Mut<TouchPointer>| {
        let mut window=world.get_mut::<Window>(entity).unwrap();
        pointer.native_cursor_frame.capture(entity,&window);
        publish_android_touch_cursor(entity,&mut window,Vec2::new(831.0,56.0),&mut None);
        clear_android_touch_cursor(&mut window);
    });
    assert!(world.get::<Window>(entity).unwrap().cursor_position().is_none());
    world.run_system_once(host::process).unwrap();
    assert_eq!(world.resource::<NativePlayerUiState>().hero.page,HeroPage::Equipment);
    world.run_system_once(restore_android_touch_cursor_frame).unwrap();
    assert_eq!(world.get::<Window>(entity).unwrap().cursor_position(),Some(original));
}
