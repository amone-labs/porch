//! Weekly insight, the observed part: replay a week of hook events through the
//! state rules (`state::apply`, docs/design/states.md) and count where the
//! agents' time went. No model here; the model-written evaluation is ADR 0012.

use crate::event::Event;
use crate::lang::Lang;
use crate::state::{self, State};
use crate::writer::Provider;
use crate::{digest, goals, insight_eval, paths, settings, summary, time};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// A waiting stretch counts up to this; a running gap longer than this counts as none.
pub const CAP_MS: u64 = digest::IDLE_GAP_MS;
/// Where sessions outside any git repo go. The name is empty on purpose: the
/// screen and the CLI write the label ("기타" / "Other").
pub const OTHER: &str = "";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DayObs {
    pub date: String,
    pub running_ms: u64,
    pub turn_done_ms: u64,
    pub question_ms: u64,
    pub failed_ms: u64,
    pub permission_ms: u64,
    pub requests: usize,
    pub stop_failures: usize,
    pub tool_calls: usize,
    pub tool_errors: BTreeMap<String, usize>,
    pub permission_requests: usize,
}

impl DayObs {
    pub fn waiting_ms(&self) -> u64 {
        self.turn_done_ms + self.question_ms + self.failed_ms + self.permission_ms
    }

    pub fn tool_error_total(&self) -> usize {
        self.tool_errors.values().sum()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Concurrency {
    pub one_ms: u64,
    pub two_ms: u64,
    pub three_plus_ms: u64,
    /// One session stopped while another ran.
    pub overlap_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectTime {
    pub name: String,
    pub running_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Observed {
    /// Monday, local calendar.
    pub week_start: String,
    /// Last moment counted: the week's end, or now while the week runs.
    pub through: u64,
    pub week_end: u64,
    /// Seven days, Monday first; days without records are zero.
    pub days: Vec<DayObs>,
    /// Stopped time by the local hour its stretch began; 24 entries.
    pub hours_waiting_ms: Vec<u64>,
    pub concurrency: Concurrency,
    /// Running time by project, largest first.
    pub projects: Vec<ProjectTime>,
    pub requests: usize,
    pub sessions: usize,
    /// The first day porch has records for, when that falls after this week's Monday.
    pub recorded_from: Option<String>,
}

impl Observed {
    pub fn running_ms(&self) -> u64 {
        self.days.iter().map(|d| d.running_ms).sum()
    }
    pub fn waiting_ms(&self) -> u64 {
        self.days.iter().map(DayObs::waiting_ms).sum()
    }
    pub fn failed_ms(&self) -> u64 {
        self.days.iter().map(|d| d.failed_ms).sum()
    }
    pub fn stop_failures(&self) -> usize {
        self.days.iter().map(|d| d.stop_failures).sum()
    }
    pub fn tool_calls(&self) -> usize {
        self.days.iter().map(|d| d.tool_calls).sum()
    }
    pub fn tool_error_total(&self) -> usize {
        self.days.iter().map(DayObs::tool_error_total).sum()
    }
}

/// What `observe` needs besides the events. Folder lookups come in as functions
/// so the replay stays pure.
pub struct Context<'a> {
    pub week_start: &'a str,
    pub offset: i64,
    pub now: u64,
    pub excluded: &'a dyn Fn(&str) -> bool,
    pub project: &'a dyn Fn(&str) -> Option<String>,
}

/// Next Monday 00:00 local.
pub fn week_end(week_start: &str, offset: i64) -> u64 {
    time::add_days(week_start, 7).and_then(|d| time::local_day_window(&d, offset)).map(|(s, _)| s).unwrap_or(0)
}

pub fn observe(events: &[Event], cx: &Context) -> Observed {
    let start = time::local_day_window(cx.week_start, cx.offset).map(|(s, _)| s).unwrap_or(0);
    let end = week_end(cx.week_start, cx.offset).max(start);
    let through = cx.now.clamp(start, end);
    let dates: Vec<String> = (0..7).filter_map(|i| time::add_days(cx.week_start, i)).collect();
    let mut o = Observed {
        week_start: cx.week_start.to_owned(),
        through,
        week_end: end,
        days: dates.iter().map(|d| DayObs { date: d.clone(), ..Default::default() }).collect(),
        hours_waiting_ms: vec![0; 24],
        ..Default::default()
    };

    let mut by_session: BTreeMap<(String, String), Vec<&Event>> = BTreeMap::new();
    for e in events.iter().filter(|e| e.t >= start && e.t < through) {
        if let Some(sid) = &e.session {
            by_session.entry((e.agent.clone(), sid.clone())).or_default().push(e);
        }
    }

    let mut spans: Vec<(u64, u64, bool)> = Vec::new();
    let mut by_project: BTreeMap<String, u64> = BTreeMap::new();
    for (key, mut evs) in by_session {
        evs.sort_by_key(|e| e.t);
        // A session with nothing but start and end events is a probe, not work
        // (Orca's rate-limit checks: SessionStart, then SessionEnd seconds later).
        if evs.iter().all(|e| e.event == "SessionStart" || e.event == "SessionEnd") {
            continue;
        }
        let cwd = evs.iter().find_map(|e| e.cwd.clone());
        if cwd.as_deref().is_some_and(|c| (cx.excluded)(c)) {
            continue;
        }
        o.sessions += 1;
        let project = cwd.as_deref().and_then(|c| (cx.project)(c)).unwrap_or_else(|| OTHER.to_owned());
        let mut replay = BTreeMap::new();
        for (i, e) in evs.iter().enumerate() {
            count(&mut o, e, cx.offset);
            state::apply(&mut replay, e);
            let Some(next) = evs.get(i + 1) else { break };
            let st = replay.get(&key).map(|s| s.state).unwrap_or(State::Ended);
            let since = replay.get(&key).map(|s| s.state_since).unwrap_or(e.t);
            let gap = next.t - e.t;
            let len = match st {
                State::Running if gap > CAP_MS => 0,
                State::Running => gap,
                State::Ended => 0,
                // The cap is once per wait stretch, not once per event gap:
                // events that keep the state (Notification, SubagentStop,
                // unknown kinds) must not restart it.
                _ => gap.min((since + CAP_MS).saturating_sub(e.t)),
            };
            let Some(day) = dates.iter().position(|d| *d == time::local_date(e.t, cx.offset)) else { continue };
            if len == 0 {
                continue;
            }
            let d = &mut o.days[day];
            match st {
                State::Running => {
                    d.running_ms += len;
                    *by_project.entry(project.clone()).or_default() += len;
                }
                State::TurnDone => d.turn_done_ms += len,
                State::Question => d.question_ms += len,
                State::Failed => d.failed_ms += len,
                State::Permission => d.permission_ms += len,
                State::Ended => {}
            }
            if st != State::Running {
                o.hours_waiting_ms[time::local_hour(e.t, cx.offset) as usize] += len;
            }
            spans.push((e.t, e.t + len, st == State::Running));
        }
    }
    o.concurrency = concurrency(&spans);
    let mut projects: Vec<ProjectTime> = by_project.into_iter().map(|(name, running_ms)| ProjectTime { name, running_ms }).collect();
    projects.sort_by(|a, b| b.running_ms.cmp(&a.running_ms).then_with(|| a.name.cmp(&b.name)));
    o.projects = projects;
    o.requests = o.days.iter().map(|d| d.requests).sum();
    o
}

fn count(o: &mut Observed, e: &Event, offset: i64) {
    let date = time::local_date(e.t, offset);
    let Some(d) = o.days.iter_mut().find(|d| d.date == date) else { return };
    match e.event.as_str() {
        "UserPromptSubmit" => d.requests += 1,
        "StopFailure" => d.stop_failures += 1,
        "PostToolUse" | "PermissionDenied" => d.tool_calls += 1,
        "PostToolUseFailure" => {
            d.tool_calls += 1;
            // No tool name to show; "?" is the same in both languages.
            *d.tool_errors.entry(e.tool.clone().unwrap_or_else(|| "?".into())).or_default() += 1;
        }
        "PermissionRequest" if e.tool.as_deref() != Some(state::ASK_TOOL) => d.permission_requests += 1,
        _ => {}
    }
}

/// Lay every counted stretch on one time axis: how long 1, 2, 3+ sessions were
/// going at once, and how long one stood stopped while another ran.
fn concurrency(spans: &[(u64, u64, bool)]) -> Concurrency {
    let mut points: Vec<(u64, i32, bool)> = spans.iter().flat_map(|&(s, e, running)| [(s, 1, running), (e, -1, running)]).collect();
    // At the same instant, ends before starts: back-to-back stretches do not overlap.
    points.sort_by_key(|p| (p.0, p.1));
    let (mut running, mut stopped) = (0i32, 0i32);
    let mut c = Concurrency::default();
    let mut last: Option<u64> = None;
    for (t, step, is_running) in points {
        if let Some(l) = last {
            let len = t - l;
            match running + stopped {
                0 => {}
                1 => c.one_ms += len,
                2 => c.two_ms += len,
                _ => c.three_plus_ms += len,
            }
            if running > 0 && stopped > 0 {
                c.overlap_ms += len;
            }
        }
        if is_running {
            running += step;
        } else {
            stopped += step;
        }
        last = Some(t);
    }
    c
}

// ---------- checkpoints ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesPoint {
    pub label: String,
    pub value: f64,
    pub tip: String,
}

/// One thing in the week worth a look, picked by rule from the observed numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// "day_focus" | "tool_streak" | "hour_band"
    pub kind: String,
    pub title: String,
    /// The one line it gets under Highlights.
    pub line: String,
    pub details: Vec<String>,
    /// How much of its total the checkpoint covers, 0–1; the order.
    pub share: f64,
    /// The day to open, for a checkpoint about one day.
    pub date: Option<String>,
    /// What a next-week goal on it would count.
    pub goal_key: String,
    pub unit: String,
    pub series: Vec<SeriesPoint>,
    pub highlight: Vec<usize>,
}

/// "59분" / "59 min", then "1.3시간" / "1.3 h".
pub fn hm(ms: u64, lang: Lang) -> String {
    let min = (ms + 30_000) / 60_000;
    // Tenths of an hour, rounded half-up so the core and the screen (JS toFixed)
    // read 1.25 h the same way.
    let tenths = (ms + 180_000) / 360_000;
    match lang {
        Lang::Ko if min < 60 => format!("{min}분"),
        Lang::Ko => format!("{}.{}시간", tenths / 10, tenths % 10),
        Lang::En if min < 60 => format!("{min} min"),
        Lang::En => format!("{}.{} h", tenths / 10, tenths % 10),
    }
}

/// "2026-09-29" -> "9월 29일 (화)" (the year dropped).
fn day_label(date: &str) -> String {
    let long = time::long_date_ko(date);
    long.split_once("년 ").map(|(_, rest)| rest.to_owned()).unwrap_or(long)
}

fn day_tick(date: &str) -> String {
    date.rsplit('-').next().map(|d| d.trim_start_matches('0').to_owned()).unwrap_or_default()
}

/// Up to three, the largest share first.
pub fn checkpoints(o: &Observed, lang: Lang) -> Vec<Checkpoint> {
    let mut out: Vec<Checkpoint> = [day_focus(o, lang), tool_streak(o, lang), hour_band(o, lang)].into_iter().flatten().collect();
    out.sort_by(|a, b| b.share.total_cmp(&a.share));
    out.truncate(3);
    out
}

fn day_focus(o: &Observed, lang: Lang) -> Option<Checkpoint> {
    let total = o.stop_failures();
    let (top, day) = o.days.iter().enumerate().fold(None::<(usize, &DayObs)>, |best, (i, d)| match best {
        Some((_, b)) if b.stop_failures >= d.stop_failures => best,
        _ => Some((i, d)),
    })?;
    let n = day.stop_failures;
    if n < 5 || n * 2 < total {
        return None;
    }
    let when = time::short_date(&day.date, lang);
    let failed = hm(o.failed_ms(), lang);
    let (title, line, details, unit) = match lang {
        Lang::Ko => (
            format!("오류로 멈춤, {when}에 집중"),
            format!("오류로 멈춘 차례 {total}회({failed}) · {when}에 {n}회 집중"),
            vec![format!("{when}에 전체 {total}회 중 {n}회 집중"), "인사이트 집계용 훅 기록에는 오류 문구 미포함".into()],
            "회".to_owned(),
        ),
        Lang::En => (
            format!("Stopped on error on {when}"),
            format!("{total} turns stopped on error ({failed}), {n} of them on {when}"),
            vec![format!("{n} of {total} on {when}"), "Hook records used for Insights do not include error text".into()],
            " times".to_owned(),
        ),
    };
    Some(Checkpoint {
        kind: "day_focus".into(),
        title,
        line,
        details,
        share: n as f64 / total as f64,
        date: Some(day.date.clone()),
        goal_key: "stop_failures".into(),
        unit,
        series: o
            .days
            .iter()
            .map(|d| SeriesPoint {
                label: day_tick(&d.date),
                value: d.stop_failures as f64,
                tip: match lang {
                    Lang::Ko => format!("{} · 오류로 멈춤 {}회 · {}", day_label(&d.date), d.stop_failures, hm(d.failed_ms, lang)),
                    Lang::En => format!("{} · Stopped on error: {} · {}", time::short_date(&d.date, lang), d.stop_failures, hm(d.failed_ms, lang)),
                },
            })
            .collect(),
        highlight: vec![top],
    })
}

/// Longest run of `true`: (length, first index).
fn longest_run(flags: impl Iterator<Item = bool>) -> (usize, usize) {
    let (mut best, mut best_at, mut run, mut run_at) = (0, 0, 0, 0);
    for (i, f) in flags.enumerate() {
        if !f {
            run = 0;
            continue;
        }
        if run == 0 {
            run_at = i;
        }
        run += 1;
        if run > best {
            (best, best_at) = (run, run_at);
        }
    }
    (best, best_at)
}

fn tool_streak(o: &Observed, lang: Lang) -> Option<Checkpoint> {
    let all = o.tool_error_total();
    let mut tools: BTreeMap<&str, usize> = BTreeMap::new();
    for d in &o.days {
        for (k, v) in &d.tool_errors {
            *tools.entry(k).or_default() += v;
        }
    }
    let mut best: Option<(&str, usize, usize, usize)> = None;
    for (&tool, &n) in &tools {
        if n == 0 || n * 2 < all {
            continue;
        }
        let (run, at) = longest_run(tool_counts(o, tool).into_iter().map(|c| c > 0));
        if run >= 3 && best.is_none_or(|b| n > b.1) {
            best = Some((tool, n, run, at));
        }
    }
    let (tool, n, run, at) = best?;
    let from = time::short_date(&o.days[at].date, lang);
    let counts = tool_counts(o, tool);
    let peak = counts.iter().enumerate().fold(0, |p, (i, &c)| if c > counts[p] { i } else { p });
    let (title, line, details, unit) = match lang {
        Lang::Ko => (
            format!("{tool} 오류 {run}일 연속"),
            format!("도구 오류 {all}건 중 {tool} {n}건, {from}부터 {run}일 연속"),
            vec![
                format!("도구 오류 {all}건 중 {n}건"),
                "같은 명령의 반복 실패 여부는 프로젝트 점검에서 확인 가능(최근 30일, 저장된 하루 요약 기준)".into(),
            ],
            "건".to_owned(),
        ),
        Lang::En => (
            format!("{tool} errors on {run} consecutive days"),
            format!("{tool} accounted for {n} of {all} tool errors this week, with errors on {run} consecutive days starting {from}"),
            vec![
                format!("{n} of {all} tool errors"),
                "Check Projects for commands that failed repeatedly in saved daily summaries from the last 30 days".into(),
            ],
            if counts[peak] == 1 { " error" } else { " errors" }.to_owned(),
        ),
    };
    Some(Checkpoint {
        kind: "tool_streak".into(),
        title,
        line,
        details,
        share: n as f64 / all as f64,
        date: None,
        goal_key: format!("tool_errors:{tool}"),
        unit,
        series: o
            .days
            .iter()
            .zip(&counts)
            .map(|(d, &c)| SeriesPoint {
                label: day_tick(&d.date),
                value: c as f64,
                tip: match lang {
                    Lang::Ko => format!("{} · {tool} 오류 {c}건 · 그날 도구 오류 {}건", day_label(&d.date), d.tool_error_total()),
                    Lang::En => format!(
                        "{} · {c} {tool} {} · {} {} that day",
                        time::short_date(&d.date, lang),
                        if c == 1 { "error" } else { "errors" },
                        d.tool_error_total(),
                        if d.tool_error_total() == 1 { "tool error" } else { "tool errors" }
                    ),
                },
            })
            .collect(),
        highlight: vec![peak],
    })
}

/// Errors of one tool, day by day.
fn tool_counts(o: &Observed, tool: &str) -> Vec<usize> {
    o.days.iter().map(|d| d.tool_errors.get(tool).copied().unwrap_or(0)).collect()
}

/// Hour bands no wider than this count as "concentrated".
const MAX_BAND_HOURS: usize = 4;

fn hour_band(o: &Observed, lang: Lang) -> Option<Checkpoint> {
    let h = &o.hours_waiting_ms;
    let total: u64 = h.iter().sum();
    let max = h.iter().copied().max().filter(|m| *m > 0)?;
    let dense = |i: usize| h[i] > 0 && h[i] * 10 >= max * 7;
    // (first hour, last hour, stopped ms)
    let mut bands: Vec<(usize, usize, u64)> = Vec::new();
    let mut i = 0;
    while i < h.len() {
        if !dense(i) {
            i += 1;
            continue;
        }
        let first = i;
        let mut sum = 0;
        while i < h.len() && dense(i) {
            sum += h[i];
            i += 1;
        }
        if i - first <= MAX_BAND_HOURS {
            bands.push((first, i - 1, sum));
        }
    }
    bands.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    bands.truncate(2);
    let top: u64 = bands.iter().map(|b| b.2).sum();
    if bands.is_empty() || top * 10 < total * 3 {
        return None;
    }
    bands.sort_by_key(|b| b.0);
    let name = |b: &(usize, usize, u64)| match lang {
        Lang::Ko => format!("{}시–{}시", b.0, b.1 + 1),
        Lang::En => format!("{}–{}h", b.0, b.1 + 1),
    };
    let joined = bands.iter().map(name).collect::<Vec<_>>().join(match lang {
        Lang::Ko => "와 ",
        Lang::En => " and ",
    });
    let parts = bands.iter().map(|b| format!("{} {}", name(b), hm(b.2, lang))).collect::<Vec<_>>().join(", ");
    let (title, line, details, unit) = match lang {
        Lang::Ko => (
            format!("멈춰 있던 시간, {joined}에 집중"),
            format!("멈춰 있던 시간 {} · {joined}에 {} 집중", hm(total, lang), hm(top, lang)),
            vec![format!("{} 중 {parts}", hm(total, lang)), "그 시간에 화면을 언제 봤는지는 기록에 없음".into()],
            "분".to_owned(),
        ),
        Lang::En => (
            format!("Idle time in {joined}"),
            format!("Idle time {} · {} in {joined}", hm(total, lang), hm(top, lang)),
            vec![format!("{parts} out of {}", hm(total, lang)), "The records do not show when you looked at the screen".into()],
            " min".to_owned(),
        ),
    };
    Some(Checkpoint {
        kind: "hour_band".into(),
        title,
        line,
        details,
        share: top as f64 / total as f64,
        date: None,
        goal_key: "waiting_ms".into(),
        unit,
        series: h
            .iter()
            .enumerate()
            .map(|(hour, &ms)| SeriesPoint {
                label: hour.to_string(),
                value: ms as f64 / 60_000.0,
                tip: {
                    let mins = (ms + 30_000) / 60_000;
                    let pct = if total > 0 { ms as f64 * 100.0 / total as f64 } else { 0.0 };
                    match lang {
                        Lang::Ko => format!("{hour}시 · 멈춰 있던 시간 {mins}분 · 이 주의 멈춰 있던 시간 대비 {pct:.0}%"),
                        Lang::En => format!("{}–{}h · Idle time {mins} min · {pct:.0}% of the week's idle time", hour, hour + 1),
                    }
                },
            })
            .collect(),
        highlight: bands.iter().flat_map(|b| b.0..=b.1).collect(),
    })
}

/// The weekly summary notification body: its headline, then how many checkpoints.
pub fn week_notice(headline: &str, checkpoints: usize, lang: Lang) -> String {
    if checkpoints == 0 {
        return headline.to_owned();
    }
    match lang {
        Lang::Ko => format!("{headline} · 체크포인트 {checkpoints}"),
        Lang::En => format!("{headline} · {checkpoints} checkpoint{}", if checkpoints == 1 { "" } else { "s" }),
    }
}

// ---------- reading ----------

/// Hook events with `start <= t < end`. Event files are named by UTC date, so a
/// local week reaches into the UTC day before its Monday.
pub fn read_events_in(dir: &Path, start: u64, end: u64) -> Vec<Event> {
    let mut out = Vec::new();
    if end <= start {
        return out;
    }
    let last = time::utc_date(end - 1);
    let mut date = time::utc_date(start);
    loop {
        if let Ok(text) = fs::read_to_string(dir.join(format!("{date}.jsonl"))) {
            out.extend(text.lines().filter_map(|l| serde_json::from_str::<Event>(l).ok()).filter(|e| e.t >= start && e.t < end));
        }
        if date >= last {
            break;
        }
        match time::add_days(&date, 1) {
            Some(next) => date = next,
            None => break,
        }
    }
    out
}

/// The local day of the first event porch ever recorded.
pub fn recorded_from_in(dir: &Path, offset: i64) -> Option<String> {
    let first = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|f| f.file_name().to_str().and_then(|n| n.strip_suffix(".jsonl")).map(str::to_owned))
        .filter(|d| time::local_day_window(d, 0).is_some())
        .min()?;
    let text = fs::read_to_string(dir.join(format!("{first}.jsonl"))).ok()?;
    let t = text.lines().filter_map(|l| serde_json::from_str::<Event>(l).ok()).map(|e| e.t).min()?;
    Some(time::local_date(t, offset))
}

