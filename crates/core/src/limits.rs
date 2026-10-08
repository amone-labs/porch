//! Usage limits per agent account: how much of the 5-hour and weekly windows
//! is used, and when each starts over. Claude's come from its status line
//! (see `statusline`), Codex's from its own session files.

use crate::statusline::{self, Window};
use crate::paths;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountLimits {
    /// "claude" | "codex"
    pub agent: String,
    /// "Claude Code", "Codex", "Codex (Orca 2)"
    pub label: String,
    pub five_hour: Option<Window>,
    pub weekly: Option<Window>,
    /// A spend limit set by an organization gateway (Claude only).
    pub spend: Option<Window>,
    /// When the agent last reported them (unix ms).
    pub at: u64,
}

/// A window whose reset time has passed starts over at zero.
fn current(w: Option<Window>, now: u64) -> Option<Window> {
    w.map(|w| match w.resets_at {
        Some(r) if r <= now => Window { used_percent: 0.0, resets_at: None },
        _ => w,
    })
}

pub fn all(now: u64) -> Vec<AccountLimits> {
    let mut out = Vec::new();
    if let Some(l) = statusline::load_live().limits {
        out.push(AccountLimits {
            agent: "claude".into(),
            label: "Claude Code".into(),
            five_hour: current(l.five_hour, now),
            weekly: current(l.seven_day, now),
            spend: current(l.spend, now),
            at: l.at,
        });
    }
    let homes = paths::codex_homes();
    let orca: Vec<&PathBuf> = homes.iter().filter(|h| h.to_string_lossy().contains("/orca/")).collect();
    for home in &homes {
        let Some((five, week, at)) = codex_latest(home) else { continue };
        let label = match orca.iter().position(|h| *h == home) {
            Some(_) if orca.len() == 1 => "Codex (Orca)".to_owned(),
            Some(i) => format!("Codex (Orca {})", i + 1),
            None => "Codex".to_owned(),
        };
        out.push(AccountLimits { agent: "codex".into(), label, five_hour: current(five, now), weekly: current(week, now), spend: None, at });
    }
    out
}

fn codex_window(v: &Value) -> Option<Window> {
    Some(Window { used_percent: v["used_percent"].as_f64()?, resets_at: v["resets_at"].as_u64().map(|s| s * 1000) })
}

/// The newest rate limits Codex wrote in this home: the last `token_count`
/// in its most recent session files.
fn codex_latest(home: &Path) -> Option<(Option<Window>, Option<Window>, u64)> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    // Session files sit in sessions/YYYY/MM/DD; the last few days are enough.
    let now = crate::time::now_ms();
    for back in 0..7u64 {
        let date = crate::time::utc_date(now.saturating_sub(back * 86_400_000));
        let dir = home.join("sessions").join(date.replace('-', "/"));
        for f in fs::read_dir(dir).into_iter().flatten().flatten() {
            if let Ok(m) = f.metadata().and_then(|m| m.modified()) {
                files.push((m, f.path()));
            }
        }
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, path) in files.into_iter().take(5) {
        if let Some(found) = last_limits(&path) {
            return Some(found);
        }
    }
    None
}

fn last_limits(path: &Path) -> Option<(Option<Window>, Option<Window>, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(512 * 1024)));
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    text.lines().rev().find_map(|line| {
        if !line.contains("rate_limits") {
            return None;
        }
        let r: Value = serde_json::from_str(line).ok()?;
        let rl = &r["payload"]["rate_limits"];
        if r["payload"]["type"] != "token_count" || !rl.is_object() {
            return None;
        }
        let at = r["timestamp"].as_str().and_then(crate::time::parse_rfc3339_ms).unwrap_or(0);
        Some((codex_window(&rl["primary"]), codex_window(&rl["secondary"]), at))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_limits_from_the_last_token_count() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("rollout.jsonl");
        let line = |pct: f64| {
            serde_json::json!({"timestamp": "2026-09-30T01:00:00Z", "type": "event_msg", "payload": {"type": "token_count",
                "rate_limits": {"primary": {"used_percent": pct, "window_minutes": 300, "resets_at": 1790745042u64},
                                "secondary": {"used_percent": 17.0, "window_minutes": 10080, "resets_at": 1791076724u64}}}})
            .to_string()
        };
        fs::write(&f, [line(40.0), line(81.0)].join("\n")).unwrap();
        let (five, week, _) = last_limits(&f).unwrap();
        assert_eq!(five.unwrap().used_percent, 81.0);
        assert_eq!(week.unwrap().resets_at, Some(1_791_076_724_000));
        assert_eq!(current(five, 1_790_745_042_000).unwrap().used_percent, 0.0);
    }
}
