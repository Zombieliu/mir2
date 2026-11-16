//! Guild-only GDI label images. Other UI typography is deliberately unchanged.
use super::{
    guild_pixel_text::{rasterize_guild_text, GuildPixelTextStyle},
    typography::crystal_text_font,
    CrystalRect,
};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    text::LineBreak,
    ui::UiSystems,
};
use std::collections::HashMap;

pub(super) use super::guild_pixel_text::GuildTextAlign;
const CACHE_BYTES: usize = 15 * 1024 * 1024;
const CACHE_ENTRIES: usize = 256;
fn use_gdi(active: bool, locale: crate::native_i18n::Locale) -> bool {
    !active || locale == crate::native_i18n::Locale::English
}

#[derive(Component)]
pub(crate) struct GuildPixelLabel {
    pub(crate) text: String,
    pub(crate) rect: CrystalRect,
    family: String,
    point_size: f32,
    color: [u8; 4],
    align: u8,
    word_break: bool,
    vertical_center: bool,
    gdi_eligible: bool,
}

#[derive(Component, Debug)]
pub(crate) enum GuildPixelTextRendering {
    Gdi96Dpi,
    ExistingTextFallback,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    family: String,
    width: u32,
    height: u32,
    point_bits: u32,
    color: [u8; 4],
    align: u8,
    word_break: bool,
    vertical_center: bool,
}
struct Entry {
    handle: Handle<Image>,
    bytes: usize,
    used: u64,
}
#[derive(Resource, Default)]
struct GuildImageCache {
    entries: HashMap<Key, Entry>,
    failed: HashMap<Key, u64>,
    bytes: usize,
    clock: u64,
}
impl GuildImageCache {
    fn advance(&mut self) -> u64 {
        self.clock = self.clock.saturating_add(1);
        self.clock
    }
    fn failed(&mut self, key: Key, at: u64) {
        if self.failed.len() >= 128 {
            if let Some(old) = self
                .failed
                .iter()
                .min_by_key(|(_, t)| **t)
                .map(|(k, _)| k.clone())
            {
                self.failed.remove(&old);
            }
        }
        self.failed.insert(key, at);
    }
    fn reserve(&mut self, bytes: usize) -> bool {
        if bytes >= CACHE_BYTES {
            return false;
        }
        while self.bytes.saturating_add(bytes) >= CACHE_BYTES || self.entries.len() >= CACHE_ENTRIES
        {
            let Some(old) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
            else {
                return false;
            };
            // Dropping the cache handle lets Bevy reclaim unused assets. An
            // ImageNode still on screen owns its own strong handle; never
            // forcibly remove an asset that a live node is drawing.
            if let Some(entry) = self.entries.remove(&old) {
                self.bytes -= entry.bytes;
            }
        }
        true
    }
}

pub(crate) struct GuildPixelTextPlugin;
impl Plugin for GuildPixelTextPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuildImageCache>().add_systems(
            PostUpdate,
            materialize_guild_labels.before(UiSystems::Layout),
        );
    }
}

pub(super) fn spawn_guild_label(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    point_size: f32,
    color: Color,
    align: GuildTextAlign,
    word_break: bool,
    vertical_center: bool,
) {
    let gdi_eligible = use_gdi(crate::native_i18n::active(), crate::native_i18n::locale());
    let family = if gdi_eligible {
        "Arial"
    } else {
        crate::native_i18n::locale().font_family()
    }
    .to_owned();
    let rgba = color.to_srgba();
    let channel = |n: f32| (n.clamp(0., 1.) * 255.).round() as u8;
    let (align_key, justify) = match align {
        GuildTextAlign::Left => (0, Justify::Left),
        GuildTextAlign::Center => (1, Justify::Center),
        GuildTextAlign::Right => (2, Justify::Right),
    };
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        Text::new(text),
        crystal_text_font(point_size * 96. / 72.),
        TextColor(color),
        TextLayout::new(
            justify,
            if word_break {
                LineBreak::WordBoundary
            } else {
                LineBreak::NoWrap
            },
        ),
        GuildPixelLabel {
            text: text.to_owned(),
            rect,
            family,
            point_size,
            color: [
                channel(rgba.red),
                channel(rgba.green),
                channel(rgba.blue),
                channel(rgba.alpha),
            ],
            align: align_key,
            word_break,
            vertical_center,
            gdi_eligible,
        },
    ));
}