/// The week holding `any_date`, counted from this Mac's hook events.
pub fn observe_week(any_date: &str, offset: i64, now: u64, lang: Lang) -> Result<Observed, String> {
    let monday = summary::week_start(any_date, offset).ok_or_else(|| time::bad_date(lang))?;
    let start = time::local_day_window(&monday, offset).map(|(s, _)| s).ok_or_else(|| time::bad_date(lang))?;
    let dir = paths::events_dir();
    let events = read_events_in(&dir, start, week_end(&monday, offset).min(now.max(start)));
    let mut excluded = settings::excluded();
    excluded.push(summary::runner_dir().to_string_lossy().into_owned());
    let roots = RefCell::new(HashMap::new());
    let is_excluded = |c: &str| digest::is_excluded(c, &excluded);
    let project = |c: &str| digest::repo_of_dir(c, &mut roots.borrow_mut()).map(|r| r.rsplit('/').next().unwrap_or(&r).to_owned());
    let mut o = observe(&events, &Context { week_start: &monday, offset, now, excluded: &is_excluded, project: &project });
    o.recorded_from = recorded_from_in(&dir, offset).filter(|d| *d > monday);
    Ok(o)
}

/// Last week's goal and how this week compares with it.
#[derive(Debug, Clone, Serialize)]
pub struct LastGoal {
    pub goal: goals::Goal,
    pub result: goals::GoalResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightView {
    pub observed: Observed,
    pub checkpoints: Vec<Checkpoint>,
    /// The week has ended: its numbers are final, and it can be evaluated and given a goal.
    pub finished: bool,
    /// Settings' `insight_eval`.
    pub eval_on: bool,
    pub has_week_summary: bool,
    pub evaluation: Option<insight_eval::Evaluation>,
    /// Last week's scores by item, for "Last week N of 5".
    pub last_scores: BTreeMap<String, u8>,
    pub skipped: Option<String>,
    pub error: Option<String>,
    pub disputed: Vec<String>,
    pub days_without_summary: Vec<String>,
    pub provider: Option<Provider>,
    pub model: Option<String>,
    /// The goal chosen on this week, compared next week.
    pub goal: Option<goals::Goal>,
    pub last_goal: Option<LastGoal>,
    /// Finished weeks only: goals come from finished weeks.
    pub candidates: Vec<goals::Candidate>,
}

pub fn view(any_date: &str, offset: i64, now: u64, lang: Lang) -> Result<InsightView, String> {
    let monday = summary::week_start(any_date, offset).ok_or_else(|| time::bad_date(lang))?;
    let observed = insight_eval::observed(&monday, offset, now, lang)?;
    let checkpoints = checkpoints(&observed, lang);
    let saved = insight_eval::load(&monday).unwrap_or_default();
    let prev = time::add_days(&monday, -7).unwrap_or_default();
    let last_scores = insight_eval::load(&prev).and_then(|r| r.evaluation).map(|e| e.scores.into_iter().map(|s| (s.item, s.score)).collect()).unwrap_or_default();
    let eval_on = settings::load().insight_eval;
    let finished = observed.through >= observed.week_end;
    let all = goals::load();
    let last_goal = all.iter().find(|g| g.week_start == prev).map(|g| LastGoal { goal: g.clone(), result: goals::goal_result(g, &observed, saved.evaluation.as_ref(), offset) });
    let candidates = if finished { goals::candidates(&checkpoints, &observed, saved.evaluation.as_ref().filter(|_| eval_on), lang) } else { Vec::new() };
    Ok(InsightView {
        finished,
        eval_on,
        has_week_summary: summary::load_week(&monday).is_some_and(|w| w.summary.is_some()),
        last_scores,
        last_goal,
        candidates,
        goal: all.into_iter().find(|g| g.week_start == monday),
        evaluation: saved.evaluation,
        skipped: saved.skipped,
        error: saved.error,
        disputed: saved.disputed,
        days_without_summary: saved.days_without_summary,
        provider: saved.provider,
        model: saved.model,
        observed,
        checkpoints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;
    const MONDAY: &str = "2026-09-28";

    fn t0() -> u64 {
        time::local_day_window(MONDAY, 0).unwrap().0
    }

    fn ev(min: u64, session: &str, event: &str) -> Event {
        Event { v: 1, t: t0() + min * MIN, agent: "claude".into(), event: event.into(), session: Some(session.into()), cwd: Some("/r/a".into()), ..Default::default() }
    }

    fn tool(mut e: Event, name: &str) -> Event {
        e.tool = Some(name.into());
        e
    }

    fn at(mut e: Event, cwd: &str) -> Event {
        e.cwd = Some(cwd.into());
        e
    }

    fn excluded(c: &str) -> bool {
        c.starts_with("/x/")
    }

    fn project(c: &str) -> Option<String> {
        c.strip_prefix("/r/").map(|p| p.split('/').next().unwrap().to_owned())
    }

    fn cx(now: u64) -> Context<'static> {
        Context { week_start: MONDAY, offset: 0, now, excluded: &excluded, project: &project }
    }

    fn whole_week() -> u64 {
        t0() + 30 * 24 * 60 * MIN
    }

    #[test]
    fn running_and_waiting_follow_the_state_rules() {
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            tool(ev(2, "s1", "PreToolUse"), "Bash"),
            tool(ev(3, "s1", "PostToolUse"), "Bash"),
            ev(5, "s1", "Stop"),
            ev(8, "s1", "UserPromptSubmit"),
            ev(9, "s1", "Stop"),
        ];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.days[0].running_ms, 6 * MIN); // 0→5 and 8→9
        assert_eq!(o.days[0].turn_done_ms, 3 * MIN); // 5→8; nothing after the last Stop
        assert_eq!((o.days[0].requests, o.days[0].tool_calls), (2, 1));
        assert_eq!(o.sessions, 1);
    }

