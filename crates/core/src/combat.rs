//! Fight time for DPS. Quest time also counts the search before the fight, chasing a monster that
//! left the area, carving after it and time away from the keyboard, which waters DPS down. Fight time
//! starts with the first damage anyone deals and stops when nobody has dealt any for `IDLE` seconds.

use serde::{Deserialize, Serialize};

/// Seconds without damage after which time stops counting until the next hit.
pub const IDLE: f32 = 15.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CombatClock {
    /// Party damage at the last update
    total: f32,
    /// Quest time at the last update and at the last hit
    prev: Option<f32>,
    last_hit: Option<f32>,
    /// Fight time so far
    secs: f32,
}

impl CombatClock {
    /// Feeds the quest time and the party's total damage, once per frame.
    pub fn update(&mut self, elapsed: f32, total: f32) {
        // A new quest (or a reset damage meter) starts over
        if self.prev.is_some_and(|p| elapsed < p) || total < self.total - 0.5 {
            *self = CombatClock::default();
        }
        if let (Some(p), Some(hit)) = (self.prev, self.last_hit) {
            let until = (hit + IDLE).min(elapsed);
            self.secs += (until - p).max(0.0);
        }
        if total > self.total + 0.01 {
            self.last_hit = Some(elapsed);
        }
        self.total = total;
        self.prev = Some(elapsed);
    }

    pub fn seconds(&self) -> f32 {
        self.secs
    }

    /// DPS for `damage` over the fight time; 0 before the first second of fighting.
    pub fn dps(&self, damage: f32) -> f32 {
        if self.secs < 1.0 { 0.0 } else { damage / self.secs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_before_the_fight_and_long_pauses_dont_count() {
        let mut c = CombatClock::default();
        // 60 s of searching
        for t in 0..=60 {
            c.update(t as f32, 0.0);
        }
        assert_eq!(c.seconds(), 0.0);
        // A hit every 5 s from 65 s to 100 s
        for t in 61..=100 {
            let dmg = ((t - 60) / 5 * 100) as f32;
            c.update(t as f32, dmg);
        }
        let fight = c.seconds();
        assert!((fight - 35.0).abs() < 1.5, "{fight}");
        // Away for two minutes: only IDLE more seconds count
        for t in 101..=220 {
            c.update(t as f32, 800.0);
        }
        assert!((c.seconds() - (fight + IDLE)).abs() < 1.5, "{}", c.seconds());
        assert!((c.dps(800.0) - 800.0 / c.seconds()).abs() < 0.01);
    }

    #[test]
    fn a_new_quest_starts_over() {
        let mut c = CombatClock::default();
        for t in 0..=30 {
            c.update(t as f32, t as f32 * 10.0);
        }
        assert!(c.seconds() > 20.0);
        c.update(0.0, 0.0);
        assert_eq!(c.seconds(), 0.0);
    }
}