fn materialize_guild_labels(
    mut commands: Commands,
    mut cache: ResMut<GuildImageCache>,
    images: Option<ResMut<Assets<Image>>>,
    labels: Query<(Entity, &GuildPixelLabel), Added<GuildPixelLabel>>,
) {
    let Some(mut images) = images else {
        return;
    };
    for (entity, label) in &labels {
        let width = label.rect.width.round() as u32;
        let height = label.rect.height.round() as u32;
        let key = Key {
            text: label.text.clone(),
            family: label.family.clone(),
            width,
            height,
            point_bits: label.point_size.to_bits(),
            color: label.color,
            align: label.align,
            word_break: label.word_break,
            vertical_center: label.vertical_center,
        };
        let at = cache.advance();
        // Bundled non-English fonts live in Bevy/Fontique, not the Windows
        // GDI font table. CreateFontW may silently substitute; retain the
        // established Text path instead of claiming a false GDI match.
        let handle = if !label.gdi_eligible {
            None
        } else if let Some(entry) = cache.entries.get_mut(&key) {
            entry.used = at;
            Some(entry.handle.clone())
        } else if cache.failed.contains_key(&key) {
            None
        } else if width == 0
            || height == 0
            || width > 1024
            || height > 1024
            || !label.point_size.is_finite()
        {
            cache.failed(key.clone(), at);
            None
        } else {
            let style = GuildPixelTextStyle {
                font_family: label.family.clone(),
                point_size: label.point_size,
                dpi: 96,
                bold: false,
                foreground: label.color,
                outline: Some([0, 0, 0, 255]),
                align: match label.align {
                    1 => GuildTextAlign::Center,
                    2 => GuildTextAlign::Right,
                    _ => GuildTextAlign::Left,
                },
                vertical_center: label.vertical_center,
                single_line: false,
                word_break: label.word_break,
            };
            match rasterize_guild_text(&label.text, width, height, &style) {
                Ok(raster)
                    if raster.width == width
                        && raster.height == height
                        && raster.rgba.len() == width as usize * height as usize * 4 =>
                {
                    let bytes = raster.rgba.len();
                    if cache.reserve(bytes) {
                        let mut image = Image::new(
                            Extent3d {
                                width,
                                height,
                                depth_or_array_layers: 1,
                            },
                            TextureDimension::D2,
                            raster.rgba.clone(),
                            TextureFormat::Rgba8UnormSrgb,
                            RenderAssetUsages::default(),
                        );
                        image.sampler = ImageSampler::nearest();
                        let handle = images.add(image);
                        cache.bytes += bytes;
                        cache.entries.insert(
                            key.clone(),
                            Entry {
                                handle: handle.clone(),
                                bytes,
                                used: at,
                            },
                        );
                        Some(handle)
                    } else {
                        cache.failed(key.clone(), at);
                        None
                    }
                }
                _ => {
                    cache.failed(key.clone(), at);
                    None
                }
            }
        };
        if let Some(handle) = handle {
            commands
                .entity(entity)
                .remove::<(Text, TextFont, TextColor, TextLayout, TextShadow)>()
                .insert((ImageNode::new(handle), GuildPixelTextRendering::Gdi96Dpi));
        } else {
            // Explicitly keep the existing Text fallback on non-Windows or
            // failed GDI. This node is not silently reported as pixel parity.
            commands
                .entity(entity)
                .insert(GuildPixelTextRendering::ExistingTextFallback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(text: &str) -> Key {
        Key {
            text: text.into(),
            family: "Arial".into(),
            width: 120,
            height: 200,
            point_bits: 8f32.to_bits(),
            color: [255; 4],
            align: 0,
            word_break: true,
            vertical_center: false,
        }
    }
    #[test]
    fn guild_image_cache_lru_drops_only_its_handle_and_stays_below_sixteen_mib() {
        let mut images = Assets::<Image>::default();
        let mut cache = GuildImageCache::default();
        let first = images.add(Image::default());
        let visible = first.clone();
        cache.entries.insert(
            key("old"),
            Entry {
                handle: first,
                bytes: 8 * 1024 * 1024,
                used: 1,
            },
        );
        cache.bytes = 8 * 1024 * 1024;
        assert!(cache.reserve(8 * 1024 * 1024));
        assert_eq!(cache.bytes, 0);
        assert!(
            images.contains(visible.id()),
            "LRU must not destroy an on-screen image"
        );
        assert!(!cache.reserve(CACHE_BYTES));
    }
    #[test]
    fn guild_negative_cache_is_bounded_and_key_keeps_font_alignment_and_padding_policy() {
        let mut cache = GuildImageCache::default();
        for i in 0..140 {
            cache.failed(key(&i.to_string()), i);
        }
        assert_eq!(cache.failed.len(), 128);
        assert!(!cache.failed.contains_key(&key("0")));
        let a = key("hahah");
        let mut b = a.clone();
        b.align = 2;
        assert!(a != b);
        b = a.clone();
        b.word_break = false;
        assert!(a != b);
    }
    #[test]
    fn guild_gdi_is_english_only_without_touching_bundled_nonenglish_font_paths() {
        use crate::native_i18n::Locale;
        for locale in Locale::ALL {
            assert!(use_gdi(false, locale));
            assert_eq!(use_gdi(true, locale), locale == Locale::English);
        }
    }
}
