//! Register and remove `porch-hook` in agent config files.
//!
//! These files belong to the user and to other tools (Orca, for one, installs
//! its own hooks), so the rules are strict:
//!
//! - Our entries are recognised only by [`MARKER`] at the end of the command.
//!   Nothing else is ever touched.
//! - An existing entry of ours is updated in place; a missing one is appended
//!   at the end. Never insert in front: Codex keys hook trust by position
//!   (`hooks.json:stop:0:0`), so shifting another tool's hook revokes the
//!   user's approval of it.
//! - We never write Codex trust hashes. The user approves our hook in Codex.
//! - A file that does not parse is left alone and reported as an error.
//! - Before a write, the original is copied aside; the new content is written
//!   to a temp file and renamed over the original.

use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "# porch";
/// Markers this app wrote under earlier names. Entries carrying them are ours
/// too, so a reinstall updates them in place instead of adding duplicates.
pub const LEGACY_MARKERS: &[&str] = &["# session-board"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn from_name(name: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    pub fn events(self) -> &'static [&'static str] {
        match self {
            Agent::Claude => &[
                "SessionStart",
                "SessionEnd",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "PostToolUseFailure",
                "PermissionRequest",
                "PermissionDenied",
                "Notification",
                "Stop",
                "StopFailure",
                "SubagentStart",
                "SubagentStop",
                "PreCompact",
                "PostCompact",
            ],
            Agent::Codex => &[
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PermissionRequest",
                "PostToolUse",
                "SubagentStart",
                "SubagentStop",
                "Stop",
            ],
        }
    }

    /// The config file that holds this agent's hooks inside its config dir.
    pub fn hooks_file(self, config_dir: &Path) -> PathBuf {
        match self {
            Agent::Claude => config_dir.join("settings.json"),
            Agent::Codex => config_dir.join("hooks.json"),
        }
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The command registered for one event. The `-x` guard and `|| true` keep a
/// missing or broken binary from ever surfacing as a hook error in the agent.
pub fn hook_command(bin: &Path, agent: Agent, event: &str) -> String {
    let b = shell_quote(&bin.to_string_lossy());
    format!(
        "[ -x {b} ] && {b} --agent {} --event {event} || true {MARKER}",
        agent.name()
    )
}

fn hook_entry(bin: &Path, agent: Agent, event: &str) -> Value {
    let mut h = Map::new();
    h.insert("type".into(), json!("command"));
    h.insert("command".into(), json!(hook_command(bin, agent, event)));
    if agent == Agent::Claude {
        h.insert("timeout".into(), json!(5));
    }
    json!({ "hooks": [Value::Object(h)] })
}

fn is_ours(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hs| {
            hs.iter().any(|h| {
                h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| {
                        let c = c.trim_end();
                        c.ends_with(MARKER) || LEGACY_MARKERS.iter().any(|m| c.ends_with(m))
                    })
            })
        })
        .unwrap_or(false)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

/// Add or refresh our entry for every event this agent supports.
pub fn install_into(config: &mut Value, agent: Agent, bin: &Path) -> Result<Plan, String> {
    let root = config
        .as_object_mut()
        .ok_or("config root is not a JSON object")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("\"hooks\" is not a JSON object")?;

    let mut plan = Plan::default();
    for &event in agent.events() {
        let want = hook_entry(bin, agent, event);
        let groups = hooks
            .entry(event)
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .ok_or_else(|| format!("hooks.{event} is not an array"))?;
        let ours: Vec<usize> = (0..groups.len()).filter(|&i| is_ours(&groups[i])).collect();
        match ours.split_first() {
            None => {
                groups.push(want);
                plan.added.push(event.to_owned());
            }
            Some((&first, extra)) => {
                if groups[first] != want {
                    groups[first] = want;
                    plan.updated.push(event.to_owned());
                }
                // Duplicates of ours (from a broken earlier run) go; remove
                // from the back so the indices we hold stay valid.
                for &i in extra.iter().rev() {
                    groups.remove(i);
                    plan.removed.push(event.to_owned());
                }
            }
        }
    }
    Ok(plan)
}

/// Remove every entry of ours, under any event name.
pub fn uninstall_from(config: &mut Value) -> Result<Plan, String> {
    let mut plan = Plan::default();
    let Some(hooks) = config
        .as_object_mut()
        .and_then(|r| r.get_mut("hooks"))
        .and_then(Value::as_object_mut)
    else {
        return Ok(plan);
    };
    let mut emptied = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else { continue };
        let before = groups.len();
        groups.retain(|g| !is_ours(g));
        for _ in groups.len()..before {
            plan.removed.push(event.clone());
        }
        if before > 0 && groups.is_empty() {
            emptied.push(event.clone());
        }
    }
    // An event list that held only our entry did not exist before we came.
    for e in emptied {
        hooks.shift_remove(&e);
    }
    Ok(plan)
}

