//! The one line `porch-hook` appends per hook call.
//!
//! Only metadata crosses into this format. Prompts, tool input, tool output
//! and notification text never do; `from_payload` copies an explicit
//! allowlist of short scalar fields and nothing else.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT_VERSION: u32 = 1;

/// Longest string we copy from a payload. Longer values are dropped, not
/// truncated, so a half path never looks like a real one.
const MAX_FIELD_LEN: usize = 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    /// unix ms when the hook ran
    pub t: u64,
    /// "claude" | "codex"
    pub agent: String,
    /// hook event name, e.g. "PermissionRequest"
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub log: Option<String>,
    /// Pairs a PermissionRequest / PreToolUse with the PostToolUse that
    /// resolves it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_use: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub turn: Option<String>,
    /// Notification subtype when the agent sends one (never the message).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subagent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subagent_type: Option<String>,
    /// SessionStart source (startup/resume/...) or SessionEnd reason
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub why: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ppid: Option<u32>,
    #[serde(skip_serializing_if = "Term::is_empty", default)]
    pub term: Term,
    /// Top-level key names of the raw payload, sorted. Names only, never
    /// values: this is how we learn an agent's hook format without storing
    /// its content.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Term {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub program: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub orca_pane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tmux_pane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub iterm_session: Option<String>,
}

impl Term {
    pub fn is_empty(&self) -> bool {
        self == &Term::default()
    }

    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        Term {
            program: get("TERM_PROGRAM"),
            orca_pane: get("ORCA_PANE_KEY"),
            tmux_pane: get("TMUX_PANE"),
            iterm_session: get("ITERM_SESSION_ID"),
        }
        .clean()
    }

    fn clean(mut self) -> Self {
        for f in [
            &mut self.program,
            &mut self.orca_pane,
            &mut self.tmux_pane,
            &mut self.iterm_session,
        ] {
            *f = f.take().filter(|s| !s.is_empty() && s.len() <= MAX_FIELD_LEN);
        }
        self
    }
}

fn short_str(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| {
        payload
            .get(*k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= MAX_FIELD_LEN)
            .map(str::to_owned)
    })
}

impl Event {
    /// Build an event from a hook payload.
    ///
    /// `fallback_event` is the event name the installer baked into the hook
    /// command; the payload's own `hook_event_name` wins when present.
    pub fn from_payload(
        agent: &str,
        fallback_event: &str,
        payload: &Value,
        t: u64,
        ppid: Option<u32>,
        term: Term,
    ) -> Event {
        let mut keys: Vec<String> = payload
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        Event {
            v: FORMAT_VERSION,
            t,
            agent: agent.to_owned(),
            event: short_str(payload, &["hook_event_name"])
                .unwrap_or_else(|| fallback_event.to_owned()),
            session: short_str(payload, &["session_id", "thread_id"]),
            tool: short_str(payload, &["tool_name"]),
            cwd: short_str(payload, &["cwd"]),
            log: short_str(payload, &["transcript_path", "rollout_path"]),
            tool_use: short_str(payload, &["tool_use_id", "call_id"]),
            turn: short_str(payload, &["turn_id"]),
            note: short_str(payload, &["notification_type"]),
            subagent_id: short_str(payload, &["agent_id"]),
            subagent_type: short_str(payload, &["agent_type"]),
            why: short_str(payload, &["source", "reason"]),
            ppid,
            term,
            keys,
        }
    }

    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap_or_default();
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn claude_permission_payload() -> Value {
        json!({
            "session_id": "abc",
            "transcript_path": "/Users/me/.claude/projects/x/abc.jsonl",
            "cwd": "/Users/me/repo",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf secret-dir"},
            "prompt": "please do the secret thing",
            "message": "Claude needs your permission"
        })
    }

    #[test]
    fn copies_metadata_only() {
        let e = Event::from_payload(
            "claude",
            "Stop",
            &claude_permission_payload(),
            1,
            Some(7),
            Term::default(),
        );
        assert_eq!(e.event, "PermissionRequest");
        assert_eq!(e.session.as_deref(), Some("abc"));
        assert_eq!(e.tool.as_deref(), Some("Bash"));
        let line = e.to_line();
        assert!(!line.contains("secret"), "content leaked: {line}");
        assert!(!line.contains("permission\""), "message leaked: {line}");
        // key names are kept so we can learn the format, values are not
        assert!(e.keys.contains(&"tool_input".to_owned()));
    }

    #[test]
    fn falls_back_to_installed_event_name() {
        let e = Event::from_payload("codex", "Stop", &json!({"thread_id": "t1"}), 1, None, Term::default());
        assert_eq!(e.event, "Stop");
        assert_eq!(e.session.as_deref(), Some("t1"));
    }

    #[test]
    fn drops_oversized_fields() {
        let long = "x".repeat(MAX_FIELD_LEN + 1);
        let e = Event::from_payload("claude", "Stop", &json!({"cwd": long}), 1, None, Term::default());
        assert_eq!(e.cwd, None);
    }

    #[test]
    fn one_line_round_trips() {
        let e = Event::from_payload("claude", "Stop", &claude_permission_payload(), 5, None, Term::default());
        let line = e.to_line();
        assert_eq!(line.matches('\n').count(), 1);
        let back: Event = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(back, e);
    }
}
