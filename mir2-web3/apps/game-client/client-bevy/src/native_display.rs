//! Local display choices for hosts that preserve Crystal's 1024×768 stage.
//! This preference changes presentation only; it never enters the protocol.

use crate::{
    native_i18n::{self, Locale},
    native_shell::{NativeShellModel, NativeShellScreen},
};
use bevy::{prelude::*, ui::FocusPolicy};

pub const STAGE_WIDTH: u32 = 1024;
pub const STAGE_HEIGHT: u32 = 768;

/// Physical origin and exact 4:3 viewport for the fixed original stage.
/// A minimized/invalid target smaller than 4×3 has no drawable viewport.
pub fn stage_viewport(physical: UVec2) -> (UVec2, UVec2) {
    let units = (physical.x / 4).min(physical.y / 3);
    if units == 0 {
        return (UVec2::ZERO, UVec2::ZERO);
    }
    let size = UVec2::new(units * 4, units * 3);
    ((physical - size) / 2, size)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionChoice {
    Auto,
    R1024x768,
    R1280x960,
    R1440x1080,
    R1600x1200,
    R1920x1440,
    R2048x1536,
}
impl ResolutionChoice {
    pub const ALL: [Self; 7] = [
        Self::Auto,
        Self::R1024x768,
        Self::R1280x960,
        Self::R1440x1080,
        Self::R1600x1200,
        Self::R1920x1440,
        Self::R2048x1536,
    ];
    pub const fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::R1024x768 => "1024x768",
            Self::R1280x960 => "1280x960",
            Self::R1440x1080 => "1440x1080",
            Self::R1600x1200 => "1600x1200",
            Self::R1920x1440 => "1920x1440",
            Self::R2048x1536 => "2048x1536",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.code() == code)
    }
    pub const fn size(self) -> Option<UVec2> {
        match self {
            Self::Auto => None,
            Self::R1024x768 => Some(UVec2::new(1024, 768)),
            Self::R1280x960 => Some(UVec2::new(1280, 960)),
            Self::R1440x1080 => Some(UVec2::new(1440, 1080)),
            Self::R1600x1200 => Some(UVec2::new(1600, 1200)),
            Self::R1920x1440 => Some(UVec2::new(1920, 1440)),
            Self::R2048x1536 => Some(UVec2::new(2048, 1536)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayEnvironment {
    /// Physical client space after subtracting taskbar, window frame and a margin.
    pub available: UVec2,
    pub os_scale_factor: f32,
}
impl Default for DisplayEnvironment {
    fn default() -> Self {
        Self {
            available: UVec2::new(1280, 960),
            os_scale_factor: 1.0,
        }
    }
}
impl DisplayEnvironment {
    pub fn normalized(self) -> Self {
        Self {
            available: self.available.max(UVec2::new(4, 3)),
            os_scale_factor: if self.os_scale_factor.is_finite() {
                self.os_scale_factor.clamp(0.5, 4.0)
            } else {
                1.0
            },
        }
    }
    pub fn supports(self, choice: ResolutionChoice) -> bool {
        choice
            .size()
            .is_none_or(|size| size.x <= self.available.x && size.y <= self.available.y)
    }
    pub fn auto_size(self) -> UVec2 {
        let environment = self.normalized();
        // A 4u×3u client guarantees exact aspect ratio. Quantize downward in
        // 64×48 pixel steps so Auto never requests a window larger than work area.
        let fitting_units = (environment.available.x / 4).min(environment.available.y / 3);
        let desired_units = (256.0 * environment.os_scale_factor).round() as u32;
        let bounded = fitting_units.min(desired_units).max(1);
        let units = if bounded >= 16 {
            bounded / 16 * 16
        } else {
            bounded
        };
        UVec2::new(units * 4, units * 3)
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct NativeDisplaySettings {
    pub choice: ResolutionChoice,
    pub environment: DisplayEnvironment,
    pub revision: u64,
}
impl Default for NativeDisplaySettings {
    fn default() -> Self {
        Self {
            choice: ResolutionChoice::Auto,
            environment: DisplayEnvironment::default(),
            revision: 0,
        }
    }
}
impl NativeDisplaySettings {
    pub fn new(choice: ResolutionChoice, environment: DisplayEnvironment) -> Self {
        let mut value = Self {
            choice,
            environment: environment.normalized(),
            revision: 0,
        };
        if !value.environment.supports(value.choice) {
            value.choice = ResolutionChoice::Auto;
        }
        value
    }
    pub fn size(&self) -> UVec2 {
        self.choice
            .size()
            .filter(|_| self.environment.supports(self.choice))
            .unwrap_or_else(|| self.environment.auto_size())
    }
    pub fn scale(&self) -> f32 {
        self.size().x as f32 / STAGE_WIDTH as f32
    }
    pub fn select(&mut self, choice: ResolutionChoice) -> bool {
        if self.choice == choice || !self.environment.supports(choice) {
            return false;
        }
        self.choice = choice;
        self.revision = self.revision.wrapping_add(1);
        true
    }
    pub fn set_environment(&mut self, environment: DisplayEnvironment) {
        let environment = environment.normalized();
        if environment == self.environment {
            return;
        }
        self.environment = environment;
        if !self.environment.supports(self.choice) {
            self.choice = ResolutionChoice::Auto;
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

/// Shared by the actual window host and the offscreen presentation fixture.
pub fn preserve_original_stage(projection: &mut Projection) {
    if let Projection::Orthographic(orthographic) = projection {
        orthographic.scaling_mode = bevy::camera::ScalingMode::Fixed {
            width: STAGE_WIDTH as f32,
            height: STAGE_HEIGHT as f32,
        };
        orthographic.scale = 1.0;
    }
}

fn copy(locale: Locale, key: &str) -> &'static str {
    let values: [&str; 9] = match key {
        "display" => [
            "Display settings",
            "顯示設定",
            "Configurações de vídeo",
            "Настройки экрана",
            "डिस्प्ले सेटिंग",
            "Pengaturan tampilan",
            "Cài đặt hiển thị",
            "การตั้งค่าการแสดงผล",
            "إعدادات العرض",
        ],
        "resolution" => [
            "Window size",
            "窗口解析度",
            "Tamanho da janela",
            "Размер окна",
            "विंडो का आकार",
            "Ukuran jendela",
            "Kích thước cửa sổ",
            "ขนาดหน้าต่าง",
            "حجم النافذة",
        ],
        "auto" => [
            "Automatic (recommended)",
            "自動（建議）",
            "Automático (recomendado)",
            "Авто (рекомендуется)",
            "स्वचालित (अनुशंसित)",
            "Otomatis (disarankan)",
            "Tự động (khuyến nghị)",
            "อัตโนมัติ (แนะนำ)",
            "تلقائي (موصى به)",
        ],
        "unavailable" => [
            "Does not fit this display",
            "無法容納於此螢幕",
            "Não cabe nesta tela",
            "Не помещается на экране",
            "इस स्क्रीन पर नहीं समाता",
            "Tidak muat di layar ini",
            "Không vừa màn hình này",
            "ไม่พอดีกับหน้าจอนี้",
            "لا تناسب هذه الشاشة",
        ],
        _ => [""; 9],
    };
    values[locale.index()]
}

fn dimensions(size: UVec2, locale: Locale) -> String {
    let text = format!("{} × {}", size.x, size.y);
    // Width×height is an LTR value even in an Arabic paragraph. The standard
    // isolates also protect it when a translated unavailable suffix follows.
    if locale.is_rtl() {
        format!("\u{2066}{text}\u{2069}")
    } else {
        text
    }
}

fn choice_label(choice: ResolutionChoice, locale: Locale, supported: bool) -> String {
    let label = choice
        .size()
        .map(|size| dimensions(size, locale))
        .unwrap_or_else(|| copy(locale, "auto").into());
    if supported {
        label
    } else {
        format!("{label} — {}", copy(locale, "unavailable"))
    }
}

#[derive(Component)]
struct DisplaySelector;
#[derive(Component)]
struct DisplayMenu;
#[derive(Component)]
struct DisplayToggle;
#[derive(Component)]
struct CurrentResolution;
#[derive(Component)]
struct DisplayHeading;
#[derive(Component, Clone, Copy)]
pub struct DisplayChoice(pub ResolutionChoice);
#[derive(Component, Clone, Copy)]
struct ChoiceText(ResolutionChoice);
#[derive(Resource, Default)]
struct DisplayMenuState {
    open: bool,
}

pub struct NativeDisplayUiPlugin;
impl Plugin for NativeDisplayUiPlugin {
    fn build(&self, app: &mut App) {
        // The host inserts settings before installing this plugin; other hosts
        // remain untouched unless they explicitly opt into the display controls.
        app.init_resource::<NativeDisplaySettings>()
            .init_resource::<DisplayMenuState>()
            .add_systems(Startup, spawn_selector)
            .add_systems(
                Update,
                (toggle_menu, choose_resolution, refresh_selector).chain(),
            );
    }
}

fn spawn_selector(mut commands: Commands) {
    commands
        .spawn((
            DisplaySelector,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(12.0),
                left: px(12.0),
                width: px(328.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GlobalZIndex(24998),
            FocusPolicy::Block,
        ))
        .with_children(|root| {
            root.spawn((
                Button,
                DisplayToggle,
                Node {
                    min_height: px(50.0),
                    padding: UiRect::all(px(6.0)),
                    border: UiRect::all(px(1.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.65, 0.50, 0.23)),
                BackgroundColor(Color::srgb(0.10, 0.08, 0.04)),
                FocusPolicy::Block,
            ))
            .with_children(|button| {
                button.spawn((
                    CurrentResolution,
                    Text::new(""),
                    crate::crystal_ui::typography::crystal_text_font(13.0),
                    TextColor(Color::srgb(0.95, 0.91, 0.77)),
                    TextLayout::justify(Justify::Center),
                ));
            });
            root.spawn((
                DisplayMenu,
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    bottom: px(56.0),
                    left: px(0.0),
                    width: percent(100.0),
                    padding: UiRect::all(px(6.0)),
                    row_gap: px(3.0),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                GlobalZIndex(24999),
                BorderColor::all(Color::srgb(0.65, 0.50, 0.23)),
                BackgroundColor(Color::srgb(0.055, 0.045, 0.03)),
                FocusPolicy::Block,
            ))
            .with_children(|menu| {
                menu.spawn((
                    DisplayHeading,
                    Text::new(""),
                    crate::crystal_ui::typography::crystal_text_font(14.0),
                    TextColor(Color::srgb(0.95, 0.91, 0.77)),
                    TextLayout::justify(Justify::Center),
                ));
                for choice in ResolutionChoice::ALL {
                    menu.spawn((
                        Button,
                        DisplayChoice(choice),
                        Node {
                            min_height: px(32.0),
                            padding: UiRect::axes(px(5.0), px(4.0)),
                            border: UiRect::all(px(1.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.45, 0.36, 0.18)),
                        BackgroundColor(Color::srgb(0.10, 0.08, 0.04)),
                        FocusPolicy::Block,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            ChoiceText(choice),
                            Text::new(""),
                            crate::crystal_ui::typography::crystal_text_font(12.0),
                            TextColor(Color::srgb(0.95, 0.91, 0.77)),
                            TextLayout::justify(Justify::Center),
                        ));
                    });
                }
            });
        });
}

fn toggle_menu(
    toggles: Query<&Interaction, (With<DisplayToggle>, Changed<Interaction>)>,
    hovered: Query<&Interaction, Or<(With<DisplayToggle>, With<DisplayChoice>)>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut menu: ResMut<DisplayMenuState>,
) {
    if mouse.is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
        && !hovered
            .iter()
            .any(|interaction| *interaction != Interaction::None)
    {
        menu.open = false;
    }
    for interaction in &toggles {
        if *interaction == Interaction::Pressed {
            menu.open = !menu.open;
        }
    }
}

fn choose_resolution(
    buttons: Query<(&Interaction, &DisplayChoice), Changed<Interaction>>,
    shell: Option<Res<NativeShellModel>>,
    mut settings: ResMut<NativeDisplaySettings>,
    mut menu: ResMut<DisplayMenuState>,
) {
    if !shell.is_some_and(|shell| shell.screen == NativeShellScreen::Login) {
        return;
    }
    for (interaction, choice) in &buttons {
        if *interaction == Interaction::Pressed && settings.environment.supports(choice.0) {
            settings.select(choice.0);
            menu.open = false;
        }
    }
}

fn refresh_selector(
    shell: Option<Res<NativeShellModel>>,
    settings: Res<NativeDisplaySettings>,
    mut menu: ResMut<DisplayMenuState>,
    mut roots: Query<&mut Node, (With<DisplaySelector>, Without<DisplayMenu>)>,
    mut menus: Query<&mut Node, (With<DisplayMenu>, Without<DisplaySelector>)>,
    mut current: Query<
        &mut Text,
        (
            With<CurrentResolution>,
            Without<DisplayHeading>,
            Without<ChoiceText>,
        ),
    >,
    mut heading: Query<
        &mut Text,
        (
            With<DisplayHeading>,
            Without<CurrentResolution>,
            Without<ChoiceText>,
        ),
    >,
    mut options: Query<
        (&ChoiceText, &mut Text, &mut TextColor),
        (Without<CurrentResolution>, Without<DisplayHeading>),
    >,
    mut backgrounds: Query<(&DisplayChoice, &Interaction, &mut BackgroundColor)>,
) {
    let visible = shell.is_some_and(|shell| shell.screen == NativeShellScreen::Login);
    if !visible {
        menu.open = false;
    }
    for mut root in &mut roots {
        root.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut menus {
        node.display = if visible && menu.open {
            Display::Flex
        } else {
            Display::None
        };
    }
    let locale = native_i18n::locale();
    let size = settings.size();
    let label = if settings.choice == ResolutionChoice::Auto {
        format!(
            "{}: {}\n{}  ^",
            copy(locale, "resolution"),
            copy(locale, "auto"),
            dimensions(size, locale)
        )
    } else {
        format!(
            "{}: {}  ^",
            copy(locale, "resolution"),
            dimensions(size, locale)
        )
    };
    for mut text in &mut current {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
    for mut text in &mut heading {
        let label = copy(locale, "display");
        if text.0 != label {
            text.0 = label.into();
        }
    }
    for (choice, mut text, mut color) in &mut options {
        let supported = settings.environment.supports(choice.0);
        let label = choice_label(choice.0, locale, supported);
        if text.0 != label {
            text.0 = label;
        }
        color.0 = if supported {
            Color::srgb(0.95, 0.91, 0.77)
        } else {
            Color::srgb(0.57, 0.55, 0.50)
        };
    }
    for (choice, interaction, mut background) in &mut backgrounds {
        background.0 = if !settings.environment.supports(choice.0) {
            Color::srgb(0.07, 0.06, 0.04)
        } else if settings.choice == choice.0 {
            Color::srgb(0.30, 0.22, 0.08)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.21, 0.16, 0.07)
        } else {
            Color::srgb(0.10, 0.08, 0.04)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage_viewport_is_centered_exact_four_three_and_guards_minimized_targets() {
        for (physical, origin, size) in [
            (
                UVec2::new(1920, 1080),
                UVec2::new(240, 0),
                UVec2::new(1440, 1080),
            ),
            (
                UVec2::new(2560, 1440),
                UVec2::new(320, 0),
                UVec2::new(1920, 1440),
            ),
            (
                UVec2::new(3840, 2160),
                UVec2::new(480, 0),
                UVec2::new(2880, 2160),
            ),
            (
                UVec2::new(2560, 1600),
                UVec2::new(214, 0),
                UVec2::new(2132, 1599),
            ),
            (
                UVec2::new(1080, 1920),
                UVec2::new(0, 555),
                UVec2::new(1080, 810),
            ),
            (UVec2::new(1280, 960), UVec2::ZERO, UVec2::new(1280, 960)),
        ] {
            assert_eq!(stage_viewport(physical), (origin, size));
            assert_eq!(size.x * 3, size.y * 4);
            assert!(origin.x + size.x <= physical.x && origin.y + size.y <= physical.y);
            assert!(physical.x - size.x - origin.x <= origin.x + 1);
            assert!(physical.y - size.y - origin.y <= origin.y + 1);
        }
        for physical in [UVec2::ZERO, UVec2::new(3, 20), UVec2::new(100, 2)] {
            assert_eq!(stage_viewport(physical), (UVec2::ZERO, UVec2::ZERO));
        }
    }
    #[test]
    fn automatic_size_matches_notebook_dpi_and_always_fits_work_area() {
        for (available, dpi, expected) in [
            (UVec2::new(1880, 1000), 1.0, UVec2::new(1024, 768)),
            (UVec2::new(1880, 1000), 1.5, UVec2::new(1280, 960)),
            (UVec2::new(2480, 1340), 1.5, UVec2::new(1536, 1152)),
            (UVec2::new(3720, 2020), 2.0, UVec2::new(2048, 1536)),
        ] {
            let size = DisplayEnvironment {
                available,
                os_scale_factor: dpi,
            }
            .auto_size();
            assert_eq!(size, expected);
            assert!(size.x <= available.x && size.y <= available.y);
            assert_eq!(size.x * 3, size.y * 4);
        }
        for width in [640, 800, 1024, 1920, 2560, 3840] {
            for height in [480, 700, 1000, 1300, 2000] {
                for dpi in [1.0, 1.5, 2.0, 4.0, f32::NAN] {
                    let size = DisplayEnvironment {
                        available: UVec2::new(width, height),
                        os_scale_factor: dpi,
                    }
                    .auto_size();
                    assert!(size.x <= width && size.y <= height);
                    assert_eq!(size.x * 3, size.y * 4);
                }
            }
        }
    }
    #[test]
    fn unavailable_resolution_cannot_be_selected_and_monitor_change_recovers_auto() {
        let mut settings = NativeDisplaySettings::new(
            ResolutionChoice::Auto,
            DisplayEnvironment {
                available: UVec2::new(1900, 1000),
                os_scale_factor: 1.5,
            },
        );
        assert!(!settings.select(ResolutionChoice::R1440x1080));
        assert_eq!(settings.choice, ResolutionChoice::Auto);
        assert!(settings.select(ResolutionChoice::R1280x960));
        assert_eq!(settings.size(), UVec2::new(1280, 960));
        settings.set_environment(DisplayEnvironment {
            available: UVec2::new(1000, 700),
            os_scale_factor: 1.0,
        });
        assert_eq!(settings.choice, ResolutionChoice::Auto);
        assert_eq!(settings.size(), UVec2::new(896, 672));
    }
    #[test]
    fn every_resolution_and_nine_language_labels_are_complete() {
        for choice in ResolutionChoice::ALL {
            assert_eq!(ResolutionChoice::from_code(choice.code()), Some(choice));
            if let Some(size) = choice.size() {
                assert_eq!(size.x * 3, size.y * 4);
            }
        }
        for locale in Locale::ALL {
            for key in ["display", "resolution", "auto", "unavailable"] {
                assert!(!copy(locale, key).is_empty());
            }
            if locale != Locale::English {
                assert_ne!(copy(locale, "resolution"), "Resolution");
            }
        }
        assert!(ResolutionChoice::from_code("1920x1080").is_none());
    }

    #[test]
    fn arabic_dimensions_keep_width_before_height_in_mixed_direction_labels() {
        let size = UVec2::new(1440, 1080);
        assert_eq!(dimensions(size, Locale::English), "1440 × 1080");
        assert_eq!(
            dimensions(size, Locale::Arabic),
            "\u{2066}1440 × 1080\u{2069}"
        );
        assert!(
            choice_label(ResolutionChoice::R1440x1080, Locale::Arabic, false)
                .starts_with("\u{2066}1440 × 1080\u{2069}")
        );
    }

    #[test]
    fn login_selector_accepts_supported_choices_and_never_changes_in_game() {
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::Login,
            ..default()
        })
        .insert_resource(NativeDisplaySettings::new(
            ResolutionChoice::Auto,
            DisplayEnvironment {
                available: UVec2::new(1880, 1000),
                os_scale_factor: 1.5,
            },
        ))
        .add_plugins(NativeDisplayUiPlugin);
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<DisplaySelector>>()
            .single(app.world())
            .unwrap();
        let toggle = app
            .world_mut()
            .query_filtered::<Entity, With<DisplayToggle>>()
            .single(app.world())
            .unwrap();
        *app.world_mut().get_mut::<Interaction>(toggle).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().resource::<DisplayMenuState>().open);
        let find = |world: &mut World, choice| {
            world
                .query::<(Entity, &DisplayChoice)>()
                .iter(world)
                .find(|(_, marker)| marker.0 == choice)
                .unwrap()
                .0
        };
        let unavailable = find(app.world_mut(), ResolutionChoice::R1440x1080);
        *app.world_mut().get_mut::<Interaction>(unavailable).unwrap() = Interaction::Pressed;
        app.update();
        assert_eq!(
            app.world().resource::<NativeDisplaySettings>().choice,
            ResolutionChoice::Auto
        );
        assert!(app.world().resource::<DisplayMenuState>().open);
        let allowed = find(app.world_mut(), ResolutionChoice::R1280x960);
        *app.world_mut().get_mut::<Interaction>(allowed).unwrap() = Interaction::Pressed;
        app.update();
        assert_eq!(
            app.world().resource::<NativeDisplaySettings>().choice,
            ResolutionChoice::R1280x960
        );
        assert!(!app.world().resource::<DisplayMenuState>().open);
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
        let small = find(app.world_mut(), ResolutionChoice::R1024x768);
        *app.world_mut().get_mut::<Interaction>(small).unwrap() = Interaction::Pressed;
        app.update();
        assert_eq!(
            app.world().resource::<NativeDisplaySettings>().choice,
            ResolutionChoice::R1280x960
        );
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn login_display_labels_switch_in_all_nine_locales() {
        for locale in Locale::ALL {
            native_i18n::with_locale(locale, || {
                let mut app = App::new();
                app.insert_resource(NativeShellModel {
                    screen: NativeShellScreen::Login,
                    ..default()
                })
                .add_plugins(NativeDisplayUiPlugin);
                // with_locale is thread-local; keep this pure fixture's
                // systems on the fixture thread instead of changing globals.
                app.edit_schedule(Update, |schedule| {
                    schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::default());
                });
                app.update();
                let current = app
                    .world_mut()
                    .query_filtered::<&Text, With<CurrentResolution>>()
                    .single(app.world())
                    .unwrap();
                assert!(current.0.contains(copy(locale, "resolution")));
                assert!(current.0.contains(copy(locale, "auto")));
                let labels = app
                    .world_mut()
                    .query::<(&ChoiceText, &Text)>()
                    .iter(app.world())
                    .map(|(choice, text)| (choice.0, text.0.clone()))
                    .collect::<Vec<_>>();
                let large = &labels
                    .iter()
                    .find(|(choice, _)| *choice == ResolutionChoice::R2048x1536)
                    .unwrap()
                    .1;
                assert!(large.contains(copy(locale, "unavailable")));
            });
        }
    }

    #[derive(Resource, Default)]
    struct DisplayCapture(Option<Image>);
    #[derive(Component)]
    struct DisplayWorldReference;
    #[derive(Component)]
    struct DisplayNameReference;

    /// Actual production login/HUD widgets and bundled fonts, at the physical
    /// client sizes selected for 1080p/1440p/4K notebook work areas. Image-target
    /// projection changes are confined to this explicit offscreen fixture.
    #[test]
    #[ignore = "offline GPU evidence; run alone with real MIR2_I18N_VISUAL_ASSET_ROOT and a fresh MIR2_DISPLAY_VISUAL_OUTPUT"]
    fn nine_locale_login_and_scaled_hud_offscreen() {
        use crate::crystal_ui::{
            hud::{CrystalHudAction, CrystalHudRoot, Mir2CrystalHudPlugin},
            login::CrystalLoginAction,
            metrics::CrystalStageTransform,
            spec,
        };
        use crate::native_shell_ui::i18n_visual_tests::{
            i18n_offscreen_app_configured, i18n_text_layouts, warm_i18n_images,
        };
        use bevy::{render::render_resource::Extent3d, ui::UiGlobalTransform};
        use serde_json::json;
        use std::{fs, path::PathBuf};
        let assets = PathBuf::from(
            std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("real packaged UI assets"),
        );
        let output = PathBuf::from(
            std::env::var_os("MIR2_DISPLAY_VISUAL_OUTPUT").expect("fresh evidence directory"),
        );
        assert!(output.is_absolute() && !output.exists() && !output.starts_with(&assets));
        fs::create_dir(&output).unwrap();
        fs::write(
            output.join("display-layouts.json"),
            b"{\"passed\":false,\"complete\":false}",
        )
        .unwrap();
        let previous_locale = native_i18n::locale();
        native_i18n::activate(Locale::English);
        let (mut app, target, camera) = i18n_offscreen_app_configured(&assets, true, |app| {
            app.add_plugins((NativeDisplayUiPlugin, Mir2CrystalHudPlugin));
            // The offline helper omits the audio device plugin. Desktop overlay
            // startup needs its typed container only with the audio feature;
            // the shared phone UI must compile without enabling that backend.
            #[cfg(feature = "native-ui")]
            app.init_resource::<Assets<bevy::audio::AudioSource>>();
            app.insert_resource(crate::options_effects::OptionsRuntime::with_config_path(
                output.join("fixture-options.json"),
            ))
            .insert_resource(
                crate::skill_binding_persistence::SkillBindingPersistenceRuntime::with_config_path(
                    output.join("fixture-skill-bindings.json"),
                ),
            );
        });
        app.init_resource::<DisplayCapture>()
            .insert_resource(crate::map::MapModel {
                center_x: 334,
                center_y: 259,
                mini_map_index: Some(1),
                map_width: Some(1000),
                map_height: Some(1000),
                ..default()
            })
            .insert_resource(crate::read_model::UiReadModel {
                player: crate::read_model::PlayerStats {
                    name: Some("Display QA".into()),
                    level: 9,
                    hp: 85,
                    max_hp: 130,
                    mp: 42,
                    max_mp: 80,
                    gold: 12345,
                    experience: 100,
                    max_experience: 1000,
                    map_name: Some("BichonProvince".into()),
                    ..default()
                },
            });
        app.world_mut().spawn((
            DisplayWorldReference,
            Sprite::from_color(Color::srgb(0.2, 0.6, 0.3), Vec2::new(96.0, 64.0)),
            Transform::from_xyz(-256.0, 128.0, 0.0),
        ));
        let image = app
            .world()
            .resource::<AssetServer>()
            .load("original-ui/NPC/83/0.png");
        app.world_mut().spawn((
            Sprite::from_image(image),
            Transform::from_xyz(0.0, 20.0, 1.0),
        ));
        app.world_mut().spawn((
            DisplayNameReference,
            Node {
                position_type: PositionType::Absolute,
                left: px(472.0),
                top: px(305.0),
                width: px(80.0),
                height: px(20.0),
                display: Display::None,
                ..default()
            },
            Text::new("Display QA"),
            crate::crystal_ui::typography::crystal_text_font(12.0),
            TextColor(Color::WHITE),
            TextLayout::justify(Justify::Center),
        ));
        preserve_original_stage(&mut app.world_mut().get_mut::<Projection>(camera).unwrap());
        let profiles = [
            (
                "1080p-100",
                UVec2::new(1880, 1000),
                1.0,
                UVec2::new(1024, 768),
            ),
            (
                "1080p-150",
                UVec2::new(1880, 1000),
                1.5,
                UVec2::new(1280, 960),
            ),
            (
                "1440p-150",
                UVec2::new(2480, 1340),
                1.5,
                UVec2::new(1536, 1152),
            ),
            (
                "4k-200",
                UVec2::new(3720, 2020),
                2.0,
                UVec2::new(2048, 1536),
            ),
        ];
        let mut cases = Vec::new();
        let mut failures = Vec::new();
        for (profile, available, dpi, expected) in profiles {
            let settings = NativeDisplaySettings::new(
                ResolutionChoice::Auto,
                DisplayEnvironment {
                    available,
                    os_scale_factor: dpi,
                },
            );
            assert_eq!(settings.size(), expected);
            let factor = settings.scale();
            app.insert_resource(UiScale(factor))
                .insert_resource(settings);
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(&target)
                .unwrap()
                .resize(Extent3d {
                    width: expected.x,
                    height: expected.y,
                    depth_or_array_layers: 1,
                });
            for locale in Locale::ALL {
                native_i18n::activate(locale);
                app.world_mut().resource_mut::<NativeShellModel>().screen =
                    NativeShellScreen::Login;
                app.world_mut().resource_mut::<DisplayMenuState>().open = true;
                warm_i18n_images(&mut app);
                let mut rows = i18n_text_layouts(&mut app);
                for row in &mut rows {
                    let rect = row["rect"].as_array().unwrap();
                    let rect: Vec<_> = rect.iter().map(|value| value.as_f64().unwrap()).collect();
                    row["nodeOutsideViewport"] = json!(
                        rect[0] < -1.0
                            || rect[1] < -1.0
                            || rect[0] + rect[2] > expected.x as f64 + 1.0
                            || rect[1] + rect[3] > expected.y as f64 + 1.0
                    );
                    if row["glyphs"] == 0
                        || row["missingGlyphs"] != 0
                        || row["layoutExceedsNode"] != false
                        || row["nodeOutsideViewport"] != false
                    {
                        failures.push(json!({"profile":profile,"locale":locale.code(),"row":row}));
                    }
                }
                let world = app.world_mut();
                let controls = world.query::<(&CrystalLoginAction, &ComputedNode, &UiGlobalTransform)>().iter(world)
                    .filter_map(|(action,node,transform)| {
                        let rect = match action {
                            CrystalLoginAction::FocusAccount => spec::login::ACCOUNT_FIELD,
                            CrystalLoginAction::FocusPassword => spec::login::PASSWORD_FIELD,
                            CrystalLoginAction::Login => spec::login::OK.rect,
                            _ => return None,
                        };
                        let center = rect.center();
                        assert!(transform.translation.distance(Vec2::new(center.0,center.1)*factor) <= 1.5);
                        assert!(node.size().distance(Vec2::new(rect.width,rect.height)*factor) <= 1.5);
                        let stage = CrystalStageTransform::fit_native(expected.x as f32, expected.y as f32);
                        let logical = stage.physical_to_logical(transform.translation.x,transform.translation.y);
                        assert!(rect.contains(logical.0,logical.1));
                        Some(json!({"action":format!("{action:?}"),"center":transform.translation.to_array(),"size":node.size().to_array(),"manualHit":true}))
                    }).collect::<Vec<_>>();
                assert_eq!(controls.len(), 3);
                let file = format!("{profile}-{}-login.png", locale.code());
                capture_display(&mut app, &target, &output.join(&file), expected);
                cases.push(json!({"profile":profile,"locale":locale.code(),"physical":expected.to_array(),
                    "uiScale":factor,"screen":"login","file":file,"controls":controls,"textRows":rows}));
            }
            app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
            for mut node in app
                .world_mut()
                .query_filtered::<&mut Node, With<DisplayNameReference>>()
                .iter_mut(app.world_mut())
            {
                node.display = Display::Flex;
            }
            for locale in Locale::ALL {
                native_i18n::activate(locale);
                warm_i18n_images(&mut app);
                let mut rows = i18n_text_layouts(&mut app);
                for row in &mut rows {
                    let rect = row["rect"].as_array().unwrap();
                    let rect: Vec<_> = rect.iter().map(|value| value.as_f64().unwrap()).collect();
                    row["nodeOutsideViewport"] = json!(
                        rect[0] < -1.0
                            || rect[1] < -1.0
                            || rect[0] + rect[2] > expected.x as f64 + 1.0
                            || rect[1] + rect[3] > expected.y as f64 + 1.0
                    );
                    if row["glyphs"] == 0
                        || row["missingGlyphs"] != 0
                        || row["layoutExceedsNode"] != false
                        || row["nodeOutsideViewport"] != false
                    {
                        failures.push(json!({"profile":profile,"locale":locale.code(),"screen":"hud","row":row}));
                    }
                }
                let world = app.world_mut();
                let (hud, transform) = world
                    .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<CrystalHudRoot>>()
                    .single(world)
                    .unwrap();
                assert!(hud.size().distance(expected.as_vec2()) <= 1.0);
                assert!(transform.translation.distance(expected.as_vec2() / 2.0) <= 1.0);
                let controls = world.query::<(&CrystalHudAction,&ComputedNode,&UiGlobalTransform)>().iter(world)
                .filter_map(|(action,node,transform)| {
                    let rect = match action { CrystalHudAction::Inventory => spec::hud::INVENTORY.rect,
                        CrystalHudAction::BigMap => spec::hud::BIG_MAP.rect, _ => return None };
                    let center=rect.center();
                    assert!(transform.translation.distance(Vec2::new(center.0,center.1)*factor)<=1.5);
                    assert!(node.size().distance(Vec2::new(rect.width,rect.height)*factor)<=1.5);
                    Some(json!({"action":format!("{action:?}"),"center":transform.translation.to_array(),"size":node.size().to_array()}))
                }).collect::<Vec<_>>();
                assert_eq!(controls.len(), 2);
                let sprite = world
                    .query_filtered::<&GlobalTransform, With<DisplayWorldReference>>()
                    .single(world)
                    .unwrap()
                    .translation();
                let camera_component = world.get::<Camera>(camera).unwrap();
                assert_eq!(camera_component.physical_target_size(), Some(expected));
                assert_eq!(camera_component.target_scaling_factor(), Some(1.0));
                let position = camera_component
                    .world_to_viewport(world.get::<GlobalTransform>(camera).unwrap(), sprite)
                    .unwrap();
                assert!(position.distance(Vec2::new(256.0, 256.0) * factor) < 0.1);
                let file = format!("{profile}-{}-hud.png", locale.code());
                capture_display(&mut app, &target, &output.join(&file), expected);
                cases.push(json!({"profile":profile,"locale":locale.code(),"physical":expected.to_array(),"uiScale":factor,
                "screen":"hud","file":file,"controls":controls,"worldReference":position.to_array(),"hudFillsClient":true,"textRows":rows}));
            }
            for mut node in app
                .world_mut()
                .query_filtered::<&mut Node, With<DisplayNameReference>>()
                .iter_mut(app.world_mut())
            {
                node.display = Display::None;
            }
        }
        native_i18n::activate(previous_locale);
        fs::write(output.join("display-layouts.json"),serde_json::to_vec_pretty(&json!({
            "passed":failures.is_empty(),"complete":true,"livePlayerAcceptance":false,"systemFonts":false,
            "virtualStage":[1024,768],"cases":cases,"failures":failures,
        })).unwrap()).unwrap();
        assert!(
            failures.is_empty(),
            "display/font layout failures: {failures:?}"
        );
    }

    fn capture_display(
        app: &mut App,
        target: &Handle<Image>,
        path: &std::path::Path,
        expected: UVec2,
    ) {
        use crate::native_shell_ui::i18n_visual_tests::i18n_step;
        use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
        use std::time::{Duration, Instant};
        assert!(!path.exists());
        app.world_mut().resource_mut::<DisplayCapture>().0 = None;
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |captured: On<ScreenshotCaptured>, mut output: ResMut<DisplayCapture>| {
                    output.0 = Some(captured.image.clone());
                },
            );
        let started = Instant::now();
        while app.world().resource::<DisplayCapture>().0.is_none() {
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "display capture timed out"
            );
            i18n_step(app);
            std::thread::sleep(Duration::from_millis(5));
        }
        let image = app
            .world_mut()
            .resource_mut::<DisplayCapture>()
            .0
            .take()
            .unwrap()
            .try_into_dynamic()
            .unwrap()
            .to_rgba8();
        assert_eq!(image.dimensions(), (expected.x, expected.y));
        image.save(path).unwrap();
    }
}
