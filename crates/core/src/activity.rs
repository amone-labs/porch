//! When each agent last reported anything, for the settings screen.
//!
//! Day files reach a megabyte, and this runs every few seconds, so each file
//! is read from its end in chunks and reading stops once every agent is found.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

const AGENTS: [&str; 2] = ["claude", "codex"];
const CHUNK: u64 = 64 * 1024;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LastSeen {
    pub agent: String,
    pub t: u64,
}

/// Only the two fields this needs, so newer event fields never break it.
#[derive(Deserialize)]
struct Stamp {
    t: u64,
    agent: String,
}

/// Latest hook event time per agent, reading day files newest first and
/// each file from its end, looking back at most `days` days.
pub fn last_seen(days: u64, now: u64) -> Vec<LastSeen> {
    last_seen_in(&crate::paths::events_dir(), days, now)
}

pub fn last_seen_in(dir: &Path, days: u64, now: u64) -> Vec<LastSeen> {
    let mut found: Vec<LastSeen> = Vec::new();
    for back in 0..days {
        if found.len() == AGENTS.len() {
            break;
        }
        let date = crate::time::utc_date(now.saturating_sub(back * DAY_MS));
        let _ = scan_from_end(&dir.join(format!("{date}.jsonl")), &mut found);
    }
    found.sort_by_key(|s| AGENTS.iter().position(|a| *a == s.agent));
    found
}

/// Walks one file backwards, recording the first (= last written) line per
/// agent not already found.
fn scan_from_end(path: &Path, found: &mut Vec<LastSeen>) -> std::io::Result<()> {
    let mut f = File::open(path)?;
    let mut end = f.metadata()?.len();
    // The start of a line cut by the previous (later) chunk's boundary.
    let mut carry: Vec<u8> = Vec::new();
    while end > 0 && found.len() < AGENTS.len() {
        let start = end.saturating_sub(CHUNK);
        let mut buf = vec![0u8; (end - start) as usize];
        f.seek(SeekFrom::Start(start))?;
        f.read_exact(&mut buf)?;
        buf.extend_from_slice(&carry);
        // Unless this chunk starts the file, its first line may be cut.
        let cut = if start > 0 { buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len()) } else { 0 };
        let (head, rest) = buf.split_at(cut);
        let body = if start > 0 && !rest.is_empty() { &rest[1..] } else { rest };
        for line in body.rsplit(|&b| b == b'\n') {
            note(line, found);
            if found.len() == AGENTS.len() {
                return Ok(());
            }
        }
        carry = head.to_vec();
        end = start;
    }
    Ok(())
}

