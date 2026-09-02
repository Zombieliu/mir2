//! Crystal `Client/MirGraphics/MLibrary.cs:959-1059`.
//! `GetTrueSize` measures nonzero-alpha bounds; Draw still uses the full bitmap.

/// Return only the source visible-bounds size, never a cropped image or offset.
/// The all-transparent source frame deliberately retains its full dimensions.
pub(super) fn crystal_true_size_rgba8(
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Option<(i32, i32)> {
    let width = i32::try_from(width).ok().filter(|width| *width > 0)?;
    let height = i32::try_from(height).ok().filter(|height| *height > 0)?;
    let expected = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    if pixels.len() != expected {
        return None;
    }
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        let x = (index % width as usize) as i32;
        let y = (index / width as usize) as i32;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    if right == 0 {
        Some((width, height))
    } else {
        Some((right - left, bottom - top))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn true_size_uses_any_nonzero_alpha_not_rgb_and_does_not_return_trim_origin() {
        let mut pixels = vec![255u8; 8 * 6 * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 0; // Nonzero transparent RGB is not a visible pixel.
        }
        pixels[(2 * 8 + 3) * 4 + 3] = 1;
        pixels[(4 * 8 + 6) * 4 + 3] = 255;
        assert_eq!(crystal_true_size_rgba8(8, 6, &pixels), Some((4, 3)));
        // The caller must still draw all 8x6 pixels, with no subtraction of
        // the visible region's (3,2) origin: that is Crystal's actual formula.
    }

    #[test]
    fn transparent_frame_falls_back_to_full_size_but_invalid_rgba_is_not_guessed() {
        assert_eq!(crystal_true_size_rgba8(4, 3, &[0; 4 * 3 * 4]), Some((4, 3)));
        assert_eq!(crystal_true_size_rgba8(0, 3, &[]), None);
        assert_eq!(crystal_true_size_rgba8(4, 3, &[0; 47]), None);
        assert_eq!(crystal_true_size_rgba8(4, 3, &[0; 49]), None);
        assert_eq!(crystal_true_size_rgba8(u32::MAX, 3, &[]), None);
    }
}
