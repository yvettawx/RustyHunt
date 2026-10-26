// 安装包的内容在打包时由 scripts/package.ps1 通过 RH_PAYLOAD 指定（便携版的 zip）。
// 平时编译工作区时没有这个变量，放一个空包，运行时会提示。
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("payload.zip");
    println!("cargo:rerun-if-env-changed=RH_PAYLOAD");
    match std::env::var("RH_PAYLOAD") {
        Ok(p) => {
            println!("cargo:rerun-if-changed={p}");
            std::fs::copy(&p, &out).expect("RH_PAYLOAD 指向的文件读不到");
        }
        Err(_) => std::fs::write(&out, b"").unwrap(),
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../assets/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon.ico")
            .set("ProductName", "RustyHunt")
            .set("FileDescription", "RustyHunt Setup");
        if let Err(e) = res.compile() {
            println!("cargo:warning=没有嵌入图标：{e}");
        }
    }
}
