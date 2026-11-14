fn main() {
    const ICON: &str = "../platform-windows/resources/numeron.ico";
    println!("cargo:rerun-if-changed=assets/numeron.rc");
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        assert!(
            std::path::Path::new(ICON).is_file(),
            "Numeron Windows icon is missing: {ICON}; include platform-windows/resources in the source snapshot"
        );
        // Keep the stable desktop entry and the copied update engine branded,
        // using the same multi-resolution artwork as the native game.
        embed_resource::compile_for(
            "assets/numeron.rc",
            ["Mir2Launcher", "Mir2Updater"],
            embed_resource::NONE,
        )
        .manifest_required()
        .expect("embed Numeron launcher/updater Windows icon and product metadata");
        embed_resource::compile_for_examples("assets/numeron.rc", embed_resource::NONE)
            .manifest_required()
            .expect("embed Numeron offline launcher preview icon");
    }
}
