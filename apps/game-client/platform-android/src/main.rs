//! Native Android host for the mir2-web3 Bevy client.
//!
//! M4 compile-gate slice: prove the shared runtime and this host compile for
//! Android targets (`aarch64-linux-android`, `armv7-linux-androideabi`,
//! `x86_64-linux-android`). Real-device lifecycle, touch, GPU, memory and
//! network-recovery gates are later M4 milestones per ADR-0001.
//!
//! The production Android host will be a Gradle Activity that loads this
//! crate's `cdylib`/native library and hands it a Surface + input queue. Until
//! then this binary proves the shared `build_runtime_app` path compiles for
//! Android with no DOM/canvas assumptions.

use mir2_bevy_runtime::{build_runtime_app, RuntimeWindowSpec};

fn main() {
    // Android surface lifecycle replaces this with the Activity's native
    // surface. The window spec is opaque and windowed like the desktop host.
    let _app = build_runtime_app(RuntimeWindowSpec::native("mir2-web3 (android)"));
    eprintln!("[platform-android] compile gate passed; surface wiring is a later M4 slice");
}
