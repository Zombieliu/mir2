//! Preserve the original sRGB lighting view on GLES adapters without alternate
//! texture-view formats. No palette, blend, range, shader or lighting fallback.
use bevy::image::Image;
use bevy::render::render_resource::TextureFormat;

pub(crate) fn image(width: u32, height: u32, supports_view_formats: bool) -> Image {
    if supports_view_formats {
        Image::new_target_texture(width, height, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb))
    } else {
        // Both rendering and sampling still see Rgba8UnormSrgb. Allocate that
        // native format directly instead of requesting an unsupported alternate
        // view of Rgba8Unorm. This is not a linear/darkness approximation.
        Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::TextureUsages;

    fn view_format(image: &Image) -> TextureFormat {
        image.texture_view_descriptor.as_ref().and_then(|view| view.format)
            .unwrap_or(image.texture_descriptor.format)
    }

    #[test]
    fn supported_adapters_retain_the_original_target_descriptor_and_clear_bytes() {
        let actual = image(1024, 768, true);
        let original = Image::new_target_texture(1024, 768,
            TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb));
        assert_eq!(actual.texture_descriptor, original.texture_descriptor);
        assert_eq!(actual.texture_view_descriptor, original.texture_view_descriptor);
        assert_eq!(actual.data, original.data);
        assert_eq!(actual.asset_usage, original.asset_usage);
        assert_eq!(actual.copy_on_resize, original.copy_on_resize);
    }

    #[test]
    fn unsupported_adapters_use_the_same_srgb_view_without_alternate_formats() {
        let actual = image(1024, 768, false);
        assert_eq!(actual.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
        assert!(actual.texture_descriptor.view_formats.is_empty());
        assert!(actual.texture_view_descriptor.is_none());
        assert_eq!(view_format(&actual), TextureFormat::Rgba8UnormSrgb);
    }

    #[test]
    fn both_paths_keep_the_same_gamma_size_usage_and_zero_initial_pixels() {
        for supported in [false, true] {
            let actual = image(37, 19, supported);
            assert_eq!(view_format(&actual), TextureFormat::Rgba8UnormSrgb);
            assert_eq!((actual.width(), actual.height()), (37, 19));
            assert_eq!(actual.texture_descriptor.usage, TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT);
            let pixels = actual.data.unwrap();
            assert_eq!(pixels.len(), 37 * 19 * 4);
            assert!(pixels.iter().all(|pixel| *pixel == 0));
            assert_eq!(actual.texture_descriptor.sample_count, 1);
            assert_eq!(actual.texture_descriptor.mip_level_count, 1);
        }
    }
}
