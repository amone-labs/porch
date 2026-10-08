//! Bring a session's terminal tab to the front (ADR 0013). Orca is the one
//! terminal porch can target by tab today: its hook environment carries
//! `ORCA_PANE_KEY` (`<tabId>:<leafId>`), and `orca terminal list --json`
//! names the terminal handle for that pair. Other terminals are not focusable.

use crate::agents;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Orca's macOS bundle id: `terminal switch` picks the tab inside Orca, this brings Orca itself forward.
const ORCA_BUNDLE_ID: &str = "com.stablyai.orca";
/// How long one `orca` call may take. It answers in about 0.1 s; a runtime that
/// stops answering must not leave the click hanging with no message.
const ORCA_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Orca's `ORCA_PANE_KEY`: `<tabId>:<leafId>`.
    OrcaPane(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusError {
    /// The `orca` CLI is not on this Mac.
    NoOrca,
    /// `orca` ran but could not list or switch (Orca not running, runtime unreachable).
    NotRunning,
    /// Orca has no live terminal with this session's tab and leaf.
    TabGone,
}

impl FocusError {
    pub fn code(&self) -> &'static str {
        match self {
            FocusError::NoOrca => "no_orca",
            FocusError::NotRunning => "not_running",
            FocusError::TabGone => "tab_gone",
        }
    }
}

/// Where a session's terminal can be brought to the front, from what its hook reported.
pub fn target(term_program: Option<&str>, term_pane: Option<&str>) -> Option<Target> {
    match (term_program, term_pane) {
        (Some("Orca"), Some(p)) if p.contains(':') => Some(Target::OrcaPane(p.to_owned())),
        _ => None,
    }
}

/// The handle of the live terminal for an Orca pane key, from `orca terminal list --json`.
pub fn orca_handle(list_json: &str, pane: &str) -> Option<String> {
    let (tab, leaf) = pane.split_once(':')?;
    let v: serde_json::Value = serde_json::from_str(list_json).ok()?;
    v["result"]["terminals"]
        .as_array()?
        .iter()
        .find(|t| t["tabId"] == tab && t["leafId"] == leaf && t["connected"] != false)
        .and_then(|t| t["handle"].as_str())
        .map(str::to_owned)
}

/// Switch Orca to the session's tab and bring Orca in front.
pub fn bring_to_front(t: &Target) -> Result<(), FocusError> {
    match t {
        Target::OrcaPane(pane) => {
            let bin = agents::orca_bin().ok_or(FocusError::NoOrca)?;
            let path = agents::child_path(&bin);
            let mut list = Command::new(&bin);
            list.args(["terminal", "list", "--json"]).env("PATH", &path);
            let out = output_within(list, ORCA_WAIT).filter(|o| o.status.success()).ok_or(FocusError::NotRunning)?;
            let handle = orca_handle(&String::from_utf8_lossy(&out.stdout), pane).ok_or(FocusError::TabGone)?;
            let mut switch = Command::new(&bin);
            switch.args(["terminal", "switch", "--terminal", &handle]).env("PATH", &path);
            if !output_within(switch, ORCA_WAIT).is_some_and(|o| o.status.success()) {
                return Err(FocusError::NotRunning);
            }
            let _ = Command::new("/usr/bin/open").args(["-b", ORCA_BUNDLE_ID]).output();
            Ok(())
        }
    }
}

/// Run `cmd`, killing it if it has not finished within `limit`. Stdout is read
/// on its own thread so a full pipe cannot stall the child.
fn output_within(mut cmd: Command, limit: Duration) -> Option<Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut pipe = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(Output { status, stdout: reader.join().unwrap_or_default(), stderr: Vec::new() }),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"{"ok":true,"result":{"terminals":[
        {"handle":"term_old","tabId":"tab1","leafId":"leaf1","connected":false},
        {"handle":"term_a","tabId":"tab1","leafId":"leaf1","connected":true},
        {"handle":"term_b","tabId":"tab1","leafId":"leaf2","connected":true}
    ]}}"#;

    #[test]
    fn only_orca_panes_are_targets() {
        assert_eq!(target(Some("Orca"), Some("tab1:leaf1")), Some(Target::OrcaPane("tab1:leaf1".into())));
        assert_eq!(target(Some("Orca"), None), None);
        assert_eq!(target(Some("Orca"), Some("%3")), None);
        assert_eq!(target(Some("tmux"), Some("%3")), None);
        assert_eq!(target(Some("iTerm.app"), Some("w0t0p0:ABC")), None);
        assert_eq!(target(None, Some("tab1:leaf1")), None);
    }

    #[test]
    fn handle_is_the_connected_terminal_of_that_tab_and_leaf() {
        assert_eq!(orca_handle(LIST, "tab1:leaf1").as_deref(), Some("term_a"));
        assert_eq!(orca_handle(LIST, "tab1:leaf2").as_deref(), Some("term_b"));
    }

    #[test]
    fn a_closed_tab_or_bad_output_has_no_handle() {
        assert_eq!(orca_handle(LIST, "tab9:leaf9"), None);
        assert_eq!(orca_handle(r#"{"ok":false,"error":"runtime not reachable"}"#, "tab1:leaf1"), None);
        assert_eq!(orca_handle("not json", "tab1:leaf1"), None);
        assert_eq!(orca_handle(LIST, "no-colon"), None);
    }

    #[test]
    fn a_hung_orca_gives_up_instead_of_waiting_forever() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("orca");
        std::fs::write(&bin, "#!/bin/sh\nsleep 8\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Only this test sets PORCH_ORCA_BIN.
        std::env::set_var("PORCH_ORCA_BIN", &bin);
        let started = std::time::Instant::now();
        let got = bring_to_front(&Target::OrcaPane("tab1:leaf1".into()));
        std::env::remove_var("PORCH_ORCA_BIN");
        assert_eq!(got, Err(FocusError::NotRunning));
        assert!(started.elapsed() < std::time::Duration::from_secs(6), "{:?}", started.elapsed());
    }

    #[test]
    fn error_codes_are_stable() {
        // The app maps these to its own words (app/src/screens/Now.tsx).
        assert_eq!(FocusError::NoOrca.code(), "no_orca");
        assert_eq!(FocusError::NotRunning.code(), "not_running");
        assert_eq!(FocusError::TabGone.code(), "tab_gone");
    }
}
