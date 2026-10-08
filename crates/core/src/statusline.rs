//! Claude's usage limits, through Claude Code's status line.
//!
//! Claude Code hands its status line command a JSON snapshot on stdin after
//! each response: the model, how full the context window is, and for Pro and
//! Max plans the 5-hour and 7-day limits. It is the one place those limits
//! appear, so porch takes the status line slot and passes everything through:
//!
//! - With another status line already set (Orca, a plugin, a script), porch
//!   keeps it: it records the snapshot, then runs the original command with the
//!   same input and prints what that prints.
//! - With none, porch prints its own short line, or nothing if the person
//!   turned that off.
//!
//! Turning it off restores the original setting exactly. If something else
//! later replaces the setting, `status` reports it as disconnected.

use crate::lang::Lang;
use crate::{install, paths, time};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MARKER: &str = "# porch statusline";
/// Sessions not heard from for this long are dropped from the record.
const KEEP_SESSIONS_MS: u64 = 2 * 24 * 60 * 60 * 1000;

/// What porch remembers about the setting it replaced.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Saved {
    pub enabled: bool,
    /// The `statusLine` object that was there before, or null for none.
    #[serde(default)]
    pub original: Value,
    /// With no original, print porch's own line (else print nothing).
    #[serde(default = "yes")]
    pub show_line: bool,
    /// Projects whose own status line porch wraps, by project folder.
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectWrap>,
}

/// A project with its own status line. Project settings beat the user's, so
/// porch wraps it in the project's `settings.local.json` (personal, highest
/// priority) and leaves the shared `settings.json` alone.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectWrap {
    /// The project's status line porch runs after recording.
    pub original: Value,
    /// What `settings.local.json` held under `statusLine` before (null for nothing).
    #[serde(default)]
    pub local_before: Value,
    /// porch created `settings.local.json`; remove it again if nothing else is left in it.
    #[serde(default)]
    pub created_file: bool,
    /// porch added the file to the repo's local exclude list (`.git/info/exclude`).
    #[serde(default)]
    pub excluded: bool,
}

fn yes() -> bool {
    true
}

fn saved_file() -> PathBuf {
    paths::data_dir().join("statusline.json")
}

fn live_file() -> PathBuf {
    paths::data_dir().join("claude-live.json")
}

pub fn load_saved() -> Saved {
    fs::read_to_string(saved_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Saved { show_line: true, ..Default::default() })
}

