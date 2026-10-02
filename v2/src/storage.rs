//! Thin wrapper around `localStorage`. Every call tolerates storage being
//! unavailable (private mode, blocked cookies): reads return `None`, writes
//! are dropped, and the game keeps working.

use web_sys::Storage;

const PREFIX: &str = "ms2.";

fn store() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn get(key: &str) -> Option<String> {
    store()?.get_item(&format!("{PREFIX}{key}")).ok().flatten()
}

pub fn set(key: &str, value: &str) {
    if let Some(s) = store() {
        let _ = s.set_item(&format!("{PREFIX}{key}"), value);
    }
}

pub fn remove(key: &str) {
    if let Some(s) = store() {
        let _ = s.remove_item(&format!("{PREFIX}{key}"));
    }
}

/// Per-difficulty statistics, stored as `played,won,streak,best_streak,best_ms`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub played: u32,
    pub won: u32,
    pub streak: u32,
    pub best_streak: u32,
    pub best_ms: Option<u32>,
}

impl Stats {
    pub fn parse(raw: &str) -> Stats {
        let mut parts = raw.split(',').map(|p| p.trim().parse::<u32>().ok());
        let mut next = || parts.next().flatten();
        Stats {
            played: next().unwrap_or(0),
            won: next().unwrap_or(0),
            streak: next().unwrap_or(0),
            best_streak: next().unwrap_or(0),
            best_ms: next(),
        }
    }

    pub fn serialize(&self) -> String {
        let best = self.best_ms.map(|b| b.to_string()).unwrap_or_default();
        format!(
            "{},{},{},{},{}",
            self.played, self.won, self.streak, self.best_streak, best
        )
    }

    pub fn load(level: &str) -> Stats {
        get(&format!("stats.{level}"))
            .map(|s| Stats::parse(&s))
            .unwrap_or_default()
    }

    pub fn save(&self, level: &str) {
        set(&format!("stats.{level}"), &self.serialize());
    }

    /// Records a finished game. Returns true when it set a new best time.
    pub fn record(&mut self, won: bool, ms: u32) -> bool {
        self.played += 1;
        if !won {
            self.streak = 0;
            return false;
        }
        self.won += 1;
        self.streak += 1;
        self.best_streak = self.best_streak.max(self.streak);
        let is_best = self.best_ms.is_none_or(|b| ms < b);
        if is_best {
            self.best_ms = Some(ms);
        }
        is_best
    }

    pub fn win_rate(&self) -> u32 {
        (self.won * 100 + self.played / 2)
            .checked_div(self.played)
            .unwrap_or(0)
    }
}

/// `12.3s`, or `1:02.5` past a minute.
pub fn format_ms(ms: u32) -> String {
    let tenths = ms / 100;
    let (secs, t) = (tenths / 10, tenths % 10);
    if secs >= 60 {
        format!("{}:{:02}.{}", secs / 60, secs % 60, t)
    } else {
        format!("{secs}.{t}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_round_trip_and_record() {
        let mut s = Stats::default();
        assert!(s.record(true, 5000));
        assert!(!s.record(true, 6000));
        assert!(s.record(true, 4000));
        assert!(!s.record(false, 100));
        assert_eq!(
            s,
            Stats {
                played: 4,
                won: 3,
                streak: 0,
                best_streak: 3,
                best_ms: Some(4000)
            }
        );
        assert_eq!(Stats::parse(&s.serialize()), s);
        assert_eq!(s.win_rate(), 75);
        assert_eq!(Stats::parse("garbage"), Stats::default());
    }

    #[test]
    fn formats_times() {
        assert_eq!(format_ms(12_345), "12.3s");
        assert_eq!(format_ms(62_500), "1:02.5");
        assert_eq!(format_ms(0), "0.0s");
    }
}
