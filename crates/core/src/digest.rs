//! Gather one local day of work per project: what was asked, what the agents
//! answered, which files changed, which commits landed, and counts that need
//! no interpretation. This is the raw material the summary is written from.
//!
//! Sessions are grouped by *project*: the main checkout of the git repo the
//! session ran in, so parallel worktrees roll up under the repo they belong to.

use crate::lang::Lang;
use crate::paths;
use crate::usage::{self, Tokens, Usage};
use crate::time::{local_hour, parse_rfc3339_ms};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Gaps longer than this between two activity marks are not counted as work.
pub const IDLE_GAP_MS: u64 = 10 * 60 * 1000;

const MAX_PROMPTS: usize = 40;
const MAX_REPLIES: usize = 20;
const MAX_COMMANDS: usize = 30;
const MAX_FILES: usize = 60;
const PROMPT_CHARS: usize = 300;
/// A session's first request often carries the whole brief; keep more of it.
pub const FIRST_PROMPT_CHARS: usize = 1_500;
const REPLY_CHARS: usize = 400;
const COMMAND_CHARS: usize = 100;
const TURN_REPLY_CHARS: usize = 600;
const ERROR_CHARS: usize = 200;
const MAX_TURN_ERRORS: usize = 3;
/// Prompt a turn gets when the day opens in the middle of a conversation.
pub const CONTINUED: &str = "(앞선 대화에 이어서)";
/// How `CONTINUED` reads in English material. Saved turns keep `CONTINUED`
/// as their marker in every language; only the material shows this instead.
pub const CONTINUED_EN: &str = "(continued from an earlier conversation)";

/// A Codex command that failed, as saved on its turn and read by `health::failed_command`.
fn command_failed(cmd: &str, code: i64, lang: Lang) -> String {
    match lang {
        Lang::Ko => format!("명령 `{cmd}` 종료 코드 {code}"),
        Lang::En => format!("Command `{cmd}` exited with {code}"),
    }
}