    #[test]
    fn long_gaps_count_nothing_while_running_and_ten_minutes_while_stopped() {
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            ev(20, "s1", "Stop"),             // running gap of 20 minutes: 0
            ev(50, "s1", "UserPromptSubmit"), // stopped for 30 minutes: 10
            ev(51, "s1", "Stop"),
        ];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.days[0].running_ms, MIN);
        assert_eq!(o.days[0].turn_done_ms, CAP_MS);
    }

    #[test]
    fn events_that_keep_the_state_do_not_restart_the_wait_cap() {
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            ev(1, "s1", "Stop"),
            ev(5, "s1", "Notification"),
            ev(9, "s1", "SubagentStop"),
            ev(13, "s1", "Notification"),
            ev(60, "s1", "UserPromptSubmit"),
            ev(61, "s1", "Stop"),
        ];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.days[0].turn_done_ms, CAP_MS); // one wait from 1 to 60: ten minutes, not 4 + 4 + 4 + 10
        assert_eq!(o.hours_waiting_ms[0], CAP_MS);
        assert_eq!(o.days[0].running_ms, 2 * MIN);
    }

    #[test]
    fn each_waiting_state_has_its_own_bucket() {
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            tool(ev(1, "s1", "PermissionRequest"), "Bash"),
            tool(ev(3, "s1", "PostToolUse"), "Bash"),
            tool(ev(4, "s1", "PreToolUse"), state::ASK_TOOL),
            tool(ev(6, "s1", "PostToolUse"), state::ASK_TOOL),
            ev(7, "s1", "StopFailure"),
            ev(9, "s1", "UserPromptSubmit"),
            ev(10, "s1", "Stop"),
        ];
        let d = &observe(&events, &cx(whole_week())).days[0];
        assert_eq!(d.permission_ms, 2 * MIN);
        assert_eq!(d.question_ms, 2 * MIN);
        assert_eq!(d.failed_ms, 2 * MIN);
        assert_eq!(d.running_ms, 4 * MIN);
        assert_eq!((d.stop_failures, d.permission_requests, d.requests), (1, 1, 2));
    }

    #[test]
    fn stretches_belong_to_the_day_and_hour_they_began() {
        let tue_13 = 24 * 60 + 13 * 60;
        let events = [ev(tue_13, "s1", "Stop"), ev(tue_13 + 5, "s1", "UserPromptSubmit")];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.days[1].date, "2026-09-29");
        assert_eq!(o.days[1].turn_done_ms, 5 * MIN);
        assert_eq!(o.hours_waiting_ms[13], 5 * MIN);
        assert_eq!(o.hours_waiting_ms.len(), 24);
    }

    #[test]
    fn concurrency_counts_sessions_side_by_side() {
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            ev(5, "s1", "PostToolUse"),
            ev(10, "s1", "Stop"),
            ev(2, "s2", "UserPromptSubmit"),
            ev(4, "s2", "Stop"),
            ev(7, "s2", "UserPromptSubmit"),
            ev(8, "s2", "Stop"),
        ];
        let c = observe(&events, &cx(whole_week())).concurrency;
        assert_eq!(c.one_ms, 4 * MIN); // 0→2, 8→10
        assert_eq!(c.two_ms, 6 * MIN); // 2→8
        assert_eq!(c.three_plus_ms, 0);
        assert_eq!(c.overlap_ms, 3 * MIN); // s2 stopped 4→7 while s1 ran
    }

    #[test]
    fn sessions_with_only_start_and_end_are_not_counted() {
        let mut probe_start = ev(10, "probe", "SessionStart");
        probe_start.t = t0() + 10 * MIN;
        let mut probe_end = ev(10, "probe", "SessionEnd");
        probe_end.t = t0() + 10 * MIN + 3_000; // Orca's rate-limit probe: start, end 3 s later
        let events = [ev(0, "s1", "UserPromptSubmit"), ev(3, "s1", "Stop"), probe_start, probe_end];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.sessions, 1);
        assert_eq!(o.days[0].turn_done_ms, 0); // the probe's 3 s of Done (turn over) add nothing
    }

    #[test]
    fn concurrency_counts_three_sessions_side_by_side() {
        // s1 runs 0→10, s2 runs 2→8, s3 runs 4→6.
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            ev(5, "s1", "PostToolUse"),
            ev(10, "s1", "Stop"),
            ev(2, "s2", "UserPromptSubmit"),
            ev(4, "s2", "PostToolUse"),
            ev(8, "s2", "Stop"),
            ev(4, "s3", "UserPromptSubmit"),
            ev(5, "s3", "PostToolUse"),
            ev(6, "s3", "Stop"),
        ];
        let c = observe(&events, &cx(whole_week())).concurrency;
        // By hand: 1 session 0→2 and 8→10 = 4 min; 2 sessions 2→4 and 6→8 = 4 min;
        // 3 sessions 4→5 and 5→6 = 2 min.
        assert_eq!(c.one_ms, 4 * MIN);
        assert_eq!(c.two_ms, 4 * MIN);
        assert_eq!(c.three_plus_ms, 2 * MIN);
    }

    #[test]
    fn excluded_folders_drop_out_and_projects_group_running_time() {
        let events = [
            ev(0, "s1", "UserPromptSubmit"),
            ev(3, "s1", "Stop"),
            at(ev(0, "s2", "UserPromptSubmit"), "/x/secret"),
            at(ev(9, "s2", "Stop"), "/x/secret"),
            at(ev(0, "s3", "UserPromptSubmit"), "/tmp/z"),
            at(ev(1, "s3", "Stop"), "/tmp/z"),
        ];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.sessions, 2);
        assert_eq!(o.requests, 2);
        assert_eq!(o.projects, [ProjectTime { name: "a".into(), running_ms: 3 * MIN }, ProjectTime { name: OTHER.into(), running_ms: MIN }]);
    }

    #[test]
    fn empty_week_is_all_zero() {
        let o = observe(&[], &cx(whole_week()));
        assert_eq!(o.days.len(), 7);
        assert_eq!(o.days[6].date, "2026-10-04");
        assert_eq!((o.running_ms(), o.waiting_ms(), o.requests, o.sessions), (0, 0, 0, 0));
        assert_eq!(o.through, o.week_end);
        assert_eq!(o.week_end, t0() + 7 * 24 * 60 * MIN);
    }

    #[test]
    fn session_running_across_week_start() {
        let mut before = ev(0, "s1", "UserPromptSubmit");
        before.t = t0() - 30 * MIN;
        let events = [before, ev(5, "s1", "PostToolUse"), ev(6, "s1", "Stop")];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.days[0].running_ms, MIN);
        assert_eq!(o.requests, 0);
    }

    #[test]
    fn unsorted_events_count_the_same() {
        let sorted = [
            ev(0, "s1", "UserPromptSubmit"),
            ev(3, "s1", "PostToolUse"),
            ev(5, "s1", "Stop"),
            ev(8, "s1", "UserPromptSubmit"),
        ];
        let mut shuffled = sorted.clone();
        shuffled.reverse();
        assert_eq!(observe(&sorted, &cx(whole_week())), observe(&shuffled, &cx(whole_week())));
    }

    #[test]
    fn unknown_event_kinds_do_not_change_state() {
        let events = [ev(0, "s1", "UserPromptSubmit"), ev(2, "s1", "sessionEnd"), ev(4, "s1", "Stop")];
        let o = observe(&events, &cx(whole_week()));
        assert_eq!(o.days[0].running_ms, 4 * MIN);
    }

    #[test]
    fn a_running_week_stops_at_now() {
        let events = [ev(0, "s1", "UserPromptSubmit"), ev(5, "s1", "Stop"), ev(8, "s1", "UserPromptSubmit")];
        let o = observe(&events, &cx(t0() + 6 * MIN));
        assert_eq!(o.through, t0() + 6 * MIN);
        assert_eq!(o.days[0].running_ms, 5 * MIN);
        assert_eq!(o.days[0].turn_done_ms, 0); // the next event is past `through`
        assert_eq!(o.requests, 1);
    }

    fn line(ts: &str, event: &str) -> String {
        let t = time::parse_rfc3339_ms(ts).unwrap();
        format!(r#"{{"v":1,"t":{t},"agent":"claude","event":"{event}","session":"s1"}}"#)
    }

    #[test]
    fn reads_the_utc_file_before_a_local_monday() {
        let dir = tempfile::tempdir().unwrap();
        // KST Monday 2026-09-28 00:00 is 2026-09-27 15:00 UTC.
        std::fs::write(dir.path().join("2026-09-27.jsonl"), [line("2026-09-27T14:00:00Z", "Stop"), line("2026-09-27T15:30:00Z", "UserPromptSubmit")].join("\n")).unwrap();
        std::fs::write(dir.path().join("2026-09-28.jsonl"), [line("2026-09-28T01:00:00Z", "Stop"), "not json".to_owned()].join("\n")).unwrap();
        let kst = 9 * 3600;
        let start = time::local_day_window(MONDAY, kst).unwrap().0;
        let got = read_events_in(dir.path(), start, week_end(MONDAY, kst));
        let kinds: Vec<&str> = got.iter().map(|e| e.event.as_str()).collect();
        assert_eq!(kinds, ["UserPromptSubmit", "Stop"]);
    }

    #[test]
    fn recorded_from_is_the_first_event_day() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2026-09-27.jsonl"), line("2026-09-27T16:00:00Z", "Stop")).unwrap();
        std::fs::write(dir.path().join("2026-09-29.jsonl"), line("2026-09-29T01:00:00Z", "Stop")).unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        assert_eq!(recorded_from_in(dir.path(), 9 * 3600).as_deref(), Some("2026-09-28"));
        assert_eq!(recorded_from_in(tempfile::tempdir().unwrap().path(), 0), None);
    }

    fn week_with(f: impl Fn(&mut DayObs, usize)) -> Observed {
        let mut o = observe(&[], &cx(whole_week()));
        for (i, d) in o.days.iter_mut().enumerate() {
            f(d, i);
        }
        o
    }

    #[test]
    fn day_focus_needs_half_and_five() {
        let stops = [0, 8, 1, 16, 1, 1, 0];
        let o = week_with(|d, i| d.stop_failures = stops[i]);
        let c = &checkpoints(&o, Lang::Ko)[0];
        assert_eq!(c.kind, "day_focus");
        assert_eq!(c.title, "오류로 멈춤, 10월 1일에 집중");
        assert_eq!(c.date.as_deref(), Some("2026-10-01"));
        assert_eq!(c.highlight, [3]);
        assert_eq!(c.goal_key, "stop_failures");
        assert!(c.line.starts_with("오류로 멈춘 차례 27회"));
        let few = week_with(|d, i| d.stop_failures = [0, 4, 0, 0, 0, 0, 0][i]);
        assert!(checkpoints(&few, Lang::Ko).is_empty());
        let spread = week_with(|d, i| d.stop_failures = [5, 5, 1, 0, 0, 0, 0][i]);
        assert!(checkpoints(&spread, Lang::Ko).is_empty());
    }

    #[test]
    fn tool_streak_needs_three_days_and_half_the_errors() {
        let bash = [0, 7, 15, 18, 9, 19, 3];
        let o = week_with(|d, i| {
            d.tool_errors.insert("Bash".into(), bash[i]);
            if i == 2 {
                d.tool_errors.insert("Read".into(), 8);
            }
        });
        let c = checkpoints(&o, Lang::Ko).into_iter().find(|c| c.kind == "tool_streak").unwrap();
        assert_eq!(c.title, "Bash 오류 6일 연속");
        assert_eq!(c.line, "도구 오류 79건 중 Bash 71건, 9월 29일부터 6일 연속");
        assert_eq!(c.goal_key, "tool_errors:Bash");
        let short = week_with(|d, i| {
            d.tool_errors.insert("Bash".into(), [0, 5, 5, 0, 5, 5, 0][i]);
        });
        assert!(checkpoints(&short, Lang::Ko).is_empty());
        let minor = week_with(|d, i| {
            d.tool_errors.insert("Bash".into(), [0, 1, 1, 1, 0, 0, 0][i]); // 3 of 10: under half
            if i == 0 {
                d.tool_errors.insert("Read".into(), 7); // most errors, but one day only
            }
        });
        assert!(checkpoints(&minor, Lang::Ko).iter().all(|c| c.kind != "tool_streak"));
    }

    #[test]
    fn hour_band_finds_the_dense_hours() {
        let mins: [(usize, u64); 13] = [(8, 3), (9, 10), (10, 38), (11, 62), (12, 53), (13, 61), (14, 15), (15, 55), (16, 34), (17, 6), (21, 50), (22, 72), (23, 22)];
        let mut o = observe(&[], &cx(whole_week()));
        for (h, m) in mins {
            o.hours_waiting_ms[h] = m * MIN;
        }
        let c = checkpoints(&o, Lang::Ko).into_iter().find(|c| c.kind == "hour_band").unwrap();
        assert_eq!(c.title, "멈춰 있던 시간, 11시–14시와 22시–23시에 집중");
        assert_eq!(c.highlight, [11, 12, 13, 22]);
        assert_eq!(c.series.len(), 24);
        assert_eq!(c.goal_key, "waiting_ms");
    }

    #[test]
    fn an_even_spread_is_not_a_band() {
        let mut o = observe(&[], &cx(whole_week()));
        o.hours_waiting_ms = vec![10 * MIN; 24];
        assert!(checkpoints(&o, Lang::Ko).is_empty());
    }

    #[test]
    fn no_checkpoints_without_records() {
        assert!(checkpoints(&observe(&[], &cx(whole_week())), Lang::Ko).is_empty());
    }

    #[test]
    fn hm_reads_minutes_then_hours() {
        assert_eq!(hm(0, Lang::Ko), "0분");
        assert_eq!(hm(59 * MIN, Lang::Ko), "59분");
        assert_eq!(hm(78 * MIN, Lang::Ko), "1.3시간");
    }

    #[test]
    fn hm_rounds_half_up_like_the_screen() {
        // 75 min: 1.25 h. Rust's {:.1} is ties-to-even (1.2); JS toFixed is ties-away (1.3).
        assert_eq!(hm(4_500_000, Lang::Ko), "1.3시간");
        assert_eq!(hm(4_500_000, Lang::En), "1.3 h");
    }

    #[test]
    fn observe_week_reports_a_bad_date_in_the_language() {
        assert_eq!(observe_week("2026-9-1", 0, 0, Lang::En).unwrap_err(), time::bad_date(Lang::En));
        assert_eq!(observe_week("2026-9-1", 0, 0, Lang::Ko).unwrap_err(), time::bad_date(Lang::Ko));
    }

    #[test]
    fn english_details_do_not_end_with_a_period() {
        let stops = [0, 8, 1, 16, 1, 1, 0];
        let bash = [0, 7, 15, 18, 9, 19, 3];
        let mut o = week_with(|d, i| {
            d.stop_failures = stops[i];
            d.tool_errors.insert("Bash".into(), bash[i]);
        });
        for (h, m) in [(11, 62u64), (12, 53), (13, 61), (22, 72)] {
            o.hours_waiting_ms[h] = m * MIN;
        }
        let cps = checkpoints(&o, Lang::En);
        let kinds: Vec<&str> = cps.iter().map(|c| c.kind.as_str()).collect();
        assert!(kinds.contains(&"day_focus") && kinds.contains(&"tool_streak") && kinds.contains(&"hour_band"), "{kinds:?}");
        for c in &cps {
            for line in &c.details {
                assert!(!line.ends_with('.'), "{line}");
            }
        }
    }

    #[test]
    fn week_notice_adds_the_checkpoint_count() {
        assert_eq!(week_notice("결제 개편 마무리", 0, Lang::Ko), "결제 개편 마무리");
        assert_eq!(week_notice("결제 개편 마무리", 3, Lang::Ko), "결제 개편 마무리 · 체크포인트 3");
    }

    #[test]
    fn checkpoints_speak_english() {
        let stops = [0, 8, 1, 16, 1, 1, 0];
        let o = week_with(|d, i| d.stop_failures = stops[i]);
        let c = &checkpoints(&o, Lang::En)[0];
        assert_eq!(c.title, "Stopped on error on Oct 1");
        assert!(c.line.starts_with("27 turns stopped on error"));
        assert_eq!(checkpoints(&o, Lang::Ko)[0].title, "오류로 멈춤, 10월 1일에 집중");
    }

    #[test]
    fn hour_band_titles_read_the_same_hours_in_both_languages() {
        let mut o = observe(&[], &cx(whole_week()));
        for (h, m) in [(11, 62), (12, 53), (13, 61), (22, 72), (3, 5)] {
            o.hours_waiting_ms[h] = m * MIN;
        }
        let ko = checkpoints(&o, Lang::Ko).into_iter().find(|c| c.kind == "hour_band").unwrap();
        let en = checkpoints(&o, Lang::En).into_iter().find(|c| c.kind == "hour_band").unwrap();
        assert_eq!(ko.title, "멈춰 있던 시간, 11시–14시와 22시–23시에 집중");
        assert_eq!(en.title, "Idle time in 11–14h and 22–23h");
    }

    #[test]
    fn hm_and_notice_follow_the_language() {
        assert_eq!(hm(59 * MIN, Lang::En), "59 min");
        assert_eq!(hm(78 * MIN, Lang::En), "1.3 h");
        assert_eq!(hm(78 * MIN, Lang::Ko), "1.3시간");
        assert_eq!(week_notice("Payments done", 1, Lang::En), "Payments done · 1 checkpoint");
        assert_eq!(week_notice("Payments done", 3, Lang::En), "Payments done · 3 checkpoints");
        assert_eq!(week_notice("결제 개편 마무리", 3, Lang::Ko), "결제 개편 마무리 · 체크포인트 3");
    }

    #[test]
    fn projects_outside_a_repo_have_no_name() {
        let events = [at(ev(0, "s3", "UserPromptSubmit"), "/tmp/z"), at(ev(1, "s3", "Stop"), "/tmp/z")];
        assert_eq!(observe(&events, &cx(whole_week())).projects[0].name, "");
    }
}
