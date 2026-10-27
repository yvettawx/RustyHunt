// The installer payload (the portable zip) is given by scripts/package.ps1 through RH_PAYLOAD when packaging.
// Ordinary workspace builds don't set it, so an empty payload is embedded and the installer says so at runtime.
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("payload.zip");
    println!("cargo:rerun-if-env-changed=RH_PAYLOAD");
    match std::env::var("RH_PAYLOAD") {
        Ok(p) => {
            println!("cargo:rerun-if-changed={p}");
            std::fs::copy(&p, &out).expect("cannot read the file RH_PAYLOAD points to");
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
            println!("cargo:warning=icon not embedded: {e}");
        }
    }
}
