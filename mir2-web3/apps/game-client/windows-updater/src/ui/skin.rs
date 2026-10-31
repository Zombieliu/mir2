//! Presentation-only GDI skin. Launcher state and actions remain in `windows`.

use super::stage;
use std::{io::Cursor, mem::size_of, ptr};
use windows_sys::Win32::{
    Foundation::{COLORREF, HWND, RECT, SIZE},
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{
            DRAWITEMSTRUCT, ODS_DISABLED, ODS_FOCUS, ODS_HOTLIGHT, ODS_NOFOCUSRECT, ODS_SELECTED,
        },
        WindowsAndMessaging::{
            DrawIconEx, GetClientRect, GetWindowTextW, LoadIconW, DI_NORMAL, HICON,
        },
    },
};

const HERO: &[u8] = include_bytes!("../../assets/launcher-hero-v2.png");
const MAX_SOURCE_BYTES: usize = 12 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 32 * 1024 * 1024;
const MAX_DIMENSION: u32 = 4096;

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}

const PANEL: COLORREF = rgb(17, 19, 23);
const CARD: COLORREF = rgb(27, 29, 33);
const GOLD: COLORREF = rgb(201, 169, 105);
const MUTED: COLORREF = rgb(165, 165, 160);
const WARM_WHITE: COLORREF = rgb(241, 231, 211);

fn px(logical: i32, dpi: u32) -> i32 {
    ((i64::from(logical) * i64::from(dpi.clamp(48, 768)) + 48) / 96) as i32
}

fn rect(left: i32, top: i32, right: i32, bottom: i32, dpi: u32) -> RECT {
    RECT {
        left: px(left, dpi),
        top: px(top, dpi),
        right: px(right, dpi),
        bottom: px(bottom, dpi),
    }
}

struct Hero {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    info: BITMAPINFO,
}

fn image_bytes(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err("Launcher artwork dimensions are outside the supported range".into());
    }
    let bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|count| count.checked_mul(4))
        .filter(|bytes| *bytes <= MAX_DECODED_BYTES)
        .ok_or("Launcher artwork exceeds the decoded image limit")?;
    Ok(bytes)
}

/// GDI's 32-bit BI_RGB layout is BGRX; composite alpha before handing it to GDI.
fn bgrx(source: &[u8], channels: usize, target: &mut [u8]) -> Result<(), String> {
    if !matches!(channels, 3 | 4)
        || source.len() % channels != 0
        || target.len() != source.len() / channels * 4
    {
        return Err("Launcher artwork has an inconsistent pixel buffer".into());
    }
    for (source, target) in source
        .chunks_exact(channels)
        .zip(target.chunks_exact_mut(4))
    {
        let alpha = if channels == 4 { source[3] as u32 } else { 255 };
        for (out, input, background) in [(0, 2, 23), (1, 1, 19), (2, 0, 17)] {
            target[out] =
                ((source[input] as u32 * alpha + background * (255 - alpha) + 127) / 255) as u8;
        }
        target[3] = 255;
    }
    Ok(())
}

