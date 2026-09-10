pub fn load() -> mir2_client_bevy::crystal_ui::overlays::hero_dialog::pointer::HeroPointerSettings {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetDoubleClickTime() -> u32;
        }
        // Read-only native setting; same operating-system interval used by WinForms.
        let value = unsafe { GetDoubleClickTime() };
        return mir2_client_bevy::crystal_ui::overlays::hero_dialog::pointer::HeroPointerSettings {
            double_click_ms: u64::from(value),
        };
    }
    #[cfg(not(windows))]
    Default::default()
}
