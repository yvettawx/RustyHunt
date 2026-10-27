//! rustyhunt-native.dll: injected into the game process by the main program when the damage hook is on.
//!
//! Once loaded it opens a port on 127.0.0.1 and waits for the main program (protocol in `rh_core::native`).
//! When the main program sends the damage function's address it installs the hook: every time the local
//! player hits a large monster it records the damage and whether it crit. It only reads the arguments and
//! never changes the game's results.

#![cfg(windows)]

use std::ffi::c_void;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use retour::RawDetour;
use rh_core::native::{PORT, Stats};

/// The game's damage function: monster, damage, position, tenderized, crit; the remaining arguments are unused.
type DealDamage = unsafe extern "C" fn(*mut u8, i32, *mut c_void, i32, i32, i32, i32, i8, i32);

/// Offset of the monster ID in the monster struct
const MONSTER_ID_OFFSET: usize = 0x12280;

/// One bit per monster ID telling whether it is a large monster (including the training area pillar, 59).
/// Data from upstream HunterPie's WorldUtils.cpp.
const LARGE_LO: u64 = 0xE808_00FF_FFFF_FE93;
const LARGE_HI: u64 = 0x0000_003F_FF83_FFFF;

fn is_large(id: i32) -> bool {
    match id {
        0..=63 => (LARGE_LO >> id) & 1 == 1,
        64..=127 => (LARGE_HI >> (id - 64)) & 1 == 1,
        _ => false,
    }
}

static STATS: Mutex<Stats> = Mutex::new(Stats {
    total: 0.0,
    critical: 0.0,
    hits: 0,
    critical_hits: 0,
});
static TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);

struct Hook {
    detour: RawDetour,
    target: usize,
}

// RawDetour holds raw pointers; only used from the server thread, behind a lock
unsafe impl Send for Hook {}

static HOOK: Mutex<Option<Hook>> = Mutex::new(None);

unsafe extern "C" fn deal_damage(target: *mut u8, damage: i32, position: *mut c_void, tenderized: i32, crit: i32, unk0: i32, unk1: i32, unk2: i8, attack: i32) {
    let t = TRAMPOLINE.load(Ordering::Acquire);
    if t != 0 {
        let original: DealDamage = unsafe { std::mem::transmute::<usize, DealDamage>(t) };
        unsafe { original(target, damage, position, tenderized, crit, unk0, unk1, unk2, attack) };
    }
    if target.is_null() || damage <= 0 {
        return;
    }
    let id = unsafe { std::ptr::read_unaligned(target.add(MONSTER_ID_OFFSET) as *const i32) };
    if !is_large(id) {
        return;
    }
    // Never wait on the game thread: if the lock is taken, drop this hit
    if let Ok(mut s) = STATS.try_lock() {
        s.total += damage as f64;
        s.hits += 1;
        if crit != 0 {
            s.critical += damage as f64;
            s.critical_hits += 1;
        }
    }
}

fn hook(address: usize) -> Result<(), String> {
    let mut h = HOOK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = h.as_ref() {
        if old.target == address {
            unsafe { old.detour.enable() }.map_err(|e| e.to_string())?;
            return Ok(());
        }
        unsafe { old.detour.disable() }.map_err(|e| e.to_string())?;
        *h = None;
    }
    let detour = unsafe { RawDetour::new(address as *const (), deal_damage as *const ()) }.map_err(|e| e.to_string())?;
    TRAMPOLINE.store(detour.trampoline() as *const () as usize, Ordering::Release);
    unsafe { detour.enable() }.map_err(|e| e.to_string())?;
    *h = Some(Hook { detour, target: address });
    Ok(())
}

fn unhook() {
    let h = HOOK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hk) = h.as_ref() {
        let _ = unsafe { hk.detour.disable() };
    }
}

