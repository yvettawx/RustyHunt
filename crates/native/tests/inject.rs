//! Injects the DLL into a temporary process and checks that it loads, the port accepts connections and the protocol works.
//! Build the DLL first with cargo build -p rustyhunt-native, then run
//! cargo test -p rustyhunt-native --test inject -- --ignored

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

#[test]
#[ignore]
fn injects_into_child_process() {
    let dll = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("rustyhunt_native.dll");
    assert!(dll.is_file(), "先编译 DLL：{}", dll.display());
    let mut child = std::process::Command::new("ping")
        .args(["-n", "30", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let p = platform::process::Process::open_writable(child.id()).unwrap();
    p.inject_dll(&dll).unwrap();
    let mut s = None;
    for _ in 0..30 {
        if let Ok(c) = TcpStream::connect(("127.0.0.1", rh_core::native::PORT)) {
            s = Some(c);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let s = s.expect("连不上 DLL 开的端口");
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut ask = |q: &str| {
        (&s).write_all(format!("{q}\n").as_bytes()).unwrap();
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        line.trim().to_string()
    };
    assert!(ask("HELLO").starts_with("RUSTYHUNT-NATIVE"));
    assert_eq!(ask("STATS"), rh_core::native::Stats::default().format());
    assert!(ask("HOOK 0").starts_with("ERR"));
    child.kill().unwrap();
    let _ = child.wait();
}