pub enum Action<'a> {
    Install { agent: Agent, bin: &'a Path },
    Uninstall,
}

/// Apply an action to one hooks file on disk. With `dry_run` nothing is
/// written; the returned plan says what would change.
pub fn apply_to_file(path: &Path, action: Action, dry_run: bool) -> io::Result<Plan> {
    let existed = path.exists();
    let original = if existed { fs::read_to_string(path)? } else { String::new() };
    let mut config: Value = if original.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(&original).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is not valid JSON, left untouched: {e}", path.display()),
            )
        })?
    };

    let plan = match action {
        Action::Install { agent, bin } => install_into(&mut config, agent, bin),
        Action::Uninstall => uninstall_from(&mut config),
    }
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display())))?;

    if plan.is_empty() || dry_run {
        return Ok(plan);
    }

    write_settings(path, &original, &config)?;
    Ok(plan)
}

/// Write `config` over `path` the way every porch edit does: a timestamped
/// backup of the old file, a temp file, then a rename, keeping permissions and
/// the old file's trailing newline.
pub fn write_settings(path: &Path, original: &str, config: &Value) -> io::Result<()> {
    let existed = path.exists();
    let mut out = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
    if original.is_empty() || original.ends_with('\n') {
        out.push('\n');
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if existed {
        let backup = path.with_file_name(format!(
            "{file_name}.porch-backup-{}",
            crate::time::now_ms()
        ));
        fs::copy(path, &backup)?;
    } else if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_file_name(format!(".{file_name}.porch-tmp"));
    fs::write(&tmp, out)?;
    if existed {
        fs::set_permissions(&tmp, fs::metadata(path)?.permissions())?;
    }
    fs::rename(&tmp, path)
}

/// Copy a hook binary to the stable location hooks point at
/// ([`crate::paths::installed_hook_bin`]). Written to a temp name and renamed,
/// because agents may be executing the old binary at that moment. Returns
/// whether the file changed.
pub fn stage_hook_binary(src: &Path) -> io::Result<bool> {
    let dest = crate::paths::installed_hook_bin();
    if fs::read(src)? == fs::read(&dest).unwrap_or_default() {
        return Ok(false);
    }
    if let Some(d) = dest.parent() {
        fs::create_dir_all(d)?;
    }
    let tmp = dest.with_file_name("porch-hook.tmp");
    fs::copy(src, &tmp)?;
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, &dest)?;
    Ok(true)
}

/// Every hooks file we manage on this machine: Claude Code's settings and
/// each Codex home.
pub fn default_targets() -> Vec<(Agent, PathBuf)> {
    let mut out = vec![(Agent::Claude, Agent::Claude.hooks_file(&crate::paths::default_claude_dir()))];
    out.extend(crate::paths::codex_homes().iter().map(|d| (Agent::Codex, Agent::Codex.hooks_file(d))));
    out
}