fn answer(line: &str) -> String {
    let mut parts = line.split_whitespace();
    match parts.next().unwrap_or("") {
        "HELLO" => format!("RUSTYHUNT-NATIVE {}", env!("CARGO_PKG_VERSION")),
        "HOOK" => match parts.next().and_then(|a| usize::from_str_radix(a.trim_start_matches("0x"), 16).ok()) {
            Some(a) if a != 0 => match hook(a) {
                Ok(()) => "OK".into(),
                Err(e) => format!("ERR {e}"),
            },
            _ => "ERR bad address".into(),
        },
        "UNHOOK" => {
            unhook();
            "OK".into()
        }
        "STATS" => STATS.lock().unwrap_or_else(|e| e.into_inner()).format(),
        "CLEAR" => {
            *STATS.lock().unwrap_or_else(|e| e.into_inner()) = Stats::default();
            "OK".into()
        }
        _ => "ERR unknown request".into(),
    }
}

fn serve(stream: TcpStream) {
    let Ok(mut out) = stream.try_clone() else { return };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let reply = answer(line.trim());
        if out.write_all(reply.as_bytes()).and_then(|_| out.write_all(b"\n")).is_err() {
            break;
        }
    }
}

fn server() {
    let Ok(listener) = TcpListener::bind(("127.0.0.1", PORT)) else { return };
    for stream in listener.incoming().flatten() {
        // Serve one connection at a time: when the main program disconnects, wait for the next one
        serve(stream);
    }
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(_module: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        // Do nothing else in DllMain: start a thread and return
        let _ = std::thread::Builder::new().name("rustyhunt-native".into()).spawn(server);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    /// Stand-in with the same signature as the game's damage function
    #[inline(never)]
    unsafe extern "C" fn fake_damage(target: *mut u8, damage: i32, _: *mut c_void, _: i32, _: i32, _: i32, _: i32, _: i8, _: i32) {
        CALLS.fetch_add(1, Ordering::SeqCst);
        std::hint::black_box((target, damage));
    }

    #[test]
    fn hook_counts_damage_and_calls_original() {
        let mut monster = vec![0u8; MONSTER_ID_OFFSET + 8];
        monster[MONSTER_ID_OFFSET..MONSTER_ID_OFFSET + 4].copy_from_slice(&7i32.to_le_bytes());
        let f: DealDamage = std::hint::black_box(fake_damage);
        hook(f as usize).unwrap();
        answer("CLEAR");
        unsafe {
            f(monster.as_mut_ptr(), 120, std::ptr::null_mut(), 0, 1, 0, 0, 0, 0);
            f(monster.as_mut_ptr(), 80, std::ptr::null_mut(), 0, 0, 0, 0, 0, 0);
        }
        assert_eq!(CALLS.load(Ordering::SeqCst), 2, "the original function still runs");
        let s = Stats::parse(&answer("STATS")).unwrap();
        assert_eq!(
            s,
            Stats {
                total: 200.0,
                critical: 120.0,
                hits: 2,
                critical_hits: 1
            }
        );
        unhook();
        unsafe { f(monster.as_mut_ptr(), 50, std::ptr::null_mut(), 0, 0, 0, 0, 0, 0) };
        assert_eq!(Stats::parse(&answer("STATS")).unwrap().hits, 2, "no counting after unhooking");
        assert_eq!(CALLS.load(Ordering::SeqCst), 3);
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn bitsets_match_upstream() {
        // Upstream writes this in binary, in groups of ten by monster ID
        assert_eq!(LARGE_LO, 0b1110_1000000010_0000000000_1111111111_1111111111_1111111111_1010010011);
        assert_eq!(LARGE_HI, 0b00000000_0000000000_0000000011_1111111111_1110000011_1111111111_111111);
    }

    #[test]
    fn large_monsters() {
        // IDs 0, 1 and 7 are large monsters, 2 isn't; anything past the bit table isn't
        assert!(is_large(0));
        assert!(is_large(7));
        assert!(!is_large(2));
        assert!(!is_large(200));
    }

    #[test]
    fn protocol_answers() {
        assert!(answer("HELLO").starts_with("RUSTYHUNT-NATIVE "));
        assert_eq!(answer("CLEAR"), "OK");
        assert_eq!(answer("STATS"), Stats::default().format());
        assert!(answer("HOOK 0").starts_with("ERR"));
        assert!(answer("NOPE").starts_with("ERR"));
    }
}
