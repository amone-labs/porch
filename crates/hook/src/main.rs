//! `porch-hook --agent <claude|codex> --event <Name>`
//! `porch-hook statusline`
//!
//! Called by the agent on every hook event with the payload on stdin.
//! Appends one metadata-only line to today's event file and exits.
//!
//! As Claude Code's status line (`statusline`), it records the snapshot on
//! stdin (model, context, usage limits) and then prints the line: the original
//! status line command's output when there was one, else porch's own.
//!
//! Contract with the agent: print nothing, exit 0 on every path, finish fast.
//! A failure here must never become a failure in the user's session.

use porch_core::event::{Event, Term};
use porch_core::{paths, time};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

/// Payloads carry tool output; never buffer more than this.
const MAX_STDIN: u64 = 8 * 1024 * 1024;

fn main() {
    let _ = std::panic::catch_unwind(run);
    std::process::exit(0);
}

fn run() {
    if std::env::args().nth(1).as_deref() == Some("statusline") {
        statusline();
        return;
    }
    let (agent, event_name) = parse_args(std::env::args().skip(1));
    let Some(agent) = agent else { return };

    let mut raw = Vec::new();
    let _ = io::stdin().take(MAX_STDIN).read_to_end(&mut raw);
    let payload = serde_json::from_slice(&raw).unwrap_or(serde_json::Value::Null);

    let t = time::now_ms();
    let term = Term::from_env(|k| std::env::var(k).ok());
    let event = Event::from_payload(
        &agent,
        event_name.as_deref().unwrap_or("Unknown"),
        &payload,
        t,
        Some(std::os::unix::process::parent_id()),
        term,
    );

    if event.cwd.as_deref().is_some_and(is_excluded) {
        return;
    }
    let _ = append(&paths::events_file(&time::utc_date(t)), &event.to_line());
}

fn statusline() {
    let mut raw = Vec::new();
    let _ = io::stdin().take(MAX_STDIN).read_to_end(&mut raw);
    // `statusline --project <folder>` when wrapping a project's own status line.
    let args: Vec<String> = std::env::args().collect();
    let project = args.iter().position(|a| a == "--project").and_then(|i| args.get(i + 1)).map(String::as_str);
    let own = porch_core::statusline::handle(&raw, project);
    if let Some(cmd) = porch_core::statusline::original_command(project) {
        // Run what was there before with the same input, and show its output as is.
        if let Ok(mut child) = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(&raw);
            }
            let _ = child.wait();
        }
    } else if let Some(line) = own {
        let _ = io::stdout().write_all(line.as_bytes());
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> (Option<String>, Option<String>) {
    let (mut agent, mut event) = (None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--agent" => agent = args.next(),
            "--event" => event = args.next(),
            _ => {}
        }
    }
    (agent, event)
}

fn is_excluded(cwd: &str) -> bool {
    let Ok(list) = fs::read_to_string(paths::excluded_file()) else {
        return false;
    };
    list.lines()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .any(|p| cwd == p || cwd.starts_with(&format!("{}/", p.trim_end_matches('/'))))
}

/// One `write` on an `O_APPEND` file per line, so concurrent hooks append
/// whole lines. POSIX does not promise this for regular files; the reader
/// skips any line that does not parse and counts it.
fn append(path: &Path, line: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(line.as_bytes())
}