/// `default_targets()` limited to one agent, or all of them for `None`.
pub fn targets_for(agent: Option<Agent>) -> Vec<(Agent, PathBuf)> {
    default_targets().into_iter().filter(|(a, _)| agent.is_none_or(|want| *a == want)).collect()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TargetStatus {
    pub agent: &'static str,
    pub file: String,
    /// "installed" | "missing" | "partial" | "error"
    pub state: String,
    pub detail: Option<String>,
}

/// Why a hooks file is only partly ours, in `lang`.
fn partial_detail(added: usize, updated: usize, lang: crate::lang::Lang) -> String {
    match lang {
        crate::lang::Lang::Ko => format!("빠진 사건 {added}, 바뀐 항목 {updated}"),
        crate::lang::Lang::En => format!("{added} events missing, {updated} entries changed"),
    }
}

/// Where our hooks stand in each file, computed with a dry run.
pub fn status(bin: &Path) -> Vec<TargetStatus> {
    let lang = crate::lang::Lang::current();
    default_targets()
        .into_iter()
        .map(|(agent, file)| {
            let (state, detail) = match apply_to_file(&file, Action::Install { agent, bin }, true) {
                Ok(p) if p.is_empty() => ("installed".to_owned(), None),
                Ok(p) if p.added.len() == agent.events().len() => ("missing".to_owned(), None),
                Ok(p) => ("partial".to_owned(), Some(partial_detail(p.added.len(), p.updated.len(), lang))),
                Err(e) => ("error".to_owned(), Some(e.to_string())),
            };
            TargetStatus { agent: agent.name(), file: file.to_string_lossy().into_owned(), state, detail }
        })
        .collect()
}

/// One agent's hooks files and whether the agent is on this Mac at all.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    pub agent: &'static str,
    pub present: bool,
    pub targets: Vec<TargetStatus>,
}

/// `status` grouped by agent, both agents always listed.
pub fn agents_status(bin: &Path) -> Vec<AgentStatus> {
    group(status(bin), |a| match a {
        Agent::Claude => crate::paths::default_claude_dir().is_dir(),
        Agent::Codex => !crate::paths::codex_homes().is_empty(),
    })
}

