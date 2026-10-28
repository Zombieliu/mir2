//! Safe boundary to the separately audited GDI crate. This module remains under
//! client-bevy's forbid(unsafe_code); all Win32 FFI and raster tests live there.

pub(super) use mir2_crystal_gdi_text::{
    rasterize_guild_text, rasterize_guild_text_uncached, GuildPixelText, GuildPixelTextStyle,
    GuildTextAlign,
};
