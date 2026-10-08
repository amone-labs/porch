use std::env;
use std::path::PathBuf;

pub const APP_DIR_NAME: &str = "porch";

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Where exported reports go, as a browser download would.
pub fn downloads_dir() -> PathBuf {
    home().join("Downloads")
}

/// Root of everything porch writes. `PORCH_HOME` overrides it for tests.
pub fn data_dir() -> PathBuf {
    if let Some(p) = env::var_os("PORCH_HOME") {
        return PathBuf::from(p);
    }
    home().join("Library/Application Support").join(APP_DIR_NAME)
}

/// Data dir names this app used before; see [`migrate_legacy_data_dir`].
const LEGACY_DIR_NAMES: &[&str] = &["session-board"];

/// Move a data dir left by an earlier app name to the current one, once.
/// Does nothing when the current dir already exists or `PORCH_HOME` is set.
pub fn migrate_legacy_data_dir() {
    if env::var_os("PORCH_HOME").is_some() {
        return;
    }
    let base = home().join("Library/Application Support");
    let current = base.join(APP_DIR_NAME);
    if current.exists() {
        return;
    }
    if let Some(old) = LEGACY_DIR_NAMES.iter().map(|n| base.join(n)).find(|p| p.is_dir()) {
        let _ = std::fs::rename(old, current);
    }
}

pub fn events_dir() -> PathBuf {
    data_dir().join("events")
}

pub fn events_file(date: &str) -> PathBuf {
    events_dir().join(format!("{date}.jsonl"))
}

/// One path prefix per line; sessions whose cwd starts with one are ignored
/// by the hook before anything is written.
pub fn excluded_file() -> PathBuf {
    data_dir().join("excluded")
}

/// Where `porch install` copies the hook binary, so rebuilding never breaks
/// hooks that are already registered.
pub fn installed_hook_bin() -> PathBuf {
    data_dir().join("bin/porch-hook")
}

pub fn default_claude_dir() -> PathBuf {
    home().join(".claude")
}

/// Every Codex config dir we know how to find: `$CODEX_HOME`, `~/.codex`,
/// and Orca's per-account homes. Only dirs that exist are returned.
pub fn codex_homes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = env::var_os("CODEX_HOME") {
        out.push(PathBuf::from(p));
    }
    out.push(home().join(".codex"));
    let orca = home().join("Library/Application Support/orca/codex-accounts");
    if let Ok(entries) = std::fs::read_dir(&orca) {
        let mut accounts: Vec<_> = entries.flatten().map(|e| e.path().join("home")).collect();
        accounts.sort();
        out.extend(accounts);
    }
    let mut seen = Vec::new();
    out.retain(|p| p.is_dir() && !seen.contains(p) && {
        seen.push(p.clone());
        true
    });
    out
}
