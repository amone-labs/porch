//! Events in, session state out. Pure: no files, no clock.
//!
//! This is `docs/design/states.md` in code. Change the doc first, then the
//! tests below, then this file.

use crate::event::Event;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Permission,
    Question,
    Failed,
    TurnDone,
    Running,
    Ended,
}

pub const ASK_TOOL: &str = "AskUserQuestion";

/// A request the agent is blocked on until a matching tool event arrives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pending {
    /// `tool_use_id` when the agent sends one; otherwise we match by tool name.
    pub tool_use: Option<String>,
    pub tool: Option<String>,
}

impl Pending {
    fn from_event(e: &Event) -> Self {
        Pending { tool_use: e.tool_use.clone(), tool: e.tool.clone() }
    }

    fn resolved_by(&self, e: &Event) -> bool {
        match (&self.tool_use, &e.tool_use) {
            (Some(a), Some(b)) => a == b,
            // One side lacks an id: fall back to the tool name, and to "any
            // tool event" when neither side names one.
            _ => match (&self.tool, &e.tool) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Session {
    pub agent: String,
    pub session: String,
    pub state: State,
    pub pending: Option<Pending>,
    pub subagents: BTreeSet<String>,
    pub first_event_at: u64,
    pub last_event_at: u64,
    pub cwd: Option<String>,
    pub log: Option<String>,
    pub term_program: Option<String>,
    pub term_pane: Option<String>,
    /// When `state` last changed: how long it has been waiting, or running.
    pub state_since: u64,
    /// When the person last sent a prompt; the current turn began then.
    pub turn_started_at: Option<u64>,
    /// Tool failures since that prompt.
    pub turn_errors: usize,
}

impl Session {
    pub fn key(&self) -> (String, String) {
        (self.agent.clone(), self.session.clone())
    }

    fn new(agent: &str, session: &str, t: u64) -> Self {
        Session {
            agent: agent.to_owned(),
            session: session.to_owned(),
            state: State::TurnDone,
            pending: None,
            subagents: BTreeSet::new(),
            first_event_at: t,
            last_event_at: t,
            cwd: None,
            log: None,
            term_program: None,
            term_pane: None,
            state_since: t,
            turn_started_at: None,
            turn_errors: 0,
        }
    }

    fn set_state(&mut self, to: State, t: u64) {
        if self.state != to {
            self.state = to;
            self.state_since = t;
        }
    }

    /// Screen order (docs/design/states.md, "Screen order"): what needs the user comes first.
    pub fn priority(&self) -> u8 {
        match self.state {
            State::Permission => 0,
            State::Question => 1,
            State::Failed => 2,
            State::Running => 3,
            State::TurnDone => 4,
            State::Ended => 5,
        }
    }

    /// Waiting on the person: the menu bar count and "Waiting on you".
    pub fn needs_you(&self) -> bool {
        matches!(self.state, State::Permission | State::Question | State::Failed)
    }
}

/// What the reducer did with an event.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    Changed,
    /// Known event that carries no state (e.g. Notification).
    Ignored,
    /// Event kind we do not know; counted, never mapped onto a state.
    Unknown,
    /// No session id: cannot be attributed to a session.
    Orphan,
}

pub fn apply(sessions: &mut BTreeMap<(String, String), Session>, e: &Event) -> Applied {
    let Some(sid) = e.session.as_deref() else {
        return Applied::Orphan;
    };
    let s = sessions
        .entry((e.agent.clone(), sid.to_owned()))
        .or_insert_with(|| Session::new(&e.agent, sid, e.t));

    s.last_event_at = s.last_event_at.max(e.t);
    s.first_event_at = s.first_event_at.min(e.t);
    if e.cwd.is_some() {
        s.cwd = e.cwd.clone();
    }
    if e.log.is_some() {
        s.log = e.log.clone();
    }
    if e.term.program.is_some() {
        s.term_program = e.term.program.clone();
    }
    if let Some(p) = e.term.orca_pane.as_ref().or(e.term.tmux_pane.as_ref()).or(e.term.iterm_session.as_ref()) {
        s.term_pane = Some(p.clone());
    }

    match e.event.as_str() {
        "SessionStart" => {
            if s.state == State::Ended {
                s.set_state(State::TurnDone, e.t);
            }
        }
        "UserPromptSubmit" => {
            s.set_state(State::Running, e.t);
            s.pending = None;
            s.turn_started_at = Some(e.t);
            s.turn_errors = 0;
        }
        "PreToolUse" => {
            if e.tool.as_deref() == Some(ASK_TOOL) {
                s.set_state(State::Question, e.t);
                s.pending = Some(Pending::from_event(e));
            } else if s.pending.is_none() {
                s.set_state(State::Running, e.t);
            }
        }
        "PermissionRequest" => {
            s.set_state(State::Permission, e.t);
            s.pending = Some(Pending::from_event(e));
        }
        "PostToolUse" | "PostToolUseFailure" | "PermissionDenied" => {
            if e.event == "PostToolUseFailure" {
                s.turn_errors += 1;
            }
            if s.pending.as_ref().is_none_or(|p| p.resolved_by(e)) {
                s.pending = None;
                s.set_state(State::Running, e.t);
            }
        }
        "Stop" => {
            s.set_state(State::TurnDone, e.t);
            s.pending = None;
        }
        "StopFailure" => {
            s.set_state(State::Failed, e.t);
            s.pending = None;
        }
        "SubagentStart" => {
            if let Some(id) = &e.subagent_id {
                s.subagents.insert(id.clone());
            }
            return Applied::Changed;
        }
        "SubagentStop" => {
            if let Some(id) = &e.subagent_id {
                s.subagents.remove(id);
            }
            return Applied::Changed;
        }
        "PreCompact" | "PostCompact" => {
            if s.pending.is_none() {
                s.set_state(State::Running, e.t);
            }
        }
        "SessionEnd" => {
            s.set_state(State::Ended, e.t);
            s.pending = None;
            s.subagents.clear();
        }
        "Notification" => return Applied::Ignored,
        _ => return Applied::Unknown,
    }
    Applied::Changed
}

/// Status reported by Claude Code's own session registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryStatus {
    Busy,
    WaitingInput,
    WaitingPermission,
    Idle,
}

/// Merge the registry's view into a session. The newer source wins
/// (`docs/design/states.md`, "When two sources disagree").
pub fn merge_registry(s: &mut Session, status: RegistryStatus, status_at: u64) {
    if status_at < s.last_event_at {
        return;
    }
    match status {
        RegistryStatus::Busy => {
            s.set_state(State::Running, status_at);
            s.pending = None;
        }
        RegistryStatus::WaitingInput => s.set_state(State::Question, status_at),
        RegistryStatus::WaitingPermission => s.set_state(State::Permission, status_at),
        RegistryStatus::Idle => {
            if matches!(s.state, State::Running | State::Permission | State::Question) {
                s.set_state(State::TurnDone, status_at);
                s.pending = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(t: u64, event: &str) -> Event {
        Event {
            v: 1,
            t,
            agent: "claude".into(),
            event: event.into(),
            session: Some("s1".into()),
            ..Default::default()
        }
    }

    fn tool(mut e: Event, tool: &str, id: &str) -> Event {
        e.tool = Some(tool.into());
        e.tool_use = Some(id.into());
        e
    }

    #[test]
    fn times_and_errors_of_the_turn() {
        let s = run(&[
            ev(10, "UserPromptSubmit"),
            ev(20, "PreToolUse"),
            ev(30, "PostToolUseFailure"),
            ev(40, "PostToolUseFailure"),
            ev(50, "PermissionRequest"),
        ]);
        assert_eq!((s.state, s.state_since, s.turn_started_at, s.turn_errors), (State::Permission, 50, Some(10), 2));
        // A new prompt starts a new turn: errors reset.
        let s = run(&[ev(10, "UserPromptSubmit"), ev(20, "PostToolUseFailure"), ev(30, "Stop"), ev(40, "UserPromptSubmit")]);
        assert_eq!((s.state_since, s.turn_started_at, s.turn_errors), (40, Some(40), 0));
        // Staying in a state keeps the time it began.
        let s = run(&[ev(10, "UserPromptSubmit"), ev(20, "PreToolUse"), ev(30, "PostToolUse")]);
        assert_eq!((s.state, s.state_since), (State::Running, 10));
    }

    fn run(events: &[Event]) -> Session {
        let mut m = BTreeMap::new();
        for e in events {
            apply(&mut m, e);
        }
        m.into_values().next().unwrap()
    }

    #[test]
    fn prompt_tool_stop() {
        let s = run(&[
            ev(1, "SessionStart"),
            ev(2, "UserPromptSubmit"),
            tool(ev(3, "PreToolUse"), "Bash", "a"),
            tool(ev(4, "PostToolUse"), "Bash", "a"),
        ]);
        assert_eq!(s.state, State::Running);
        let s = run(&[ev(2, "UserPromptSubmit"), ev(5, "Stop")]);
        assert_eq!(s.state, State::TurnDone);
        assert!(!s.needs_you());
    }

    #[test]
    fn new_session_is_idle() {
        let s = run(&[ev(1, "SessionStart")]);
        assert_eq!(s.state, State::TurnDone);
    }

    #[test]
    fn permission_waits_until_its_own_tool_resolves() {
        let s = run(&[
            ev(1, "UserPromptSubmit"),
            tool(ev(2, "PreToolUse"), "Bash", "a"),
            tool(ev(3, "PermissionRequest"), "Bash", "a"),
            // a parallel tool finishing must not clear the prompt
            tool(ev(4, "PostToolUse"), "Read", "b"),
        ]);
        assert_eq!(s.state, State::Permission);
        let s = run(&[
            tool(ev(3, "PermissionRequest"), "Bash", "a"),
            tool(ev(5, "PostToolUse"), "Bash", "a"),
        ]);
        assert_eq!(s.state, State::Running);
        let s = run(&[
            tool(ev(3, "PermissionRequest"), "Bash", "a"),
            tool(ev(5, "PermissionDenied"), "Bash", "a"),
        ]);
        assert_eq!(s.state, State::Running);
    }

    #[test]
    fn permission_without_ids_matches_by_tool_name() {
        let mut req = ev(1, "PermissionRequest");
        req.tool = Some("Bash".into());
        let mut other = ev(2, "PostToolUse");
        other.tool = Some("Read".into());
        assert_eq!(run(&[req.clone(), other]).state, State::Permission);
        let mut same = ev(2, "PostToolUse");
        same.tool = Some("Bash".into());
        assert_eq!(run(&[req, same]).state, State::Running);
    }

    #[test]
    fn ask_user_question_is_a_question() {
        let s = run(&[ev(1, "UserPromptSubmit"), tool(ev(2, "PreToolUse"), ASK_TOOL, "q")]);
        assert_eq!(s.state, State::Question);
        let s = run(&[tool(ev(2, "PreToolUse"), ASK_TOOL, "q"), tool(ev(3, "PostToolUse"), ASK_TOOL, "q")]);
        assert_eq!(s.state, State::Running);
    }

    #[test]
    fn stop_failure_is_failed_and_needs_you() {
        let s = run(&[ev(1, "UserPromptSubmit"), ev(2, "StopFailure")]);
        assert_eq!(s.state, State::Failed);
        assert!(s.needs_you());
    }

    #[test]
    fn next_prompt_runs_again() {
        let s = run(&[ev(1, "Stop"), ev(2, "UserPromptSubmit")]);
        assert_eq!(s.state, State::Running);
    }

    #[test]
    fn session_end_after_stop_is_ended() {
        let s = run(&[ev(1, "Stop"), ev(2, "SessionEnd")]);
        assert_eq!(s.state, State::Ended);
        assert!(!s.needs_you());
    }

    #[test]
    fn needs_you_is_permission_question_or_failed() {
        for (state, want) in [
            (State::Permission, true),
            (State::Question, true),
            (State::Failed, true),
            (State::Running, false),
            (State::TurnDone, false),
            (State::Ended, false),
        ] {
            let mut s = run(&[ev(1, "Stop")]);
            s.state = state;
            assert_eq!(s.needs_you(), want, "{state:?}");
        }
    }

    #[test]
    fn subagents_are_counted_not_stateful() {
        let mut a = ev(2, "SubagentStart");
        a.subagent_id = Some("x".into());
        let s = run(&[ev(1, "Stop"), a.clone()]);
        assert_eq!((s.state, s.subagents.len()), (State::TurnDone, 1));
        let mut b = ev(3, "SubagentStop");
        b.subagent_id = Some("x".into());
        assert!(run(&[ev(1, "Stop"), a, b]).subagents.is_empty());
    }

    #[test]
    fn unknown_and_orphan_events_do_not_change_state() {
        let mut m = BTreeMap::new();
        apply(&mut m, &ev(1, "Stop"));
        assert_eq!(apply(&mut m, &ev(2, "BrandNewHook")), Applied::Unknown);
        let mut no_id = ev(3, "UserPromptSubmit");
        no_id.session = None;
        assert_eq!(apply(&mut m, &no_id), Applied::Orphan);
        assert_eq!(m.values().next().unwrap().state, State::TurnDone);
    }

    #[test]
    fn registry_wins_only_when_newer() {
        let mut s = run(&[ev(10, "Stop")]);
        merge_registry(&mut s, RegistryStatus::Busy, 5);
        assert_eq!(s.state, State::TurnDone);
        merge_registry(&mut s, RegistryStatus::WaitingPermission, 11);
        assert_eq!(s.state, State::Permission);
        merge_registry(&mut s, RegistryStatus::Idle, 12);
        assert_eq!(s.state, State::TurnDone);
    }

    #[test]
    fn priority_order_matches_doc() {
        let order = [State::Permission, State::Question, State::Failed, State::Running, State::TurnDone, State::Ended];
        let p: Vec<u8> = order
            .iter()
            .map(|&state| {
                let mut s = run(&[ev(1, "Stop")]);
                s.state = state;
                s.priority()
            })
            .collect();
        assert!(p.windows(2).all(|w| w[0] < w[1]), "{p:?}");
    }
}