fn note(line: &[u8], found: &mut Vec<LastSeen>) {
    if line.iter().all(u8::is_ascii_whitespace) {
        return;
    }
    let Ok(s) = serde_json::from_slice::<Stamp>(line) else { return };
    if AGENTS.contains(&s.agent.as_str()) && !found.iter().any(|f| f.agent == s.agent) {
        found.push(LastSeen { agent: s.agent, t: s.t });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const DAY: u64 = 86_400_000;
    // 2026-09-30T10:00:00Z
    const NOW: u64 = 1_790_762_400_000;

    fn line(agent: &str, t: u64) -> String {
        format!("{{\"v\":1,\"t\":{t},\"agent\":\"{agent}\",\"event\":\"Stop\"}}\n")
    }

    fn write(dir: &Path, ms: u64, body: &str) {
        fs::write(dir.join(format!("{}.jsonl", crate::time::utc_date(ms))), body).unwrap();
    }

    fn found(v: &[LastSeen]) -> Vec<(&str, u64)> {
        v.iter().map(|s| (s.agent.as_str(), s.t)).collect()
    }

    #[test]
    fn last_line_per_agent_in_one_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), NOW, &[line("claude", 1), line("codex", 2), line("claude", 3)].concat());
        assert_eq!(found(&last_seen_in(dir.path(), 7, NOW)), [("claude", 3), ("codex", 2)]);
    }

    #[test]
    fn falls_back_to_earlier_days_within_limit() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), NOW, &line("claude", NOW - 1));
        write(dir.path(), NOW - 3 * DAY, &line("codex", NOW - 3 * DAY));
        assert_eq!(found(&last_seen_in(dir.path(), 7, NOW)), [("claude", NOW - 1), ("codex", NOW - 3 * DAY)]);
        assert_eq!(found(&last_seen_in(dir.path(), 3, NOW)), [("claude", NOW - 1)]);
    }

    #[test]
    fn newer_day_wins_over_older_day() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), NOW, &line("claude", 20));
        write(dir.path(), NOW - DAY, &line("claude", 10));
        assert_eq!(found(&last_seen_in(dir.path(), 7, NOW)), [("claude", 20)]);
    }

    #[test]
    fn line_straddling_chunk_boundary_is_read_whole() {
        let dir = tempfile::tempdir().unwrap();
        let codex = line("codex", 5);
        let base = "{\"v\":1,\"t\":9,\"agent\":\"claude\",\"event\":\"Stop\",\"pad\":\"\"}\n".len();
        let tail_len = CHUNK as usize - 10;
        let tail = format!("{{\"v\":1,\"t\":9,\"agent\":\"claude\",\"event\":\"Stop\",\"pad\":\"{}\"}}\n", "x".repeat(tail_len - base));
        assert_eq!(tail.len(), tail_len);
        // The file's last CHUNK bytes start 10 bytes before the codex line ends.
        write(dir.path(), NOW, &format!("{codex}{tail}"));
        assert_eq!(found(&last_seen_in(dir.path(), 1, NOW)), [("claude", 9), ("codex", 5)]);
    }

    #[test]
    fn last_line_without_newline_straddling_chunk_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let codex = line("codex", 5);
        // A final claude line, no trailing newline, longer than one chunk, so
        // the file's last CHUNK bytes start inside it.
        let base = "{\"v\":1,\"t\":9,\"agent\":\"claude\",\"event\":\"Stop\",\"pad\":\"\"}".len();
        let tail_len = CHUNK as usize + 10;
        let tail = format!("{{\"v\":1,\"t\":9,\"agent\":\"claude\",\"event\":\"Stop\",\"pad\":\"{}\"}}", "x".repeat(tail_len - base));
        assert_eq!(tail.len(), tail_len);
        assert!(!tail.ends_with('\n'));
        write(dir.path(), NOW, &format!("{codex}{tail}"));
        assert_eq!(found(&last_seen_in(dir.path(), 1, NOW)), [("claude", 9), ("codex", 5)]);
    }

    #[test]
    fn straddling_line_several_chunks_back() {
        let dir = tempfile::tempdir().unwrap();
        let codex = line("codex", 5);
        let head = "{\"v\":1,\"t\":9,\"agent\":\"claude\",\"event\":\"Stop\",\"pad\":\"";
        let tail = "\"}\n";
        let pad = |len: usize| format!("{head}{}{tail}", "x".repeat(len - head.len() - tail.len()));
        // Claude lines fill 3 * CHUNK - 10 bytes, so the boundary three chunks
        // from the end falls 10 bytes before the codex line ends: the codex
        // line is read in two pieces, on the third pass and the fourth.
        let last = line("claude", 11);
        let mut body = codex.clone();
        for len in [CHUNK as usize, CHUNK as usize, CHUNK as usize - 10 - last.len()] {
            body.push_str(&pad(len));
        }
        body.push_str(&last);
        assert_eq!(body.len() as u64, 3 * CHUNK - 10 + codex.len() as u64);
        write(dir.path(), NOW, &body);
        assert_eq!(found(&last_seen_in(dir.path(), 1, NOW)), [("claude", 11), ("codex", 5)]);
    }

    #[test]
    fn many_chunks_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut body = line("codex", 1);
        for t in 0..3000 {
            body.push_str(&line("claude", 100 + t));
        }
        assert!(body.len() as u64 > 2 * CHUNK);
        write(dir.path(), NOW, &body);
        assert_eq!(found(&last_seen_in(dir.path(), 1, NOW)), [("claude", 3099), ("codex", 1)]);
    }

    #[test]
    fn skips_broken_lines_and_unknown_agents() {
        let dir = tempfile::tempdir().unwrap();
        let body = [line("claude", 1), "not json\n".into(), line("gemini", 9), "{\"t\":\"x\"}\n".into()].concat();
        write(dir.path(), NOW, &body);
        assert_eq!(found(&last_seen_in(dir.path(), 1, NOW)), [("claude", 1)]);
    }

    #[test]
    fn trailing_blank_lines_and_missing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let body = format!("{}\n\n{}", line("claude", 1), line("codex", 2).trim_end());
        write(dir.path(), NOW, &body);
        assert_eq!(found(&last_seen_in(dir.path(), 1, NOW)), [("claude", 1), ("codex", 2)]);
    }

    #[test]
    fn agent_names_match_install() {
        assert_eq!(AGENTS, crate::install::Agent::ALL.map(|a| a.name()));
    }

    #[test]
    fn missing_or_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(last_seen_in(dir.path(), 7, NOW).is_empty());
        write(dir.path(), NOW, "");
        assert!(last_seen_in(dir.path(), 7, NOW).is_empty());
    }
}
