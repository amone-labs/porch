//! Next week's goal: one per finished week, compared
//! with the week after over a period of the same length. A before/after
//! comparison only; nothing here says the goal caused the change.

use crate::insight::{self, Checkpoint, Observed};
use crate::insight_eval::{self, Evaluation, ITEMS, MIN_REQUESTS};
use crate::lang::Lang;
use crate::{paths, summary, time};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_CANDIDATES: usize = 4;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    /// The finished week it was chosen on; the week after is compared with it.
    pub week_start: String,
    /// "stop_failures" | "tool_errors:<tool>" | "waiting_ms" | "score:<item>" | "pivots:redo"
    pub key: String,
    /// In the language it was chosen in; the screen names goals from `key`.
    pub label: String,
    pub baseline: u64,
    pub unit: String,
    /// The period `baseline` covers, local midnight to local midnight.
    pub baseline_from: u64,
    pub baseline_to: u64,
    pub set_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    goals: Vec<Goal>,
}

pub fn store_path() -> PathBuf {
    paths::data_dir().join("goals.json")
}

/// The saved goals; none when the file is missing or does not parse (the
/// next save writes it again).
pub fn read_in(path: &Path) -> Vec<Goal> {
    fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Store>(&t).ok()).map(|s| s.goals).unwrap_or_default()
}

pub fn load() -> Vec<Goal> {
    read_in(&store_path())
}

/// Read, change and save the goals under one lock shared by the app and the CLI.
fn with_goals_in<T>(path: &Path, f: impl FnOnce(&mut Vec<Goal>) -> Result<T, String>) -> Result<T, String> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock")).map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let mut goals = read_in(path);
    let out = f(&mut goals)?;
    let text = serde_json::to_string_pretty(&Store { goals }).map_err(|e| e.to_string())?;
    summary::save_json(path, &text)?;
    Ok(out)
}

/// The number `key` counts over a week, or None when that week has nothing to count it from.
pub fn value_of(key: &str, o: &Observed, ev: Option<&Evaluation>) -> Option<u64> {
    match key.split_once(':') {
        None if key == "stop_failures" => Some(o.stop_failures() as u64),
        None if key == "waiting_ms" => Some(o.waiting_ms()),
        Some(("tool_errors", tool)) if !tool.is_empty() => Some(o.days.iter().filter_map(|d| d.tool_errors.get(tool)).sum::<usize>() as u64),
        Some(("score", item)) if ITEMS.contains(&item) => ev?.scores.iter().find(|s| s.item == item).map(|s| u64::from(s.score)),
        Some(("pivots", "redo")) => ev.map(|e| e.pivot_counts.redo as u64),
        _ => None,
    }
}

/// How a goal on `key` is named and counted in `lang`: (label, unit). None for an unknown key.
pub fn describe(key: &str, lang: Lang) -> Option<(String, String)> {
    let en = lang == Lang::En;
    let pick = |ko: String, ko_unit: &str, en_label: String, en_unit: &str| if en { (en_label, en_unit.to_owned()) } else { (ko, ko_unit.to_owned()) };
    Some(match key.split_once(':') {
        None if key == "stop_failures" => pick("오류로 멈춤 줄이기".into(), "회", "Fewer stops on error".into(), " times"),
        None if key == "waiting_ms" => pick("멈춰 있던 시간 줄이기".into(), "", "Less idle time".into(), ""),
        Some(("tool_errors", tool)) if !tool.is_empty() => pick(format!("{tool} 오류 줄이기"), "건", format!("Fewer {tool} errors"), " errors"),
        Some(("score", item)) => {
            let c = insight_eval::rubric(lang).iter().find(|c| c.item == item)?;
            pick(format!("{} 개선하기", c.name), "점", format!("Better {}", c.name.to_lowercase()), "/5")
        }
        Some(("pivots", "redo")) => pick("다시 말한 요청 줄이기".into(), "건", "Fewer repeated requests".into(), " requests"),
        _ => return None,
    })
}

