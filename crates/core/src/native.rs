//! Protocol between the main program and rustyhunt-native.dll injected into the game.
//!
//! The DLL opens a TCP port on 127.0.0.1; request and reply, one message per line:
//!
//! | Request | Reply |
//! |---|---|
//! | `HELLO` | `RUSTYHUNT-NATIVE <version>` |
//! | `HOOK <hex address>` | `OK` or `ERR <reason>` |
//! | `UNHOOK` | `OK` |
//! | `STATS` | `STATS <total damage> <crit damage> <hits> <crits>` |
//! | `CLEAR` | `OK` |

/// Differs from upstream HunterPie's port, so both programs can run at the same time.
pub const PORT: u16 = 22013;

pub const DLL_NAME: &str = "rustyhunt-native.dll";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    pub total: f64,
    pub critical: f64,
    pub hits: u32,
    pub critical_hits: u32,
}

impl Stats {
    pub fn format(&self) -> String {
        format!("STATS {} {} {} {}", self.total, self.critical, self.hits, self.critical_hits)
    }

    pub fn parse(line: &str) -> Option<Stats> {
        let mut it = line.trim().strip_prefix("STATS ")?.split_whitespace();
        Some(Stats {
            total: it.next()?.parse().ok()?,
            critical: it.next()?.parse().ok()?,
            hits: it.next()?.parse().ok()?,
            critical_hits: it.next()?.parse().ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_round_trip() {
        let s = Stats {
            total: 1234.0,
            critical: 400.5,
            hits: 30,
            critical_hits: 9,
        };
        assert_eq!(Stats::parse(&s.format()), Some(s));
        assert_eq!(Stats::parse("STATS 1 2"), None);
    }
}
