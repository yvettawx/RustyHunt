// 给 exe 加图标和版本信息（资源管理器、开始菜单、任务管理器里显示）。
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
        // 没有资源编译器时照样编译，只是没有图标
        println!("cargo:warning=没有嵌入图标：{e}");
    }
}