fn group(statuses: Vec<TargetStatus>, present: impl Fn(Agent) -> bool) -> Vec<AgentStatus> {
    Agent::ALL
        .into_iter()
        .map(|a| AgentStatus {
            agent: a.name(),
            present: present(a),
            targets: statuses.iter().filter(|s| s.agent == a.name()).cloned().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin() -> PathBuf {
        PathBuf::from("/Users/me/Library/Application Support/porch/bin/porch-hook")
    }

    fn with_foreign_hooks() -> Value {
        json!({
            "model": "opus",
            "hooks": {
                "Stop": [
                    {"hooks": [{"type": "command", "command": "/opt/other-tool/stop"}]},
                    {"matcher": "*", "hooks": [{"type": "command", "command": "orca-hook.sh", "timeout": "10"}]}
                ]
            },
            "zzz_last_key": true
        })
    }

    #[test]
    fn command_is_quoted_guarded_and_marked() {
        let c = hook_command(&bin(), Agent::Claude, "Stop");
        assert!(c.starts_with("[ -x '/Users/me/Library/Application Support/porch/bin/porch-hook' ]"));
        assert!(c.contains("--agent claude --event Stop"));
        assert!(c.ends_with(MARKER));
    }

    #[test]
    fn install_appends_after_foreign_hooks_and_keeps_them() {
        let mut c = with_foreign_hooks();
        let plan = install_into(&mut c, Agent::Claude, &bin()).unwrap();
        assert_eq!(plan.added.len(), Agent::Claude.events().len());
        let stop = c["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 3);
        assert_eq!(stop[0]["hooks"][0]["command"], "/opt/other-tool/stop");
        assert_eq!(stop[1]["hooks"][0]["command"], "orca-hook.sh");
        assert!(is_ours(&stop[2]));
        assert_eq!(c["model"], "opus");
        // key order preserved
        let keys: Vec<_> = c.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["model", "hooks", "zzz_last_key"]);
    }

    #[test]
    fn install_is_idempotent_and_keeps_position() {
        let mut c = with_foreign_hooks();
        install_into(&mut c, Agent::Codex, &bin()).unwrap();
        // another tool appends after us
        c["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(json!({"hooks": [{"type": "command", "command": "later-tool"}]}));
        let before = c.clone();
        let plan = install_into(&mut c, Agent::Codex, &bin()).unwrap();
        assert!(plan.is_empty(), "{plan:?}");
        assert_eq!(c, before);
    }

    #[test]
    fn changed_binary_path_updates_in_place() {
        let mut c = with_foreign_hooks();
        install_into(&mut c, Agent::Codex, &bin()).unwrap();
        let plan = install_into(&mut c, Agent::Codex, Path::new("/other/porch-hook")).unwrap();
        assert_eq!(plan.updated.len(), Agent::Codex.events().len());
        let stop = c["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 3);
        assert!(stop[2]["hooks"][0]["command"].as_str().unwrap().contains("/other/porch-hook"));
    }

    #[test]
    fn uninstall_removes_only_ours_and_drops_lists_we_created() {
        let original = with_foreign_hooks();
        let mut c = original.clone();
        install_into(&mut c, Agent::Claude, &bin()).unwrap();
        let plan = uninstall_from(&mut c).unwrap();
        assert_eq!(plan.removed.len(), Agent::Claude.events().len());
        assert_eq!(c, original);
    }

    #[test]
    fn pre_rename_entries_are_replaced_in_place() {
        let mut c = with_foreign_hooks();
        c["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(json!({"hooks": [{"type": "command", "command": "old-hook --agent claude || true # session-board"}]}));
        install_into(&mut c, Agent::Claude, &bin()).unwrap();
        let stop = c["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 3, "legacy entry must be updated, not duplicated");
        assert!(stop[2]["hooks"][0]["command"].as_str().unwrap().ends_with(MARKER));
        let mut u = c.clone();
        uninstall_from(&mut u).unwrap();
        assert_eq!(u["hooks"]["Stop"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn codex_entries_have_no_timeout() {
        let mut c = json!({});
        install_into(&mut c, Agent::Codex, &bin()).unwrap();
        assert!(c["hooks"]["Stop"][0]["hooks"][0].get("timeout").is_none());
        assert!(c["hooks"].get("StopFailure").is_none());
    }

    #[test]
    fn file_round_trip_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = serde_json::to_string_pretty(&with_foreign_hooks()).unwrap() + "\n";
        fs::write(&path, &original).unwrap();

        let dry = apply_to_file(&path, Action::Install { agent: Agent::Claude, bin: &bin() }, true).unwrap();
        assert!(!dry.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), original, "dry run wrote");

        apply_to_file(&path, Action::Install { agent: Agent::Claude, bin: &bin() }, false).unwrap();
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("porch-backup"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), original);

        apply_to_file(&path, Action::Uninstall, false).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn invalid_json_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let err = apply_to_file(&path, Action::Install { agent: Agent::Claude, bin: &bin() }, false);
        assert!(err.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn missing_file_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("home/hooks.json");
        apply_to_file(&path, Action::Install { agent: Agent::Codex, bin: &bin() }, false).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(is_ours(&v["hooks"]["Stop"][0]));
    }

    #[test]
    fn from_name_round_trips_and_rejects_unknown() {
        for a in Agent::ALL {
            assert_eq!(Agent::from_name(a.name()), Some(a));
        }
        assert_eq!(Agent::from_name("gemini"), None);
        assert_eq!(Agent::from_name(""), None);
    }

    #[test]
    fn targets_for_one_agent_only() {
        let claude = targets_for(Some(Agent::Claude));
        assert_eq!(claude.len(), 1);
        assert!(claude.iter().all(|(a, _)| *a == Agent::Claude));
        assert!(targets_for(Some(Agent::Codex)).iter().all(|(a, _)| *a == Agent::Codex));
        assert_eq!(targets_for(None).len(), default_targets().len());
    }

    fn target(agent: &'static str, file: &str, state: &str) -> TargetStatus {
        TargetStatus { agent, file: file.into(), state: state.into(), detail: None }
    }

    #[test]
    fn group_puts_every_target_under_its_agent() {
        let statuses = vec![
            target("claude", "/h/.claude/settings.json", "installed"),
            target("codex", "/h/.codex/hooks.json", "installed"),
            target("codex", "/orca/a/home/hooks.json", "missing"),
        ];
        let g = group(statuses, |a| a == Agent::Claude);
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].agent, g[0].present, g[0].targets.len()), ("claude", true, 1));
        assert_eq!((g[1].agent, g[1].present, g[1].targets.len()), ("codex", false, 2));
    }

    #[test]
    fn agents_status_always_lists_both() {
        let g = group(Vec::new(), |_| false);
        assert_eq!(g.iter().map(|s| s.agent).collect::<Vec<_>>(), ["claude", "codex"]);
        assert!(g.iter().all(|s| !s.present && s.targets.is_empty()));
    }

    #[test]
    fn partial_detail_in_both_languages() {
        assert_eq!(partial_detail(2, 1, crate::lang::Lang::Ko), "빠진 사건 2, 바뀐 항목 1");
        assert_eq!(partial_detail(2, 1, crate::lang::Lang::En), "2 events missing, 1 entries changed");
    }
}
