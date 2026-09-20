fn main() {
    println!("cargo:rerun-if-changed=resources/numeron.rc");
    println!("cargo:rerun-if-changed=resources/numeron.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("resources/numeron.rc", embed_resource::NONE)
            .manifest_required()
            .expect("embed numeron Windows icon and product metadata");
    }
}
