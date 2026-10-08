//! Build the current board: replay event files, fill in from Claude Code's
//! session registry, check liveness, attach per-worktree git counts. This is
//! the I/O shell around the pure `state` module.

use crate::event::Event;
use crate::paths;
use crate::state::{self, Applied, RegistryStatus, Session, State};
use crate::time;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Days of event files replayed on each build.
pub const REPLAY_DAYS: u64 = 3;
/// A Codex session with no event for this long is hidden: Codex has no
/// registry, so liveness is unknown.
pub const CODEX_QUIET_HIDE_MS: u64 = 12 * 60 * 60 * 1000;
/// Running with no event for this long gets the "last heard" marker.
pub const STALE_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub session: Session,
    pub title: String,
    /// None when liveness cannot be checked (Codex).
    pub alive: Option<bool>,
    pub stale: bool,
    /// The session's terminal can be brought to the front (`focus::target`).
    pub focusable: bool,
    /// The last thing the person asked, one line.
    pub last_prompt: Option<String>,
    /// The model answering now, full id ("claude-opus-5-5").
    pub model: Option<String>,
    /// How full the context window is, 0..100, when known.
    pub context_percent: Option<f64>,
    /// Tokens in the context window, when the percentage is not known.
    pub context_tokens: Option<u64>,
    /// What a permission prompt wants to do: "Bash · pnpm test".
    pub ask: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Worktree {
    pub root: String,
    pub branch: Option<String>,
    pub dirty: Option<usize>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Default, Serialize)]
pub struct Counters {
    pub bad_lines: usize,
    pub orphan_events: usize,
    pub unknown_events: BTreeMap<String, usize>,
    pub from_registry_only: usize,
}

#[derive(Debug, Serialize)]
pub struct Board {
    pub now: u64,
    pub worktrees: Vec<Worktree>,
    pub counters: Counters,
}

// ---------- events ----------

pub fn read_events(days: u64, now: u64, counters: &mut Counters) -> Vec<Event> {
    let mut out = Vec::new();
    for back in (0..days).rev() {
        let date = time::utc_date(now.saturating_sub(back * 86_400_000));
        let Ok(text) = fs::read_to_string(paths::events_file(&date)) else { continue };
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<Event>(line) {
                Ok(e) => out.push(e),
                Err(_) => counters.bad_lines += 1,
            }
        }
    }
    out.sort_by_key(|e| e.t);
    out
}

// ---------- Claude Code session registry ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub pid: u32,
    pub session_id: String,
    pub cwd: Option<String>,
    pub proc_start: Option<String>,
    pub status: Option<String>,
    pub waiting_for: Option<String>,
    pub log_path: Option<String>,
    pub name: Option<String>,
    pub status_updated_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl RegistryEntry {
    pub fn status(&self) -> Option<RegistryStatus> {
        Some(match (self.status.as_deref()?, self.waiting_for.as_deref()) {
            ("busy", _) => RegistryStatus::Busy,
            ("waiting", Some("input needed")) => RegistryStatus::WaitingInput,
            ("waiting", _) => RegistryStatus::WaitingPermission,
            ("idle", _) => RegistryStatus::Idle,
            _ => return None,
        })
    }
}

pub fn read_registry(claude_dir: &Path) -> Vec<RegistryEntry> {
    let Ok(dir) = fs::read_dir(claude_dir.join("sessions")) else { return vec![] };
    dir.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
        .collect()
}

fn squash_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// pid -> process start time as `ps -o lstart` prints it, for live pids only.
fn live_processes(pids: &[u32]) -> HashMap<u32, String> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let list = pids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    // Claude Code records `procStart` in UTC; `ps` prints local time unless told otherwise.
    let Ok(out) = Command::new("ps").env("TZ", "UTC").args(["-o", "pid=,lstart=", "-p", &list]).output() else {
        return HashMap::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let (pid, start) = l.split_once(char::is_whitespace)?;
            Some((pid.parse().ok()?, squash_ws(start)))
        })
        .collect()
}

/// A registry entry is alive when its pid runs and, if the entry records a
/// start time, the process started then (pids get reused).
fn is_alive(r: &RegistryEntry, live: &HashMap<u32, String>) -> bool {
    match (live.get(&r.pid), &r.proc_start) {
        (Some(started), Some(recorded)) => *started == squash_ws(recorded),
        (Some(_), None) => true,
        (None, _) => false,
    }
}

// ---------- git ----------

fn git(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_owned())
}

// ---------- build ----------

pub struct Sources {
    pub claude_dirs: Vec<PathBuf>,
    pub with_git: bool,
    /// Read each live session's transcript tail for its last prompt and what
    /// a permission prompt asks. Off where only counts are needed.
    pub with_detail: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Sources { claude_dirs: vec![paths::default_claude_dir()], with_git: true, with_detail: true }
    }
}