/// A failed tool call whose tool is not known.
fn unknown_tool(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => "도구",
        Lang::En => "tool",
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Commit {
    /// repo name when the commit is from a repo other than the project's own
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub hash: String,
    pub t: u64,
    pub subject: String,
    pub insertions: u64,
    pub deletions: u64,
    /// Written by the person using porch (the repo's git user). None on
    /// reports saved before authors were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mine: Option<bool>,
    /// Author name, kept for commits by someone else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

/// One request and everything the agent did for it: from a prompt the person
/// typed to the next one. The unit time and trouble are counted in.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Turn {
    /// "t1", "t2"… in the order the day's turns started; how the summary points at them.
    #[serde(default)]
    pub id: String,
    pub start: u64,
    pub end: u64,
    /// Active time this turn accounts for; the day's turns add up to its active time.
    #[serde(default)]
    pub active_ms: u64,
    pub prompt: String,
    /// The session's first request (not a day opening mid-conversation): kept to
    /// `FIRST_PROMPT_CHARS` and always shown in full to the summarizer.
    #[serde(default)]
    pub first: bool,
    /// The agent's last answer in the turn.
    #[serde(default)]
    pub reply: Option<String>,
    #[serde(default)]
    pub tool_calls: usize,
    #[serde(default)]
    pub tool_errors: usize,
    #[serde(default)]
    pub denials: usize,
    /// The person stopped the agent mid-turn.
    #[serde(default)]
    pub interrupted: bool,
    /// The first few failures, "Bash `cargo test`: error[E0425]…".
    #[serde(default)]
    pub errors: Vec<String>,
    /// Files this turn edited (Edit, Write, MultiEdit, NotebookEdit). Claude only for now.
    #[serde(default)]
    pub files: Vec<String>,
    /// The repo most of `files` live in; None when the turn edited nothing or
    /// no edited file is inside a git repo.
    /// Health counts the turn toward this repo instead of the session's folder.
    #[serde(default)]
    pub root: Option<String>,
    /// Model tokens spent in the turn, subagents included.
    #[serde(default)]
    pub tokens: Tokens,
    /// List-price cost of those tokens (see `usage`); `unpriced` tokens had no known price.
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub unpriced: u64,
    /// The model that answered last in the turn.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(skip)]
    pub marks: Vec<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionDigest {
    pub agent: String,
    pub session: String,
    pub title: Option<String>,
    pub cwd: String,
    pub branch: Option<String>,
    pub first_at: u64,
    pub last_at: u64,
    pub prompts: Vec<String>,
    pub replies: Vec<String>,
    pub recaps: Vec<String>,
    pub commands: Vec<String>,
    pub files: BTreeSet<String>,
    pub tool_calls: usize,
    pub tool_errors: usize,
    pub denials: usize,
    #[serde(default)]
    pub turns: Vec<Turn>,
    /// Tokens and cost in the window, by model and hour.
    #[serde(default)]
    pub usage: Usage,
    #[serde(skip)]
    pub marks: Vec<u64>,
}

impl SessionDigest {
    /// Record activity at `t`, in the session and in the turn under way.
    fn mark(&mut self, t: u64) {
        self.marks.push(t);
        self.turn(t).marks.push(t);
    }

    /// The turn under way, opening a continuation turn when the day starts mid-conversation.
    fn turn(&mut self, t: u64) -> &mut Turn {
        if self.turns.is_empty() {
            self.turns.push(Turn { start: t, prompt: CONTINUED.into(), ..Default::default() });
        }
        self.turns.last_mut().expect("just ensured")
    }

    /// `first`: this is the session's first request, not a day opening mid-conversation.
    fn start_turn(&mut self, t: u64, prompt: &str, first: bool) {
        let first = first && self.turns.is_empty();
        let n = if first { FIRST_PROMPT_CHARS } else { PROMPT_CHARS };
        self.turns.push(Turn { start: t, prompt: clip(prompt, n), first, ..Default::default() });
        self.mark(t);
    }

    /// Count one model call at `t`: in the session, by hour, and in the turn under way then.
    fn spend(&mut self, t: u64, model: &str, tokens: &Tokens, offset: i64) {
        if tokens.total() == 0 {
            return;
        }
        self.usage.record(model, tokens, Some(local_hour(t, offset) as usize));
        let turn = match self.turns.iter().rposition(|x| x.start <= t) {
            Some(i) => &mut self.turns[i],
            None => self.turn(t),
        };
        turn.tokens.add(tokens);
        match usage::cost(model, tokens) {
            Some(c) => turn.cost += c,
            None => turn.unpriced += tokens.total(),
        }
        turn.model = Some(model.to_owned());
    }

    /// `what` names the failed call; `output` is what it printed.
    fn error(&mut self, t: u64, what: &str, output: &str) {
        self.tool_errors += 1;
        let turn = self.turn(t);
        turn.tool_errors += 1;
        if turn.errors.len() < MAX_TURN_ERRORS {
            turn.errors.push(format!("{what}: {}", ends(output, 50, ERROR_CHARS - 50)));
        }
    }
}

/// Text inside a tool result, whether a plain string or content blocks.
fn result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(b) => b.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub sessions: usize,
    pub prompts: usize,
    pub active_minutes: u64,
    pub tool_calls: usize,
    pub tool_errors: usize,
    pub denials: usize,
    pub files_touched: usize,
    pub commits: usize,
    /// Of `commits`, the ones the person using porch wrote. None on old reports.
    #[serde(default)]
    pub my_commits: Option<usize>,
    pub insertions: u64,
    pub deletions: u64,
    /// activity marks per local hour, 0..24
    pub by_hour: Vec<u32>,
    /// Tokens and list-price cost. None on reports saved before usage was read.
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectDigest {
    pub name: String,
    pub root: String,
    /// Other repos this project's sessions edited files in (e.g. after a `cd`).
    #[serde(default)]
    pub related_roots: Vec<String>,
    pub sessions: Vec<SessionDigest>,
    pub commits: Vec<Commit>,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DayDigest {
    pub date: String,
    pub start: u64,
    pub end: u64,
    pub projects: Vec<ProjectDigest>,
    pub metrics: Metrics,
    /// Usage of sessions run in scratch folders, by agent: spent, but in no
    /// project. Part of `metrics.usage`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scratch_usage: BTreeMap<String, Usage>,
    /// The day's usage by agent (scratch folders included) and by project name.
    /// Kept apart from `projects` because a recounted report may lack sessions
    /// and projects it now counts.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub agent_usage: BTreeMap<String, Usage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub project_usage: BTreeMap<String, Usage>,
    /// How usage was counted (`USAGE_COUNTING`); older saved days are counted again.
    #[serde(default)]
    pub usage_counting: u32,
}

/// 1: Claude responses counted once a day across sessions, from their last
/// record; Codex forks, restarted totals and sessions filed on earlier days;
/// scratch-folder sessions included.
pub const USAGE_COUNTING: u32 = 1;

fn clip(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

/// Head and tail of a long tool output: failures put the reason at the end.
fn ends(s: &str, head: usize, tail: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = s.chars().count();
    if n <= head + tail {
        return s;
    }
    let h: String = s.chars().take(head).collect();
    let t: String = s.chars().skip(n - tail).collect();
    format!("{h} … {t}")
}

fn push_capped(v: &mut Vec<String>, s: String, cap: usize) {
    if !s.is_empty() && v.len() < cap && v.last() != Some(&s) {
        v.push(s);
    }
}

/// Text a human typed, as opposed to tool results, command echoes and
/// system reminders, which agents also store as "user" messages.
pub(crate) fn human_text(content: &Value) -> Option<String> {
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| matches!(b["type"].as_str(), Some("text") | Some("input_text")))
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    let t = text.trim();
    (!t.is_empty() && !t.starts_with('<') && !t.starts_with("Caveat:")).then(|| t.to_owned())
}

fn in_window(t: u64, start: u64, end: u64) -> bool {
    t >= start && t < end
}

// ---------- Claude Code ----------

fn claude_transcripts(claude_dir: &Path, start: u64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(projects) = fs::read_dir(claude_dir.join("projects")) else { return out };
    for p in projects.flatten() {
        let Ok(files) = fs::read_dir(p.path()) else { continue };
        for f in files.flatten() {
            let path = f.path();
            if path.extension().is_none_or(|x| x != "jsonl") {
                continue;
            }
            let recent = f
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .is_some_and(|d| d.as_millis() as u64 >= start);
            if recent {
                out.push(path);
            }
        }
    }
    out
}

/// One session's day. `seen` holds the response ids already counted that day:
/// Claude Code copies a conversation into a new session file when it is
/// continued elsewhere or branched, so the same response can sit in two files.
/// A session in an `excluded` folder is dropped before it claims any id.
fn read_claude(path: &Path, start: u64, end: u64, offset: i64, seen: &mut HashSet<String>, excluded: &[String], lang: Lang) -> Option<SessionDigest> {
    let text = fs::read_to_string(path).ok()?;
    let mut d = SessionDigest {
        agent: "claude".into(),
        session: path.file_stem()?.to_string_lossy().into_owned(),
        ..Default::default()
    };
    // tool_use id -> "Bash `cargo test`", so a failed result can say what failed.
    let mut calls: HashMap<String, String> = HashMap::new();
    let mut replies = Replies::default();
    // Any of this conversation before the window: the day opens mid-conversation.
    let mut before = false;
    for line in text.lines() {
        let Ok(r) = serde_json::from_str::<Value>(line) else { continue };
        let kind = r["type"].as_str().unwrap_or("");
        if kind == "ai-title" {
            if let Some(t) = r["aiTitle"].as_str() {
                d.title = Some(clip(t, 120));
            }
            continue;
        }
        let Some(t) = r["timestamp"].as_str().and_then(parse_rfc3339_ms) else { continue };
        // Read past the window: a response begun before midnight finishes after it.
        if kind == "assistant" {
            replies.add(&r, t);
        }
        // Only the conversation counts: an answer, or something the person asked.
        // Local commands and meta records before the window do not.
        let asked = kind == "user"
            && r["isMeta"].as_bool() != Some(true)
            && r["isCompactSummary"].as_bool() != Some(true)
            && human_text(&r["message"]["content"]).is_some();
        if t < start && r["isSidechain"].as_bool() != Some(true) && (kind == "assistant" || asked) {
            before = true;
        }
        if !in_window(t, start, end) {
            continue;
        }
        if r["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        if let Some(c) = r["cwd"].as_str() {
            d.cwd = c.to_owned();
        }
        if let Some(b) = r["gitBranch"].as_str().filter(|b| !b.is_empty()) {
            d.branch = Some(b.to_owned());
        }
        let msg = &r["message"];
        match kind {
            "user" => {
                if r["toolDenialKind"].is_string() {
                    d.denials += 1;
                    d.turn(t).denials += 1;
                }
                for b in msg["content"].as_array().into_iter().flatten() {
                    if b["type"] == "tool_result" && b["is_error"] == true {
                        let what = b["tool_use_id"].as_str().and_then(|id| calls.get(id)).cloned().unwrap_or_else(|| unknown_tool(lang).into());
                        d.error(t, &what, &result_text(&b["content"]));
                    }
                }
                // Claude Code writes the summary it carries over after compacting as a
                // user record; it is not something the person asked.
                let compacted = r["isCompactSummary"].as_bool() == Some(true);
                if r["isMeta"].as_bool() != Some(true) && !compacted {
                    if let Some(h) = human_text(&msg["content"]) {
                        if h.starts_with("[Request interrupted") {
                            d.mark(t);
                            d.turn(t).interrupted = true;
                        } else {
                            push_capped(&mut d.prompts, clip(&h, PROMPT_CHARS), MAX_PROMPTS);
                            d.start_turn(t, &h, !before);
                        }
                    }
                }
            }
            "assistant" => {
                d.mark(t);
                for b in msg["content"].as_array().into_iter().flatten() {
                    if b["type"] != "tool_use" {
                        continue;
                    }
                    d.tool_calls += 1;
                    d.turn(t).tool_calls += 1;
                    let input = &b["input"];
                    let name = b["name"].as_str().unwrap_or("");
                    let mut what = name.to_owned();
                    match name {
                        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
                            if let Some(f) = input["file_path"].as_str().or(input["notebook_path"].as_str()) {
                                what = format!("{name} {}", f.rsplit('/').next().unwrap_or(f));
                                if d.files.len() < MAX_FILES {
                                    d.files.insert(f.to_owned());
                                }
                                let turn = d.turn(t);
                                if turn.files.len() < MAX_FILES && !turn.files.iter().any(|x| x == f) {
                                    turn.files.push(f.to_owned());
                                }
                            }
                        }
                        "Bash" => {
                            if let Some(c) = input["command"].as_str() {
                                what = format!("Bash `{}`", clip(c, 80));
                                push_capped(&mut d.commands, clip(c, COMMAND_CHARS), MAX_COMMANDS);
                            }
                        }
                        _ => {}
                    }
                    if let Some(id) = b["id"].as_str() {
                        calls.insert(id.to_owned(), what);
                    }
                }
                if msg["stop_reason"] == "end_turn" {
                    let text: String = msg["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.trim().is_empty() {
                        d.turn(t).reply = Some(clip(&text, TURN_REPLY_CHARS));
                    }
                    push_capped(&mut d.replies, clip(&text, REPLY_CHARS), MAX_REPLIES);
                }
            }
            "system" if r["subtype"] == "away_summary" => {
                if let Some(c) = r["content"].as_str() {
                    push_capped(&mut d.recaps, clip(c, REPLY_CHARS), MAX_REPLIES);
                }
            }
            _ => {}
        }
    }
    // Subagents write their own files next to the session: `<session>/subagents/*.jsonl`.
    let sub = path.with_extension("").join("subagents");
    for f in fs::read_dir(&sub).into_iter().flatten().flatten() {
        let Ok(text) = fs::read_to_string(f.path()) else { continue };
        for line in text.lines().filter(|l| l.contains("\"assistant\"")) {
            let Ok(r) = serde_json::from_str::<Value>(line) else { continue };
            let Some(t) = r["timestamp"].as_str().and_then(parse_rfc3339_ms) else { continue };
            if r["type"] == "assistant" {
                replies.add(&r, t);
            }
        }
    }
    if is_excluded(&d.cwd, excluded) {
        return None;
    }
    for (t, model, tokens) in replies.started_in(start, end, seen) {
        d.spend(t, &model, &tokens, offset);
    }
    finish_turns(&mut d);
    (!d.marks.is_empty()).then_some(d)
}

/// A session's model responses with their usage. One response is stored as
/// several records: they repeat its usage, and the later ones carry more
/// output, so the last record with any usage holds the whole. An all-zero
/// usage is skipped: OpenRouter-backed models write a zero placeholder first.
#[derive(Default)]
struct Replies {
    /// (first record's time, response id, model, usage of its last record)
    all: Vec<(u64, Option<String>, String, Tokens)>,
    index: HashMap<String, usize>,
}

impl Replies {
    fn add(&mut self, r: &Value, t: u64) {
        let msg = &r["message"];
        let Some(model) = msg["model"].as_str().filter(|m| !m.starts_with('<')) else { return };
        if !msg["usage"].is_object() {
            return;
        }
        let tokens = Tokens::from_claude(&msg["usage"]);
        if tokens.total() == 0 {
            return;
        }
        let id = msg["id"].as_str().map(str::to_owned);
        if let Some(&i) = id.as_ref().and_then(|id| self.index.get(id)) {
            self.all[i].3 = tokens;
            return;
        }
        if let Some(id) = &id {
            self.index.insert(id.clone(), self.all.len());
        }
        self.all.push((t, id, model.to_owned(), tokens));
    }

    /// Responses that began in the window and were not counted already today.
    fn started_in(self, start: u64, end: u64, seen: &mut HashSet<String>) -> Vec<(u64, String, Tokens)> {
        self.all
            .into_iter()
            .filter(|(t, id, _, _)| in_window(*t, start, end) && id.as_ref().is_none_or(|id| seen.insert(id.clone())))
            .map(|(t, _, model, tokens)| (t, model, tokens))
            .collect()
    }
}

/// Close each turn at its last activity; drop continuation turns with nothing in them.
fn finish_turns(d: &mut SessionDigest) {
    d.turns.retain(|t| !t.marks.is_empty());
    for t in &mut d.turns {
        t.end = t.marks.iter().copied().max().unwrap_or(t.start);
    }
}

// ---------- Codex ----------

fn codex_rollouts(start: u64, end: u64, offset: i64) -> Vec<PathBuf> {
    codex_rollouts_in(&paths::codex_homes(), start, end, offset)
}

/// Rollouts written to since the window began. They are filed under the day
/// they started (`sessions/YYYY/MM/DD`), and a long session keeps writing to
/// its file for days, so every earlier day's folder is looked at; archived
/// sessions sit in one flat folder. Folders for days after the window are skipped.
fn codex_rollouts_in(homes: &[PathBuf], start: u64, end: u64, offset: i64) -> Vec<PathBuf> {
    let last_day = crate::time::local_date(end.saturating_sub(1), offset).replace('-', "/");
    let mut out = Vec::new();
    for home in homes {
        let mut dirs = vec![home.join("archived_sessions")];
        let sessions = home.join("sessions");
        for y in subdirs(&sessions) {
            for m in subdirs(&y) {
                for d in subdirs(&m) {
                    let filed = d.strip_prefix(&sessions).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    if filed <= last_day {
                        dirs.push(d);
                    }
                }
            }
        }
        for dir in dirs {
            let Ok(files) = fs::read_dir(&dir) else { continue };
            out.extend(files.flatten().filter(|f| modified_since(f, start)).map(|f| f.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl")));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

fn modified_since(f: &fs::DirEntry, start: u64) -> bool {
    f.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .is_some_and(|d| d.as_millis() as u64 >= start)
}

fn read_codex(path: &Path, start: u64, end: u64, offset: i64, lang: Lang) -> Option<SessionDigest> {
    let text = fs::read_to_string(path).ok()?;
    let mut d = SessionDigest { agent: "codex".into(), ..Default::default() };
    // Codex reports running totals; the day's share is the growth inside the window.
    let mut model = String::from("codex");
    let mut total: Option<Tokens> = None;
    // Any of this conversation before the window: the day opens mid-conversation.
    let mut before = false;
    // A subagent thread's first message is the parent agent's task, not the person's.
    let mut subagent = false;
    for line in text.lines() {
        let Ok(r) = serde_json::from_str::<Value>(line) else { continue };
        let p = &r["payload"];
        if r["type"] == "session_meta" {
            subagent = p["thread_source"] == "subagent" || p["source"]["subagent"].is_object();
            d.session = p["id"].as_str().unwrap_or_default().to_owned();
            d.cwd = p["cwd"].as_str().unwrap_or_default().to_owned();
            d.branch = p["git"]["branch"].as_str().map(str::to_owned);
            continue;
        }
        if r["type"] == "turn_context" {
            if let Some(m) = p["model"].as_str() {
                model = m.to_owned();
            }
        }
        let Some(t) = r["timestamp"].as_str().and_then(parse_rfc3339_ms) else { continue };
        if r["type"] == "event_msg" && p["type"] == "token_count" && p["info"]["total_token_usage"].is_object() {
            let now = Tokens::from_codex(&p["info"]["total_token_usage"]);
            let call = p["info"]["last_token_usage"].is_object().then(|| Tokens::from_codex(&p["info"]["last_token_usage"]));
            let grew = match (total, call) {
                // A forked rollout starts from its parent's total: only this call is new.
                (None, Some(call)) if now.total() >= call.total() => call,
                // The total started over: this call is all there is.
                (Some(before), Some(call)) if now.total() < before.total() => call,
                (Some(before), _) => now.since(&before),
                (None, _) => now,
            };
            total = Some(now);
            if in_window(t, start, end) {
                d.spend(t, &model, &grew, offset);
            }
            continue;
        }
        // Codex writes developer and environment records when a session starts; only
        // an answer or something the person asked means the day opens mid-conversation.
        let said = r["type"] == "response_item"
            && p["type"] == "message"
            && (p["role"] == "assistant" || (p["role"] == "user" && human_text(&p["content"]).is_some()));
        if t < start && said {
            before = true;
        }
        if !in_window(t, start, end) {
            continue;
        }
        match (r["type"].as_str(), p["type"].as_str()) {
            (Some("response_item"), Some("message")) if p["role"] == "user" => {
                if let Some(h) = human_text(&p["content"]) {
                    push_capped(&mut d.prompts, clip(&h, PROMPT_CHARS), MAX_PROMPTS);
                    d.start_turn(t, &h, !before && !subagent);
                }
            }
            (Some("response_item"), Some("function_call" | "custom_tool_call")) => {
                d.mark(t);
                d.tool_calls += 1;
                d.turn(t).tool_calls += 1;
            }
            // A shell command finished; a non-zero exit is the one failure Codex records.
            (Some("event_msg"), Some("item_completed")) if p["item"]["type"] == "CommandExecution" => {
                let item = &p["item"];
                let code = item["exit_code"].as_i64().unwrap_or(0);
                let cmd = match &item["command"] {
                    Value::Array(a) => a.last().and_then(|x| x.as_str()).unwrap_or("").to_owned(),
                    Value::String(c) => c.clone(),
                    _ => String::new(),
                };
                let out = item["stderr"].as_str().filter(|x| !x.trim().is_empty()).or(item["aggregated_output"].as_str()).unwrap_or("");
                // grep and rg exit 1 with nothing printed when nothing matched: a search, not a failure.
                let no_match = code == 1 && out.trim().is_empty() && ["rg ", "grep "].iter().any(|g| cmd.trim_start().starts_with(g));
                if code != 0 && !no_match {
                    d.error(t, &command_failed(&clip(&cmd, 80), code, lang), out);
                }
            }
            (Some("event_msg"), Some("task_complete")) => {
                d.mark(t);
                if let Some(m) = p["last_agent_message"].as_str() {
                    d.turn(t).reply = Some(clip(m, TURN_REPLY_CHARS));
                    push_capped(&mut d.replies, clip(m, REPLY_CHARS), MAX_REPLIES);
                }
            }
            (Some("event_msg"), Some("turn_aborted")) => {
                d.mark(t);
                d.turn(t).interrupted = true;
            }
            _ => {}
        }
    }
    finish_turns(&mut d);
    (!d.marks.is_empty() && !d.cwd.is_empty()).then_some(d)
}

// ---------- git ----------

fn git(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_owned())
}

/// Main checkout of the repo `cwd` is in (worktrees roll up to it), or `cwd`
/// itself when it is not a git repo.
fn project_root(cwd: &str) -> String {
    let common = git(cwd, &["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    match common {
        Some(c) if c.ends_with("/.git") => c.trim_end_matches("/.git").to_owned(),
        _ => git(cwd, &["rev-parse", "--show-toplevel"]).unwrap_or_else(|| cwd.to_owned()),
    }
}

/// Repo a directory belongs to, cached. Temp folders and folders that no
/// longer exist belong to none.
pub(crate) fn repo_of_dir(dir: &str, cache: &mut HashMap<String, String>) -> Option<String> {
    if is_temp_path(dir) || !Path::new(dir).is_dir() {
        return None;
    }
    git_root_of(dir, cache)
}

/// Main checkout of the git repo `dir` is in, cached. A folder that is not in
/// a git repo belongs to none, so a turn that only edited e.g. a plan file
/// outside any repo counts toward no project.
fn git_root_of(dir: &str, cache: &mut HashMap<String, String>) -> Option<String> {
    let root = cache.entry(dir.to_owned()).or_insert_with(|| project_root(dir)).clone();
    Path::new(&root).join(".git").exists().then_some(root)
}

/// Give each turn the repo most of its edited files live in. A tie goes to
/// the turn's own project, then to the first repo by name.
fn assign_turn_roots(projects: &mut [ProjectDigest], mut root_of: impl FnMut(&str) -> Option<String>) {
    for p in projects.iter_mut() {
        let own = p.root.clone();
        for t in p.sessions.iter_mut().flat_map(|s| s.turns.iter_mut()) {
            let mut count: BTreeMap<String, usize> = BTreeMap::new();
            for f in &t.files {
                let Some(dir) = Path::new(f).parent().map(|d| d.to_string_lossy().into_owned()) else { continue };
                if let Some(r) = root_of(&dir) {
                    *count.entry(r).or_default() += 1;
                }
            }
            t.root = count.values().copied().max().and_then(|n| {
                if count.get(&own) == Some(&n) {
                    Some(own.clone())
                } else {
                    count.iter().find(|&(_, &c)| c == n).map(|(r, _)| r.clone())
                }
            });
        }
    }
}

/// Commits in `root` between `start` and `end`, on any branch, each marked
/// with whether the repo's own git user (`user.email`, else `user.name`)
/// wrote it. Shared repos carry teammates' commits too.
fn commits(root: &str, start: u64, end: u64) -> Vec<Commit> {
    let since = format!("--since=@{}", start / 1000);
    let until = format!("--until=@{}", end / 1000);
    let me_email = git(root, &["config", "user.email"]).map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty());
    let me_name = git(root, &["config", "user.name"]).map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    let Some(out) = git(
        root,
        &["log", "--all", "--no-merges", &since, &until, "--pretty=format:@@%h\t%at\t%ae\t%an\t%s", "--shortstat"],
    ) else {
        return vec![];
    };
    let mut v: Vec<Commit> = Vec::new();
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("@@") {
            let mut parts = rest.splitn(5, '\t');
            let hash = parts.next().unwrap_or_default().to_owned();
            let t = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0) * 1000;
            let email = parts.next().unwrap_or_default().trim().to_lowercase();
            let name = parts.next().unwrap_or_default().trim().to_owned();
            let subject = parts.next().unwrap_or_default().to_owned();
            let mine = match (&me_email, &me_name) {
                (Some(e), _) if *e == email => true,
                (_, Some(n)) if *n == name => true,
                (None, None) => true,
                _ => false,
            };
            v.push(Commit { hash, t, subject, mine: Some(mine), author: (!mine).then_some(name), ..Default::default() });
        } else if let Some(c) = v.last_mut() {
            for part in line.split(',') {
                let n: u64 = part.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
                if part.contains("insertion") {
                    c.insertions = n;
                } else if part.contains("deletion") {
                    c.deletions = n;
                }
            }
        }
    }
    v
}

// ---------- assemble ----------

fn active_minutes(marks: &mut [u64]) -> u64 {
    marks.sort_unstable();
    let ms: u64 = marks
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&gap| gap <= IDLE_GAP_MS)
        .sum();
    ms / 60_000
}

fn metrics(sessions: &[SessionDigest], commits: &[Commit], offset: i64) -> Metrics {
    let mut marks: Vec<u64> = sessions.iter().flat_map(|s| s.marks.iter().copied()).collect();
    let mut by_hour = vec![0u32; 24];
    for &t in &marks {
        by_hour[local_hour(t, offset) as usize] += 1;
    }
    let files: BTreeSet<&String> = sessions.iter().flat_map(|s| &s.files).collect();
    Metrics {
        sessions: sessions.len(),
        prompts: sessions.iter().flat_map(|s| &s.turns).filter(|t| t.prompt != CONTINUED).count(),
        active_minutes: active_minutes(&mut marks),
        tool_calls: sessions.iter().map(|s| s.tool_calls).sum(),
        tool_errors: sessions.iter().map(|s| s.tool_errors).sum(),
        denials: sessions.iter().map(|s| s.denials).sum(),
        files_touched: files.len(),
        commits: commits.len(),
        my_commits: Some(commits.iter().filter(|c| c.mine != Some(false)).count()),
        insertions: commits.iter().map(|c| c.insertions).sum(),
        deletions: commits.iter().map(|c| c.deletions).sum(),
        by_hour,
        usage: Some(sessions.iter().fold(Usage::default(), |mut u, s| {
            u.merge(&s.usage);
            u
        })),
    }
}

/// Read commit authors into a day saved before they were recorded, so older
/// reports can say how many commits were the user's own. Only commits change.
pub fn fill_commit_authors(d: &mut DayDigest) {
    if d.metrics.my_commits.is_some() {
        return;
    }
    for p in &mut d.projects {
        let mut all = if Path::new(&p.root).is_dir() { commits(&p.root, d.start, d.end) } else { vec![] };
        for r in &p.related_roots {
            let name = r.rsplit('/').next().map(str::to_owned);
            all.extend(commits(r, d.start, d.end).into_iter().map(|mut c| {
                c.repo = name.clone();
                c
            }));
        }
        all.sort_by_key(|c| c.t);
        p.metrics.commits = all.len();
        p.metrics.my_commits = Some(all.iter().filter(|c| c.mine != Some(false)).count());
        p.metrics.insertions = all.iter().map(|c| c.insertions).sum();
        p.metrics.deletions = all.iter().map(|c| c.deletions).sum();
        p.commits = all;
    }
    let all: Vec<&Commit> = d.projects.iter().flat_map(|p| &p.commits).collect();
    d.metrics.commits = all.len();
    d.metrics.my_commits = Some(all.iter().filter(|c| c.mine != Some(false)).count());
    d.metrics.insertions = all.iter().map(|c| c.insertions).sum();
    d.metrics.deletions = all.iter().map(|c| c.deletions).sum();
}

/// Scratch locations (agent scratchpads, temp checkouts). Sessions there are real
/// and stay on the live board, but they are not projects: summaries and project
/// history leave them out.
pub fn is_temp_path(cwd: &str) -> bool {
    ["/tmp/", "/private/tmp/", "/var/folders/", "/private/var/folders/"]
        .iter()
        .any(|p| cwd.starts_with(p) || cwd == p.trim_end_matches('/'))
}

pub struct Options {
    pub claude_dirs: Vec<PathBuf>,
    /// cwd prefixes never included (the summarizer's own runner dir, user exclusions)
    pub excluded: Vec<String>,
    /// The language of the text porch writes onto turns (failed commands).
    pub lang: Lang,
}

pub fn collect_day(date: &str, offset: i64, opts: &Options) -> Option<DayDigest> {
    let (start, end) = crate::time::local_day_window(date, offset)?;
    let mut sessions: Vec<SessionDigest> = Vec::new();
    let mut seen = HashSet::new();
    for dir in &opts.claude_dirs {
        let mut files = claude_transcripts(dir, start);
        // A copied response goes to whichever file is read first; a fixed order keeps that stable.
        files.sort();
        sessions.extend(files.iter().filter_map(|p| read_claude(p, start, end, offset, &mut seen, &opts.excluded, opts.lang)));
    }
    sessions.extend(codex_rollouts(start, end, offset).iter().filter_map(|p| read_codex(p, start, end, offset, opts.lang)));
    Some(assemble(date, start, end, sessions, opts, offset))
}

pub(crate) fn is_excluded(cwd: &str, excluded: &[String]) -> bool {
    excluded.iter().any(|e| cwd == *e || cwd.starts_with(&format!("{}/", e.trim_end_matches('/'))))
}

fn assemble(date: &str, start: u64, end: u64, mut sessions: Vec<SessionDigest>, opts: &Options, offset: i64) -> DayDigest {
    sessions.retain(|s| !is_excluded(&s.cwd, &opts.excluded));
    let mut scratch_usage: BTreeMap<String, Usage> = BTreeMap::new();
    let mut agent_usage: BTreeMap<String, Usage> = BTreeMap::new();
    for s in &sessions {
        agent_usage.entry(s.agent.clone()).or_default().merge(&s.usage);
    }
    agent_usage.retain(|_, u| u.tokens.total() > 0);
    sessions.retain(|s| {
        if !is_temp_path(&s.cwd) {
            return true;
        }
        scratch_usage.entry(s.agent.clone()).or_default().merge(&s.usage);
        false
    });
    scratch_usage.retain(|_, u| u.tokens.total() > 0);

    let mut roots: HashMap<String, String> = HashMap::new();
    let mut by_project: BTreeMap<String, Vec<SessionDigest>> = BTreeMap::new();
    for mut s in sessions {
        s.first_at = s.marks.iter().copied().min().unwrap_or(0);
        s.last_at = s.marks.iter().copied().max().unwrap_or(0);
        let root = roots.entry(s.cwd.clone()).or_insert_with(|| project_root(&s.cwd)).clone();
        by_project.entry(root).or_default().push(s);
    }

    let project_roots: BTreeSet<String> = by_project.keys().cloned().collect();
    let mut dir_roots: HashMap<String, String> = HashMap::new();
    let mut projects: Vec<ProjectDigest> = by_project
        .into_iter()
        .map(|(root, mut sessions)| {
            sessions.sort_by_key(|s| s.first_at);
            // Repos reached through edited files that are not their own project today.
            let mut related = BTreeSet::new();
            for f in sessions.iter().flat_map(|s| &s.files) {
                let Some(dir) = Path::new(f).parent().map(|d| d.to_string_lossy().into_owned()) else { continue };
                if dir.starts_with(&format!("{root}/")) || dir == root || !Path::new(&dir).is_dir() {
                    continue;
                }
                let r = dir_roots.entry(dir.clone()).or_insert_with(|| project_root(&dir)).clone();
                if r != root && !project_roots.contains(&r) && Path::new(&r).join(".git").exists() {
                    related.insert(r);
                }
            }
            let mut all_commits = if Path::new(&root).is_dir() { commits(&root, start, end) } else { vec![] };
            for r in &related {
                let name = r.rsplit('/').next().map(str::to_owned);
                all_commits.extend(commits(r, start, end).into_iter().map(|mut c| {
                    c.repo = name.clone();
                    c
                }));
            }
            all_commits.sort_by_key(|c| c.t);
            let metrics = metrics(&sessions, &all_commits, offset);
            let name = root.rsplit('/').next().unwrap_or(&root).to_owned();
            ProjectDigest { name, root, related_roots: related.into_iter().collect(), sessions, commits: all_commits, metrics }
        })
        .collect();
    projects.sort_by_key(|p| std::cmp::Reverse(p.metrics.active_minutes));
    assign_turn_roots(&mut projects, |dir| repo_of_dir(dir, &mut dir_roots));
    number_and_time_turns(&mut projects);

    let all_sessions: Vec<SessionDigest> = projects.iter().flat_map(|p| p.sessions.clone()).collect();
    let all_commits: Vec<Commit> = projects.iter().flat_map(|p| p.commits.clone()).collect();
    let mut metrics = metrics(&all_sessions, &all_commits, offset);
    let mut project_usage: BTreeMap<String, Usage> = BTreeMap::new();
    for p in &projects {
        if let Some(u) = p.metrics.usage.as_ref().filter(|u| u.tokens.total() > 0) {
            project_usage.entry(p.name.clone()).or_default().merge(u);
        }
    }
    if let Some(u) = &mut metrics.usage {
        for s in scratch_usage.values() {
            u.merge(s);
        }
    }
    DayDigest { date: date.to_owned(), start, end, projects, metrics, scratch_usage, agent_usage, project_usage, usage_counting: USAGE_COUNTING }
}

/// Name turns t1, t2… in the order they started, and split the day's active
/// time among them: every counted gap between two moments of activity goes to
/// the turn the earlier moment belongs to. With sessions running side by side
/// the time is shared out, not counted twice, so the turns add up to the day.
fn number_and_time_turns(projects: &mut [ProjectDigest]) {
    let mut refs: Vec<(u64, usize, usize, usize)> = Vec::new();
    for (pi, p) in projects.iter().enumerate() {
        for (si, s) in p.sessions.iter().enumerate() {
            for (ti, t) in s.turns.iter().enumerate() {
                refs.push((t.start, pi, si, ti));
            }
        }
    }
    refs.sort();
    for (n, &(_, pi, si, ti)) in refs.iter().enumerate() {
        projects[pi].sessions[si].turns[ti].id = format!("t{}", n + 1);
    }
    let mut marks: Vec<(u64, usize, usize, usize)> = Vec::new();
    for &(_, pi, si, ti) in &refs {
        marks.extend(projects[pi].sessions[si].turns[ti].marks.iter().map(|&m| (m, pi, si, ti)));
    }
    marks.sort();
    for w in marks.windows(2) {
        let gap = w[1].0 - w[0].0;
        if gap <= IDLE_GAP_MS {
            let (_, pi, si, ti) = w[0];
            projects[pi].sessions[si].turns[ti].active_ms += gap;
        }
    }
}

/// Turns worth reading in full: where the time went and where it went wrong.
/// The rest are listed one line each so the summarizer still sees the whole day.
fn notable(turns: &[&Turn]) -> BTreeSet<String> {
    let mut by_time: Vec<&&Turn> = turns.iter().collect();
    by_time.sort_by_key(|t| std::cmp::Reverse(t.active_ms));
    let mut out: BTreeSet<String> = by_time.iter().take(15).filter(|t| t.active_ms >= 3 * 60_000).map(|t| t.id.clone()).collect();
    out.extend(turns.iter().filter(|t| t.tool_errors > 0 || t.denials > 0 || t.interrupted).map(|t| t.id.clone()));
    out
}

fn hhmm_local(ms: u64, offset: i64) -> String {
    let secs = (ms as i64 / 1000 + offset).rem_euclid(86_400);
    format!("{:02}:{:02}", secs / 3600, secs % 3600 / 60)
}

/// Compact text the summarizer reads, with porch's own labels in `lang`
/// (ADR 0011). The day prompts name these labels ("Request", "Usual"…);
/// change both together. Paths are shown relative to the project root.
pub fn to_prompt_text(day: &DayDigest, offset: i64, lang: Lang) -> String {
    use std::fmt::Write as _;
    let en = lang == Lang::En;
    let mut out = String::new();
    let _ = writeln!(out, "{} {}", if en { "Date:" } else { "날짜:" }, day.date);
    for p in &day.projects {
        let m = &p.metrics;
        let mine = m.my_commits.unwrap_or(m.commits);
        let _ = if en {
            writeln!(
                out,
                "\n## Project: {} ({})\nNumbers: sessions {}, requests {}, active {} min, tool calls {}, tool errors {}, denied permissions {}, files changed {}, commits {} (mine {}) (+{} -{})",
                p.name, p.root, m.sessions, m.prompts, m.active_minutes, m.tool_calls, m.tool_errors, m.denials, m.files_touched, m.commits, mine, m.insertions, m.deletions
            )
        } else {
            writeln!(
                out,
                "\n## 프로젝트: {} ({})\n수치: 세션 {}, 요청 {}, 작업 {}분, 도구 {}회, 도구 오류 {}, 거절한 권한 {}, 고친 파일 {}, 커밋 {}(내 커밋 {}) (+{} -{})",
                p.name, p.root, m.sessions, m.prompts, m.active_minutes, m.tool_calls, m.tool_errors, m.denials, m.files_touched, m.commits, mine, m.insertions, m.deletions
            )
        };
        for c in &p.commits {
            let repo = c.repo.as_deref().map(|r| format!(" [{r}]")).unwrap_or_default();
            let who = c
                .author
                .as_deref()
                .filter(|_| c.mine == Some(false))
                .map(|a| if en { format!(" (other: {a})") } else { format!(" (다른 사람: {a})") })
                .unwrap_or_default();
            let _ = writeln!(out, "{}{repo} {}: {} (+{} -{}){who}", if en { "Commit" } else { "커밋" }, c.hash, c.subject, c.insertions, c.deletions);
        }
        for s in &p.sessions {
            let rel = |f: &String| f.strip_prefix(&format!("{}/", p.root)).unwrap_or(f).to_owned();
            let title = s.title.as_deref().unwrap_or(if en { "untitled" } else { "제목 없음" });
            let branch = s.branch.as_deref().unwrap_or("-");
            let _ = if en {
                writeln!(out, "\n### Session [{}] {title} (branch {branch})", s.agent)
            } else {
                writeln!(out, "\n### 세션 [{}] {title} (브랜치 {branch})", s.agent)
            };
            for x in &s.recaps {
                let _ = writeln!(out, "- {} {x}", if en { "Summary:" } else { "요약:" });
            }
            if !s.files.is_empty() {
                let _ = writeln!(out, "- {} {}", if en { "Files changed:" } else { "고친 파일:" }, s.files.iter().map(rel).collect::<Vec<_>>().join(", "));
            }
            let turns: Vec<&Turn> = s.turns.iter().collect();
            let full = notable(&turns);
            for t in &turns {
                let min = (t.active_ms + 30_000) / 60_000;
                let mut line = if en {
                    format!("- [{}] {} · {min} min", t.id, hhmm_local(t.start, offset))
                } else {
                    format!("- [{}] {} · {min}분", t.id, hhmm_local(t.start, offset))
                };
                if t.tool_calls > 0 {
                    let _ = if en { write!(line, " · {} tool calls", t.tool_calls) } else { write!(line, " · 도구 {}", t.tool_calls) };
                }
                if t.tool_errors > 0 {
                    let _ = if en { write!(line, " · {} tool errors", t.tool_errors) } else { write!(line, " · 오류 {}", t.tool_errors) };
                }
                if t.denials > 0 {
                    let _ = if en { write!(line, " · {} permission denials", t.denials) } else { write!(line, " · 거절 {}", t.denials) };
                }
                if t.interrupted {
                    line.push_str(if en { " · interrupted by the person" } else { " · 사람이 중단함" });
                }
                let prompt = if en && t.prompt == CONTINUED { CONTINUED_EN } else { t.prompt.as_str() };
                let request = if en { "Request:" } else { "요청:" };
                let notable = full.contains(&t.id);
                if notable || t.first {
                    let _ = writeln!(out, "{line}\n  {request} {prompt}");
                }
                if notable {
                    for e in &t.errors {
                        let _ = writeln!(out, "  {} {e}", if en { "Failed:" } else { "실패:" });
                    }
                    if let Some(r) = &t.reply {
                        let _ = writeln!(out, "  {} {r}", if en { "Final reply:" } else { "끝난 답:" });
                    }
                }
                if !notable && !t.first {
                    let _ = writeln!(out, "{line} · {request} {}", clip(prompt, 90));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use serde_json::json;

    fn material_day() -> DayDigest {
        DayDigest {
            date: "2026-10-02".into(),
            projects: vec![ProjectDigest {
                name: "porch".into(),
                root: "/r/porch".into(),
                metrics: Metrics { sessions: 2, prompts: 3, active_minutes: 68, tool_calls: 9, tool_errors: 4, denials: 1, files_touched: 1, commits: 1, my_commits: Some(0), insertions: 3, deletions: 1, ..Default::default() },
                commits: vec![Commit { hash: "abc1234".into(), subject: "fix: tray count".into(), insertions: 3, deletions: 1, author: Some("Kim".into()), mine: Some(false), ..Default::default() }],
                sessions: vec![
                    SessionDigest {
                        agent: "claude".into(),
                        branch: Some("main".into()),
                        recaps: vec!["tray count fixed".into()],
                        files: ["/r/porch/src/a.rs".to_owned()].into(),
                        turns: vec![
                            Turn { id: "t1".into(), prompt: CONTINUED.into(), active_ms: 60_000, ..Default::default() },
                            Turn {
                                id: "t2".into(),
                                prompt: "fix the build".into(),
                                active_ms: 3_600_000,
                                tool_calls: 9,
                                tool_errors: 4,
                                denials: 1,
                                interrupted: true,
                                errors: vec!["Bash `cargo test`: failed".into()],
                                reply: Some("done".into()),
                                ..Default::default()
                            },
                            Turn { id: "t3".into(), prompt: "rename it".into(), active_ms: 120_000, ..Default::default() },
                        ],
                        ..Default::default()
                    },
                    SessionDigest {
                        agent: "codex".into(),
                        title: Some("tray".into()),
                        turns: vec![Turn { id: "t4".into(), prompt: "make the tray count right".into(), first: true, active_ms: 300_000, ..Default::default() }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// The Korean material as it was before languages existed: it must not change.
    const MATERIAL_KO: &str = "날짜: 2026-10-02\n\n## 프로젝트: porch (/r/porch)\n수치: 세션 2, 요청 3, 작업 68분, 도구 9회, 도구 오류 4, 거절한 권한 1, 고친 파일 1, 커밋 1(내 커밋 0) (+3 -1)\n커밋 abc1234: fix: tray count (+3 -1) (다른 사람: Kim)\n\n### 세션 [claude] 제목 없음 (브랜치 main)\n- 요약: tray count fixed\n- 고친 파일: src/a.rs\n- [t1] 00:00 · 1분 · 요청: (앞선 대화에 이어서)\n- [t2] 00:00 · 60분 · 도구 9 · 오류 4 · 거절 1 · 사람이 중단함\n  요청: fix the build\n  실패: Bash `cargo test`: failed\n  끝난 답: done\n- [t3] 00:00 · 2분 · 요청: rename it\n\n### 세션 [codex] tray (브랜치 -)\n- [t4] 00:00 · 5분\n  요청: make the tray count right\n";

    #[test]
    fn korean_material_stays_the_same() {
        assert_eq!(to_prompt_text(&material_day(), 0, Lang::Ko), MATERIAL_KO);
    }

    #[test]
    fn english_material_has_no_korean_of_its_own() {
        let day = material_day();
        let en = to_prompt_text(&day, 0, Lang::En);
        assert!(!crate::lang::has_hangul(&en), "{en}");
        for w in ["Date:", "## Project:", "Request:", "Failed:", "Final reply:", "(other: Kim)", "interrupted by the person", CONTINUED_EN, "untitled", "· 9 tool calls", "· 1 permission denials"] {
            assert!(en.contains(w), "{w}\n{en}");
        }
        // The same numbers in the same order in both languages.
        let ko = to_prompt_text(&day, 0, Lang::Ko);
        let digits = |s: &str| s.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(digits(&ko), digits(&en));
    }

    #[test]
    fn tool_failures_are_written_in_the_language() {
        assert_eq!(command_failed("cargo test", 101, Lang::Ko), "명령 `cargo test` 종료 코드 101");
        assert_eq!(command_failed("cargo test", 101, Lang::En), "Command `cargo test` exited with 101");
        assert_eq!((unknown_tool(Lang::Ko), unknown_tool(Lang::En)), ("도구", "tool"));
    }

    #[test]
    fn human_text_skips_tool_results_and_reminders() {
        assert_eq!(human_text(&json!("fix the build")).as_deref(), Some("fix the build"));
        assert_eq!(human_text(&json!("<command-name>/fork</command-name>")), None);
        assert_eq!(human_text(&json!([{"type": "tool_result", "content": "ok"}])), None);
        assert_eq!(
            human_text(&json!([{"type": "input_text", "text": "codex prompt"}])).as_deref(),
            Some("codex prompt")
        );
    }

    #[test]
    fn temp_paths_are_not_projects() {
        assert!(is_temp_path("/private/tmp/claude-501/x/scratchpad"));
        assert!(is_temp_path("/var/folders/ab/T/checkout"));
        assert!(!is_temp_path("/Users/me/Repositories/tmp-tool"));
    }

    #[test]
    fn commits_know_whose_they_are() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        let run = |args: &[&str], env: &[(&str, &str)]| {
            let ok = Command::new("git").arg("-C").arg(root).args(args).envs(env.iter().copied()).output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        run(&["init", "-q"], &[]);
        run(&["config", "user.email", "me@example.com"], &[]);
        run(&["config", "user.name", "Me"], &[]);
        run(&["config", "commit.gpgsign", "false"], &[]);
        run(&["commit", "-q", "--allow-empty", "-m", "mine"], &[]);
        run(
            &["commit", "-q", "--allow-empty", "-m", "theirs"],
            &[("GIT_AUTHOR_NAME", "Pat"), ("GIT_AUTHOR_EMAIL", "pat@example.com")],
        );
        let now = crate::time::now_ms();
        let c = commits(root, now - 3_600_000, now + 60_000);
        let got: Vec<(&str, Option<bool>, Option<&str>)> = c.iter().map(|c| (c.subject.as_str(), c.mine, c.author.as_deref())).collect();
        assert!(got.contains(&("mine", Some(true), None)));
        assert!(got.contains(&("theirs", Some(false), Some("Pat"))));
        let m = metrics(&[], &c, 0);
        assert_eq!((m.commits, m.my_commits), (2, Some(1)));
    }

    #[test]
    fn active_minutes_ignores_long_gaps() {
        let min = 60_000;
        let mut marks = vec![0, 2 * min, 4 * min, 60 * min, 61 * min];
        assert_eq!(active_minutes(&mut marks), 5);
    }

    #[test]
    fn clip_counts_chars_not_bytes() {
        assert_eq!(clip("가나다라", 2), "가나…");
        assert_eq!(clip("a  b\n c", 10), "a b c");
    }

    #[test]
    fn claude_transcript_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let lines = [
            json!({"type": "ai-title", "aiTitle": "Fix login"}),
            json!({"type": "user", "timestamp": "2026-09-27T01:00:00Z", "cwd": "/r", "gitBranch": "main",
                   "message": {"content": "please fix login"}}),
            json!({"type": "assistant", "timestamp": "2026-09-27T01:01:00Z",
                   "message": {"stop_reason": "tool_use", "content": [
                       {"type": "tool_use", "name": "Edit", "input": {"file_path": "/r/src/login.rs"}},
                       {"type": "tool_use", "name": "Bash", "input": {"command": "cargo test"}}]}}),
            json!({"type": "user", "timestamp": "2026-09-27T01:02:00Z",
                   "message": {"content": [{"type": "tool_result", "is_error": true, "content": "fail"}]}}),
            json!({"type": "assistant", "timestamp": "2026-09-27T01:03:00Z",
                   "message": {"stop_reason": "end_turn", "content": [{"type": "text", "text": "Fixed it."}]}}),
            json!({"type": "user", "timestamp": "2026-09-28T01:00:00Z", "message": {"content": "tomorrow"}}),
        ];
        fs::write(&path, lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")).unwrap();
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.title.as_deref(), Some("Fix login"));
        assert_eq!(d.prompts, ["please fix login"]);
        assert_eq!(d.replies, ["Fixed it."]);
        assert_eq!(d.commands, ["cargo test"]);
        assert!(d.files.contains("/r/src/login.rs"));
        assert_eq!((d.tool_calls, d.tool_errors), (2, 1));
        assert_eq!(d.branch.as_deref(), Some("main"));
        assert_eq!(d.turns[0].files, ["/r/src/login.rs"]);
    }

    #[test]
    fn claude_compact_summary_is_not_a_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let lines = [
            json!({"type": "user", "timestamp": "2026-09-27T01:00:00Z", "cwd": "/r", "message": {"content": "start work"}}),
            json!({"type": "assistant", "timestamp": "2026-09-27T01:01:00Z",
                   "message": {"stop_reason": "end_turn", "content": [{"type": "text", "text": "ok"}]}}),
            json!({"type": "user", "timestamp": "2026-09-27T01:30:00Z", "isCompactSummary": true, "isVisibleInTranscriptOnly": true,
                   "message": {"content": "This session is being continued from a previous conversation that ran out of context."}}),
            json!({"type": "user", "timestamp": "2026-09-27T01:31:00Z", "message": {"content": "next step"}}),
        ];
        write_lines(&path, &lines);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.prompts, ["start work", "next step"]);
        assert_eq!(d.turns.len(), 2);
        assert!(d.turns.iter().all(|t| !t.prompt.starts_with("This session")));
        let compact_at = parse_rfc3339_ms("2026-09-27T01:30:00Z").unwrap();
        assert!(!d.marks.contains(&compact_at));
    }

    #[test]
    fn claude_compact_summary_in_blocks_is_not_a_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let lines = [
            json!({"type": "user", "timestamp": "2026-09-27T01:30:00Z", "cwd": "/r", "isCompactSummary": true,
                   "message": {"content": [{"type": "text", "text": "This session is being continued from a previous conversation."}]}}),
            json!({"type": "user", "timestamp": "2026-09-27T01:31:00Z", "message": {"content": "next step"}}),
        ];
        write_lines(&path, &lines);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.prompts, ["next step"]);
        assert_eq!(d.turns.len(), 1);
        assert_eq!(d.turns[0].prompt, "next step");
    }

    fn user(ts: &str, text: &str) -> Value {
        json!({"type": "user", "timestamp": ts, "cwd": "/r", "message": {"content": text}})
    }

    fn day(date: &str) -> (u64, u64) {
        crate::time::local_day_window(date, 0).unwrap()
    }

    #[test]
    fn claude_first_prompt_keeps_1500_chars() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let long = "가".repeat(400);
        write_lines(&path, &[user("2026-09-27T01:00:00Z", &long), user("2026-09-27T01:05:00Z", &long)]);
        let (s, e) = day("2026-09-27");
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert!(d.turns[0].first);
        assert_eq!(d.turns[0].prompt.chars().count(), 400);
        assert!(!d.turns[1].first);
        assert_eq!(d.turns[1].prompt.chars().count(), 301); // 300 + "…"
        assert_eq!(d.prompts[0].chars().count(), 301); // the session list stays at 300
    }

    #[test]
    fn first_prompt_clips_at_1500() {
        let dir = tempfile::tempdir().unwrap();
        let (s, e) = day("2026-09-27");
        let exact = dir.path().join("a.jsonl");
        write_lines(&exact, &[user("2026-09-27T01:00:00Z", &"a".repeat(1_500))]);
        let d = read_claude(&exact, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.turns[0].prompt, "a".repeat(1_500));
        let over = dir.path().join("b.jsonl");
        write_lines(&over, &[user("2026-09-27T01:00:00Z", &"a".repeat(1_501))]);
        let d = read_claude(&over, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.turns[0].prompt, format!("{}…", "a".repeat(1_500)));
    }

    #[test]
    fn day_opening_mid_conversation_has_no_first_turn() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        write_lines(&path, &[user("2026-09-26T23:00:00Z", "yesterday"), user("2026-09-27T01:00:00Z", &"가".repeat(400))]);
        let (s, e) = day("2026-09-27");
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert!(d.turns.iter().all(|t| !t.first));
        assert_eq!(d.turns[0].prompt.chars().count(), 301);
    }

    #[test]
    fn sidechain_before_the_window_does_not_hide_the_first_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let side = json!({"type": "user", "timestamp": "2026-09-26T23:00:00Z", "isSidechain": true, "message": {"content": "sub"}});
        write_lines(&path, &[side, user("2026-09-27T01:00:00Z", &"가".repeat(400))]);
        let (s, e) = day("2026-09-27");
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert!(d.turns[0].first);
        assert_eq!(d.turns[0].prompt.chars().count(), 400);
    }

    #[test]
    fn prompt_after_compaction_is_not_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let compact = json!({"type": "user", "timestamp": "2026-09-27T01:30:00Z", "cwd": "/r", "isCompactSummary": true,
                             "message": {"content": "This session is being continued from a previous conversation."}});
        write_lines(&path, &[user("2026-09-27T01:00:00Z", "start"), compact, user("2026-09-27T01:31:00Z", &"가".repeat(400))]);
        let (s, e) = day("2026-09-27");
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.turns.len(), 2);
        assert!(d.turns[0].first);
        assert!(!d.turns[1].first);
        assert_eq!(d.turns[1].prompt.chars().count(), 301);
    }

    #[test]
    fn codex_first_prompt_keeps_1500_chars() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let msg = |ts: &str, text: &str| json!({"type": "response_item", "timestamp": ts,
            "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": text}]}});
        let long = "가".repeat(400);
        write_lines(&path, &[
            json!({"type": "session_meta", "timestamp": "2026-09-27T00:59:00Z", "payload": {"id": "c1", "cwd": "/r"}}),
            msg("2026-09-27T01:00:00Z", &long),
            msg("2026-09-27T01:05:00Z", &long),
        ]);
        let (s, e) = day("2026-09-27");
        let d = read_codex(&path, s, e, 0, Lang::Ko).unwrap();
        assert!(d.turns[0].first);
        assert_eq!(d.turns[0].prompt.chars().count(), 400);
        assert!(!d.turns[1].first);
        assert_eq!(d.turns[1].prompt.chars().count(), 301);
    }

    fn codex_msg(ts: &str, role: &str, text: &str) -> Value {
        json!({"type": "response_item", "timestamp": ts,
               "payload": {"type": "message", "role": role, "content": [{"type": "input_text", "text": text}]}})
    }

    #[test]
    fn codex_session_started_before_midnight_keeps_its_first_prompt() {
        // Codex writes developer and environment records when a session starts; they are not conversation.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        write_lines(&path, &[
            json!({"type": "session_meta", "timestamp": "2026-09-26T23:59:00Z", "payload": {"id": "c1", "cwd": "/r"}}),
            codex_msg("2026-09-26T23:59:01Z", "developer", "You are Codex."),
            codex_msg("2026-09-26T23:59:01Z", "user", "<environment_context>cwd /r</environment_context>"),
            codex_msg("2026-09-27T00:05:00Z", "user", &"가".repeat(400)),
        ]);
        let (s, e) = day("2026-09-27");
        let d = read_codex(&path, s, e, 0, Lang::Ko).unwrap();
        assert!(d.turns[0].first);
        assert_eq!(d.turns[0].prompt.chars().count(), 400);
    }

    #[test]
    fn claude_command_before_midnight_keeps_the_first_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let caveat = json!({"type": "user", "timestamp": "2026-09-26T23:59:00Z", "isMeta": true, "message": {"content": "Caveat: local command"}});
        write_lines(&path, &[
            caveat,
            user("2026-09-26T23:59:01Z", "<command-name>/model</command-name>"),
            user("2026-09-27T00:05:00Z", &"가".repeat(400)),
        ]);
        let (s, e) = day("2026-09-27");
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert!(d.turns[0].first);
        assert_eq!(d.turns[0].prompt.chars().count(), 400);
    }

    #[test]
    fn codex_subagent_thread_has_no_first_prompt() {
        // A subagent thread's first message is the parent agent's task, not the person's request.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        write_lines(&path, &[
            json!({"type": "session_meta", "timestamp": "2026-09-27T00:59:00Z",
                   "payload": {"id": "c2", "cwd": "/r", "thread_source": "subagent", "parent_thread_id": "c1"}}),
            codex_msg("2026-09-27T01:00:00Z", "user", &"가".repeat(400)),
        ]);
        let (s, e) = day("2026-09-27");
        let d = read_codex(&path, s, e, 0, Lang::Ko).unwrap();
        assert!(!d.turns[0].first);
        assert_eq!(d.turns[0].prompt.chars().count(), 301);
    }

    fn one_turn_day(turn: Turn) -> DayDigest {
        let session = SessionDigest { agent: "claude".into(), turns: vec![turn], ..Default::default() };
        let project = ProjectDigest { name: "p".into(), root: "/p".into(), sessions: vec![session], ..Default::default() };
        DayDigest { date: "2026-09-27".into(), projects: vec![project], ..Default::default() }
    }

    #[test]
    fn to_prompt_text_shows_first_prompt_in_full() {
        let long = "가".repeat(400);
        let turn = Turn { id: "t1".into(), prompt: long.clone(), first: true, reply: Some("done".into()), ..Default::default() };
        let text = to_prompt_text(&one_turn_day(turn), 0, Lang::Ko);
        assert!(text.contains(&format!("요청: {long}")));
        assert!(!text.contains("끝난 답: done")); // not notable: the reply stays out
    }

    #[test]
    fn to_prompt_text_first_and_notable_prints_once() {
        let turn = Turn {
            id: "t1".into(), prompt: "fix it".into(), first: true, reply: Some("done".into()),
            tool_errors: 1, errors: vec!["Bash `cargo test`: fail".into()], ..Default::default()
        };
        let text = to_prompt_text(&one_turn_day(turn), 0, Lang::Ko);
        assert_eq!(text.matches("요청: fix it").count(), 1);
        assert!(text.contains("실패: Bash `cargo test`: fail"));
        assert!(text.contains("끝난 답: done"));
    }

    #[test]
    fn claude_usage_skips_zero_placeholder_before_real_usage() {
        // OpenRouter-backed models write a thinking block with all-zero usage first,
        // then the same response id again with the real usage.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let record = |ts: &str, kind: &str, input: u64, output: u64, cached: u64| json!({
            "type": "assistant", "timestamp": ts,
            "message": {"id": "gen-1", "model": "deepseek/deepseek-v4.1-flash", "content": [{"type": kind}],
                        "usage": {"input_tokens": input, "output_tokens": output, "cache_read_input_tokens": cached}}});
        let lines = [
            json!({"type": "user", "timestamp": "2026-09-27T01:00:00Z", "cwd": "/r", "message": {"content": "go"}}),
            record("2026-09-27T01:00:01Z", "thinking", 0, 0, 0),
            record("2026-09-27T01:00:02Z", "text", 21_776, 261, 100),
            record("2026-09-27T01:00:02Z", "tool_use", 21_776, 261, 100),
        ];
        fs::write(&path, lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")).unwrap();
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.usage.tokens, Tokens { input: 21_776, output: 261, cache_read: 100, ..Default::default() });
    }

    fn write_lines(path: &Path, lines: &[Value]) {
        fs::write(path, lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")).unwrap();
    }

    fn claude_reply(ts: &str, id: &str, output: u64) -> Value {
        json!({"type": "assistant", "timestamp": ts,
               "message": {"id": id, "model": "claude-opus-5", "content": [{"type": "text", "text": "ok"}],
                           "usage": {"input_tokens": 10, "output_tokens": output, "cache_read_input_tokens": 100}}})
    }

    #[test]
    fn claude_usage_takes_the_last_record_of_a_streamed_response() {
        // Claude Code writes one response as several records; later ones carry more output.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        write_lines(&path, &[
            json!({"type": "user", "timestamp": "2026-09-27T01:00:00Z", "cwd": "/r", "message": {"content": "go"}}),
            claude_reply("2026-09-27T01:00:01Z", "msg-1", 37),
            claude_reply("2026-09-27T01:00:02Z", "msg-1", 37),
            claude_reply("2026-09-27T01:00:03Z", "msg-1", 211),
        ]);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(d.usage.tokens, Tokens { input: 10, output: 211, cache_read: 100, ..Default::default() });
        assert_eq!(d.turns[0].tokens.output, 211);
    }

    #[test]
    fn claude_response_copied_into_another_session_counts_once() {
        // Continuing a session elsewhere (`continued-in`) or `/branch` copies the
        // conversation, ids and timestamps included, into a new session file.
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.jsonl"), dir.path().join("b.jsonl"));
        let user = json!({"type": "user", "timestamp": "2026-09-27T01:00:00Z", "cwd": "/r", "message": {"content": "go"}});
        write_lines(&a, &[user.clone(), claude_reply("2026-09-27T01:00:01Z", "msg-1", 50)]);
        write_lines(&b, &[user, claude_reply("2026-09-27T01:00:01Z", "msg-1", 50),
                          claude_reply("2026-09-27T02:00:00Z", "msg-2", 7)]);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let mut seen = Default::default();
        let first = read_claude(&a, s, e, 0, &mut seen, &[], Lang::Ko).unwrap();
        let copy = read_claude(&b, s, e, 0, &mut seen, &[], Lang::Ko).unwrap();
        assert_eq!(first.usage.tokens.output, 50);
        assert_eq!(copy.usage.tokens.output, 7, "the copied response is not counted again");
    }

    #[test]
    fn excluded_session_does_not_claim_a_copied_response() {
        // The summarizer's runner (or a folder the user left out) holds a copy that sorts first.
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.jsonl"), dir.path().join("b.jsonl"));
        let user = |cwd: &str| json!({"type": "user", "timestamp": "2026-09-27T01:00:00Z", "cwd": cwd, "message": {"content": "go"}});
        write_lines(&a, &[user("/runner"), claude_reply("2026-09-27T01:00:01Z", "msg-1", 50)]);
        write_lines(&b, &[user("/r"), claude_reply("2026-09-27T01:00:01Z", "msg-1", 50)]);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let mut seen = Default::default();
        let excluded = ["/runner".to_owned()];
        assert!(read_claude(&a, s, e, 0, &mut seen, &excluded, Lang::Ko).is_none());
        assert_eq!(read_claude(&b, s, e, 0, &mut seen, &excluded, Lang::Ko).unwrap().usage.tokens.output, 50);
    }

    #[test]
    fn claude_response_across_midnight_counts_on_the_day_it_started() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        write_lines(&path, &[
            json!({"type": "user", "timestamp": "2026-09-27T23:59:00Z", "cwd": "/r", "message": {"content": "go"}}),
            claude_reply("2026-09-27T23:59:59Z", "msg-1", 37),
            claude_reply("2026-09-28T00:00:02Z", "msg-1", 211),
            json!({"type": "user", "timestamp": "2026-09-28T00:01:00Z", "cwd": "/r", "message": {"content": "next"}}),
        ]);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let day1 = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(day1.usage.tokens.output, 211, "the whole response, read past midnight");
        let (s, e) = crate::time::local_day_window("2026-09-28", 0).unwrap();
        let day2 = read_claude(&path, s, e, 0, &mut Default::default(), &[], Lang::Ko).unwrap();
        assert_eq!(day2.usage.tokens.total(), 0, "not counted again the next day");
    }

    fn codex_tokens(ts: &str, total: (u64, u64, u64), last: (u64, u64, u64)) -> Value {
        let u = |(input, cached, output): (u64, u64, u64)| json!({"input_tokens": input, "cached_input_tokens": cached, "output_tokens": output});
        json!({"timestamp": ts, "type": "event_msg",
               "payload": {"type": "token_count", "info": {"total_token_usage": u(total), "last_token_usage": u(last)}}})
    }

    fn codex_rollout(path: &Path, events: &[Value]) {
        let mut lines = vec![
            json!({"timestamp": "2026-09-27T01:00:00Z", "type": "session_meta", "payload": {"id": "s1", "cwd": "/r"}}),
            json!({"timestamp": "2026-09-27T01:00:00Z", "type": "turn_context", "payload": {"model": "gpt-5.5"}}),
            json!({"timestamp": "2026-09-27T01:00:00Z", "type": "response_item",
                   "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "go"}]}}),
        ];
        lines.extend(events.iter().cloned());
        write_lines(path, &lines);
    }

    #[test]
    fn codex_fork_does_not_count_the_parent_total_it_starts_from() {
        // A forked rollout's first total already holds the parent's usage.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fork.jsonl");
        codex_rollout(&path, &[
            codex_tokens("2026-09-27T01:00:05Z", (18_000_000, 0, 300_000), (200_000, 0, 1_000)),
            codex_tokens("2026-09-27T01:01:00Z", (18_100_000, 0, 302_000), (100_000, 0, 2_000)),
        ]);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_codex(&path, s, e, 0, Lang::Ko).unwrap();
        assert_eq!(d.usage.tokens.total(), 303_000);
    }

    #[test]
    fn codex_total_that_starts_over_still_counts_its_call() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reset.jsonl");
        codex_rollout(&path, &[
            codex_tokens("2026-09-27T01:00:05Z", (1_000, 0, 0), (1_000, 0, 0)),
            codex_tokens("2026-09-27T01:01:00Z", (2_000, 0, 0), (1_000, 0, 0)),
            codex_tokens("2026-09-27T01:02:00Z", (82, 0, 0), (82, 0, 0)),
            codex_tokens("2026-09-27T01:03:00Z", (100, 0, 0), (18, 0, 0)),
        ]);
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        let d = read_codex(&path, s, e, 0, Lang::Ko).unwrap();
        assert_eq!(d.usage.tokens.total(), 2_100);
    }

    #[test]
    fn codex_rollouts_include_sessions_filed_on_earlier_days() {
        // Rollouts are filed by the day they started; a long session keeps writing later.
        let home = tempfile::tempdir().unwrap();
        let old = home.path().join("sessions/2026/09/20");
        let archived = home.path().join("archived_sessions");
        let future = home.path().join("sessions/2026/09/28");
        for d in [&old, &archived, &future] {
            fs::create_dir_all(d).unwrap();
        }
        for f in [old.join("a.jsonl"), archived.join("b.jsonl"), future.join("c.jsonl")] {
            fs::write(f, "{}").unwrap();
        }
        let (s, e) = crate::time::local_day_window("2026-09-27", 0).unwrap();
        // Files written now: all were modified after the day began.
        let found = codex_rollouts_in(&[home.path().to_path_buf()], s.min(crate::time::now_ms()), e, 0);
        let names: Vec<String> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["b.jsonl", "a.jsonl"], "filed before or on the day, not after it");
    }

    #[test]
    fn scratch_sessions_count_in_usage_but_not_as_projects() {
        let session = |cwd: &str, input: u64| {
            let mut s = SessionDigest { agent: "claude".into(), cwd: cwd.into(), marks: vec![1], ..Default::default() };
            s.usage.record("claude-opus-5", &Tokens { input, ..Default::default() }, Some(9));
            s
        };
        let opts = Options { claude_dirs: vec![], excluded: vec!["/r/runner".into()], lang: Lang::Ko };
        let d = assemble("2026-09-27", 0, 1, vec![
            session("/no-such-project-root/app", 100),
            session("/private/tmp/claude-501/x/scratchpad", 20),
            session("/r/runner", 5),
        ], &opts, 0);
        assert!(d.projects.iter().all(|p| !p.root.starts_with("/private/tmp")));
        assert_eq!(d.scratch_usage["claude"].tokens.input, 20);
        assert_eq!(d.agent_usage["claude"].tokens.input, 120);
        assert_eq!(d.project_usage.values().map(|u| u.tokens.input).sum::<u64>(), 100);
        assert_eq!(d.metrics.usage.as_ref().unwrap().tokens.input, 120, "projects plus scratch, not excluded folders");
    }

    #[test]
    fn turns_belong_to_the_repo_they_edited() {
        let turn = |files: &[&str]| Turn { files: files.iter().map(|f| f.to_string()).collect(), ..Default::default() };
        let mut projects = vec![ProjectDigest {
            root: "/r/a".into(),
            sessions: vec![SessionDigest {
                turns: vec![
                    turn(&["/r/b/src/x.rs", "/r/b/y.rs", "/r/a/z.rs"]),
                    turn(&[]),
                    // A tie goes to the session's own project.
                    turn(&["/r/a/1.rs", "/r/b/2.rs"]),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }];
        assign_turn_roots(&mut projects, |dir| ["/r/a", "/r/b"].iter().find(|r| dir.starts_with(*r)).map(|r| r.to_string()));
        let roots: Vec<Option<&str>> = projects[0].sessions[0].turns.iter().map(|t| t.root.as_deref()).collect();
        assert_eq!(roots, [Some("/r/b"), None, Some("/r/a")]);
    }

    #[test]
    fn temp_and_missing_dirs_have_no_repo() {
        let mut cache = HashMap::new();
        assert_eq!(repo_of_dir("/private/tmp/claude-501/x", &mut cache), None);
        assert_eq!(repo_of_dir("/no/such/dir/anywhere", &mut cache), None);
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_owned();
        let ok = Command::new("git").arg("-C").arg(&root).args(["init", "-q"]).output().unwrap().status.success();
        assert!(ok);
        let sub = dir.path().join("src");
        fs::create_dir(&sub).unwrap();
        // tempdir lives under /var/folders, which is_temp_path rejects: check the git lookup directly.
        let real = fs::canonicalize(&root).unwrap().to_str().unwrap().to_owned();
        assert_eq!(project_root(sub.to_str().unwrap()), real);
        assert_eq!(git_root_of(sub.to_str().unwrap(), &mut cache).as_deref(), Some(real.as_str()));
        // A folder that is not in any git repo belongs to none.
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(git_root_of(plain.path().to_str().unwrap(), &mut cache), None);
    }
}