fn decode_hero(source: &[u8]) -> Result<Hero, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Launcher artwork exceeds the encoded image limit".into());
    }
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(source),
        png::Limits {
            bytes: MAX_DECODED_BYTES,
        },
    );
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("Unable to read launcher artwork: {error}"))?;
    let width = reader.info().width;
    let height = reader.info().height;
    let size = image_bytes(width, height)?;
    let output_size = reader
        .output_buffer_size()
        .filter(|bytes| *bytes <= MAX_DECODED_BYTES)
        .ok_or("Launcher artwork output size is unsupported")?;
    let mut decoded = vec![0; output_size];
    let output = reader
        .next_frame(&mut decoded)
        .map_err(|error| format!("Unable to decode launcher artwork: {error}"))?;
    if output.width != width || output.height != height || output.bit_depth != png::BitDepth::Eight
    {
        return Err("Launcher artwork frame format is unsupported".into());
    }
    let channels = match output.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err("Launcher artwork must contain RGB or RGBA pixels".into()),
    };
    let mut pixels = vec![0; size];
    bgrx(&decoded[..output.buffer_size()], channels, &mut pixels)?;

    // Composite a soft lower-left scrim only in this decoded copy. The source
    // artwork stays untouched and the hero's face and upper armour stay clear.
    for y in 0..height as usize {
        let down = (y as f32 / height as f32 - 0.45).max(0.0) / 0.55;
        let opacity = (down * down * (3.0 - 2.0 * down) * 0.90).min(0.90);
        let top = ((0.20 - y as f32 / height as f32) / 0.20).clamp(0.0, 1.0);
        let header_opacity = top * top * 0.26;
        if opacity == 0.0 && header_opacity == 0.0 {
            continue;
        }
        for x in 0..width as usize {
            let across = ((0.76 - x as f32 / width as f32) / 0.18).clamp(0.0, 1.0);
            let keep = (1.0 - opacity * across) * (1.0 - header_opacity * across);
            let offset = (y * width as usize + x) * 4;
            for byte in &mut pixels[offset..offset + 3] {
                *byte = (*byte as f32 * keep) as u8;
            }
        }
    }
    let mut info = BITMAPINFO::default();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        // A negative height declares top-down rows, matching the PNG decoder.
        biHeight: -(height as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: size as u32,
        ..Default::default()
    };
    Ok(Hero {
        pixels,
        width: width as i32,
        height: height as i32,
        info,
    })
}

#[derive(Default)]
struct Fonts {
    brand: HFONT,
    title: HFONT,
    label: HFONT,
    body: HFONT,
    primary: HFONT,
    secondary: HFONT,
    footer: HFONT,
}

impl Fonts {
    fn new(dpi: u32) -> Self {
        unsafe {
            Self {
                brand: font(dpi, 24, 700, windows_sys::core::w!("Segoe UI")),
                title: font(dpi, 48, 700, windows_sys::core::w!("Georgia")),
                label: font(dpi, 11, 600, windows_sys::core::w!("Segoe UI")),
                body: font(dpi, 15, 400, windows_sys::core::w!("Segoe UI")),
                primary: font(dpi, 18, 600, windows_sys::core::w!("Segoe UI")),
                secondary: font(dpi, 14, 500, windows_sys::core::w!("Segoe UI")),
                footer: font(dpi, 10, 500, windows_sys::core::w!("Segoe UI")),
            }
        }
    }

    fn complete(&self) -> bool {
        self.handles().iter().all(|handle| !handle.is_null())
    }

    fn handles(&self) -> [HFONT; 7] {
        [
            self.brand,
            self.title,
            self.label,
            self.body,
            self.primary,
            self.secondary,
            self.footer,
        ]
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        for handle in self.handles() {
            if !handle.is_null() {
                unsafe { DeleteObject(handle) };
            }
        }
    }
}

