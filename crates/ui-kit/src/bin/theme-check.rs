//! Theme package checker.
//!
//! Usage: theme-check <theme folder> [more folders...]
//! Exits with 0 when everything passes.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() || dirs.iter().any(|d| d == "-h" || d == "--help") {
        eprintln!("usage: theme-check <theme folder> [more folders...]");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for d in &dirs {
        let r = ui_kit::check::check(Path::new(d));
        match &r.theme {
            Some(t) => println!(
                "{d}: {} {} ({} files, {:.1} KB, {} module templates)",
                t.manifest.name,
                t.manifest.version,
                r.files,
                r.bytes as f64 / 1024.0,
                t.modules.len()
            ),
            None => println!("{d}"),
        }
        for e in &r.errors {
            println!("  error    {e}");
        }
        for w in &r.warnings {
            println!("  warning  {w}");
        }
        if r.ok() {
            println!("  ok");
        }
        failed |= !r.ok();
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
