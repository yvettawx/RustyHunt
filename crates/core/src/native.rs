//! 主程序和注入游戏的 rustyhunt-native.dll 之间的协议。
//!
//! DLL 在 127.0.0.1 上开一个 TCP 端口，一问一答，每条消息一行：
//!
//! | 请求 | 回答 |
//! |---|---|
//! | `HELLO` | `RUSTYHUNT-NATIVE <版本>` |
//! | `HOOK <十六进制地址>` | `OK` 或 `ERR <原因>` |
//! | `UNHOOK` | `OK` |
//! | `STATS` | `STATS <总伤害> <会心伤害> <命中次数> <会心次数>` |
//! | `CLEAR` | `OK` |

/// 和上游 HunterPie 的端口不同，两个程序同时开着也不冲突。
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
