//! Small desktop/host launcher for the Android cdylib.
//!
//! Android packaging builds `src/lib.rs` as a cdylib; the library's
//! `#[bevy_main]` entry emits the native `android_main` symbol. Keeping this
//! binary thin lets host checks use the same app construction without claiming
//! to be an Android Activity.

fn main() {
    mir2_platform_android::main();
}