/// "71건", "27 times", "8.0시간": a goal's number as the CLI and the model's material write it.
pub fn format_value(key: &str, value: u64, unit: &str, lang: Lang) -> String {
    if key == "waiting_ms" {
        insight::hm(value, lang)
    } else {
        let unit = if lang == Lang::En && value == 1 {
            match unit {
                " times" => " time",
                " errors" => " error",
                " requests" => " request",
                other => other,
            }
        } else {
            unit
        };
        format!("{value}{unit}")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    pub key: String,
    pub label: String,
    pub value: u64,
    pub unit: String,
    /// "observed" | "score" | "pivots"
    pub kind: &'static str,
}

/// What a finished week offers as next week's goal (up to 4): the
/// checkpoints' numbers, items scored 3 or lower, and repeated requests.
pub fn candidates(checkpoints: &[Checkpoint], o: &Observed, ev: Option<&Evaluation>, lang: Lang) -> Vec<Candidate> {
    let mut keys: Vec<(String, &'static str)> = Vec::new();
    for c in checkpoints {
        if !keys.iter().any(|(k, _)| *k == c.goal_key) {
            keys.push((c.goal_key.clone(), "observed"));
        }
    }
    if let Some(e) = ev {
        keys.extend(e.scores.iter().filter(|s| s.score <= 3).map(|s| (format!("score:{}", s.item), "score")));
        if e.pivot_counts.redo > 0 {
            keys.push(("pivots:redo".into(), "pivots"));
        }
    }
    keys.into_iter()
        .filter_map(|(key, kind)| {
            let value = value_of(&key, o, ev)?;
            let (label, unit) = describe(&key, lang)?;
            Some(Candidate { key, label, value, unit, kind })
        })
        .take(MAX_CANDIDATES)
        .collect()
}

/// When a week's counting began: its Monday, or the first recorded day when porch started mid-week.
fn covered_from(o: &Observed, offset: i64) -> u64 {
    let day = o.recorded_from.as_deref().filter(|d| *d > o.week_start.as_str()).unwrap_or(&o.week_start);
    time::local_day_window(day, offset).map_or(0, |(s, _)| s)
}

/// Whole local days between two instants; an hour gained or lost to summer time does not count.
fn days(from: u64, to: u64) -> u64 {
    (to.saturating_sub(from) + DAY_MS / 2) / DAY_MS
}

/// Choose `key` as the goal set on the finished week `o`. One per week: a new choice replaces it.
pub fn set_goal_in(path: &Path, o: &Observed, ev: Option<&Evaluation>, key: &str, now: u64, offset: i64, lang: Lang) -> Result<Goal, String> {
    if o.through < o.week_end {
        return Err(match lang {
            Lang::Ko => "주가 끝나면 다음 주 목표를 고를 수 있습니다".into(),
            Lang::En => "You can choose next week's goal once this week ends".into(),
        });
    }
    let (Some(baseline), Some((label, unit))) = (value_of(key, o, ev), describe(key, lang)) else {
        return Err(match lang {
            Lang::Ko => format!("이 주에는 고를 수 없는 목표입니다: {key}"),
            Lang::En => format!("That goal can't be chosen for this week: {key}"),
        });
    };
    let goal = Goal { week_start: o.week_start.clone(), key: key.into(), label, baseline, unit, baseline_from: covered_from(o, offset), baseline_to: o.through, set_at: now };
    with_goals_in(path, |goals| {
        goals.retain(|g| g.week_start != goal.week_start);
        goals.push(goal.clone());
        Ok(goal.clone())
    })
}

pub fn clear_goal_in(path: &Path, week_start: &str) -> Result<(), String> {
    with_goals_in(path, |goals| {
        goals.retain(|g| g.week_start != week_start);
        Ok(())
    })
}

/// Choose `key` as the goal on the finished week holding `any_date`.
pub fn set_goal(any_date: &str, key: &str, offset: i64, now: u64, lang: Lang) -> Result<Goal, String> {
    let monday = summary::week_start(any_date, offset).ok_or_else(|| time::bad_date(lang))?;
    let o = insight_eval::observed(&monday, offset, now, lang)?;
    let ev = insight_eval::load(&monday).and_then(|r| r.evaluation);
    set_goal_in(&store_path(), &o, ev.as_ref(), key, now, offset, lang)
}

pub fn clear_goal(any_date: &str, offset: i64, lang: Lang) -> Result<(), String> {
    let monday = summary::week_start(any_date, offset).ok_or_else(|| time::bad_date(lang))?;
    clear_goal_in(&store_path(), &monday)
}

/// How the week after compares with a goal's baseline (`goal_result`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GoalResult {
    pub before: u64,
    /// The week after, once it ended and is comparable.
    pub after: Option<u64>,
    /// While the week after is still running: its count so far, never compared.
    pub so_far: Option<u64>,
    pub comparable: bool,
    /// "in_progress" | "coverage" | "few_requests" | "no_evaluation"
    pub reason: Option<&'static str>,
}

/// Compare `g` with the week after it (`o`, and that week's evaluation `ev`):
/// only a finished week, of the same length in local days, with enough
/// requests, and for a score or repeat goal, evaluated.
pub fn goal_result(g: &Goal, o: &Observed, ev: Option<&Evaluation>, offset: i64) -> GoalResult {
    let value = value_of(&g.key, o, ev);
    let not = |reason| GoalResult { before: g.baseline, after: None, so_far: None, comparable: false, reason: Some(reason) };
    if o.through < o.week_end {
        return GoalResult { so_far: value, ..not("in_progress") };
    }
    if days(covered_from(o, offset), o.through) != days(g.baseline_from, g.baseline_to) {
        return not("coverage");
    }
    if o.requests < MIN_REQUESTS {
        return not("few_requests");
    }
    match value {
        Some(v) => GoalResult { before: g.baseline, after: Some(v), so_far: None, comparable: true, reason: None },
        None => not("no_evaluation"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insight::DayObs;
    use crate::insight_eval::{PivotCounts, Score};

    fn finished(monday: &str, requests: usize) -> Observed {
        let end = insight::week_end(monday, 0);
        let mut days: Vec<DayObs> = (0..7).map(|i| DayObs { date: time::add_days(monday, i).unwrap(), ..Default::default() }).collect();
        days[2].stop_failures = 27;
        days[3].tool_errors.insert("Bash".into(), 71);
        days[3].turn_done_ms = 8 * 3_600_000;
        Observed { week_start: monday.into(), through: end, week_end: end, days, requests, hours_waiting_ms: vec![0; 24], ..Default::default() }
    }

    fn eval(scores: &[(&str, u8)], redo: usize) -> Evaluation {
        Evaluation {
            scores: scores.iter().map(|(i, s)| Score { item: (*i).into(), score: *s, ..Default::default() }).collect(),
            pivot_counts: PivotCounts { redo, ..Default::default() },
            ..Default::default()
        }
    }

    fn goal(key: &str, baseline: u64, monday: &str) -> Goal {
        let from = time::local_day_window(monday, 0).unwrap().0;
        Goal { week_start: monday.into(), key: key.into(), baseline, baseline_from: from, baseline_to: insight::week_end(monday, 0), ..Default::default() }
    }

    #[test]
    fn english_units_read_singular_at_one() {
        let en = |key: &str| describe(key, Lang::En).unwrap().1;
        assert_eq!(format_value("stop_failures", 1, &en("stop_failures"), Lang::En), "1 time");
        assert_eq!(format_value("stop_failures", 2, &en("stop_failures"), Lang::En), "2 times");
        assert_eq!(format_value("tool_errors:Bash", 1, &en("tool_errors:Bash"), Lang::En), "1 error");
        assert_eq!(format_value("tool_errors:Bash", 2, &en("tool_errors:Bash"), Lang::En), "2 errors");
        assert_eq!(format_value("pivots:redo", 1, &en("pivots:redo"), Lang::En), "1 request");
        assert_eq!(format_value("pivots:redo", 2, &en("pivots:redo"), Lang::En), "2 requests");
        assert_eq!(format_value("score:clarity", 1, &en("score:clarity"), Lang::En), "1/5");
        let ko = |key: &str| describe(key, Lang::Ko).unwrap().1;
        assert_eq!(format_value("stop_failures", 1, &ko("stop_failures"), Lang::Ko), "1회");
        assert_eq!(format_value("tool_errors:Bash", 1, &ko("tool_errors:Bash"), Lang::Ko), "1건");
    }

    #[test]
    fn value_of_counts_each_key() {
        let o = finished("2026-09-28", 40);
        let e = eval(&[("clarity", 3)], 2);
        assert_eq!(value_of("stop_failures", &o, None), Some(27));
        assert_eq!(value_of("tool_errors:Bash", &o, None), Some(71));
        assert_eq!(value_of("tool_errors:Edit", &o, None), Some(0));
        assert_eq!(value_of("waiting_ms", &o, None), Some(8 * 3_600_000));
        assert_eq!(value_of("score:clarity", &o, Some(&e)), Some(3));
        assert_eq!(value_of("score:verify", &o, Some(&e)), None);
        assert_eq!(value_of("score:clarity", &o, None), None);
        assert_eq!(value_of("pivots:redo", &o, Some(&e)), Some(2));
        assert_eq!(value_of("mood", &o, Some(&e)), None);
    }

    #[test]
    fn candidates_come_from_checkpoints_low_scores_and_repeats() {
        let o = finished("2026-09-28", 40);
        let cps = insight::checkpoints(&o, Lang::Ko);
        let keys = |ev: Option<&Evaluation>| candidates(&cps, &o, ev, Lang::Ko).into_iter().map(|c| c.key).collect::<Vec<_>>();
        assert_eq!(keys(None), ["stop_failures"]);
        let e = eval(&[("verify", 4), ("delegate", 2), ("clarity", 3), ("rationale", 1)], 1);
        assert_eq!(keys(Some(&e)), ["stop_failures", "score:delegate", "score:clarity", "score:rationale"]);
        let c = &candidates(&cps, &o, Some(&e), Lang::Ko)[1];
        assert_eq!((c.label.as_str(), c.value, c.unit.as_str(), c.kind), ("역할 분담 개선하기", 2, "점", "score"));
        let en = &candidates(&cps, &o, Some(&eval(&[], 1)), Lang::En)[1];
        assert_eq!((en.key.as_str(), en.label.as_str()), ("pivots:redo", "Fewer repeated requests"));
    }

    #[test]
    fn a_goal_is_set_only_on_a_finished_week_and_replaced_by_a_new_choice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.json");
        let mut o = finished("2026-09-28", 40);
        let end = o.week_end;
        o.through = end - 1;
        assert!(set_goal_in(&path, &o, None, "stop_failures", end, 0, Lang::Ko).is_err());
        o.through = end;
        let g = set_goal_in(&path, &o, None, "stop_failures", end, 0, Lang::Ko).unwrap();
        assert_eq!((g.baseline, g.unit.as_str(), g.baseline_to - g.baseline_from), (27, "회", 7 * DAY_MS));
        set_goal_in(&path, &o, None, "tool_errors:Bash", end, 0, Lang::Ko).unwrap();
        let saved = read_in(&path);
        assert_eq!(saved.len(), 1);
        assert_eq!((saved[0].key.as_str(), saved[0].baseline, saved[0].label.as_str()), ("tool_errors:Bash", 71, "Bash 오류 줄이기"));
        assert!(set_goal_in(&path, &o, None, "score:clarity", end, 0, Lang::Ko).is_err(), "no evaluation, no score goal");
        clear_goal_in(&path, "2026-09-28").unwrap();
        assert!(read_in(&path).is_empty());
    }

    #[test]
    fn a_broken_goals_file_reads_empty_and_is_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.json");
        fs::write(&path, "{not json").unwrap();
        assert!(read_in(&path).is_empty());
        let o = finished("2026-09-28", 40);
        set_goal_in(&path, &o, None, "waiting_ms", o.week_end, 0, Lang::En).unwrap();
        assert_eq!(read_in(&path)[0].label, "Less idle time");
    }

    #[test]
    fn a_goal_compares_only_a_finished_week_of_the_same_length() {
        let g = goal("stop_failures", 40, "2026-09-28");
        let r = goal_result(&g, &finished("2026-10-05", 40), None, 0);
        assert_eq!((r.before, r.after, r.comparable, r.reason), (40, Some(27), true, None));

        let mut running = finished("2026-10-05", 40);
        running.through -= DAY_MS;
        let r = goal_result(&g, &running, None, 0);
        assert_eq!((r.so_far, r.after, r.comparable, r.reason), (Some(27), None, false, Some("in_progress")));

        let mut short = goal("stop_failures", 40, "2026-09-28");
        short.baseline_from += 3 * DAY_MS;
        let r = goal_result(&short, &finished("2026-10-05", 40), None, 0);
        assert_eq!((r.after, r.comparable, r.reason), (None, false, Some("coverage")));

        assert_eq!(goal_result(&g, &finished("2026-10-05", 19), None, 0).reason, Some("few_requests"));

        let score = goal("score:clarity", 3, "2026-09-28");
        assert_eq!(goal_result(&score, &finished("2026-10-05", 40), None, 0).reason, Some("no_evaluation"));
        let r = goal_result(&score, &finished("2026-10-05", 40), Some(&eval(&[("clarity", 4)], 0)), 0);
        assert_eq!((r.after, r.comparable), (Some(4), true));
    }

    #[test]
    fn a_week_an_hour_short_for_summer_time_still_compares() {
        let mut g = goal("stop_failures", 40, "2026-09-28");
        g.baseline_to -= 3_600_000;
        assert!(goal_result(&g, &finished("2026-10-05", 40), None, 0).comparable);
    }
}
