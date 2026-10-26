//! 主题包校验工具。
//!
//! 用法：theme-check <主题文件夹> [更多文件夹...]
//! 全部通过时退出码为 0。

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() || dirs.iter().any(|d| d == "-h" || d == "--help") {
        eprintln!("用法：theme-check <主题文件夹> [更多文件夹...]");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for d in &dirs {
        let r = ui_kit::check::check(Path::new(d));
        match &r.theme {
            Some(t) => println!("{d}：{} {}（{} 个文件，{:.1} KB，{} 个模块模板）", t.manifest.name, t.manifest.version, r.files, r.bytes as f64 / 1024.0, t.modules.len()),
            None => println!("{d}"),
        }
        for e in &r.errors {
            println!("  错误  {e}");
        }
        for w in &r.warnings {
            println!("  提醒  {w}");
        }
        if r.ok() {
            println!("  通过");
        }
        failed |= !r.ok();
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
