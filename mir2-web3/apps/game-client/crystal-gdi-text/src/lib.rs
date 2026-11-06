//! Guild-only MirLabel rasterization. No global font/layout policy is changed.
//!
//! Padding and multiline vertical alignment follow WinForms TextRenderer:
//! https://source.dot.net/System.Windows.Forms/System/Windows/Forms/Rendering/TextExtensions.cs.html
//! Windows uses memory DCs only; other platforms explicitly request the caller's
//! existing text fallback. Pixel parity still requires the independent reference.
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GuildTextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub struct GuildPixelTextStyle {
    pub font_family: String,
    pub point_size: f32,
    pub dpi: u32,
    pub bold: bool,
    pub foreground: [u8; 4],
    pub outline: Option<[u8; 4]>,
    pub align: GuildTextAlign,
    pub vertical_center: bool,
    pub single_line: bool,
    pub word_break: bool,
}

#[derive(Debug)]
pub struct GuildPixelText {
    pub width: u32,
    pub height: u32,
    /// Straight RGBA, top-down, matching a transparent MirLabel bitmap.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct CacheKey {
    text: String,
    width: u32,
    height: u32,
    font: String,
    point_bits: u32,
    dpi: u32,
    bold: bool,
    foreground: [u8; 4],
    outline: Option<[u8; 4]>,
    align: GuildTextAlign,
    vertical_center: bool,
    single_line: bool,
    word_break: bool,
}

const CACHE_BYTES: usize = 2 * 1024 * 1024;
const CACHE_ENTRIES: usize = 128;

#[derive(Default)]
struct RasterCache {
    entries: HashMap<CacheKey, Arc<GuildPixelText>>,
    fifo: VecDeque<CacheKey>,
    bytes: usize,
}

fn cache() -> &'static Mutex<RasterCache> {
    static CACHE: OnceLock<Mutex<RasterCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(RasterCache::default()))
}

fn validate(
    text: &str,
    width: u32,
    height: u32,
    style: &GuildPixelTextStyle,
) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > 2048
        || height > 512
        || u64::from(width) * u64::from(height) * 4 > CACHE_BYTES as u64
        || text.encode_utf16().count() > 4096
        || text.contains('\0')
        || style.font_family.is_empty()
        || style.font_family.encode_utf16().count() > 31
        || style.font_family.contains('\0')
        || !style.point_size.is_finite()
        || !(1.0..=64.0).contains(&style.point_size)
        || !(48..=384).contains(&style.dpi)
        || style.foreground[3] != 255
        || style.outline.is_some_and(|color| color[3] != 255)
    {
        return Err("invalid or oversized Guild text raster request".into());
    }
    Ok(())
}

pub fn rasterize_guild_text(
    text: &str,
    width: u32,
    height: u32,
    style: &GuildPixelTextStyle,
) -> Result<Arc<GuildPixelText>, String> {
    validate(text, width, height, style)?;
    let key = CacheKey {
        text: text.into(),
        width,
        height,
        font: style.font_family.clone(),
        point_bits: style.point_size.to_bits(),
        dpi: style.dpi,
        bold: style.bold,
        foreground: style.foreground,
        outline: style.outline,
        align: style.align,
        vertical_center: style.vertical_center,
        single_line: style.single_line,
        word_break: style.word_break,
    };
    if let Ok(cache) = cache().try_lock() {
        if let Some(image) = cache.entries.get(&key) {
            return Ok(Arc::clone(image));
        }
    }
    // Never hold a cache lock across GDI allocation or rasterization.
    let image = Arc::new(rasterize_guild_text_uncached(text, width, height, style)?);
    if let Ok(mut cache) = cache().try_lock() {
        if let Some(existing) = cache.entries.get(&key) {
            return Ok(Arc::clone(existing));
        }
        while cache.entries.len() >= CACHE_ENTRIES || cache.bytes + image.rgba.len() > CACHE_BYTES {
            let Some(oldest) = cache.fifo.pop_front() else {
                break;
            };
            if let Some(old) = cache.entries.remove(&oldest) {
                cache.bytes -= old.rgba.len();
            }
        }
        cache.bytes += image.rgba.len();
        cache.fifo.push_back(key.clone());
        cache.entries.insert(key, Arc::clone(&image));
    }
    Ok(image)
}

