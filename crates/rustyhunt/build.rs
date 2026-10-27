// Adds the icon and version info to the exe (shown in Explorer, the Start menu and Task Manager).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico")
        .set("ProductName", "RustyHunt")
        .set("FileDescription", "RustyHunt")
        .set("LegalCopyright", "Apache License 2.0");
    if let Err(e) = res.compile() {
        // Still builds without a resource compiler, just without the icon
        println!("cargo:warning=没有嵌入图标：{e}");
    }
}
