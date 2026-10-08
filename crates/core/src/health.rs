//! Project health: where each project kept getting held up over recent days,
//! read from saved day reports and the open-work ledger. Pure counting;
//! nothing here calls the summarizer. Kinds come from the day summaries,
//! minutes from the recorded turns, each turn counted once toward its own repo.

use crate::lang::Lang;
use crate::open::{Kind, OpenItem};
use crate::summary::{Report, BLOCKER_KINDS};
use crate::time;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const DEFAULT_DAYS: u64 = 30;
const DAY_MS: u64 = 86_400_000;
const REPEAT_KIND_DAYS: usize = 3;
const REPEAT_ERROR_DAYS: usize = 2;
const BLOCKED_SHARE_PERCENT: u64 = 20;
const BLOCKED_SHARE_MIN_MINUTES: u64 = 120;
const OLD_ISSUE_DAYS: u64 = 7;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Evidence {
    pub date: String,
    pub turns: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KindStat {
    /// A listed kind, "unknown" (a kind outside the list) or "unclassified" (no kind).
    pub kind: String,
    pub label: String,
    pub blockers: usize,
    /// Time in the turns this kind's blockers point at; a turn under two kinds counts in both.
    pub minutes: u64,
    /// Blockers the summary said were not resolved that day.
    pub unresolved_that_day: usize,
    pub days: usize,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorStat {
    pub command: String,
    pub days: usize,
    pub failures: usize,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenIssue {
    pub id: String,
    pub text: String,
    pub since: String,
    pub days_open: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// "repeat_kind", "repeat_error", "blocked_share" or "old_issue".
    pub kind: String,
    pub title: String,
    pub now: u64,
    pub before: Option<u64>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PeriodTotals {
    pub active_minutes: u64,
    pub blocked_minutes: u64,
    pub blockers: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectHealth {
    pub root: String,
    pub name: String,
    pub days: usize,
    pub active_minutes: u64,
    /// Time in turns some blocker points at, each turn once. Not time spent blocked.
    pub blocked_minutes: u64,
    pub kinds: Vec<KindStat>,
    /// Commands that failed on two days or more.
    pub repeated_errors: Vec<ErrorStat>,
    pub interrupted: usize,
    pub denials: usize,
    pub open_issues: Vec<OpenIssue>,
    pub checks: Vec<Check>,
    pub previous: Option<PeriodTotals>,
    /// Days counted by the session's folder because the transcripts were gone.
    pub session_attributed_days: usize,
    /// First day the open-work ledger has, when it has any.
    pub issues_tracked_since: Option<String>,
}

#[derive(Default)]
struct KindAcc {
    blockers: usize,
    ms: u64,
    unresolved: usize,
    dates: BTreeSet<String>,
    evidence: Vec<Evidence>,
}

#[derive(Default)]
struct ErrAcc {
    failures: usize,
    dates: BTreeSet<String>,
    evidence: Vec<Evidence>,
}

#[derive(Default)]
struct Acc {
    name: String,
    dates: BTreeSet<String>,
    active_ms: u64,
    blocked_ms: u64,
    blockers: usize,
    kinds: BTreeMap<String, KindAcc>,
    errors: BTreeMap<String, ErrAcc>,
    interrupted: usize,
    denials: usize,
    session_days: BTreeSet<String>,
}

fn minutes(ms: u64) -> u64 {
    (ms + 30_000) / 60_000
}

/// The listed kind a summary's `kind` means. Trimmed and ASCII-lowercased
/// before matching, so " Test_Fail " is the same kind as "test_fail"; the
/// value stored in the report is left as it was.
fn kind_key(k: &str) -> &'static str {
    let t = k.trim();
    if t.is_empty() {
        "unclassified"
    } else {
        BLOCKER_KINDS.iter().find(|x| x.eq_ignore_ascii_case(t)).copied().unwrap_or("unknown")
    }
}

pub fn kind_label(k: &str, lang: Lang) -> &'static str {
    match (k, lang) {
        ("test_fail", Lang::Ko) => "테스트 실패",
        ("test_fail", Lang::En) => "Test, type check and build failures",
        ("auth_external", Lang::Ko) => "인증·외부 서비스",
        ("auth_external", Lang::En) => "Authentication and external services",
        ("env", Lang::Ko) => "작업 환경",
        ("env", Lang::En) => "Local environment",
        ("misread", Lang::Ko) => "요구 전달 어긋남",
        ("misread", Lang::En) => "Misunderstood requests",
        ("review_loop", Lang::Ko) => "검토 반복",
        ("review_loop", Lang::En) => "Repeated requests for changes in review",
        ("slow", Lang::Ko) => "오래 도는 작업",
        ("slow", Lang::En) => "Long-running work",
        ("other", Lang::Ko) => "기타",
        ("other", Lang::En) => "Other",
        ("unknown", Lang::Ko) => "모르는 유형",
        ("unknown", Lang::En) => "Unknown category",
        (_, Lang::Ko) => "미분류",
        (_, Lang::En) => "Unclassified",
    }
}

/// Work with no project, as the project list names it.
fn unknown_project(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => "프로젝트 모름",
        Lang::En => "Unknown project",
    }
}

fn last_segment(root: &str) -> String {
    root.rsplit('/').next().unwrap_or(root).to_owned()
}

/// The command in a turn error ("Bash `…`: out", "명령 `…` 종료 코드 1: out" or
/// "Command `…` exited with 1: out"),
/// cut to what identifies it: the last `&&`/`;` step, before any pipe, then the
/// first two words plus the values of `--filter` and `-p`. Other errors: None.
fn failed_command(error: &str) -> Option<String> {
    let rest = ["Bash `", "명령 `", "Command `"].iter().find_map(|p| error.strip_prefix(p))?;
    let cmd = rest.split('`').next()?.trim_end_matches('…');
    let step = cmd.rsplit("&&").next()?.rsplit(';').next()?;
    let step = step.split('|').next()?;
    let mut words = Vec::new();
    let mut plain = 0;
    let mut it = step.split_whitespace();
    while let Some(w) = it.next() {
        if w == "--" || w.starts_with("2>") || w.starts_with('>') {
            break;
        }
        // `--filter=x` and `-p=x` (and `--package=x`, which means the same as `-p`)
        // land on the key their space forms do.
        if let Some((flag, value)) = w.split_once('=').filter(|(_, v)| !v.is_empty()) {
            let flag = match flag {
                "--filter" => "--filter",
                "-p" | "--package" => "-p",
                _ => "",
            };
            if !flag.is_empty() {
                words.push(flag.to_owned());
                words.push(value.to_owned());
                continue;
            }
        }
        if w == "--filter" || w == "-p" || w == "--package" {
            if let Some(v) = it.next() {
                words.push(if w == "--package" { "-p".to_owned() } else { w.to_owned() });
                words.push(v.to_owned());
            }
        } else if !w.starts_with('-') && plain < 2 {
            words.push(w.to_owned());
            plain += 1;
        }
    }
    (!words.is_empty()).then(|| words.join(" "))
}

fn accumulate(reports: &[&Report], lang: Lang) -> BTreeMap<String, Acc> {
    let mut acc: BTreeMap<String, Acc> = BTreeMap::new();
    for r in reports {
        let date = r.date.clone();
        // Turn id -> (repo it counts toward, active ms).
        let mut turn_root: HashMap<&str, (&str, u64)> = HashMap::new();
        for p in &r.digest.projects {
            let turns: Vec<&crate::digest::Turn> = p.sessions.iter().flat_map(|s| s.turns.iter()).collect();
            if turns.is_empty() {
                // Saved before turns existed: the project's own minutes are all there is,
                // and the day is session-attributed by definition.
                let a = acc.entry(p.root.clone()).or_default();
                a.name = p.name.clone();
                a.dates.insert(date.clone());
                a.active_ms += p.metrics.active_minutes * 60_000;
                a.session_days.insert(date.clone());
                continue;
            }
            for t in turns {
                let root = t.root.as_deref().unwrap_or(&p.root);
                // A duplicate turn id keeps the first repo it was counted toward.
                turn_root.entry(&t.id).or_insert((root, t.active_ms));
                let a = acc.entry(root.to_owned()).or_default();
                if a.name.is_empty() {
                    a.name = if root == p.root { p.name.clone() } else { last_segment(root) };
                }
                a.dates.insert(date.clone());
                a.active_ms += t.active_ms;
                a.interrupted += usize::from(t.interrupted);
                a.denials += t.denials;
                if r.turn_files_read != Some(true) {
                    a.session_days.insert(date.clone());
                }
                for e in &t.errors {
                    if let Some(cmd) = failed_command(e) {
                        let x = a.errors.entry(cmd).or_default();
                        x.failures += 1;
                        x.dates.insert(date.clone());
                        x.evidence.push(Evidence { date: date.clone(), turns: vec![t.id.clone()], label: e.clone() });
                    }
                }
            }
        }
        let Some(s) = &r.summary else { continue };
        let mut blocked: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut by_kind: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
        for b in &s.blockers {
            let kind = kind_key(&b.kind);
            let mut by_root: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for id in &b.turns {
                if let Some(&(root, _)) = turn_root.get(id.as_str()) {
                    by_root.entry(root).or_default().push(id.as_str());
                }
            }
            if by_root.is_empty() {
                // No turn it points at is on record: fall back to the project the summary
                // named, or to the "Unknown project" bucket when no project carries that name.
                let root = r.digest.projects.iter().find(|p| p.name == b.project).map(|p| p.root.as_str()).unwrap_or("");
                by_root.insert(root, vec![]);
            }
            for (root, ids) in by_root {
                blocked.entry(root).or_default().extend(ids.iter().copied());
                by_kind.entry((root, kind)).or_default().extend(ids.iter().copied());
                let a = acc.entry(root.to_owned()).or_default();
                if a.name.is_empty() {
                    a.name = if root.is_empty() { unknown_project(lang).into() } else { last_segment(root) };
                }
                a.blockers += 1;
                let k = a.kinds.entry(kind.to_owned()).or_default();
                k.blockers += 1;
                k.unresolved += usize::from(!b.resolved);
                k.dates.insert(date.clone());
                let mut turns: Vec<String> = ids.iter().map(|x| x.to_string()).collect();
                turns.dedup();
                k.evidence.push(Evidence { date: date.clone(), turns, label: b.title.clone() });
            }
        }
        let ms = |ids: &BTreeSet<&str>| ids.iter().map(|id| turn_root[id].1).sum::<u64>();
        for (root, ids) in &blocked {
            acc.get_mut(*root).expect("entry made above").blocked_ms += ms(ids);
        }
        for ((root, kind), ids) in &by_kind {
            acc.get_mut(*root).expect("entry made above").kinds.get_mut(*kind).expect("entry made above").ms += ms(ids);
        }
    }
    acc
}

pub(crate) fn days_before(date: &str, n: u64, offset: i64) -> String {
    match time::local_day_window(date, offset) {
        Some((start, _)) => time::local_date(start.saturating_sub(n * DAY_MS), offset),
        None => date.to_owned(),
    }
}

pub(crate) fn days_after(date: &str, n: u64, offset: i64) -> String {
    match time::local_day_window(date, offset) {
        Some((start, _)) => time::local_date(start + n * DAY_MS, offset),
        None => date.to_owned(),
    }
}

fn days_between(from: &str, to: &str, offset: i64) -> u64 {
    match (time::local_day_window(from, offset), time::local_day_window(to, offset)) {
        (Some((a, _)), Some((b, _))) if b >= a => (b - a) / DAY_MS,
        _ => 0,
    }
}

/// How often a suggestion's target showed up: blockers or failures, and on how many days.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Count {
    pub items: usize,
    pub days: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EffectState {
    /// Fewer than seven days since it was applied.
    Early,
    /// Nothing recorded since it was applied.
    NoRecords,
    /// One window was counted by session folder and the other by turn files.
    Uncomparable,
    Measured,
}

/// A target's count in the `days` before a suggestion was applied and the `days` since.
/// A comparison, not a cause.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Effect {
    pub days: u64,
    pub before: Count,
    pub after: Count,
    pub state: EffectState,
}

const EFFECT_MIN_DAYS: u64 = 7;

/// `target` ("kind:env", "command:cargo test") in `scope` (a repo, or "common" for
/// every repo) between `from` and `to`, both inclusive. Counted the way health counts.
pub fn target_count(reports: &[Report], scope: &str, target: &str, from: &str, to: &str) -> Count {
    let window: Vec<&Report> = reports.iter().filter(|r| r.date.as_str() >= from && r.date.as_str() <= to).collect();
    // Counts only: project names are unused here.
    let acc = accumulate(&window, Lang::default());
    let roots: Vec<&Acc> = if scope == "common" { acc.values().collect() } else { acc.get(scope).into_iter().collect() };
    let mut items = 0;
    let mut dates = BTreeSet::new();
    if let Some(kind) = target.strip_prefix("kind:") {
        let kind = kind_key(kind);
        for a in roots {
            if let Some(k) = a.kinds.get(kind) {
                items += k.blockers;
                dates.extend(k.dates.iter().cloned());
            }
        }
    } else if let Some(cmd) = target.strip_prefix("command:") {
        for a in roots {
            if let Some(e) = a.errors.get(cmd) {
                items += e.failures;
                dates.extend(e.dates.iter().cloned());
            }
        }
    }
    Count { items, days: dates.len() }
}

/// The target before and after `applied_on`, over equal windows of up to 30 days.
pub fn effect(reports: &[Report], scope: &str, target: &str, applied_on: &str, today: &str, offset: i64) -> Effect {
    let days = (days_between(applied_on, today, offset) + 1).min(DEFAULT_DAYS);
    let after_to = days_after(applied_on, days - 1, offset);
    let before_from = days_before(applied_on, days, offset);
    let before_to = days_before(applied_on, 1, offset);
    let in_window = |from: &str, to: &str| reports.iter().filter(move |r| r.date.as_str() >= from && r.date.as_str() <= to).collect::<Vec<_>>();
    let after_reports = in_window(applied_on, &after_to);
    let before_reports = in_window(&before_from, &before_to);
    let mixed = |a: &[&Report], b: &[&Report]| a.iter().any(|r| r.turn_files_read != Some(true)) && b.iter().any(|r| r.turn_files_read == Some(true));
    let state = if days < EFFECT_MIN_DAYS {
        EffectState::Early
    } else if after_reports.is_empty() {
        EffectState::NoRecords
    } else if mixed(&before_reports, &after_reports) || mixed(&after_reports, &before_reports) {
        EffectState::Uncomparable
    } else {
        EffectState::Measured
    };
    Effect {
        days,
        before: target_count(reports, scope, target, &before_from, &before_to),
        after: target_count(reports, scope, target, applied_on, &after_to),
        state,
    }
}

fn percent(part: u64, whole: u64) -> u64 {
    (part * 100 + whole / 2).checked_div(whole).unwrap_or(0)
}

/// The same share with one decimal, for titles: a check that fires only above
/// 20% must never read "20%".
fn percent_tenths(part: u64, whole: u64) -> String {
    let t = (part * 1000 + whole / 2).checked_div(whole).unwrap_or(0);
    format!("{}.{}", t / 10, t % 10)
}

/// A check title says what the same period before had, when there was one.
fn with_before(title: String, before: Option<u64>, unit: &str, days: u64, lang: Lang) -> String {
    match (before, lang) {
        (Some(b), Lang::Ko) => format!("{title} (이전 {days}일 {b}{unit})"),
        (Some(b), Lang::En) if unit == "%" => format!("{title} (previous {days} days: {b}%)"),
        (Some(1), Lang::En) => format!("{title} (previous {days} days: 1 day)"),
        (Some(b), Lang::En) => format!("{title} (previous {days} days: {b} days)"),
        (None, _) => title,
    }
}

/// What the period's counts say is worth a look. Every check carries the
/// evidence it was made from; without evidence there is no check.
fn checks_for(a: &Acc, prev: Option<&Acc>, issues: &[OpenIssue], days: u64, lang: Lang) -> Vec<Check> {
    let mut out = Vec::new();
    for (k, x) in &a.kinds {
        if k == "unclassified" || x.dates.len() < REPEAT_KIND_DAYS {
            continue;
        }
        let n = x.dates.len() as u64;
        let before = prev.and_then(|p| p.kinds.get(k)).map(|p| p.dates.len() as u64);
        out.push(Check {
            kind: "repeat_kind".into(),
            title: with_before(
                match lang {
                    Lang::Ko => format!("{}: {n}일 막힘, {}분", kind_label(k, lang), minutes(x.ms)),
                    Lang::En => format!("{}: blockers on {n} days, {} min estimated", kind_label(k, lang), minutes(x.ms)),
                },
                before,
                "일",
                days,
                lang,
            ),
            now: n,
            before,
            evidence: x.evidence.clone(),
        });
    }
    for (c, x) in &a.errors {
        if x.dates.len() < REPEAT_ERROR_DAYS {
            continue;
        }
        let n = x.dates.len() as u64;
        let before = prev.and_then(|p| p.errors.get(c)).map(|p| p.dates.len() as u64);
        out.push(Check { kind: "repeat_error".into(), title: with_before(
                match lang {
                    Lang::Ko => format!("{c} {n}일 실패"),
                    Lang::En => format!("{c} failed on {n} days"),
                },
                before,
                "일",
                days,
                lang,
            ), now: n, before, evidence: x.evidence.clone() });
    }
    let active = minutes(a.active_ms);
    let pct = percent(a.blocked_ms, a.active_ms);
    if active >= BLOCKED_SHARE_MIN_MINUTES && a.blocked_ms * 100 > a.active_ms * BLOCKED_SHARE_PERCENT {
        let before = prev.filter(|p| p.active_ms > 0).map(|p| percent(p.blocked_ms, p.active_ms));
        let mut evidence: Vec<Evidence> = a.kinds.values().flat_map(|k| k.evidence.iter().cloned()).collect();
        evidence.sort_by(|x, y| x.date.cmp(&y.date));
        out.push(Check {
            kind: "blocked_share".into(),
            title: with_before(
                match lang {
                    Lang::Ko => format!("작업 시간의 {}%가 막힘에 걸린 요청", percent_tenths(a.blocked_ms, a.active_ms)),
                    Lang::En => format!("{}% of estimated work time went to requests with blockers", percent_tenths(a.blocked_ms, a.active_ms)),
                },
                before,
                "%",
                days,
                lang,
            ),
            now: pct,
            before,
            evidence,
        });
    }
    for i in issues.iter().filter(|i| i.days_open > OLD_ISSUE_DAYS) {
        out.push(Check {
            kind: "old_issue".into(),
            title: match lang {
                Lang::Ko => format!("{}: 풀렸다는 기록이 없음 ({}부터, {}일)", i.text, i.since, i.days_open),
                Lang::En => format!("{}: no record of it being solved (since {}, {} days)", i.text, i.since, i.days_open),
            },
            now: i.days_open,
            before: None,
            evidence: vec![Evidence { date: i.since.clone(), turns: vec![], label: i.text.clone() }],
        });
    }
    out
}

/// Health of every project worked on in the `days` up to `today`, compared
/// with the `days` before. Open issues come from the ledger, not from the
/// reports' same-day `resolved`.
pub fn compute(reports: &[Report], issues: &[OpenItem], today: &str, days: u64, offset: i64, lang: Lang) -> Vec<ProjectHealth> {
    let from = days_before(today, days.saturating_sub(1), offset);
    let prev_from = days_before(today, days.saturating_mul(2).saturating_sub(1), offset);
    let now: Vec<&Report> = reports.iter().filter(|r| r.date.as_str() >= from.as_str() && r.date.as_str() <= today).collect();
    let before: Vec<&Report> = reports.iter().filter(|r| r.date.as_str() >= prev_from.as_str() && r.date.as_str() < from.as_str()).collect();
    // The two windows were read differently when one fell back to session attribution
    // and the other read turn files. Their counts are not like for like, so nothing
    // is compared rather than compared wrongly.
    let comparable = !(before.iter().any(|r| r.turn_files_read != Some(true)) && now.iter().any(|r| r.turn_files_read == Some(true)));
    let mut cur = accumulate(&now, lang);
    let prev = if comparable { accumulate(&before, lang) } else { BTreeMap::new() };
    let tracked_since = issues.iter().map(|i| i.since.clone()).min();

    // Ledger items name their project; find its repo among the days read.
    let mut root_of_name: HashMap<String, String> = HashMap::new();
    for r in before.iter().chain(now.iter()) {
        for p in &r.digest.projects {
            root_of_name.insert(p.name.clone(), p.root.clone());
        }
    }
    let mut open_by_root: BTreeMap<String, Vec<OpenIssue>> = BTreeMap::new();
    for i in issues.iter().filter(|i| i.kind == Kind::Issue && i.closed_on.is_none()) {
        let root = root_of_name.get(&i.project).cloned().unwrap_or_default();
        open_by_root.entry(root.clone()).or_default().push(OpenIssue {
            id: i.id.clone(),
            text: i.text.clone(),
            since: i.since.clone(),
            days_open: days_between(&i.since, today, offset),
        });
        let a = cur.entry(root.clone()).or_default();
        if a.name.is_empty() {
            a.name = if root.is_empty() { unknown_project(lang).into() } else { last_segment(&root) };
        }
    }

    let mut out: Vec<ProjectHealth> = cur
        .into_iter()
        .map(|(root, a)| {
            let previous = prev.get(&root).map(|p| PeriodTotals { active_minutes: minutes(p.active_ms), blocked_minutes: minutes(p.blocked_ms), blockers: p.blockers });
            let mut kinds: Vec<KindStat> = a
                .kinds
                .iter()
                .map(|(k, x)| KindStat {
                    kind: k.clone(),
                    label: kind_label(k, lang).to_owned(),
                    blockers: x.blockers,
                    minutes: minutes(x.ms),
                    unresolved_that_day: x.unresolved,
                    days: x.dates.len(),
                    evidence: x.evidence.clone(),
                })
                .collect();
            kinds.sort_by(|a, b| b.minutes.cmp(&a.minutes).then(b.blockers.cmp(&a.blockers)));
            let mut repeated_errors: Vec<ErrorStat> = a
                .errors
                .iter()
                .filter(|(_, x)| x.dates.len() >= REPEAT_ERROR_DAYS)
                .map(|(c, x)| ErrorStat { command: c.clone(), days: x.dates.len(), failures: x.failures, evidence: x.evidence.clone() })
                .collect();
            repeated_errors.sort_by(|a, b| b.days.cmp(&a.days).then(b.failures.cmp(&a.failures)));
            let mut open_issues = open_by_root.remove(&root).unwrap_or_default();
            open_issues.sort_by(|x, y| x.since.cmp(&y.since));
            let checks = checks_for(&a, prev.get(&root), &open_issues, days, lang);
            ProjectHealth {
                name: a.name.clone(),
                days: a.dates.len(),
                active_minutes: minutes(a.active_ms),
                blocked_minutes: minutes(a.blocked_ms).min(minutes(a.active_ms)),
                kinds,
                repeated_errors,
                interrupted: a.interrupted,
                denials: a.denials,
                open_issues,
                checks,
                previous,
                session_attributed_days: a.session_days.len(),
                issues_tracked_since: tracked_since.clone(),
                root,
            }
        })
        .collect();

    out.sort_by(|a, b| b.checks.len().cmp(&a.checks.len()).then(b.active_minutes.cmp(&a.active_minutes)));
    out
}

/// `compute` over what is saved on this Mac, worded in the current language.
pub fn all(days: u64, offset: i64) -> Vec<ProjectHealth> {
    let reports = crate::summary::saved_days();
    let issues = crate::open::load(crate::summary::saved_days);
    let today = time::local_date(time::now_ms(), offset);
    compute(&reports, &issues, &today, days, offset, Lang::current())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::{DayDigest, ProjectDigest, SessionDigest, Turn};
    use crate::summary::{Blocker, Summary};
    use crate::lang::{has_hangul, Lang};

    #[test]
    fn kind_names_in_both_languages() {
        for k in crate::summary::BLOCKER_KINDS.iter().chain(&["unknown", "unclassified"]) {
            assert!(!has_hangul(kind_label(k, Lang::En)), "{k}");
            assert!(has_hangul(kind_label(k, Lang::Ko)), "{k}");
        }
        assert_eq!(kind_label("misread", Lang::En), "Misunderstood requests");
    }

    #[test]
    fn check_titles_in_english() {
        assert_eq!(with_before("Tests failing: stuck on 3 days, 40 min".into(), Some(1), "일", 30, Lang::En), "Tests failing: stuck on 3 days, 40 min (previous 30 days: 1 day)");
        assert_eq!(with_before("x".into(), Some(12), "%", 30, Lang::En), "x (previous 30 days: 12%)");
        assert_eq!(with_before("x".into(), Some(2), "일", 30, Lang::Ko), "x (이전 30일 2일)");
        assert_eq!(unknown_project(Lang::En), "Unknown project");
    }

    pub(super) fn report(date: &str, projects: Vec<ProjectDigest>, blockers: Vec<Blocker>) -> Report {
        Report {
            date: date.into(),
            generated_at: 0,
            model: None,
            provider: None,
            digest: DayDigest { date: date.into(), projects, ..Default::default() },
            summary: Some(Summary { headline: "h".into(), blockers, ..Default::default() }),
            summary_error: None,
            usual_minutes: None,
            usage_checked: true,
            turn_files_read: Some(true),
            dropped_pivots: 0,
        }
    }

    pub(super) fn project(root: &str, turns: Vec<Turn>) -> ProjectDigest {
        ProjectDigest {
            name: root.rsplit('/').next().unwrap().into(),
            root: root.into(),
            sessions: vec![SessionDigest { session: "s".into(), turns, ..Default::default() }],
            ..Default::default()
        }
    }

    pub(super) fn turn(id: &str, min: u64) -> Turn {
        Turn { id: id.into(), active_ms: min * 60_000, ..Default::default() }
    }

    pub(super) fn blocker(kind: &str, turns: &[&str]) -> Blocker {
        Blocker { title: format!("{kind} 막힘"), kind: kind.into(), turns: turns.iter().map(|t| t.to_string()).collect(), ..Default::default() }
    }

    pub(super) fn find<'a>(h: &'a [ProjectHealth], root: &str) -> &'a ProjectHealth {
        h.iter().find(|p| p.root == root).unwrap_or_else(|| panic!("no {root}"))
    }

    fn issue(id: &str, project: &str, since: &str, closed_on: Option<&str>) -> OpenItem {
        OpenItem { id: id.into(), project: project.into(), text: format!("{id} 이슈"), kind: Kind::Issue, since: since.into(), closed_on: closed_on.map(Into::into), closed_how: None }
    }

    fn checks<'a>(h: &'a [ProjectHealth], root: &str, kind: &str) -> Vec<&'a Check> {
        find(h, root).checks.iter().filter(|c| c.kind == kind).collect()
    }

    fn days_of_kind(kind: &str, dates: &[&str]) -> Vec<Report> {
        dates.iter().map(|d| report(d, vec![project("/r/a", vec![turn("t1", 5)])], vec![blocker(kind, &["t1"])])).collect()
    }

    const TODAY: &str = "2026-09-30";

    #[test]
    fn a_failed_command_is_read_in_either_language() {
        let ko = failed_command("명령 `cargo test -p core` 종료 코드 101: x");
        assert!(ko.is_some());
        assert_eq!(failed_command("Command `cargo test -p core` exited with 101: x"), ko);
    }

    #[test]
    fn overlapping_blockers_count_each_turn_once() {
        let r = report(
            "2026-09-22",
            vec![project("/r/app", vec![turn("t3", 10), turn("t4", 10), turn("t5", 10)])],
            vec![blocker("test_fail", &["t3", "t4"]), blocker("env", &["t4", "t5"])],
        );
        let h = compute(&[r], &[], TODAY, 30, 0, Lang::Ko);
        let app = find(&h, "/r/app");
        assert_eq!((app.active_minutes, app.blocked_minutes), (30, 30));
        let kind = |k: &str| app.kinds.iter().find(|x| x.kind == k).unwrap();
        assert_eq!((kind("test_fail").minutes, kind("env").minutes), (20, 20));
    }

    #[test]
    fn a_blocker_across_projects_is_split_by_turn() {
        let mut t7 = turn("t7", 12);
        t7.root = Some("/r/porch".into());
        let r = report("2026-09-29", vec![project("/r/shop", vec![t7, turn("t8", 5)])], vec![blocker("misread", &["t7", "t8"])]);
        let h = compute(&[r], &[], TODAY, 30, 0, Lang::Ko);
        let (porch, shop) = (find(&h, "/r/porch"), find(&h, "/r/shop"));
        assert_eq!((porch.name.as_str(), porch.active_minutes, porch.blocked_minutes), ("porch", 12, 12));
        assert_eq!((shop.active_minutes, shop.blocked_minutes), (5, 5));
        assert_eq!(porch.kinds[0].blockers, 1);
        assert_eq!(shop.kinds[0].blockers, 1);
    }

    #[test]
    fn blocked_never_exceeds_active() {
        let r = report(
            "2026-09-20",
            vec![project("/r/a", vec![turn("t1", 3), turn("t2", 7)])],
            vec![blocker("slow", &["t1", "t1", "t2"]), blocker("slow", &["t2", "t9"]), blocker("", &["t1"])],
        );
        for p in compute(&[r], &[], TODAY, 30, 0, Lang::Ko) {
            assert!(p.blocked_minutes <= p.active_minutes, "{}", p.root);
        }
    }

    #[test]
    fn kinds_outside_the_list_are_unknown_and_empty_is_unclassified() {
        let r = report("2026-09-20", vec![project("/r/a", vec![turn("t1", 5), turn("t2", 5)])], vec![blocker("flaky", &["t1"]), blocker("", &["t2"])]);
        let h = compute(&[r], &[], TODAY, 30, 0, Lang::Ko);
        let kinds: Vec<&str> = find(&h, "/r/a").kinds.iter().map(|k| k.kind.as_str()).collect();
        assert!(kinds.contains(&"unknown") && kinds.contains(&"unclassified"), "{kinds:?}");
    }

    #[test]
    fn blocker_without_known_turns_counts_by_name() {
        let mut b = blocker("env", &["t42"]);
        b.project = "a".into();
        let mut lost = blocker("env", &[]);
        lost.project = "gone".into();
        let r = report("2026-09-20", vec![project("/r/a", vec![turn("t1", 5)])], vec![b, lost]);
        let h = compute(&[r], &[], TODAY, 30, 0, Lang::Ko);
        let a = find(&h, "/r/a");
        assert_eq!((a.kinds[0].blockers, a.blocked_minutes), (1, 0));
        // No project carries that name: it lands in "Unknown project" rather than vanishing.
        let unknown = find(&h, "");
        assert_eq!((unknown.name.as_str(), unknown.kinds[0].blockers, unknown.active_minutes), ("프로젝트 모름", 1, 0));
        assert!(h.iter().all(|p| p.name != "gone"));
    }

    #[test]
    fn a_day_without_turns_is_session_attributed() {
        let h = compute(&[report("2026-09-20", vec![project("/r/a", vec![])], vec![])], &[], TODAY, 30, 0, Lang::Ko);
        assert_eq!(find(&h, "/r/a").session_attributed_days, 1);
    }

    #[test]
    fn kind_keys_are_trimmed_and_lowercased() {
        let r = report("2026-09-20", vec![project("/r/a", vec![turn("t1", 5)])], vec![blocker(" Test_Fail ", &["t1"])]);
        let h = compute(&[r], &[], TODAY, 30, 0, Lang::Ko);
        let kinds: Vec<&str> = find(&h, "/r/a").kinds.iter().map(|k| k.kind.as_str()).collect();
        assert_eq!(kinds, ["test_fail"]);
    }

    #[test]
    fn days_without_turns_use_project_minutes() {
        let mut p = project("/r/old", vec![]);
        p.metrics.active_minutes = 42;
        let h = compute(&[report("2026-09-10", vec![p], vec![])], &[], TODAY, 30, 0, Lang::Ko);
        assert_eq!(find(&h, "/r/old").active_minutes, 42);
    }

    #[test]
    fn record_only_days_count_time_not_blockers() {
        let mut t = turn("t1", 9);
        t.errors = vec!["Bash `cargo test`: boom".into()];
        let mut r = report("2026-09-20", vec![project("/r/a", vec![t])], vec![]);
        r.summary = None;
        let h = compute(&[r], &[], TODAY, 30, 0, Lang::Ko);
        let a = find(&h, "/r/a");
        assert_eq!((a.active_minutes, a.blocked_minutes, a.kinds.len()), (9, 0, 0));
        assert_eq!(a.repeated_errors.len(), 0); // one day only; the command is still counted internally
    }

    #[test]
    fn no_reports_no_health() {
        assert!(compute(&[], &[], TODAY, 30, 0, Lang::Ko).is_empty());
        let old = report("2026-07-01", vec![project("/r/a", vec![turn("t1", 5)])], vec![]);
        assert!(compute(&[old], &[], TODAY, 30, 0, Lang::Ko).is_empty());
    }

    #[test]
    fn session_attributed_days_are_counted() {
        let mut r = report("2026-09-20", vec![project("/r/a", vec![turn("t1", 5)])], vec![]);
        r.turn_files_read = Some(false);
        assert_eq!(find(&compute(&[r], &[], TODAY, 30, 0, Lang::Ko), "/r/a").session_attributed_days, 1);
    }

    #[test]
    fn a_kind_on_three_days_is_a_check_two_is_not() {
        let three = compute(&days_of_kind("test_fail", &["2026-09-20", "2026-09-21", "2026-09-22"]), &[], TODAY, 30, 0, Lang::Ko);
        let c = checks(&three, "/r/a", "repeat_kind");
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].now, c[0].evidence.len()), (3, 3));
        let two = compute(&days_of_kind("test_fail", &["2026-09-20", "2026-09-21"]), &[], TODAY, 30, 0, Lang::Ko);
        assert!(checks(&two, "/r/a", "repeat_kind").is_empty());
        let unclassified = compute(&days_of_kind("", &["2026-09-20", "2026-09-21", "2026-09-22"]), &[], TODAY, 30, 0, Lang::Ko);
        assert!(checks(&unclassified, "/r/a", "repeat_kind").is_empty());
    }

    #[test]
    fn a_command_failing_on_two_days_is_a_check() {
        let day = |d: &str, e: &str| {
            let mut t = turn("t1", 5);
            t.errors = vec![e.into()];
            report(d, vec![project("/r/a", vec![t])], vec![])
        };
        let h = compute(
            &[day("2026-09-20", "Bash `cd /r && pnpm --filter @x/api test -- --run a`: FAIL"), day("2026-09-21", "Bash `pnpm --filter @x/api test 2>&1 | tail -5`: FAIL")],
            &[],
            TODAY,
            30,
            0,
            Lang::Ko,
        );
        let c = checks(&h, "/r/a", "repeat_error");
        assert_eq!(c.len(), 1);
        assert!(c[0].title.contains("pnpm --filter @x/api test"), "{}", c[0].title);
        let one = compute(&[day("2026-09-20", "Bash `cargo test`: x")], &[], TODAY, 30, 0, Lang::Ko);
        assert!(checks(&one, "/r/a", "repeat_error").is_empty());
    }

    #[test]
    fn blocked_share_needs_over_twenty_percent_of_two_hours() {
        let share = |active: u64, blocked: u64| {
            let r = report("2026-09-20", vec![project("/r/a", vec![turn("t1", blocked), turn("t2", active - blocked)])], vec![blocker("slow", &["t1"])]);
            compute(&[r], &[], TODAY, 30, 0, Lang::Ko)
        };
        let over = share(120, 25);
        let c = checks(&over, "/r/a", "blocked_share");
        assert_eq!(c.len(), 1);
        // The title keeps a decimal so a check that fires above 20% never reads "20%".
        assert_eq!(c[0].now, 21);
        assert!(c[0].title.contains("20.8%"), "{}", c[0].title);
        assert!(checks(&share(120, 24), "/r/a", "blocked_share").is_empty()); // exactly 20%
        assert!(checks(&share(100, 50), "/r/a", "blocked_share").is_empty()); // under two hours
    }

    #[test]
    fn issues_open_over_seven_days_are_checks_closed_ones_are_not() {
        let r = report("2026-09-21", vec![project("/r/a", vec![turn("t1", 5)])], vec![blocker("env", &["t1"])]);
        let issues = [
            issue("o1", "a", "2026-09-22", None),
            issue("o2", "a", "2026-09-23", None),
            issue("o3", "a", "2026-09-10", Some("2026-09-11")),
            issue("o4", "gone", "2026-09-01", None),
        ];
        let h = compute(&[r], &issues, TODAY, 30, 0, Lang::Ko);
        let a = find(&h, "/r/a");
        let open: Vec<&str> = a.open_issues.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(open, ["o1", "o2"]);
        let old = checks(&h, "/r/a", "old_issue");
        assert_eq!(old.len(), 1);
        assert!(old[0].title.contains("풀렸다는 기록이 없음"), "{}", old[0].title);
        assert_eq!(old[0].now, 8);
        let unknown = find(&h, "");
        assert_eq!((unknown.name.as_str(), unknown.open_issues.len()), ("프로젝트 모름", 1));
        assert_eq!(a.issues_tracked_since.as_deref(), Some("2026-09-01"));
    }

    #[test]
    fn a_blocker_resolved_the_next_day_is_not_open() {
        let r = report("2026-09-21", vec![project("/r/a", vec![turn("t1", 5)])], vec![blocker("env", &["t1"])]);
        let h = compute(&[r], &[issue("o1", "a", "2026-09-21", Some("2026-09-22"))], TODAY, 30, 0, Lang::Ko);
        let a = find(&h, "/r/a");
        assert!(a.open_issues.is_empty());
        assert_eq!(a.kinds[0].unresolved_that_day, 1);
    }

    #[test]
    fn checks_say_what_the_period_before_had() {
        let mut reports = days_of_kind("test_fail", &["2026-09-20", "2026-09-21", "2026-09-22"]);
        reports.extend(days_of_kind("test_fail", &["2026-08-20"]));
        let h = compute(&reports, &[], TODAY, 30, 0, Lang::Ko);
        let c = checks(&h, "/r/a", "repeat_kind");
        assert_eq!(c[0].before, Some(1));
        assert!(c[0].title.contains("이전 30일 1일"), "{}", c[0].title);
    }

    #[test]
    fn windows_read_differently_are_not_compared() {
        // The previous window fell back to session attribution while this one read
        // turn files: the counts are not like for like, so no "before" is shown.
        let mut prev = days_of_kind("test_fail", &["2026-08-20"]);
        prev[0].turn_files_read = Some(false);
        let mut reports = days_of_kind("test_fail", &["2026-09-20", "2026-09-21", "2026-09-22"]);
        reports.extend(prev);
        let h = compute(&reports, &[], TODAY, 30, 0, Lang::Ko);
        let c = checks(&h, "/r/a", "repeat_kind");
        assert_eq!(c[0].before, None);
        assert!(!c[0].title.contains("이전"), "{}", c[0].title);
        assert!(find(&h, "/r/a").previous.is_none());
    }

    #[test]
    fn failed_commands_are_normalized() {
        let f = |e: &str| failed_command(e);
        assert_eq!(f("Bash `pnpm --filter @acme/api test -- --run x`: FAIL").as_deref(), Some("pnpm --filter @acme/api test"));
        assert_eq!(f("Bash `cd /r/app && cargo test -p porch-core foo 2>&1 | tail -5`: e").as_deref(), Some("cargo test -p porch-core"));
        assert_eq!(f("명령 `npm run build` 종료 코드 1: x").as_deref(), Some("npm run"));
        assert_eq!(f("Bash `make`: x").as_deref(), Some("make"));
        assert_eq!(f("Bash `cargo clippy --all-targets -- -D warnings…`: x").as_deref(), Some("cargo clippy"));
        assert_eq!(f("Edit login.rs: String not found"), None);
        assert_eq!(f("도구: something"), None);
    }

    #[test]
    fn package_flags_share_one_key() {
        let f = |e: &str| failed_command(e);
        assert_eq!(f("Bash `pnpm --filter=@a test`: x").as_deref(), Some("pnpm --filter @a test"));
        assert_eq!(f("Bash `cargo test -p=porch-core`: x").as_deref(), Some("cargo test -p porch-core"));
        assert_eq!(f("Bash `cargo test --package=porch-core`: x").as_deref(), Some("cargo test -p porch-core"));
        assert_ne!(f("Bash `pnpm --filter=@a test`: x"), f("Bash `pnpm --filter=@b test`: x"));
    }

    fn env_day(date: &str, root: &str) -> Report {
        report(date, vec![project(root, vec![turn("t1", 5)])], vec![blocker("env", &["t1"])])
    }

    #[test]
    fn target_count_counts_kind_and_command_in_the_window() {
        let mut t = turn("t1", 5);
        t.errors = vec!["Bash `cargo test`: boom".into(), "Bash `cargo test -- x`: boom".into()];
        let reports = vec![
            env_day("2026-09-10", "/r/a"),
            report("2026-09-11", vec![project("/r/a", vec![t])], vec![blocker("env", &["t1"]), blocker("env", &["t1"])]),
            env_day("2026-09-20", "/r/a"),
            env_day("2026-09-11", "/r/b"),
        ];
        assert_eq!(target_count(&reports, "/r/a", "kind:env", "2026-09-10", "2026-09-11"), Count { items: 3, days: 2 });
        assert_eq!(target_count(&reports, "/r/a", "command:cargo test", "2026-09-01", "2026-09-30"), Count { items: 2, days: 1 });
        assert_eq!(target_count(&reports, "common", "kind:env", "2026-09-11", "2026-09-11"), Count { items: 3, days: 1 });
        assert_eq!(target_count(&reports, "/r/a", "nonsense", "2026-09-01", "2026-09-30"), Count::default());
    }

    #[test]
    fn kind_targets_match_like_health_does() {
        let r = report("2026-09-10", vec![project("/r/a", vec![turn("t1", 5)])], vec![blocker(" Env ", &["t1"])]);
        assert_eq!(target_count(&[r], "/r/a", "kind:env", "2026-09-10", "2026-09-10").items, 1);
    }

    #[test]
    fn effect_compares_equal_windows_around_applied_on() {
        // Applied 09-21, today 09-30: N = 10. Before 09-11..09-20, after 09-21..09-30.
        let reports = vec![env_day("2026-09-12", "/r/a"), env_day("2026-09-15", "/r/a"), env_day("2026-09-25", "/r/a"), env_day("2026-09-05", "/r/a")];
        let e = effect(&reports, "/r/a", "kind:env", "2026-09-21", "2026-09-30", 0);
        assert_eq!((e.days, e.state), (10, EffectState::Measured));
        assert_eq!((e.before.items, e.after.items), (2, 1));
    }

    #[test]
    fn effect_is_early_under_seven_days() {
        let reports = vec![env_day("2026-09-28", "/r/a")];
        assert_eq!(effect(&reports, "/r/a", "kind:env", "2026-09-25", "2026-09-30", 0).state, EffectState::Early); // N = 6
        assert_eq!(effect(&reports, "/r/a", "kind:env", "2026-09-24", "2026-09-30", 0).state, EffectState::Measured); // N = 7
    }

    #[test]
    fn effect_without_records_after_says_so() {
        let reports = vec![env_day("2026-09-01", "/r/a")];
        assert_eq!(effect(&reports, "/r/a", "kind:env", "2026-09-10", "2026-09-30", 0).state, EffectState::NoRecords);
    }

    #[test]
    fn effect_across_differently_read_windows_is_uncomparable() {
        let mut old = env_day("2026-09-12", "/r/a");
        old.turn_files_read = Some(false);
        let reports = vec![old, env_day("2026-09-25", "/r/a")];
        assert_eq!(effect(&reports, "/r/a", "kind:env", "2026-09-21", "2026-09-30", 0).state, EffectState::Uncomparable);
    }

    #[test]
    fn effect_window_caps_at_thirty_days() {
        let e = effect(&[env_day("2026-09-25", "/r/a")], "/r/a", "kind:env", "2026-07-01", "2026-09-30", 0);
        assert_eq!(e.days, 30);
    }
}
