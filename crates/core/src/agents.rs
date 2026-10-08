//! Find the agent CLIs on this machine.
//!
//! An app opened from Finder, the Dock or at login gets launchd's minimal
//! PATH (`/usr/bin:/bin:/usr/sbin:/sbin`), not the one from the user's
//! shell, so `Command::new("claude")` fails there even when it works from a
//! terminal. Look in the places installers use, then ask the login shell.

use std::path::{Path, PathBuf};
use std::process::Command;

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(|d| Path::new(d).join(name))
        .find(|p| is_executable(p))
}

/// Ask the user's login shell, the way a terminal would resolve the name.
/// Interactive (`-i`) so PATH set in `.zshrc` counts; stray output from rc
/// files is ignored by taking the last absolute path printed.
fn from_login_shell(name: &str) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let out = Command::new(shell).args(["-lic", &format!("command -v {name}")]).output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| l.starts_with('/'))
        .map(PathBuf::from)
        .filter(|p| is_executable(p))
}

/// The login shell's PATH, asked once. Empty when the shell says nothing usable.
fn login_path() -> &'static str {
    static PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let Ok(out) = Command::new(shell).args(["-lic", "printf '\\nPORCH_PATH=%s\\n' \"$PATH\""]).output() else {
            return String::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .rev()
            .find_map(|l| l.strip_prefix("PORCH_PATH="))
            .unwrap_or_default()
            .to_owned()
    })
}

/// PATH for running an agent CLI. Launched from Finder the app has launchd's
/// minimal PATH, and a CLI that is a script (`codex` starts with
/// `#!/usr/bin/env node`) then cannot find its interpreter. The CLI's own
/// folder (where installers put `node` beside it) comes first, then the login
/// shell's PATH, then the app's.
pub fn child_path(bin: &Path) -> String {
    join_paths(bin.parent(), login_path(), &std::env::var("PATH").unwrap_or_default())
}

fn join_paths(bin_dir: Option<&Path>, login: &str, current: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    bin_dir
        .map(|d| d.to_string_lossy().into_owned())
        .into_iter()
        .chain(login.split(':').chain(current.split(':')).map(str::to_owned))
        .filter(|d| !d.is_empty() && seen.insert(d.clone()))
        .collect::<Vec<_>>()
        .join(":")
}

/// `override_var` first, then PATH, then where installers put it, then the login shell.
fn find_bin(name: &str, override_var: &str, extra: &[PathBuf]) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(override_var).map(PathBuf::from).filter(|p| is_executable(p)) {
        return Some(p);
    }
    if let Some(p) = on_path(name) {
        return Some(p);
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let known = [
        home.join(".local/bin").join(name),
        PathBuf::from("/opt/homebrew/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
        home.join(".npm-global/bin").join(name),
        home.join(".bun/bin").join(name),
    ];
    extra.iter().cloned().chain(known).find(|p| is_executable(p)).or_else(|| from_login_shell(name))
}

/// Where the `claude` CLI is, or None when it is not installed (or not found).
/// `PORCH_CLAUDE_BIN` overrides the search.
pub fn claude_bin() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    find_bin("claude", "PORCH_CLAUDE_BIN", &[home.join(".claude/local/claude")])
}

/// Where the `codex` CLI is, or None. `PORCH_CODEX_BIN` overrides the search.
pub fn codex_bin() -> Option<PathBuf> {
    find_bin("codex", "PORCH_CODEX_BIN", &[])
}

/// Where Orca's `orca` CLI is, or None. `PORCH_ORCA_BIN` overrides the search;
/// the app bundle's copy is tried before the login shell.
pub fn orca_bin() -> Option<PathBuf> {
    find_bin("orca", "PORCH_ORCA_BIN", &[PathBuf::from("/Applications/Orca.app/Contents/Resources/bin/orca")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_when_executable() {
        // /bin/sh always exists and is executable on macOS
        std::env::set_var("PORCH_CLAUDE_BIN", "/bin/sh");
        assert_eq!(claude_bin(), Some(PathBuf::from("/bin/sh")));
        std::env::remove_var("PORCH_CLAUDE_BIN");
    }

    #[test]
    fn child_path_puts_the_cli_folder_first_without_repeats() {
        let p = join_paths(Some(Path::new("/u/.local/bin")), "/opt/homebrew/bin:/u/.local/bin:/usr/bin", "/usr/bin:/bin");
        assert_eq!(p, "/u/.local/bin:/opt/homebrew/bin:/usr/bin:/bin");
        assert_eq!(join_paths(None, "", "/usr/bin"), "/usr/bin");
    }

    #[test]
    fn non_executables_are_skipped() {
        assert!(!is_executable(Path::new("/etc/hosts")));
        assert!(is_executable(Path::new("/bin/sh")));
    }
}