/// Whether a session is on the board (docs/design/states.md, "When a session has ended"):
/// ended sessions leave at once; a Codex session, whose liveness is unknown,
/// leaves after `CODEX_QUIET_HIDE_MS` without an event.
fn visible(state: State, alive: Option<bool>, quiet_ms: u64) -> bool {
    state != State::Ended && (alive == Some(true) || quiet_ms < CODEX_QUIET_HIDE_MS)
}

pub fn build(now: u64, src: &Sources) -> Board {
    let status_line = if src.with_detail { crate::statusline::load_live() } else { Default::default() };
    let mut counters = Counters::default();
    let mut sessions = BTreeMap::new();
    for e in read_events(REPLAY_DAYS, now, &mut counters) {
        match state::apply(&mut sessions, &e) {
            Applied::Orphan => counters.orphan_events += 1,
            Applied::Unknown => *counters.unknown_events.entry(format!("{}:{}", e.agent, e.event)).or_default() += 1,
            _ => {}
        }
    }

    let registry: Vec<RegistryEntry> = src.claude_dirs.iter().flat_map(|d| read_registry(d)).collect();
    let live = live_processes(&registry.iter().map(|r| r.pid).collect::<Vec<_>>());
    let mut names = HashMap::new();
    let mut alive_claude = std::collections::HashSet::new();
    for r in &registry {
        if !is_alive(r, &live) {
            continue;
        }
        alive_claude.insert(r.session_id.clone());
        if let Some(n) = &r.name {
            names.insert(r.session_id.clone(), n.clone());
        }
        let key = ("claude".to_owned(), r.session_id.clone());
        let status_at = r.status_updated_at.or(r.updated_at).unwrap_or(0);
        let s = sessions.entry(key).or_insert_with(|| {
            counters.from_registry_only += 1;
            let e = Event {
                v: crate::event::FORMAT_VERSION,
                t: status_at,
                agent: "claude".into(),
                event: "SessionStart".into(),
                session: Some(r.session_id.clone()),
                cwd: r.cwd.clone(),
                log: r.log_path.clone(),
                ..Default::default()
            };
            let mut one = BTreeMap::new();
            state::apply(&mut one, &e);
            one.into_values().next().expect("apply inserts the session")
        });
        if s.cwd.is_none() {
            s.cwd = r.cwd.clone();
        }
        if s.log.is_none() {
            s.log = r.log_path.clone();
        }
        if let Some(st) = r.status() {
            state::merge_registry(s, st, status_at);
        }
    }

    let mut entries = Vec::new();
    for (_, mut s) in sessions {
        let alive = match s.agent.as_str() {
            "claude" => Some(alive_claude.contains(&s.session)),
            _ => None,
        };
        if alive == Some(false) && s.state != State::Ended {
            s.state = State::Ended;
            s.pending = None;
            s.subagents.clear();
        }
        let quiet = now.saturating_sub(s.last_event_at);
        if !visible(s.state, alive, quiet) {
            continue;
        }
        let stale = s.state == State::Running && quiet > STALE_MS;
        let title = names.get(&s.session).cloned().unwrap_or_else(|| {
            let base = s.cwd.as_deref().and_then(|c| c.rsplit('/').next()).unwrap_or("?");
            format!("{base} {}", &s.session[..s.session.len().min(8)])
        });
        if s.log.is_none() && s.agent == "claude" && src.with_detail {
            s.log = find_transcript(&src.claude_dirs, &s.session).map(|p| p.to_string_lossy().into_owned());
        }
        let p = match (&s.log, src.with_detail && s.state != State::Ended) {
            (Some(log), true) => peek(Path::new(log), s.pending.as_ref().and_then(|p| p.tool_use.as_deref()), s.state == State::Permission),
            _ => Peek::default(),
        };
        // Claude Code's own status line numbers are exact; the transcript is the fallback.
        let from_line = status_line.sessions.get(&s.session).filter(|l| now.saturating_sub(l.at) < 6 * 3_600_000);
        let model = from_line.and_then(|l| l.model.clone()).or(p.model);
        let context_percent = from_line.and_then(|l| l.context_percent).or_else(|| {
            let (used, window) = (p.context_tokens?, p.context_window?);
            (window > 0).then(|| used as f64 * 100.0 / window as f64)
        });
        let context_tokens = if context_percent.is_some() { None } else { p.context_tokens };
        let focusable = crate::focus::target(s.term_program.as_deref(), s.term_pane.as_deref()).is_some();
        entries.push(Entry { session: s, title, alive, stale, focusable, last_prompt: p.prompt, ask: p.ask, model, context_percent, context_tokens });
    }

    // group by worktree root
    let mut roots: HashMap<String, String> = HashMap::new();
    let mut groups: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for e in entries {
        let cwd = e.session.cwd.clone().unwrap_or_default();
        let root = if src.with_git && !cwd.is_empty() {
            roots
                .entry(cwd.clone())
                .or_insert_with(|| git(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or(cwd.clone()))
                .clone()
        } else {
            cwd
        };
        groups.entry(root).or_default().push(e);
    }
    let mut worktrees: Vec<Worktree> = groups
        .into_iter()
        .map(|(root, mut entries)| {
            entries.sort_by_key(|e| (e.session.priority(), std::cmp::Reverse(e.session.last_event_at)));
            let (branch, dirty) = if src.with_git && Path::new(&root).is_dir() {
                (
                    git(&root, &["branch", "--show-current"]).filter(|b| !b.is_empty()),
                    git(&root, &["status", "--porcelain=v2"]).map(|s| s.lines().filter(|l| !l.is_empty()).count()),
                )
            } else {
                (None, None)
            };
            Worktree { root, branch, dirty, entries }
        })
        .collect();
    worktrees.sort_by_key(|w| {
        w.entries.iter().map(|e| (e.session.priority(), std::cmp::Reverse(e.session.last_event_at))).min()
    });
    Board { now, worktrees, counters }
}

// ---------- a live session's latest ----------

/// Windows read from the end of a transcript for the board's one-line details:
/// a long turn's tool output can push the last prompt far back.
const PEEK_WINDOWS: [u64; 3] = [128 * 1024, 1024 * 1024, 4 * 1024 * 1024];

fn one_line(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

/// A Claude Code session's transcript when no hook event named it (sessions
/// started before the hooks, or seen only in the registry): `<id>.jsonl` in
/// one of the project folders. Found paths are remembered.
fn find_transcript(claude_dirs: &[PathBuf], session: &str) -> Option<PathBuf> {
    use std::sync::{Mutex, OnceLock};
    static FOUND: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    let found = FOUND.get_or_init(Default::default);
    if let Some(p) = found.lock().ok().and_then(|m| m.get(session).cloned()) {
        return Some(p);
    }
    let name = format!("{session}.jsonl");
    let hit = claude_dirs
        .iter()
        .flat_map(|d| fs::read_dir(d.join("projects")).into_iter().flatten().flatten())
        .map(|p| p.path().join(&name))
        .find(|p| p.is_file())?;
    if let Ok(mut m) = found.lock() {
        m.insert(session.to_owned(), hit.clone());
    }
    Some(hit)
}

/// What a tool call is about, for a person deciding whether to allow it.
fn describe_call(name: &str, input: &serde_json::Value) -> String {
    let field = |k: &str| input[k].as_str().map(str::to_owned);
    let what = match name {
        "Bash" => field("command"),
        "Edit" | "Write" | "MultiEdit" | "Read" | "NotebookEdit" => {
            field("file_path").or(field("notebook_path")).map(|f| f.rsplit('/').next().unwrap_or(&f).to_owned())
        }
        "WebFetch" => field("url"),
        "WebSearch" => field("query"),
        _ => input.as_object().and_then(|o| o.values().find_map(|v| v.as_str().map(str::to_owned))),
    };
    match what {
        Some(w) if !w.trim().is_empty() => format!("{name} · {}", one_line(&w, 120)),
        _ => name.to_owned(),
    }
}

/// The last prompt the person typed and, when `want_ask`, the call a
/// permission prompt is about (`pending_id`, else the last call with no result).
/// Claude Code transcripts and Codex rollouts; None where it cannot tell.
/// What a transcript's end says about a live session.
#[derive(Debug, Clone, Default, PartialEq)]
struct Peek {
    prompt: Option<String>,
    ask: Option<String>,
    model: Option<String>,
    /// Tokens the last reply read (the context in use).
    context_tokens: Option<u64>,
    /// The model's window, where the transcript states it (Codex).
    context_window: Option<u64>,
}

fn peek(log: &Path, pending_id: Option<&str>, want_ask: bool) -> Peek {
    use std::sync::{Mutex, OnceLock};
    type Key = (PathBuf, u64, Option<String>, bool);
    type Found = Peek;
    // The board is rebuilt every few seconds; a transcript that has not grown
    // gives the same answer, so keep the last one per file.
    static SEEN: OnceLock<Mutex<HashMap<PathBuf, (Key, Found)>>> = OnceLock::new();
    if log.extension().is_none_or(|x| x != "jsonl") {
        return Peek::default();
    }
    let len = fs::metadata(log).map(|m| m.len()).unwrap_or(0);
    let key: Key = (log.to_path_buf(), len, pending_id.map(str::to_owned), want_ask);
    let seen = SEEN.get_or_init(Default::default);
    if let Some((k, v)) = seen.lock().ok().and_then(|m| m.get(log).cloned()) {
        if k == key {
            return v;
        }
    }
    let mut out = Peek::default();
    for window in PEEK_WINDOWS {
        out = peek_window(log, len, window, pending_id, want_ask);
        if out.prompt.is_some() || window >= len {
            break;
        }
    }
    if let Ok(mut m) = seen.lock() {
        m.insert(log.to_path_buf(), (key, out.clone()));
    }
    out
}

fn peek_window(log: &Path, len: u64, window: u64, pending_id: Option<&str>, want_ask: bool) -> Peek {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(log) else { return Peek::default() };
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(window)));
    let mut buf = Vec::new();
    let _ = f.take(window).read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);

    let mut prompt = None;
    let (mut model, mut context_tokens, mut context_window) = (None, None, None);
    let mut calls: Vec<(String, String)> = Vec::new(); // (id, description)
    let mut answered: std::collections::HashSet<String> = Default::default();
    for line in text.lines() {
        let Ok(r) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        match r["type"].as_str() {
            Some("user") if r["isMeta"].as_bool() != Some(true) && r["isSidechain"].as_bool() != Some(true) => {
                let content = &r["message"]["content"];
                for b in content.as_array().into_iter().flatten() {
                    if let Some(id) = b["tool_use_id"].as_str() {
                        answered.insert(id.to_owned());
                    }
                }
                if let Some(h) = crate::digest::human_text(content).filter(|h| !h.starts_with("[Request interrupted")) {
                    prompt = Some(one_line(&h, 160));
                }
            }
            Some("assistant") if r["isSidechain"].as_bool() != Some(true) => {
                let m = &r["message"];
                if let Some(id) = m["model"].as_str().filter(|x| !x.starts_with('<')) {
                    model = Some(id.to_owned());
                }
                if m["usage"].is_object() {
                    context_tokens = Some(crate::usage::Tokens::from_claude(&m["usage"]).read());
                }
                if !want_ask {
                    continue;
                }
                for b in m["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "tool_use") {
                    if let (Some(id), Some(name)) = (b["id"].as_str(), b["name"].as_str()) {
                        calls.push((id.to_owned(), describe_call(name, &b["input"])));
                    }
                }
            }
            Some("response_item") if r["payload"]["type"] == "message" && r["payload"]["role"] == "user" => {
                if let Some(h) = crate::digest::human_text(&r["payload"]["content"]) {
                    prompt = Some(one_line(&h, 160));
                }
            }
            Some("turn_context") => {
                if let Some(m) = r["payload"]["model"].as_str() {
                    model = Some(m.to_owned());
                }
            }
            Some("event_msg") if r["payload"]["type"] == "token_count" => {
                let info = &r["payload"]["info"];
                if let Some(n) = info["last_token_usage"]["input_tokens"].as_u64() {
                    context_tokens = Some(n);
                }
                if let Some(w) = info["model_context_window"].as_u64() {
                    context_window = Some(w);
                }
            }
            _ => {}
        }
    }
    let ask = if want_ask {
        match pending_id {
            Some(id) => calls.iter().rev().find(|(c, _)| c == id),
            None => calls.iter().rev().find(|(c, _)| !answered.contains(c)),
        }
        .map(|(_, d)| d.clone())
    } else {
        None
    };
    Peek { prompt, ask, model, context_tokens, context_window }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_finds_the_prompt_and_the_call_waiting_for_permission() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("s.jsonl");
        let lines = [
            json!({"type": "user", "message": {"content": "run the tests please"}}),
            json!({"type": "assistant", "message": {"content": [
                {"type": "tool_use", "id": "a1", "name": "Read", "input": {"file_path": "/r/src/lib.rs"}}]}}),
            json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "a1", "content": "ok"}]}}),
            json!({"type": "assistant", "message": {"content": [
                {"type": "tool_use", "id": "a2", "name": "Bash", "input": {"command": "pnpm test --run"}}]}}),
        ];
        fs::write(&log, lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")).unwrap();
        let p = peek(&log, None, true);
        assert_eq!(p.prompt.as_deref(), Some("run the tests please"));
        assert_eq!(p.ask.as_deref(), Some("Bash · pnpm test --run"));
        assert_eq!(peek(&log, Some("a1"), true).ask.as_deref(), Some("Read · lib.rs"));
        assert_eq!(peek(&log, None, false).ask, None);
    }

    #[test]
    fn ended_sessions_leave_the_board_at_once() {
        assert!(!visible(State::Ended, Some(true), 0));
        assert!(!visible(State::Ended, None, 0));
        assert!(visible(State::TurnDone, Some(true), CODEX_QUIET_HIDE_MS * 2));
        assert!(visible(State::TurnDone, None, CODEX_QUIET_HIDE_MS - 1));
        assert!(!visible(State::TurnDone, None, CODEX_QUIET_HIDE_MS));
    }
}