fn store_saved(s: &Saved) -> Result<(), String> {
    let p = saved_file();
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    fs::write(&p, serde_json::to_string_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// A usage window: share used and when it starts over.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub used_percent: f64,
    /// Unix ms.
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    /// Behind a Claude apps gateway with a spend limit.
    pub spend: Option<Window>,
    /// When Claude Code last reported them (unix ms).
    pub at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionLive {
    pub model: Option<String>,
    pub context_percent: Option<f64>,
    pub at: u64,
}

/// The latest snapshot per session, and the latest limits.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Live {
    pub limits: Option<Limits>,
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionLive>,
}

pub fn load_live() -> Live {
    fs::read_to_string(live_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn window(v: &Value) -> Option<Window> {
    let used = v["used_percentage"].as_f64()?;
    Some(Window { used_percent: used, resets_at: v["resets_at"].as_u64().map(|s| s * 1000) })
}

/// Fold one status line snapshot into the record.
pub fn record(live: &mut Live, input: &Value, now: u64) {
    let rl = &input["rate_limits"];
    if rl.is_object() {
        let l = Limits { five_hour: window(&rl["five_hour"]), seven_day: window(&rl["seven_day"]), spend: window(&rl["spend_limit"]), at: now };
        if l.five_hour.is_some() || l.seven_day.is_some() || l.spend.is_some() {
            live.limits = Some(l);
        }
    }
    if let Some(sid) = input["session_id"].as_str() {
        live.sessions.insert(
            sid.to_owned(),
            SessionLive {
                model: input["model"]["id"].as_str().or(input["model"]["display_name"].as_str()).map(str::to_owned),
                context_percent: input["context_window"]["used_percentage"].as_f64(),
                at: now,
            },
        );
    }
    live.sessions.retain(|_, s| now.saturating_sub(s.at) < KEEP_SESSIONS_MS);
}

/// Called by `porch-hook statusline`: record, then what to print. Writes are
/// best effort; the status line must never fail because of porch.
pub fn handle(raw: &[u8], project: Option<&str>) -> Option<String> {
    let input: Value = serde_json::from_slice(raw).unwrap_or(Value::Null);
    let mut live = load_live();
    record(&mut live, &input, time::now_ms());
    if let Ok(text) = serde_json::to_string(&live) {
        let path = live_file();
        if let Some(d) = path.parent() {
            let _ = fs::create_dir_all(d);
        }
        let tmp = path.with_extension(format!("json.{}-{}.tmp", std::process::id(), crate::time::now_ms()));
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
    if original_command(project).is_some() {
        return None; // the caller runs the original command and prints its output
    }
    let saved = load_saved();
    (project.is_none() && saved.show_line).then(|| own_line(&input, live.limits.as_ref(), Lang::current()))
}

/// porch's line for someone with no status line of their own: model, context, limits.
pub fn own_line(input: &Value, limits: Option<&Limits>, lang: Lang) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(m) = input["model"]["id"].as_str() {
        parts.push(crate::usage::short_model(m));
    } else if let Some(m) = input["model"]["display_name"].as_str() {
        parts.push(m.to_owned());
    }
    if let Some(p) = input["context_window"]["used_percentage"].as_f64() {
        parts.push(match lang {
            Lang::Ko => format!("창 {}%", p.round()),
            Lang::En => format!("context {}%", p.round()),
        });
    }
    if let Some(l) = limits {
        if let Some(w) = l.five_hour {
            parts.push(match lang {
                Lang::Ko => format!("5시간 {}%", w.used_percent.round()),
                Lang::En => format!("5h {}%", w.used_percent.round()),
            });
        }
        if let Some(w) = l.seven_day {
            parts.push(match lang {
                Lang::Ko => format!("주간 {}%", w.used_percent.round()),
                Lang::En => format!("weekly {}%", w.used_percent.round()),
            });
        }
    }
    parts.join(" · ")
}

/// The original command to run after recording: the project's own when
/// called from a project wrap, else the user's.
pub fn original_command(project: Option<&str>) -> Option<String> {
    let saved = load_saved();
    let original = match project {
        Some(root) => saved.projects.get(root).map(|w| w.original.clone()).unwrap_or(Value::Null),
        None => saved.original,
    };
    original["command"].as_str().map(str::to_owned)
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub fn our_command(bin: &Path) -> String {
    let b = quote(&bin.to_string_lossy());
    format!("[ -x {b} ] && {b} statusline || true {MARKER}")
}

fn project_command(bin: &Path, root: &str) -> String {
    let b = quote(&bin.to_string_lossy());
    format!("[ -x {b} ] && {b} statusline --project {} || true {MARKER}", quote(root))
}

fn is_ours(v: &Value) -> bool {
    v["command"].as_str().is_some_and(|c| c.contains(MARKER))
}

fn settings_path() -> PathBuf {
    paths::default_claude_dir().join("settings.json")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Off,
    On,
    /// Turned on, but something has since replaced the setting.
    Disconnected,
}

pub fn status() -> Status {
    let saved = load_saved();
    if !saved.enabled {
        return Status::Off;
    }
    let cfg: Value = fs::read_to_string(settings_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    if is_ours(&cfg["statusLine"]) {
        Status::On
    } else {
        Status::Disconnected
    }
}

/// Put porch in the status line slot, keeping whatever was there to run after it.
pub fn enable(bin: &Path, show_line: bool) -> Result<(), String> {
    enable_in(&settings_path(), bin, show_line)
}

pub fn enable_in(path: &Path, bin: &Path, show_line: bool) -> Result<(), String> {
    let original_text = fs::read_to_string(path).unwrap_or_default();
    let mut cfg: Value = if original_text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&original_text).map_err(|e| format!("{} is not valid JSON, left untouched: {e}", path.display()))?
    };
    let current = cfg["statusLine"].clone();
    let mut saved = load_saved();
    if !is_ours(&current) {
        saved.original = current.clone();
    }
    saved.enabled = true;
    saved.show_line = show_line;
    // Keep the original's other settings (padding, refresh interval) so it behaves the same.
    let mut ours = if current.is_object() && !is_ours(&current) { current } else { saved.original.clone() };
    if !ours.is_object() {
        ours = json!({});
    }
    ours["type"] = json!("command");
    ours["command"] = json!(our_command(bin));
    cfg["statusLine"] = ours;
    store_saved(&saved)?;
    install::write_settings(path, &original_text, &cfg).map_err(|e| e.to_string())
}

/// Give the status line back as it was. If something else has taken the slot
/// since, leave the setting alone.
pub fn disable() -> Result<(), String> {
    disable_in(&settings_path())
}

pub fn disable_in(path: &Path) -> Result<(), String> {
    let mut saved = load_saved();
    let original_text = fs::read_to_string(path).unwrap_or_default();
    let mut cfg: Value = if original_text.trim().is_empty() { json!({}) } else { serde_json::from_str(&original_text).map_err(|e| e.to_string())? };
    if is_ours(&cfg["statusLine"]) {
        match &saved.original {
            Value::Null => {
                if let Some(o) = cfg.as_object_mut() {
                    o.remove("statusLine");
                }
            }
            v => cfg["statusLine"] = v.clone(),
        }
        install::write_settings(path, &original_text, &cfg).map_err(|e| e.to_string())?;
    }
    saved.enabled = false;
    saved.original = Value::Null;
    store_saved(&saved)?;
    for root in load_saved().projects.keys().cloned().collect::<Vec<_>>() {
        unwrap_project(&root)?;
    }
    Ok(())
}

// ---------- projects with their own status line ----------

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// A project folder whose settings set a status line of its own.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectLine {
    pub root: String,
    /// The command it runs (shortened for display by the app).
    pub command: String,
    /// porch is wrapped around it and still in place.
    pub wrapped: bool,
    /// porch wrapped it, but its settings no longer point at porch.
    pub disconnected: bool,
}

/// Folders Claude Code ran in lately, from porch's own session records.
fn recent_roots(now: u64) -> Vec<String> {
    let mut counters = crate::board::Counters::default();
    let mut roots: Vec<String> = crate::board::read_events(30, now, &mut counters).into_iter().filter(|e| e.agent == "claude").filter_map(|e| e.cwd).collect();
    roots.sort();
    roots.dedup();
    roots
}

/// Projects with their own status line: they outrank the user setting, so
/// sessions there never reach porch unless each is wrapped too.
pub fn project_lines(now: u64) -> Vec<ProjectLine> {
    let saved = load_saved();
    let mut out: Vec<ProjectLine> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for root in recent_roots(now).into_iter().chain(saved.projects.keys().cloned()) {
        if !seen.insert(root.clone()) {
            continue;
        }
        let dir = Path::new(&root).join(".claude");
        let local = read_json(&dir.join("settings.local.json")).map(|v| v["statusLine"].clone()).unwrap_or(Value::Null);
        let shared = read_json(&dir.join("settings.json")).map(|v| v["statusLine"].clone()).unwrap_or(Value::Null);
        let known = saved.projects.get(&root);
        if is_ours(&local) {
            let command = known.and_then(|w| w.original["command"].as_str()).unwrap_or("").to_owned();
            out.push(ProjectLine { root, command, wrapped: true, disconnected: false });
        } else if let Some(cmd) = local["command"].as_str().or(shared["command"].as_str()) {
            out.push(ProjectLine { root, command: cmd.to_owned(), wrapped: false, disconnected: known.is_some() });
        }
    }
    out
}

/// Write a project settings file whole (temp file, rename). No backup copy:
/// it would sit in the project folder, and porch keeps what it replaced.
fn write_project_file(path: &Path, cfg: &Value) -> Result<(), String> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("json.porch-{}.tmp", time::now_ms()));
    fs::write(&tmp, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn git(root: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

const LOCAL_ENTRY: &str = ".claude/settings.local.json";

/// Keep the local settings file out of the project's git status without
/// touching its shared ignore files: `.git/info/exclude` stays on this Mac.
fn exclude_local(root: &str) -> bool {
    let Some(top) = git(root, &["rev-parse", "--show-toplevel"]) else { return false };
    if std::process::Command::new("git").arg("-C").arg(root).args(["check-ignore", "-q", ".claude/settings.local.json"]).status().is_ok_and(|s| s.success()) {
        return false;
    }
    let Some(path) = git(root, &["rev-parse", "--git-path", "info/exclude"]) else { return false };
    let path = if Path::new(&path).is_absolute() { PathBuf::from(path) } else { Path::new(root).join(path) };
    let rel = Path::new(root).join(LOCAL_ENTRY);
    let entry = rel.strip_prefix(&top).map(|p| format!("/{}", p.to_string_lossy())).unwrap_or_else(|_| format!("/{LOCAL_ENTRY}"));
    let mut text = fs::read_to_string(&path).unwrap_or_default();
    if text.lines().any(|l| l.trim() == entry) {
        return false;
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    // git reads '#' as a comment only at the start of a line: the marker gets its own line.
    text.push_str(&format!("{MARKER}\n{entry}\n"));
    if let Some(d) = path.parent() {
        let _ = fs::create_dir_all(d);
    }
    fs::write(&path, text).is_ok()
}

fn unexclude_local(root: &str) {
    let Some(path) = git(root, &["rev-parse", "--git-path", "info/exclude"]) else { return };
    let path = if Path::new(&path).is_absolute() { PathBuf::from(path) } else { Path::new(root).join(path) };
    if let Ok(text) = fs::read_to_string(&path) {
        // Drop each marker line and the entry right after it.
        let mut kept: Vec<&str> = Vec::new();
        let mut skip_next = false;
        for l in text.lines() {
            if skip_next {
                skip_next = false;
                if l.ends_with(LOCAL_ENTRY) {
                    continue;
                }
            }
            if l.trim() == MARKER {
                skip_next = true;
                continue;
            }
            kept.push(l);
        }
        let _ = fs::write(&path, kept.join("\n") + if kept.is_empty() { "" } else { "\n" });
    }
}

/// Wrap one project's own status line.
pub fn wrap_project(root: &str, bin: &Path) -> Result<(), String> {
    let dir = Path::new(root).join(".claude");
    let local_path = dir.join("settings.local.json");
    let existed = local_path.exists();
    let mut local = if existed { read_json(&local_path).ok_or_else(|| format!("{} is not valid JSON, left untouched", local_path.display()))? } else { json!({}) };
    let local_before = local["statusLine"].clone();
    if is_ours(&local_before) {
        return Ok(());
    }
    let shared = read_json(&dir.join("settings.json")).map(|v| v["statusLine"].clone()).unwrap_or(Value::Null);
    let original = if local_before["command"].is_string() { local_before.clone() } else { shared };
    if !original["command"].is_string() {
        return Err(format!("{root} has no status line of its own"));
    }
    let mut ours = original.clone();
    ours["type"] = json!("command");
    ours["command"] = json!(project_command(bin, root));
    local["statusLine"] = ours;
    let excluded = exclude_local(root);
    let mut saved = load_saved();
    saved.projects.insert(root.to_owned(), ProjectWrap { original, local_before, created_file: !existed, excluded });
    store_saved(&saved)?;
    write_project_file(&local_path, &local)
}

/// Put a project's settings back as they were.
pub fn unwrap_project(root: &str) -> Result<(), String> {
    let mut saved = load_saved();
    let Some(w) = saved.projects.remove(root) else { return Ok(()) };
    let local_path = Path::new(root).join(".claude").join("settings.local.json");
    if let Some(mut local) = read_json(&local_path) {
        if is_ours(&local["statusLine"]) {
            match &w.local_before {
                Value::Null => {
                    if let Some(o) = local.as_object_mut() {
                        o.remove("statusLine");
                    }
                }
                v => local["statusLine"] = v.clone(),
            }
            if w.created_file && local.as_object().is_some_and(|o| o.is_empty()) {
                let _ = fs::remove_file(&local_path);
            } else {
                write_project_file(&local_path, &local)?;
            }
        }
    }
    if w.excluded {
        unexclude_local(root);
    }
    store_saved(&saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keeps_limits_and_session_context() {
        let mut live = Live::default();
        record(
            &mut live,
            &json!({
                "session_id": "s1",
                "model": {"id": "claude-opus-5-5", "display_name": "Opus 5.5"},
                "context_window": {"used_percentage": 42.4},
                "rate_limits": {"five_hour": {"used_percentage": 23.5, "resets_at": 1738425600}, "seven_day": {"used_percentage": 41.2, "resets_at": 1738857600}}
            }),
            1_000,
        );
        let l = live.limits.clone().unwrap();
        assert_eq!(l.five_hour.unwrap().resets_at, Some(1_738_425_600_000));
        assert_eq!(live.sessions["s1"].context_percent, Some(42.4));
        // A snapshot without limits (API key user, or before the first reply) keeps the last ones.
        record(&mut live, &json!({"session_id": "s2"}), 2_000);
        assert_eq!(live.limits.unwrap().at, 1_000);
        assert_eq!(own_line(&json!({"model": {"id": "claude-opus-5-5"}, "context_window": {"used_percentage": 42.4}}), Some(&l), crate::lang::Lang::Ko), "opus 5.5 · 창 42% · 5시간 24% · 주간 41%");
    }

    #[test]
    fn own_line_in_english() {
        let l = Limits {
            five_hour: Some(Window { used_percent: 23.5, resets_at: None }),
            seven_day: Some(Window { used_percent: 41.2, resets_at: None }),
            ..Default::default()
        };
        assert_eq!(
            own_line(&json!({"model": {"id": "claude-opus-5-5"}, "context_window": {"used_percentage": 42.4}}), Some(&l), crate::lang::Lang::En),
            "opus 5.5 · context 42% · 5h 24% · weekly 41%"
        );
    }

    #[test]
    fn our_command_is_marked() {
        let c = our_command(Path::new("/x/porch-hook"));
        assert!(is_ours(&json!({"command": c})));
        assert!(!is_ours(&json!({"command": "orca-statusline.sh"})));
    }
}