unsafe fn font(dpi: u32, size: i32, weight: i32, face: *const u16) -> HFONT {
    unsafe {
        CreateFontW(
            -px(size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            face,
        )
    }
}

pub(super) struct Skin {
    hero: Hero,
    panel: HBRUSH,
    card: HBRUSH,
    fonts: Fonts,
    // LoadIconW returns a process-shared resource handle; it must not be destroyed.
    icon: HICON,
}

impl Skin {
    pub(super) fn new(dpi: u32) -> Result<Self, String> {
        let mut skin = Self {
            hero: decode_hero(HERO)?,
            panel: ptr::null_mut(),
            card: ptr::null_mut(),
            fonts: Fonts::new(dpi),
            icon: unsafe { LoadIconW(GetModuleHandleW(ptr::null()), 1usize as *const u16) },
        };
        unsafe {
            skin.panel = CreateSolidBrush(PANEL);
            skin.card = CreateSolidBrush(CARD);
        }
        if skin.panel.is_null() || skin.card.is_null() || !skin.fonts.complete() {
            return Err("Unable to allocate launcher drawing resources".into());
        }
        Ok(skin)
    }

    pub(super) fn panel_brush(&self) -> HBRUSH {
        self.panel
    }

    pub(super) fn card_brush(&self) -> HBRUSH {
        self.card
    }

    pub(super) fn set_dpi(&mut self, dpi: u32) {
        let fonts = Fonts::new(dpi);
        if fonts.complete() {
            self.fonts = fonts;
        }
    }

    /// Owns the BeginPaint/EndPaint pair; call directly from WM_PAINT.
    pub(super) unsafe fn paint(
        &self,
        hwnd: HWND,
        dpi: u32,
        locale: &str,
        percent: u32,
        busy: bool,
        can_play: bool,
    ) {
        unsafe {
            let mut paint = PAINTSTRUCT::default();
            let destination = BeginPaint(hwnd, &mut paint);
            if destination.is_null() {
                EndPaint(hwnd, &paint);
                return;
            }
            let mut bounds = RECT::default();
            if GetClientRect(hwnd, &mut bounds) != 0 && bounds.right > 0 && bounds.bottom > 0 {
                let buffer = CreateCompatibleDC(destination);
                let bitmap = CreateCompatibleBitmap(destination, bounds.right, bounds.bottom);
                if !buffer.is_null() && !bitmap.is_null() {
                    let previous = SelectObject(buffer, bitmap);
                    if !previous.is_null() && previous as isize != -1 {
                        self.canvas(buffer, &bounds, dpi, locale, percent, busy, can_play);
                        BitBlt(
                            destination,
                            0,
                            0,
                            bounds.right,
                            bounds.bottom,
                            buffer,
                            0,
                            0,
                            SRCCOPY,
                        );
                        SelectObject(buffer, previous);
                    } else {
                        self.canvas(destination, &bounds, dpi, locale, percent, busy, can_play);
                    }
                } else {
                    self.canvas(destination, &bounds, dpi, locale, percent, busy, can_play);
                }
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                if !buffer.is_null() {
                    DeleteDC(buffer);
                }
            }
            EndPaint(hwnd, &paint);
        }
    }

    unsafe fn canvas(
        &self,
        dc: HDC,
        bounds: &RECT,
        dpi: u32,
        locale: &str,
        percent: u32,
        busy: bool,
        can_play: bool,
    ) {
        unsafe {
            FillRect(dc, bounds, self.panel);
            let (source_x, source_y, source_width, source_height) = cover(
                self.hero.width,
                self.hero.height,
                bounds.right,
                bounds.bottom,
            );
            SetStretchBltMode(dc, HALFTONE);
            SetBrushOrgEx(dc, 0, 0, ptr::null_mut());
            StretchDIBits(
                dc,
                0,
                0,
                bounds.right,
                bounds.bottom,
                source_x,
                source_y,
                source_width,
                source_height,
                self.hero.pixels.as_ptr().cast(),
                &self.hero.info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            fill(
                dc,
                &RECT {
                    left: px(736, dpi),
                    top: 0,
                    right: bounds.right,
                    bottom: bounds.bottom,
                },
                PANEL,
            );
            fill(dc, &rect(736, 1, 737, 679, dpi), rgb(89, 73, 49));
            fill(dc, &rect(737, 1, 738, 679, dpi), rgb(28, 27, 25));

            if !self.icon.is_null() {
                DrawIconEx(
                    dc,
                    px(28, dpi),
                    px(23, dpi),
                    self.icon,
                    px(32, dpi),
                    px(32, dpi),
                    0,
                    ptr::null_mut(),
                    DI_NORMAL,
                );
            }
            draw_text(
                dc,
                self.fonts.brand,
                "NUMERON",
                rect(72, 14, 452, 46, dpi),
                WARM_WHITE,
                DT_LEFT | DT_SINGLELINE,
            );
            draw_text(
                dc,
                self.fonts.label,
                "L E G E N D  O F  R E B I R T H",
                rect(73, 48, 473, 68, dpi),
                GOLD,
                DT_LEFT | DT_SINGLELINE,
            );

            draw_text(
                dc,
                self.fonts.label,
                "L E G E N D   O F   R E B I R T H",
                rect(42, 410, 690, 430, dpi),
                GOLD,
                DT_LEFT | DT_SINGLELINE,
            );
            let title = stage(locale, "hero-title")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            draw_text(
                dc,
                self.fonts.title,
                &title,
                rect(42, 440, 700, 516, dpi),
                WARM_WHITE,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            fill(dc, &rect(43, 516, 91, 518, dpi), GOLD);
            draw_text(
                dc,
                self.fonts.body,
                &stage(locale, "hero-caption"),
                rect(42, 525, 658, 594, dpi),
                rgb(193, 189, 178),
                DT_LEFT | DT_WORDBREAK,
            );
            draw_text(
                dc,
                self.fonts.footer,
                "W I N D O W S   ·   6 4 - B I T",
                rect(42, 622, 452, 640, dpi),
                MUTED,
                DT_LEFT | DT_SINGLELINE,
            );

            fill(dc, &rect(768, 164, 1088, 165, dpi), rgb(57, 53, 45));
            let card = rect(768, 188, 1088, 408, dpi);
            FillRect(dc, &card, self.card);
            frame(dc, &card, rgb(52, 51, 47));
            fill(dc, &rect(768, 188, 804, 190, dpi), rgb(137, 113, 70));
            let mut status_label_area = rect(788, 209, 1068, 230, dpi);
            if can_play && !busy {
                let label = stage(locale, "ready-label");
                let width = (text_width(dc, self.fonts.footer, &label) + px(16, dpi))
                    .clamp(px(48, dpi), px(128, dpi));
                let badge = RECT {
                    left: px(1068, dpi) - width,
                    top: px(205, dpi),
                    right: px(1068, dpi),
                    bottom: px(230, dpi),
                };
                status_label_area.right = badge.left - px(12, dpi);
                fill(dc, &badge, rgb(32, 39, 30));
                frame(dc, &badge, rgb(66, 75, 51));
                draw_text(
                    dc,
                    self.fonts.footer,
                    &label,
                    inset(badge, px(2, dpi)),
                    rgb(184, 199, 154),
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                );
            }
            draw_text(
                dc,
                self.fonts.label,
                &stage(locale, "status-label"),
                status_label_area,
                GOLD,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
            );

            fill(dc, &rect(768, 468, 1088, 472, dpi), rgb(44, 43, 40));
            let progress = if busy {
                percent.min(100)
            } else if can_play {
                100
            } else {
                0
            };
            if progress != 0 {
                let right = 768 + (320 * progress as i32) / 100;
                gradient(
                    dc,
                    rect(768, 468, right, 472, dpi),
                    rgb(191, 151, 75),
                    rgb(231, 206, 147),
                    false,
                );
            }
            fill(dc, &rect(42, 650, 1088, 651, dpi), rgb(67, 57, 41));
            draw_text(
                dc,
                self.fonts.footer,
                "N U M E R O N   /   M I R 2",
                rect(42, 659, 686, 675, dpi),
                rgb(133, 126, 111),
                DT_LEFT | DT_SINGLELINE,
            );
            frame(dc, bounds, rgb(100, 80, 48));
        }
    }

    pub(super) unsafe fn button(&self, item: &DRAWITEMSTRUCT, dpi: u32) {
        unsafe {
            let dc = item.hDC;
            if dc.is_null() {
                return;
            }
            let area = item.rcItem;
            let disabled = item.itemState & ODS_DISABLED != 0;
            let pressed = item.itemState & ODS_SELECTED != 0;
            let hot = item.itemState & ODS_HOTLIGHT != 0;
            let focus = item.itemState & ODS_FOCUS != 0;
            if matches!(item.CtlID, 101 | 113) {
                fill(
                    dc,
                    &area,
                    if pressed {
                        rgb(64, 55, 42)
                    } else if hot || focus {
                        rgb(38, 36, 31)
                    } else {
                        PANEL
                    },
                );
                draw_text(
                    dc,
                    self.fonts.primary,
                    if item.CtlID == 101 { "×" } else { "−" },
                    area,
                    if disabled { rgb(82, 80, 74) } else { GOLD },
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                if focus && item.itemState & ODS_NOFOCUSRECT == 0 {
                    frame(dc, &inset(area, px(2, dpi)), GOLD);
                }
                return;
            }

            let primary = item.CtlID == 105;
            let (top, bottom, border, ink) = if disabled {
                (
                    rgb(50, 48, 42),
                    rgb(40, 39, 35),
                    rgb(73, 66, 52),
                    rgb(126, 120, 106),
                )
            } else if primary && pressed {
                (
                    rgb(155, 123, 67),
                    rgb(191, 157, 93),
                    rgb(224, 194, 133),
                    rgb(31, 27, 20),
                )
            } else if primary {
                (
                    rgb(229, 207, 156),
                    rgb(180, 144, 78),
                    rgb(244, 217, 164),
                    rgb(29, 25, 18),
                )
            } else if pressed {
                (rgb(43, 39, 31), rgb(32, 30, 26), GOLD, WARM_WHITE)
            } else {
                (
                    rgb(31, 32, 34),
                    rgb(23, 25, 28),
                    if hot || focus { GOLD } else { rgb(93, 80, 57) },
                    rgb(215, 198, 164),
                )
            };
            gradient(dc, area, top, bottom, true);
            frame(dc, &area, border);
            let mut inner = inset(area, px(2, dpi).max(1));
            if primary && !disabled {
                inner.bottom = inner.top + px(1, dpi).max(1);
                fill(
                    dc,
                    &inner,
                    if pressed {
                        rgb(147, 117, 65)
                    } else {
                        rgb(245, 224, 183)
                    },
                );
            }
            let mut caption = vec![0u16; 1024];
            let length = GetWindowTextW(item.hwndItem, caption.as_mut_ptr(), caption.len() as i32);
            caption.truncate(length.max(0) as usize);
            let mut caption_area = inset(area, px(12, dpi));
            // Keep vertical padding modest even for the shorter secondary button.
            caption_area.top = area.top + px(if pressed { 1 } else { 0 }, dpi);
            caption_area.bottom = area.bottom;
            draw_wide(
                dc,
                if primary {
                    self.fonts.primary
                } else {
                    self.fonts.secondary
                },
                &caption,
                caption_area,
                ink,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            if focus && !disabled && item.itemState & ODS_NOFOCUSRECT == 0 {
                frame(
                    dc,
                    &inset(area, px(4, dpi)),
                    if primary { rgb(86, 66, 34) } else { GOLD },
                );
            }
        }
    }
}

impl Drop for Skin {
    fn drop(&mut self) {
        for brush in [self.panel, self.card] {
            if !brush.is_null() {
                unsafe { DeleteObject(brush) };
            }
        }
    }
}

fn inset(area: RECT, amount: i32) -> RECT {
    RECT {
        left: area.left + amount,
        top: area.top + amount,
        right: area.right - amount,
        bottom: area.bottom - amount,
    }
}

/// Cover the target without stretching the artwork's aspect ratio.
fn cover(width: i32, height: i32, target_width: i32, target_height: i32) -> (i32, i32, i32, i32) {
    if i64::from(width) * i64::from(target_height) > i64::from(target_width) * i64::from(height) {
        let cropped_width =
            (i64::from(height) * i64::from(target_width) / i64::from(target_height)) as i32;
        ((width - cropped_width) / 2, 0, cropped_width, height)
    } else {
        let cropped_height =
            (i64::from(width) * i64::from(target_height) / i64::from(target_width)) as i32;
        (0, (height - cropped_height) / 2, width, cropped_height)
    }
}

unsafe fn fill(dc: HDC, area: &RECT, color: COLORREF) {
    unsafe {
        let previous = SetDCBrushColor(dc, color);
        FillRect(dc, area, GetStockObject(DC_BRUSH));
        SetDCBrushColor(dc, previous);
    }
}

unsafe fn frame(dc: HDC, area: &RECT, color: COLORREF) {
    unsafe {
        let previous = SetDCBrushColor(dc, color);
        FrameRect(dc, area, GetStockObject(DC_BRUSH));
        SetDCBrushColor(dc, previous);
    }
}

unsafe fn gradient(dc: HDC, area: RECT, start: COLORREF, end: COLORREF, vertical: bool) {
    let vertex = |x, y, color: COLORREF| TRIVERTEX {
        x,
        y,
        Red: ((color & 0xff) as u16) << 8,
        Green: (((color >> 8) & 0xff) as u16) << 8,
        Blue: (((color >> 16) & 0xff) as u16) << 8,
        Alpha: 0xffff,
    };
    let vertices = [
        vertex(area.left, area.top, start),
        vertex(area.right, area.bottom, end),
    ];
    let mesh = GRADIENT_RECT {
        UpperLeft: 0,
        LowerRight: 1,
    };
    unsafe {
        GradientFill(
            dc,
            vertices.as_ptr(),
            2,
            (&mesh as *const GRADIENT_RECT).cast(),
            1,
            if vertical {
                GRADIENT_FILL_RECT_V
            } else {
                GRADIENT_FILL_RECT_H
            },
        );
    }
}

unsafe fn draw_text(dc: HDC, font: HFONT, text: &str, area: RECT, color: COLORREF, flags: u32) {
    let text: Vec<u16> = text.encode_utf16().collect();
    unsafe { draw_wide(dc, font, &text, area, color, flags) };
}

unsafe fn text_width(dc: HDC, font: HFONT, text: &str) -> i32 {
    let text: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let previous = SelectObject(dc, font);
        let mut size = SIZE::default();
        GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size);
        SelectObject(dc, previous);
        size.cx.max(0)
    }
}

unsafe fn draw_wide(
    dc: HDC,
    font: HFONT,
    text: &[u16],
    mut area: RECT,
    color: COLORREF,
    flags: u32,
) {
    if text.is_empty() {
        return;
    }
    unsafe {
        let previous_font = SelectObject(dc, font);
        let previous_color = SetTextColor(dc, color);
        let previous_mode = SetBkMode(dc, TRANSPARENT as i32);
        DrawTextW(
            dc,
            text.as_ptr(),
            text.len() as i32,
            &mut area,
            flags | DT_NOPREFIX,
        );
        SetBkMode(dc, previous_mode);
        SetTextColor(dc, previous_color);
        SelectObject(dc, previous_font);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_artwork_decodes_to_bounded_top_down_bgrx() {
        let hero = decode_hero(HERO).expect("bundled launcher artwork must decode");
        assert_eq!((hero.width, hero.height), (1536, 1024));
        assert_eq!(hero.pixels.len(), 1536 * 1024 * 4);
        assert_eq!(hero.info.bmiHeader.biHeight, -1024);
        assert!(hero.pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn dimensions_and_malformed_data_fail_without_an_unbounded_allocation() {
        for (width, height) in [(0, 1), (1, 0), (4097, 1), (1, 4097), (4096, 4096)] {
            assert!(image_bytes(width, height).is_err());
        }
        assert_eq!(image_bytes(1536, 1024).unwrap(), 6_291_456);
        assert!(decode_hero(b"not a PNG").is_err());
        assert!(decode_hero(&HERO[..HERO.len() / 2]).is_err());
    }

    #[test]
    fn rgb_and_alpha_are_converted_before_gdi_receives_pixels() {
        let mut rgb_pixel = [0; 4];
        bgrx(&[10, 20, 30], 3, &mut rgb_pixel).unwrap();
        assert_eq!(rgb_pixel, [30, 20, 10, 255]);
        let mut transparent = [0; 4];
        bgrx(&[255, 0, 0, 0], 4, &mut transparent).unwrap();
        assert_eq!(transparent, [23, 19, 17, 255]);
        assert!(bgrx(&[1, 2], 3, &mut rgb_pixel).is_err());
        assert!(bgrx(&[1, 2, 3], 3, &mut [0; 3]).is_err());
    }

    #[test]
    fn cover_preserves_the_artwork_and_dpi_geometry() {
        assert_eq!(cover(1536, 1024, 1120, 680), (0, 46, 1536, 932));
        assert_eq!(cover(1600, 800, 800, 800), (400, 0, 800, 800));
        assert_eq!(px(1120, 96), 1120);
        assert_eq!(px(1120, 144), 1680);
        assert_eq!(px(680, 192), 1360);
    }
}