/// Uncached CPU result for comparison against the independent C# reference.
pub fn rasterize_guild_text_uncached(
    text: &str,
    width: u32,
    height: u32,
    style: &GuildPixelTextStyle,
) -> Result<GuildPixelText, String> {
    validate(text, width, height, style)?;
    #[cfg(windows)]
    {
        windows::render(text, width, height, style)
    }
    #[cfg(not(windows))]
    {
        Err("Guild GDI pixel text is available only on Windows".into())
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{ffi::c_void, mem::size_of, ptr};

    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct DrawTextParams {
        size: u32,
        tab_length: i32,
        left_margin: i32,
        right_margin: i32,
        length_drawn: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct TextMetric {
        height: i32,
        ascent: i32,
        descent: i32,
        internal_leading: i32,
        external_leading: i32,
        average_width: i32,
        maximum_width: i32,
        weight: i32,
        overhang: i32,
        aspect_x: i32,
        aspect_y: i32,
        first: u16,
        last: u16,
        default_character: u16,
        break_character: u16,
        italic: u8,
        underlined: u8,
        struck_out: u8,
        pitch_family: u8,
        charset: u8,
    }
    #[repr(C)]
    struct BitmapInfo {
        header_size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        image_size: u32,
        x_pixels_per_meter: i32,
        y_pixels_per_meter: i32,
        colors_used: u32,
        colors_important: u32,
        color: u32,
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: Handle) -> Handle;
        fn DeleteDC(dc: Handle) -> i32;
        fn CreateDIBSection(
            dc: Handle,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut c_void,
            section: Handle,
            offset: u32,
        ) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
        fn SelectObject(dc: Handle, object: Handle) -> Handle;
        fn CreateFontW(
            height: i32,
            width: i32,
            escapement: i32,
            orientation: i32,
            weight: i32,
            italic: u32,
            underline: u32,
            strikeout: u32,
            charset: u32,
            output_precision: u32,
            clip_precision: u32,
            quality: u32,
            pitch_family: u32,
            face: *const u16,
        ) -> Handle;
        fn GetTextMetricsW(dc: Handle, metrics: *mut TextMetric) -> i32;
        fn SetTextColor(dc: Handle, color: u32) -> u32;
        fn SetBkMode(dc: Handle, mode: i32) -> i32;
        fn SetTextAlign(dc: Handle, alignment: u32) -> u32;
        fn GdiFlush() -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn DrawTextExW(
            dc: Handle,
            text: *mut u16,
            count: i32,
            rect: *mut Rect,
            flags: u32,
            params: *mut DrawTextParams,
        ) -> i32;
    }

    struct Font(Handle);
    impl Drop for Font {
        fn drop(&mut self) {
            unsafe {
                DeleteObject(self.0);
            }
        }
    }
    struct Surface {
        dc: Handle,
        bitmap: Handle,
        previous_bitmap: Handle,
        bits: *mut u8,
        length: usize,
    }
    impl Drop for Surface {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.previous_bitmap);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            }
        }
    }
    struct SelectedFont {
        dc: Handle,
        previous: Handle,
    }
    impl Drop for SelectedFont {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.previous);
            }
        }
    }

    fn valid_object(object: Handle) -> bool {
        !object.is_null() && object as isize != -1
    }

    fn surface(width: u32, height: u32, matte: u8) -> Result<Surface, String> {
        unsafe {
            let dc = CreateCompatibleDC(ptr::null_mut());
            if dc.is_null() {
                return Err("Guild CreateCompatibleDC failed".into());
            }
            let length = width as usize * height as usize * 4;
            let info = BitmapInfo {
                header_size: 40,
                width: width as i32,
                height: -(height as i32),
                planes: 1,
                bit_count: 32,
                compression: 0,
                image_size: length as u32,
                x_pixels_per_meter: 0,
                y_pixels_per_meter: 0,
                colors_used: 0,
                colors_important: 0,
                color: 0,
            };
            let mut bits = ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                DeleteDC(dc);
                return Err("Guild CreateDIBSection failed".into());
            }
            let previous_bitmap = SelectObject(dc, bitmap);
            if !valid_object(previous_bitmap) {
                DeleteObject(bitmap);
                DeleteDC(dc);
                return Err("Guild bitmap selection failed".into());
            }
            ptr::write_bytes(bits, matte, length);
            Ok(Surface {
                dc,
                bitmap,
                previous_bitmap,
                bits: bits.cast(),
                length,
            })
        }
    }

    fn color_ref(color: [u8; 4]) -> u32 {
        u32::from(color[0]) | u32::from(color[1]) << 8 | u32::from(color[2]) << 16
    }

    fn draw(
        surface: &Surface,
        font: &Font,
        text: &[u16],
        width: u32,
        height: u32,
        style: &GuildPixelTextStyle,
    ) -> Result<Vec<u8>, String> {
        unsafe {
            let previous = SelectObject(surface.dc, font.0);
            if !valid_object(previous) {
                return Err("Guild font selection failed".into());
            }
            let _selection = SelectedFont {
                dc: surface.dc,
                previous,
            };
            if SetBkMode(surface.dc, 1) == 0 || SetTextAlign(surface.dc, 0) == u32::MAX {
                return Err("Guild GDI text state failed".into());
            }
            let mut metrics = TextMetric::default();
            if GetTextMetricsW(surface.dc, &mut metrics) == 0 {
                return Err("Guild font metrics failed".into());
            }
            let padding = metrics.height as f32 / 6.0;
            let mut flags = match style.align {
                GuildTextAlign::Left => 0,
                GuildTextAlign::Center => 1,
                GuildTextAlign::Right => 2,
            };
            if style.vertical_center {
                flags |= 4;
            }
            if style.single_line {
                flags |= 32;
            }
            if style.word_break {
                flags |= 16;
            }
            let paint = |x: i32, y: i32, color: [u8; 4]| -> Result<(), String> {
                let mut rect = Rect {
                    left: x,
                    top: y,
                    right: x + width as i32,
                    bottom: y + height as i32,
                };
                let mut params = DrawTextParams {
                    size: size_of::<DrawTextParams>() as u32,
                    tab_length: 0,
                    left_margin: padding.ceil() as i32,
                    right_margin: (padding * 1.5).ceil() as i32,
                    length_drawn: 0,
                };
                if style.vertical_center && !style.single_line {
                    let mut measure = rect;
                    let text_height = DrawTextExW(
                        surface.dc,
                        text.as_ptr().cast_mut(),
                        text.len() as i32,
                        &mut measure,
                        flags | 0x400,
                        &mut params,
                    );
                    if text_height <= height as i32 {
                        let shift = height as i32 / 2 - text_height / 2;
                        rect.top += shift;
                        rect.bottom += shift;
                    }
                }
                if SetTextColor(surface.dc, color_ref(color)) == u32::MAX {
                    return Err("Guild GDI text color failed".into());
                }
                if !text.is_empty()
                    && DrawTextExW(
                        surface.dc,
                        text.as_ptr().cast_mut(),
                        text.len() as i32,
                        &mut rect,
                        flags,
                        &mut params,
                    ) == 0
                {
                    return Err("Guild DrawTextExW failed".into());
                }
                Ok(())
            };
            if let Some(outline) = style.outline {
                for (x, y) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
                    paint(x, y, outline)?;
                }
            }
            paint(1, 1, style.foreground)?;
            if GdiFlush() == 0 {
                return Err("Guild GDI flush failed".into());
            }
            Ok(std::slice::from_raw_parts(surface.bits, surface.length).to_vec())
        }
    }

    pub fn render(
        text: &str,
        width: u32,
        height: u32,
        style: &GuildPixelTextStyle,
    ) -> Result<GuildPixelText, String> {
        let face = style
            .font_family
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let text = text.encode_utf16().collect::<Vec<_>>();
        let font = Font(unsafe {
            CreateFontW(
                -(style.point_size * style.dpi as f32 / 72.0).round() as i32,
                0,
                0,
                0,
                if style.bold { 700 } else { 400 },
                0,
                0,
                0,
                1,
                0,
                0,
                4, // AntiAliasGridFit -> ANTIALIASED_QUALITY, matching the reference Graphics.
                0,
                face.as_ptr(),
            )
        });
        if font.0.is_null() {
            return Err("Guild CreateFontW failed".into());
        }
        let black = draw(
            &surface(width, height, 0)?,
            &font,
            &text,
            width,
            height,
            style,
        )?;
        let white = draw(
            &surface(width, height, 255)?,
            &font,
            &text,
            width,
            height,
            style,
        )?;
        let mut rgba = vec![0; black.len()];
        for ((black, white), pixel) in black
            .chunks_exact(4)
            .zip(white.chunks_exact(4))
            .zip(rgba.chunks_exact_mut(4))
        {
            // GDI's alpha bytes are not valid. WinForms' transparent bitmap
            // publishes touched pixels as opaque; dual mattes recover black
            // outline pixels too, without making the entire label opaque.
            if black[..3] != [0, 0, 0] || white[..3] != [255, 255, 255] {
                pixel.copy_from_slice(&[black[2], black[1], black[0], 255]);
            }
        }
        Ok(GuildPixelText {
            width,
            height,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> GuildPixelTextStyle {
        GuildPixelTextStyle {
            font_family: "Arial".into(),
            point_size: 8.0,
            dpi: 96,
            bold: false,
            foreground: [255; 4],
            outline: Some([0, 0, 0, 255]),
            align: GuildTextAlign::Left,
            vertical_center: false,
            single_line: false,
            word_break: true,
        }
    }

    #[test]
    fn guild_raster_rejects_unbounded_and_malformed_requests() {
        for (text, width, height) in [
            ("x", 0, 10),
            ("x", 10, 0),
            ("x", 2048, 512),
            ("x\0y", 10, 10),
        ] {
            assert!(rasterize_guild_text(text, width, height, &style()).is_err());
        }
        let mut invalid = style();
        invalid.point_size = f32::NAN;
        assert!(rasterize_guild_text("x", 10, 10, &invalid).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn guild_gdi_raster_is_cached_and_keeps_background_transparent() {
        let first = rasterize_guild_text("hahah", 120, 200, &style()).unwrap();
        let second = rasterize_guild_text("hahah", 120, 200, &style()).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(first.rgba.chunks_exact(4).any(|p| p[3] == 255));
        assert!(first.rgba.chunks_exact(4).any(|p| p == [0, 0, 0, 0]));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit offline reference dump; requires a fresh MIR2_GUILD_PIXEL_DUMP_DIR"]
    fn guild_gdi_dump_reference_rgba() {
        let directory =
            std::path::PathBuf::from(std::env::var_os("MIR2_GUILD_PIXEL_DUMP_DIR").unwrap());
        std::fs::create_dir(&directory).unwrap();
        for (id, text, width, height, align, wrap, center, gray) in [
            (
                "statistics-headers",
                "Guild Name\n\nLevel\n\nMembers",
                75,
                300,
                GuildTextAlign::Right,
                false,
                false,
                true,
            ),
            (
                "guild-hahah",
                "hahah",
                120,
                200,
                GuildTextAlign::Left,
                true,
                false,
                false,
            ),
            (
                "level-0",
                "0",
                120,
                200,
                GuildTextAlign::Left,
                true,
                false,
                false,
            ),
            (
                "members-1-of-5",
                "1/5",
                120,
                200,
                GuildTextAlign::Left,
                true,
                false,
                false,
            ),
            (
                "recruit-label",
                "Recruit Member",
                150,
                15,
                GuildTextAlign::Left,
                true,
                false,
                false,
            ),
            (
                "experience-0-percent",
                "0%",
                260,
                15,
                GuildTextAlign::Center,
                false,
                true,
                false,
            ),
        ] {
            let mut options = style();
            options.align = align;
            options.word_break = wrap;
            options.vertical_center = center;
            if gray {
                options.foreground = [128, 128, 128, 255];
            }
            let raster = rasterize_guild_text_uncached(text, width, height, &options).unwrap();
            std::fs::write(directory.join(format!("{id}.rgba")), &raster.rgba).unwrap();
            println!("{id}: {}x{} RGBA", raster.width, raster.height);
        }
    }
}
