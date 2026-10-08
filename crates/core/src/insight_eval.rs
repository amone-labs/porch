//! Weekly insight, the model-written part (ADR 0012): once a week has ended and
//! its weekly summary exists, the summary agent reads the week's first requests
//! and the direction changes and request notes the day summaries picked, and
//! scores four items against the rubric. Every score must quote a request on
//! record; one that does not match is dropped. The rubric's sentences come only
//! from `RUBRIC_KO`/`RUBRIC_EN`, never from the model.

use crate::insight::{self, Observed};
use crate::lang::Lang;
use crate::summary::{self, Pivot, Report};
use crate::writer::{self, Engine, Provider};
use crate::{goals, time};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The four items, in the order they are shown.
pub const ITEMS: [&str; 4] = ["verify", "delegate", "clarity", "rationale"];
/// A week with fewer requests is not evaluated, and a goal is not compared over it.
pub const MIN_REQUESTS: usize = 20;
const MAX_PIVOTS: usize = 5;
const MAX_REWRITES: usize = 2;
/// The evaluation's quotes are cut to this many characters.
const QUOTE_CHARS: usize = 160;

/// One item of the rubric: its name, the question it asks, and
/// what each score from 1 to 5 looks like.
pub struct Criterion {
    pub item: &'static str,
    pub name: &'static str,
    pub question: &'static str,
    pub levels: [&'static str; 5],
}

static RUBRIC_KO: [Criterion; 4] = [
    Criterion {
        item: "verify",
        name: "검증 방식",
        question: "만든 결과를 다른 방법으로 다시 확인합니까?",
        levels: ["결과를 확인 없이 받아들임", "결과를 읽어 보는 정도로만 확인", "테스트나 실행 한 가지로 확인", "만든 결과를 다른 도구로 다시 확인함", "확인 결과로 무엇을 고쳤는지까지 남김"],
    },
    Criterion {
        item: "delegate",
        name: "역할 분담",
        question: "판단은 직접 하고 반복 작업은 맡깁니까?",
        levels: ["방향 판단까지 에이전트에 맡김", "판단과 작성을 나누지 않고 맡김", "판단은 직접 하지만 맡긴 범위가 드러나지 않음", "판단은 직접 하고 작성은 에이전트에 맡김", "맡긴 일의 범위와 끝낼 조건까지 정함"],
    },
    Criterion {
        item: "clarity",
        name: "요청 명확성",
        question: "첫 요청에 원하는 결과의 모습을 적습니까?",
        levels: ["목적 없이 할 일만 적음", "목적은 있으나 대안이나 조건이 없음", "목적과 대안은 적고, 결과의 모습은 나중에 보탬", "원하는 결과의 모습까지 첫 요청에 적음", "결과의 모습과 끝낼 조건까지 첫 요청에 적음"],
    },
    Criterion {
        item: "rationale",
        name: "결정 근거",
        question: "방향을 바꿀 때 이유를 남깁니까?",
        levels: ["방향을 바꾸며 이유를 말하지 않음", "이유를 묻기는 하나 직접 말한 이유는 드묾", "방향을 바꿀 때 이유를 일부 말함", "방향을 바꿀 때마다 이유를 함께 말함", "이유와 함께 버린 안과 감수할 점까지 말함"],
    },
];

static RUBRIC_EN: [Criterion; 4] = [
    Criterion {
        item: "verify",
        name: "Verification",
        question: "Does the person check what was made by another means?",
        levels: ["Accepts results without checking", "Checks only by reading the result", "Checks with one test or run", "Checks the result again with a different tool", "Also records what the check led to fixing"],
    },
    Criterion {
        item: "delegate",
        name: "Delegation",
        question: "Does the person make the calls and hand off the repetitive work?",
        levels: ["Leaves even the direction to the agent", "Hands off without separating decisions from writing", "Makes the calls, but the handed-off scope is unclear", "Makes the calls and has the agent do the writing", "Also sets the scope of the handed-off work and when it is done"],
    },
    Criterion {
        item: "clarity",
        name: "Request clarity",
        question: "Does the first request describe what the result should look like?",
        levels: ["Lists tasks with no goal", "Has a goal but no alternatives or conditions", "States the goal and alternatives; the shape of the result comes later", "Describes the shape of the result in the first request", "Also states when the work is done in the first request"],
    },
    Criterion {
        item: "rationale",
        name: "Decision rationale",
        question: "When changing direction, does the person say why?",
        levels: ["Changes direction without saying why", "Asks for reasons, but rarely gives one", "Gives a reason for some changes of direction", "Gives a reason with every change of direction", "Also names the rejected option and the tradeoff accepted"],
    },
];

pub fn rubric(lang: Lang) -> &'static [Criterion; 4] {
    match lang {
        Lang::Ko => &RUBRIC_KO,
        Lang::En => &RUBRIC_EN,
    }
}

fn criterion(item: &str, lang: Lang) -> Option<&'static Criterion> {
    rubric(lang).iter().find(|c| c.item == item)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub item: String,
    pub score: u8,
    /// "10-04.t1"; empty only for `rationale`, which may rest on the week's direction changes as a whole.
    #[serde(default)]
    pub turn: String,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub why: String,
    /// The rubric's sentence for this score and for one step up (None at 5), from `rubric`.
    #[serde(default)]
    pub criterion: String,
    #[serde(default)]
    pub next_criterion: Option<String>,
}

/// A day summary's direction change, picked for the week. `pivot.turn` is the week id ("10-04.t9").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WeekPivot {
    pub date: String,
    #[serde(flatten)]
    pub pivot: Pivot,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rewrite {
    #[serde(default)]
    pub turn: String,
    /// Filled in by porch from `turn`.
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub rewritten: String,
    /// The part of `rewritten` that is new: empty, or word for word part of it.
    #[serde(default)]
    pub added: String,
    #[serde(default)]
    pub missing: Vec<String>,
    /// Why the request had to be said again, in one sentence.
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PivotCounts {
    pub total: usize,
    pub with_reason: usize,
    pub redo: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub evaluated_at: u64,
    /// `observed.through` when it was written: the period it read.
    pub evaluated_through: u64,
    /// `Lang::code` of the text it was written in.
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub scores: Vec<Score>,
    #[serde(default)]
    pub pivots: Vec<WeekPivot>,
    #[serde(default)]
    pub rewrites: Vec<Rewrite>,
    /// Over every direction change the week's day summaries named, not only the ones picked.
    #[serde(default)]
    pub pivot_counts: PivotCounts,
    /// Scores, direction changes and rewrites `clean` dropped.
    #[serde(default)]
    pub dropped: usize,
}

/// One week's file, `reports/insight-{monday}.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InsightReport {
    pub week_start: String,
    /// The last time this file was written by an evaluation run.
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub provider: Option<Provider>,
    #[serde(default)]
    pub model: Option<String>,
    /// The week's final numbers once it has ended (`observed.through >= observed.week_end`).
    #[serde(default)]
    pub observed: Observed,
    #[serde(default)]
    pub evaluation: Option<Evaluation>,
    /// "off" | "few_requests" | "no_day_summaries": why the last run did not ask the model.
    #[serde(default)]
    pub skipped: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    /// Items the person marked "Mark as wrong"; next week's evaluation reads them.
    #[serde(default)]
    pub disputed: Vec<String>,
    #[serde(default)]
    pub days_without_summary: Vec<String>,
}

pub fn report_path_in(dir: &Path, monday: &str) -> PathBuf {
    dir.join(format!("insight-{monday}.json"))
}

pub fn load_in(dir: &Path, monday: &str) -> Option<InsightReport> {
    fs::read_to_string(report_path_in(dir, monday)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

pub fn load(monday: &str) -> Option<InsightReport> {
    load_in(&summary::reports_dir(), monday)
}

/// Read, change and save one week's file under a lock shared by the app and
/// the CLI. The change is made on the file as it is now, so a dispute marked
/// while the model runs is not lost.
pub(crate) fn with_report_in<T>(dir: &Path, monday: &str, f: impl FnOnce(&mut InsightReport) -> T) -> Result<T, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = report_path_in(dir, monday);
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock")).map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let mut r = load_in(dir, monday).unwrap_or_else(|| InsightReport { week_start: monday.to_owned(), ..Default::default() });
    let out = f(&mut r);
    summary::save_json(&path, &serde_json::to_string(&r).map_err(|e| e.to_string())?)?;
    Ok(out)
}

/// The week's numbers: a running week is counted
/// fresh and never saved; a finished week is counted once more if what is
/// saved stopped short of its end, then saved and used as is.
pub(crate) fn observed_in(dir: &Path, monday: &str, end: u64, now: u64, fresh: impl FnOnce() -> Result<Observed, String>) -> Result<Observed, String> {
    if now < end {
        return fresh();
    }
    if let Some(o) = load_in(dir, monday).map(|r| r.observed).filter(|o| o.week_start == monday && o.through >= end) {
        return Ok(o);
    }
    let o = fresh()?;
    let keep = o.clone();
    with_report_in(dir, monday, move |r| r.observed = keep)?;
    Ok(o)
}

pub fn observed(monday: &str, offset: i64, now: u64, lang: Lang) -> Result<Observed, String> {
    observed_in(&summary::reports_dir(), monday, insight::week_end(monday, offset), now, || insight::observe_week(monday, offset, now, lang))
}

pub struct EvalOpts {
    pub any_date: String,
    pub engine: Engine,
    /// Settings' `insight_eval`: off means the model is not asked.
    pub on: bool,
}

/// What one evaluation reads, gathered from the data folder by `evaluate`.
pub(crate) struct Week {
    pub monday: String,
    pub observed: Observed,
    pub days: Vec<Report>,
    pub has_week_summary: bool,
    pub last_goal: Option<goals::Goal>,
}

/// Write the model evaluation of a finished week, after its weekly summary.
/// A failure is saved as `error` and keeps the evaluation
/// already saved; a skip is saved as `skipped` without asking the model.
pub fn evaluate(o: &EvalOpts, offset: i64, now: u64) -> Result<InsightReport, String> {
    let lang = o.engine.lang;
    let monday = summary::week_start(&o.any_date, offset).ok_or_else(|| time::bad_date(lang))?;
    if now < insight::week_end(&monday, offset) {
        return Err(not_finished(lang));
    }
    let prev = time::add_days(&monday, -7).unwrap_or_default();
    let week = Week {
        observed: observed(&monday, offset, now, lang)?,
        days: (0..7).filter_map(|i| time::add_days(&monday, i)).filter_map(|d| summary::load_day(&d)).collect(),
        has_week_summary: summary::load_week(&monday).is_some_and(|w| w.summary.is_some()),
        last_goal: goals::load().into_iter().find(|g| g.week_start == prev),
        monday,
    };
    evaluate_in(&summary::reports_dir(), week, o.on, now, &o.engine, |text| writer::run(&o.engine, text, eval_prompt(lang)))
}

pub(crate) fn evaluate_in(dir: &Path, w: Week, on: bool, now: u64, engine: &Engine, run: impl FnOnce(&str) -> Result<serde_json::Value, String>) -> Result<InsightReport, String> {
    let lang = engine.lang;
    if w.observed.through < w.observed.week_end {
        return Err(not_finished(lang));
    }
    if !w.has_week_summary {
        return Err(no_week_summary(lang));
    }
    let without = days_without_summary(&w.observed, &w.days);
    let summarized = w.days.iter().filter(|r| r.summary.is_some()).count();
    if let Some(why) = skip_reason(on, w.observed.requests, summarized) {
        return with_report_in(dir, &w.monday, |r| {
            r.skipped = Some(why.to_owned());
            r.error = None;
            r.days_without_summary = without;
            r.generated_at = now;
            r.clone()
        });
    }
    let last = time::add_days(&w.monday, -7).and_then(|prev| load_in(dir, &prev));
    let goal = w.last_goal.as_ref().map(|g| {
        let (label, unit) = goals::describe(&g.key, lang).unwrap_or_else(|| (g.label.clone(), g.unit.clone()));
        format!("{label} ({})", goals::format_value(&g.key, g.baseline, &unit, lang))
    });
    let text = material(
        &Material { monday: &w.monday, observed: &w.observed, days: &w.days, days_without_summary: &without, last_goal: goal.as_deref(), last: last.as_ref() },
        lang,
    );
    let requests = turn_requests(&w.days);
    let cands = pivot_candidates(&w.days);
    let built = run(&text).and_then(|v| serde_json::from_value::<Answer>(v).map_err(|e| bad_answer(lang, &e))).map(|a| {
        let (scores, pivots, rewrites, dropped) = clean(a, &requests, &cands, lang);
        Evaluation {
            evaluated_at: now,
            evaluated_through: w.observed.through,
            lang: Some(lang.code().into()),
            scores,
            pivots,
            rewrites,
            pivot_counts: counts(&cands),
            dropped,
        }
    });
    let saved = with_report_in(dir, &w.monday, |r| {
        r.days_without_summary = without;
        r.generated_at = now;
        r.skipped = None;
        match &built {
            Ok(e) => {
                r.evaluation = Some(e.clone());
                r.error = None;
                r.provider = Some(engine.provider);
                r.model = Some(engine.model_label());
            }
            Err(e) => r.error = Some(e.clone()),
        }
        r.clone()
    })?;
    built.map(|_| saved)
}

/// Mark an item "Mark as wrong"; next week's evaluation reads it.
pub fn dispute_in(dir: &Path, monday: &str, item: &str, lang: Lang) -> Result<InsightReport, String> {
    if !ITEMS.contains(&item) {
        return Err(match lang {
            Lang::Ko => format!("모르는 평가 항목입니다: {item}"),
            Lang::En => format!("Unknown evaluation item: {item}"),
        });
    }
    with_report_in(dir, monday, |r| {
        if !r.disputed.iter().any(|x| x == item) {
            r.disputed.push(item.to_owned());
        }
        r.clone()
    })
}

pub fn dispute(any_date: &str, item: &str, offset: i64, lang: Lang) -> Result<InsightReport, String> {
    let monday = summary::week_start(any_date, offset).ok_or_else(|| time::bad_date(lang))?;
    dispute_in(&summary::reports_dir(), &monday, item, lang)
}

/// When the newest saved evaluation was written, for the sidebar dot.
pub fn latest_evaluated_at_in(dir: &Path) -> Option<u64> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|f| {
            let n = f.file_name().to_string_lossy().into_owned();
            n.starts_with("insight-") && n.ends_with(".json")
        })
        .filter_map(|f| serde_json::from_str::<InsightReport>(&fs::read_to_string(f.path()).ok()?).ok())
        .filter_map(|r| r.evaluation.map(|e| e.evaluated_at))
        .max()
}

pub fn latest_evaluated_at() -> Option<u64> {
    latest_evaluated_at_in(&summary::reports_dir())
}

/// "10-04.t1": a turn of a day, as the week's material names it.
fn week_id(date: &str, turn: &str) -> String {
    format!("{}.{turn}", date.get(5..).unwrap_or(date))
}

/// Every turn of the week's saved days: week id -> (date, request).
pub(crate) fn turn_requests(days: &[Report]) -> BTreeMap<String, (String, String)> {
    days.iter()
        .flat_map(|r| {
            r.digest.projects.iter().flat_map(|p| p.sessions.iter()).flat_map(|s| s.turns.iter()).map(move |t| (week_id(&r.date, &t.id), (r.date.clone(), t.prompt.clone())))
        })
        .collect()
}

/// The direction changes the week's day summaries named, by week id.
pub(crate) fn pivot_candidates(days: &[Report]) -> BTreeMap<String, WeekPivot> {
    let mut out = BTreeMap::new();
    for r in days {
        for p in r.summary.iter().flat_map(|s| s.pivots.iter()) {
            let id = week_id(&r.date, &p.turn);
            out.insert(id.clone(), WeekPivot { date: r.date.clone(), pivot: Pivot { turn: id, ..p.clone() } });
        }
    }
    out
}

pub(crate) fn counts(cands: &BTreeMap<String, WeekPivot>) -> PivotCounts {
    PivotCounts {
        total: cands.len(),
        with_reason: cands.values().filter(|p| p.pivot.reason_given).count(),
        redo: cands.values().filter(|p| p.pivot.kind == "redo").count(),
    }
}

/// What the week's evaluation reads besides the rubric.
pub(crate) struct Material<'a> {
    pub monday: &'a str,
    pub observed: &'a Observed,
    pub days: &'a [Report],
    pub days_without_summary: &'a [String],
    /// Last week's goal as text: "Bash 오류 줄이기 (71건)".
    pub last_goal: Option<&'a str>,
    pub last: Option<&'a InsightReport>,
}

pub(crate) fn material(m: &Material, lang: Lang) -> String {
    let en = lang == Lang::En;
    let o = m.observed;
    let hm = |ms: u64| insight::hm(ms, lang);
    let none = if en { "none" } else { "없음" };
    let or_none = |v: Vec<String>| if v.is_empty() { none.to_owned() } else { v.join(", ") };
    let sunday = time::add_days(m.monday, 6).unwrap_or_default();
    let done_or_asking: u64 = o.days.iter().map(|d| d.turn_done_ms + d.question_ms).sum();
    let without = or_none(m.days_without_summary.iter().map(|d| d.get(5..).unwrap_or(d).to_owned()).collect());
    let goal = m.last_goal.unwrap_or(none);
    let stops = o.stop_failures();
    let stops_times = if stops == 1 { "1 time".to_owned() } else { format!("{stops} times") };
    let last_scores = or_none(m.last.and_then(|r| r.evaluation.as_ref()).map_or(Vec::new(), |e| e.scores.iter().map(|s| format!("{} {}", s.item, s.score)).collect()));
    let disputed = or_none(m.last.map_or(Vec::new(), |r| r.disputed.clone()));
    let mut out = String::new();
    let _ = if en {
        writeln!(
            out,
            "Week: {} to {sunday}\nObserved: {} requests, working {}, idle {} (Done and Has a question {}, Stopped on error {}, {stops_times}), tool errors {} of {} calls\nDays without a daily summary: {without}\nLast week's goal: {goal}\nLast week's scores: {last_scores}\nLast week's disputed items: {disputed}",
            m.monday,
            o.requests,
            hm(o.running_ms()),
            hm(o.waiting_ms()),
            hm(done_or_asking),
            hm(o.failed_ms()),
            o.tool_error_total(),
            o.tool_calls()
        )
    } else {
        writeln!(
            out,
            "주: {} – {sunday}\n관측: 요청 {}, 실행 {}, 멈춰 있던 시간 {}(차례 끝남·질문 대기 {}, 오류로 멈춤 {}, {}회), 도구 오류 {}/{}\n하루 요약이 없는 날: {without}\n지난주 목표: {goal}\n지난주 점수: {last_scores}\n지난주 이의: {disputed}",
            m.monday,
            o.requests,
            hm(o.running_ms()),
            hm(o.waiting_ms()),
            hm(done_or_asking),
            hm(o.failed_ms()),
            o.stop_failures(),
            o.tool_error_total(),
            o.tool_calls()
        )
    };

    let _ = writeln!(out, "\n## {}", if en { "First requests of sessions" } else { "세션 첫 요청" });
    let mut any = false;
    for r in m.days {
        for t in r.digest.projects.iter().flat_map(|p| p.sessions.iter()).flat_map(|s| s.turns.iter()).filter(|t| t.first) {
            let _ = writeln!(out, "[{}] {}", week_id(&r.date, &t.id), summary::squash(&t.prompt));
            any = true;
        }
    }
    if !any {
        let _ = writeln!(out, "{none}");
    }

    let _ = writeln!(out, "\n## {}", if en { "Direction change candidates" } else { "방향 전환 후보" });
    let cands = pivot_candidates(m.days);
    for p in cands.values().map(|w| &w.pivot) {
        let mut reason = match (p.reason_given, en) {
            (true, false) => "이유 있음".to_owned(),
            (false, false) => "이유 없음".to_owned(),
            (true, true) => "reason given".to_owned(),
            (false, true) => "no reason given".to_owned(),
        };
        if !p.note.is_empty() {
            let _ = write!(reason, " · {}", p.note);
        }
        let _ = writeln!(out, "[{}] {} · {} → {} · \"{}\" · {reason}", p.turn, p.kind, p.before, p.after, p.quote);
    }
    if cands.is_empty() {
        let _ = writeln!(out, "{none}");
    }

    let _ = writeln!(out, "\n## {}", if en { "Request notes" } else { "요청 메모" });
    let mut any = false;
    for r in m.days {
        for a in r.summary.iter().flat_map(|s| s.asks.iter()) {
            let missing = if a.missing.is_empty() { none.to_owned() } else { a.missing.join(", ") };
            let _ = writeln!(out, "[{}] \"{}\" · {} {missing}", week_id(&r.date, &a.turn), a.quote, if en { "missing:" } else { "빠진 것:" });
            any = true;
        }
    }
    if !any {
        let _ = writeln!(out, "{none}");
    }

    let _ = writeln!(out, "\n## {}", if en { "Rubric" } else { "평가표" });
    for c in rubric(lang) {
        let _ = writeln!(out, "- {} ({}): {}", c.item, c.name, c.question);
        for (i, l) in c.levels.iter().enumerate() {
            let _ = if en { writeln!(out, "  {}: {l}", i + 1) } else { writeln!(out, "  {}점: {l}", i + 1) };
        }
    }
    out
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawScore {
    #[serde(default)]
    item: String,
    #[serde(default)]
    score: serde_json::Value,
    #[serde(default)]
    turn: String,
    #[serde(default)]
    quote: String,
    #[serde(default)]
    why: String,
}

/// The model's answer as it came, before `clean`.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Answer {
    #[serde(default)]
    scores: Vec<RawScore>,
    #[serde(default)]
    pivots: Vec<Pivot>,
    #[serde(default)]
    rewrites: Vec<Rewrite>,
}

/// A whole number written as 3 or 3.0; anything else (3.5, "3") is not a score.
fn whole(v: &serde_json::Value) -> Option<u8> {
    let n = v.as_u64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))?;
    u8::try_from(n).ok()
}

/// What survives of the model's answer: one score per
/// item, 1 to 5, quoting the request of the turn it names (only `rationale`
/// may name none); direction changes taken as the day summaries wrote them;
/// rewrites whose quote is the request's and whose added part is in the
/// rewrite. The rubric's sentences are attached here. Returns the kept entries
/// and how many were dropped.
pub(crate) fn clean(a: Answer, requests: &BTreeMap<String, (String, String)>, cands: &BTreeMap<String, WeekPivot>, lang: Lang) -> (Vec<Score>, Vec<WeekPivot>, Vec<Rewrite>, usize) {
    let given = a.scores.len() + a.pivots.len() + a.rewrites.len();

    let mut scores: Vec<Score> = Vec::new();
    for s in a.scores {
        let Some(c) = criterion(&s.item, lang) else { continue };
        if scores.iter().any(|x| x.item == s.item) {
            continue;
        }
        let Some(n) = whole(&s.score).filter(|n| (1..=5).contains(n)) else { continue };
        let turn = s.turn.trim();
        let (turn, quote) = if turn.is_empty() {
            if s.item != "rationale" {
                continue;
            }
            (String::new(), String::new())
        } else {
            let Some(q) = requests.get(turn).and_then(|(_, r)| summary::checked_quote(&s.quote, r, QUOTE_CHARS)) else { continue };
            (turn.to_owned(), q)
        };
        scores.push(Score {
            item: s.item,
            score: n,
            turn,
            quote,
            why: summary::strip_ids(&s.why),
            criterion: c.levels[usize::from(n) - 1].to_owned(),
            next_criterion: c.levels.get(usize::from(n)).map(|l| (*l).to_owned()),
        });
    }
    scores.sort_by_key(|s| ITEMS.iter().position(|i| *i == s.item));

    let mut pivots: Vec<WeekPivot> = Vec::new();
    for p in a.pivots {
        if let Some(c) = cands.get(p.turn.trim()) {
            if !pivots.iter().any(|x| x.pivot.turn == c.pivot.turn) {
                pivots.push(c.clone());
            }
        }
    }
    pivots.truncate(MAX_PIVOTS);

    let mut rewrites: Vec<Rewrite> = Vec::new();
    for mut w in a.rewrites {
        let turn = w.turn.trim().to_owned();
        let Some((date, request)) = requests.get(&turn) else { continue };
        let Some(q) = summary::checked_quote(&w.quote, request, QUOTE_CHARS) else { continue };
        w.rewritten = w.rewritten.trim().to_owned();
        w.added = w.added.trim().to_owned();
        if w.rewritten.is_empty() || !w.rewritten.contains(w.added.as_str()) {
            continue;
        }
        w.turn = turn;
        w.date = date.clone();
        w.quote = q;
        w.note = summary::strip_ids(&w.note);
        w.missing = w.missing.iter().map(|m| summary::strip_ids(m)).filter(|m| !m.is_empty()).collect();
        rewrites.push(w);
    }
    rewrites.truncate(MAX_REWRITES);

    let dropped = given - scores.len() - pivots.len() - rewrites.len();
    (scores, pivots, rewrites, dropped)
}

/// Why a finished week is not sent to the model, in that order.
pub(crate) fn skip_reason(on: bool, requests: usize, summarized_days: usize) -> Option<&'static str> {
    if !on {
        Some("off")
    } else if requests < MIN_REQUESTS {
        Some("few_requests")
    } else if summarized_days == 0 {
        Some("no_day_summaries")
    } else {
        None
    }
}

/// Days of the week with requests but no saved day summary.
pub(crate) fn days_without_summary(o: &Observed, days: &[Report]) -> Vec<String> {
    o.days
        .iter()
        .filter(|d| d.requests > 0 && !days.iter().any(|r| r.date == d.date && r.summary.is_some()))
        .map(|d| d.date.clone())
        .collect()
}

fn not_finished(lang: Lang) -> String {
    match lang {
        Lang::Ko => "주가 끝난 뒤에 평가합니다".into(),
        Lang::En => "Insights are evaluated after the week ends".into(),
    }
}

fn no_week_summary(lang: Lang) -> String {
    match lang {
        Lang::Ko => "평가하려면 먼저 주간 요약을 만들어야 합니다".into(),
        Lang::En => "Make the weekly summary first".into(),
    }
}

fn bad_answer(lang: Lang, e: &serde_json::Error) -> String {
    match lang {
        Lang::Ko => format!("평가 JSON을 읽지 못했습니다: {e}"),
        Lang::En => format!("Could not read the evaluation JSON: {e}"),
    }
}

const EVAL_PROMPT: &str = r#"당신은 한 개발자가 한 주 동안 AI 코딩 에이전트에게 보낸 요청의 흐름을 읽고, AI를 다루는 방식을 평가표에 맞춰 평가하는 도우미입니다.

입력:
- 그 주의 관측 수치, 세션 첫 요청, 하루 요약이 뽑은 방향 전환 후보와 요청 메모, 평가표가 있습니다. [MM-DD.t번호]는 그날의 요청 번호입니다.
- "지난주 이의"에 있는 항목은 지난주 판정이 틀렸다고 사람이 표시한 것입니다. 그 항목은 근거를 다시 보고 매깁니다.
- 재료는 그 주 요청 전부가 아니라 세션 첫 요청, 방향 전환 후보, 요청 메모만 담고 있습니다. 재료에 보이지 않는다고 그 행동이 없었다고 단정하지 않습니다. 방향 전환 후보는 일이 꼬인 순간만 모은 것이므로, 그것만으로 그 주 전체의 확인 방식이나 역할 분담을 판단하지 않습니다.

쓰는 법:
- 입력이 다른 언어여도 한국어로 씁니다. 인용한 글은 원래 표기를 씁니다.
- 입력에 있는 사실만 씁니다. 사람의 성격이나 능력을 쓰지 않고, 기록된 요청과 행동만 씁니다. 쉬운 한국어로 짧게 씁니다. 줄표(—)는 쓰지 않습니다.
- scores: 평가표의 네 항목(verify, delegate, clarity, rationale)마다 하나씩 씁니다. score는 1~5 정수로, 가장 잘한 요청 하나가 아니라 그 주 요청 전체에서 가장 자주 보이는 모습에 가까운 기준 문장을 고릅니다. 방향 전환 후보와 요청 메모에 나온 다시 말한 요청과 뒤에서 보탠 조건도 함께 봅니다. turn과 quote는 그 점수를 가장 잘 보여 주는 전형적인 요청에서 고릅니다. turn은 그 요청의 MM-DD.t번호, quote는 그 요청 글의 일부를 한 글자도 고치지 않고 그대로 옮깁니다. why는 그 점수를 준 근거 한 문장이고, 그 주에 그런 요청이 얼마나 있었는지를 함께 씁니다(예: 세션 첫 요청 9개 중 6개는 조건이 뒤에서 보태짐). 근거가 될 요청이 없는 항목은 쓰지 않습니다. rationale은 방향 전환 후보 전체를 근거로 할 때 turn과 quote를 빈 글로 둘 수 있습니다.
- pivots: 방향 전환 후보 중 그 주에 가장 중요한 것을 5개까지 고릅니다. 후보에 없는 것은 쓰지 않습니다. turn만 정확히 씁니다.
- rewrites: 다시 말하게 된 요청이나 조건이 뒤에서 보태진 첫 요청 중 2개까지 고쳐 씁니다. turn은 원래 요청의 번호, quote는 원래 요청 글의 일부 그대로, rewritten은 처음부터 이렇게 보냈으면 좋았을 요청 전체, added는 rewritten 안에서 새로 넣은 부분을 rewritten에 있는 그대로, missing은 원래 요청에 없던 것을 짧은 명사구로, note는 왜 다시 말하게 됐는지 한 문장입니다. 지어낸 조건은 넣지 않고 그 주의 뒤 요청에 실제로 나온 것만 넣습니다. rewritten과 added는 그대로 복사해 보낼 글이므로, 이 글의 다른 칸과 달리 원래 요청과 같은 언어로 씁니다.
- why, note, missing 같은 글 안에는 MM-DD.t번호를 쓰지 않습니다. 번호는 turn 칸에만 넣습니다.

출력은 아래 모양의 JSON 하나만 냅니다. 다른 글이나 코드 블록 표시는 붙이지 않습니다.
{"scores": [{"item": "clarity", "score": 3, "turn": "10-04.t1", "quote": "", "why": ""}], "pivots": [{"turn": "10-04.t9"}], "rewrites": [{"turn": "10-04.t7", "quote": "", "rewritten": "", "added": "", "missing": [], "note": ""}]}"#;

const EVAL_PROMPT_EN: &str = r#"You read the flow of requests one developer sent to AI coding agents over a week, and evaluate how they work with AI against a rubric.

Input:
- The week's observed numbers, the first request of each session, the direction change candidates and request notes the daily summaries picked, and the rubric. [MM-DD.tN] is a request's number on that day.
- Items under "Last week's disputed items" are ones the person marked as judged wrong last week. Look at their evidence again before scoring them.
- The material is not every request of the week: only the sessions' first requests, the direction change candidates and the request notes. Do not conclude a behavior was absent because the material does not show it. The direction change candidates collect only the moments that went wrong, so do not judge the week's verification or delegation from them alone.

How to write:
- Write in English even when the input is in another language. Quoted text stays as written.
- Write only what the input shows. Never describe the person's character or ability; write only the requests and actions on record. Use plain, concise English. Do not use em or en dashes (— –).
- scores: one for each of the rubric's four items (verify, delegate, clarity, rationale). score is a whole number from 1 to 5: pick the level sentence closest to what the week's requests most often look like, not the single best request. Count the requests said again and the conditions added later, as the direction change candidates and request notes show them. Take turn and quote from the most typical request for that score. turn is that request's MM-DD.tN, and quote is part of that request copied exactly, not a single character changed. why is one sentence on why that score, saying how many of the week's requests looked like that (for example, 6 of 9 first requests had conditions added later). Leave out an item when no request can support it. For rationale, turn and quote may be empty when the score rests on all of the direction change candidates.
- pivots: pick up to 5 of the direction change candidates that mattered most that week. Never add one that is not a candidate. Only turn has to be exact.
- rewrites: rewrite up to 2 requests that had to be said again, or first requests whose conditions were added later. turn is the original request's number, quote is part of the original request as written, rewritten is the whole request as it would have been better sent from the start, added is the new part exactly as it appears in rewritten, missing lists what the original request lacked as short noun phrases, and note is one sentence on why it had to be said again. Add no invented conditions; only what later requests that week actually said. rewritten and added are text to copy and send, so unlike the other fields, write them in the same language as the original request.
- Never put an MM-DD.tN in why, note or missing. Numbers go only in the turn fields.

Output exactly one JSON object shaped like this. Add no other text and no code fences.
{"scores": [{"item": "clarity", "score": 3, "turn": "10-04.t1", "quote": "", "why": ""}], "pivots": [{"turn": "10-04.t9"}], "rewrites": [{"turn": "10-04.t7", "quote": "", "rewritten": "", "added": "", "missing": [], "note": ""}]}"#;

/// The evaluation prompt for a run in `lang` (ADR 0011: one per language; change both).
fn eval_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => EVAL_PROMPT,
        Lang::En => EVAL_PROMPT_EN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::{DayDigest, ProjectDigest, SessionDigest, Turn};
    use crate::summary::{Ask, Summary};

    pub(super) fn day(date: &str, turns: &[(&str, &str, bool)], summary: Option<Summary>) -> Report {
        let turns = turns.iter().map(|(id, p, first)| Turn { id: (*id).into(), prompt: (*p).into(), first: *first, ..Default::default() }).collect();
        Report {
            date: date.into(),
            generated_at: 0,
            model: None,
            provider: None,
            digest: DayDigest {
                projects: vec![ProjectDigest { name: "porch".into(), sessions: vec![SessionDigest { turns, ..Default::default() }], ..Default::default() }],
                ..Default::default()
            },
            summary,
            summary_error: None,
            usual_minutes: None,
            usage_checked: true,
            turn_files_read: Some(true),
            dropped_pivots: 0,
        }
    }

    fn requests() -> BTreeMap<String, (String, String)> {
        turn_requests(&[day("2026-10-04", &[("t1", "어떻게 할지 좋은 전략안내봐", true), ("t7", "별도 라인으로 빼", false)], None)])
    }

    fn raw(item: &str, score: serde_json::Value, turn: &str, quote: &str) -> RawScore {
        RawScore { item: item.into(), score, turn: turn.into(), quote: quote.into(), why: "근거 t1".into() }
    }

    #[test]
    fn rubric_names_every_item_in_order_in_both_languages() {
        for lang in [Lang::Ko, Lang::En] {
            assert_eq!(rubric(lang).iter().map(|c| c.item).collect::<Vec<_>>(), ITEMS);
        }
        let en: Vec<&str> = RUBRIC_EN.iter().flat_map(|c| [c.name, c.question].into_iter().chain(c.levels)).collect();
        let en = en.join(" ");
        assert!(!crate::lang::has_hangul(&en));
        assert!(!en.contains('—') && !en.contains('–'));
    }

    #[test]
    fn clean_keeps_one_score_per_item_quoting_its_request() {
        let a = Answer {
            scores: vec![
                raw("clarity", 3.into(), "10-04.t1", "좋은 전략안내봐"),
                raw("clarity", 5.into(), "10-04.t1", "좋은 전략안내봐"),
                raw("verify", 6.into(), "10-04.t1", "좋은 전략안내봐"),
                raw("delegate", 4.into(), "10-04.t1", "전략을 짜줘"),
                raw("verify", "4".into(), "10-04.t1", "좋은 전략안내봐"),
                raw("rationale", 3.into(), "", "아무 글"),
                raw("delegate", 4.into(), "", ""),
                raw("feelings", 2.into(), "10-04.t1", "좋은 전략안내봐"),
            ],
            ..Default::default()
        };
        let (scores, _, _, dropped) = clean(a, &requests(), &BTreeMap::new(), Lang::Ko);
        assert_eq!(scores.iter().map(|s| (s.item.as_str(), s.score)).collect::<Vec<_>>(), [("clarity", 3), ("rationale", 3)]);
        assert_eq!(dropped, 6);
        assert_eq!(scores[0].criterion, "목적과 대안은 적고, 결과의 모습은 나중에 보탬");
        assert_eq!(scores[0].next_criterion.as_deref(), Some("원하는 결과의 모습까지 첫 요청에 적음"));
        assert_eq!(scores[0].why, "근거");
        assert_eq!((scores[1].turn.as_str(), scores[1].quote.as_str()), ("", ""));
    }

    #[test]
    fn a_top_score_has_no_next_step() {
        let a = Answer { scores: vec![raw("verify", serde_json::json!(5.0), "10-04.t7", " “별도 라인” ")], ..Default::default() };
        let (scores, ..) = clean(a, &requests(), &BTreeMap::new(), Lang::En);
        assert_eq!(scores[0].quote, "별도 라인");
        assert_eq!(scores[0].next_criterion, None);
        assert_eq!(scores[0].criterion, "Also records what the check led to fixing");
    }

    #[test]
    fn clean_takes_direction_changes_only_from_the_day_summaries() {
        let s = Summary {
            pivots: (1..=7).map(|i| Pivot { turn: format!("t{i}"), kind: "redo".into(), quote: "q".into(), reason_given: i % 2 == 0, ..Default::default() }).collect(),
            ..Default::default()
        };
        let days = [day("2026-10-04", &[], Some(s))];
        let cands = pivot_candidates(&days);
        assert_eq!(counts(&cands), PivotCounts { total: 7, with_reason: 3, redo: 7 });
        let mut picked: Vec<Pivot> = (1..=7).rev().map(|i| Pivot { turn: format!("10-04.t{i}"), kind: "shift".into(), ..Default::default() }).collect();
        picked.push(Pivot { turn: "10-03.t1".into(), ..Default::default() });
        let (_, pivots, _, dropped) = clean(Answer { pivots: picked, ..Default::default() }, &BTreeMap::new(), &cands, Lang::Ko);
        assert_eq!(pivots.len(), 5);
        assert_eq!((pivots[0].pivot.turn.as_str(), pivots[0].pivot.kind.as_str(), pivots[0].date.as_str()), ("10-04.t7", "redo", "2026-10-04"));
        assert_eq!(dropped, 3);
    }

    #[test]
    fn clean_keeps_rewrites_whose_added_part_is_in_the_rewrite() {
        let w = |quote: &str, rewritten: &str, added: &str| Rewrite {
            turn: "10-04.t7".into(),
            quote: quote.into(),
            rewritten: rewritten.into(),
            added: added.into(),
            missing: vec!["옮길 위치".into()],
            ..Default::default()
        };
        let a = Answer {
            rewrites: vec![
                w("별도 라인으로 빼", "별도 섹션으로 빼서 결정 변경 이력 아래에 둬", "결정 변경 이력 아래에 둬"),
                w("별도 라인으로 빼", "별도 섹션으로 빼", "아래로 옮겨"),
                w("다른 글", "별도 섹션으로 빼", ""),
                w("별도 라인으로 빼", "", ""),
                w("별도 라인으로 빼", "별도 섹션으로 빼", ""),
                w("별도 라인으로 빼", "별도 섹션으로 빼", ""),
            ],
            ..Default::default()
        };
        let (_, _, rewrites, dropped) = clean(a, &requests(), &BTreeMap::new(), Lang::Ko);
        assert_eq!(rewrites.len(), 2);
        assert_eq!(rewrites[0].date, "2026-10-04");
        assert_eq!(dropped, 4);
    }

    #[test]
    fn material_names_turns_by_week_id_and_lists_what_is_missing() {
        let s = Summary {
            pivots: vec![Pivot { turn: "t2".into(), kind: "redo".into(), before: "문장을 따로 뗌".into(), after: "섹션을 아래로 옮김".into(), quote: "아래로 내리라고".into(), ..Default::default() }],
            asks: vec![Ask { turn: "t1".into(), quote: "좋은 전략안내봐".into(), missing: vec!["원하는 화면 형태".into()] }],
            ..Default::default()
        };
        let days = [day("2026-10-04", &[("t1", "어떻게 할지\n좋은 전략안내봐", true), ("t2", "아래로 내리라고", false)], Some(s))];
        let o = Observed { requests: 30, ..Default::default() };
        let without = vec!["2026-10-02".to_owned()];
        let m = Material { monday: "2026-09-28", observed: &o, days: &days, days_without_summary: &without, last_goal: Some("Bash 오류 줄이기 (71건)"), last: None };
        let ko = material(&m, Lang::Ko);
        for w in [
            "주: 2026-09-28 – 2026-10-04",
            "요청 30",
            "하루 요약이 없는 날: 10-02",
            "지난주 목표: Bash 오류 줄이기 (71건)",
            "지난주 점수: 없음",
            "[10-04.t1] 어떻게 할지 좋은 전략안내봐",
            "[10-04.t2] redo · 문장을 따로 뗌 → 섹션을 아래로 옮김 · \"아래로 내리라고\" · 이유 없음",
            "[10-04.t1] \"좋은 전략안내봐\" · 빠진 것: 원하는 화면 형태",
            "- verify (검증 방식): 만든 결과를 다른 방법으로 다시 확인합니까?",
            "  1점: 결과를 확인 없이 받아들임",
        ] {
            assert!(ko.contains(w), "{w}\n{ko}");
        }
        let en = material(&m, Lang::En);
        for w in ["Week: 2026-09-28 to 2026-10-04", "Days without a daily summary: 10-02", "no reason given", "  1: Accepts results without checking"] {
            assert!(en.contains(w), "{w}\n{en}");
        }
    }

    #[test]
    fn days_summarized_before_pivots_still_make_material() {
        let days = [day("2026-10-04", &[("t1", "고쳐줘", false)], Some(Summary::default()))];
        let o = Observed::default();
        let ko = material(&Material { monday: "2026-09-28", observed: &o, days: &days, days_without_summary: &[], last_goal: None, last: None }, Lang::Ko);
        assert_eq!(ko.matches("\n없음\n").count(), 3, "{ko}");
    }

    #[test]
    fn skips_come_in_order() {
        assert_eq!(skip_reason(false, 5, 0), Some("off"));
        assert_eq!(skip_reason(true, MIN_REQUESTS - 1, 3), Some("few_requests"));
        assert_eq!(skip_reason(true, MIN_REQUESTS, 0), Some("no_day_summaries"));
        assert_eq!(skip_reason(true, MIN_REQUESTS, 1), None);
    }

    #[test]
    fn days_without_summary_are_only_days_with_requests() {
        let days = ["2026-10-01", "2026-10-02", "2026-10-03"].iter().map(|d| insight::DayObs { date: (*d).into(), requests: 1, ..Default::default() }).collect();
        let mut o = Observed { days, ..Default::default() };
        o.days[2].requests = 0;
        let days = [day("2026-10-01", &[], Some(Summary::default())), day("2026-10-02", &[], None)];
        assert_eq!(days_without_summary(&o, &days), ["2026-10-02"]);
    }

    #[test]
    fn eval_prompts_match_and_keep_their_language() {
        assert_eq!(crate::lang::example_keys(EVAL_PROMPT), crate::lang::example_keys(EVAL_PROMPT_EN));
        assert!(!crate::lang::has_hangul(EVAL_PROMPT_EN));
        assert!(EVAL_PROMPT.contains(crate::summary::KOREAN_EVEN_IF));
        assert!(EVAL_PROMPT_EN.contains(crate::summary::ENGLISH_EVEN_IF));
        for item in ITEMS {
            assert!(EVAL_PROMPT.contains(item) && EVAL_PROMPT_EN.contains(item), "{item}");
        }
        assert_eq!((eval_prompt(Lang::Ko), eval_prompt(Lang::En)), (EVAL_PROMPT, EVAL_PROMPT_EN));
    }

    /// A real run (2026-10-06) scored clarity 5/5 on the one best request of a
    /// week full of conditions added later, and rewrote Korean requests in
    /// English. Both prompts must say otherwise.
    #[test]
    fn eval_prompts_score_the_whole_week_and_rewrite_in_the_request_language() {
        for w in ["가장 잘한 요청 하나가 아니라", "전형적인 요청", "원래 요청과 같은 언어"] {
            assert!(EVAL_PROMPT.contains(w), "{w}");
        }
        for w in ["not the single best request", "most typical request", "same language as the original request"] {
            assert!(EVAL_PROMPT_EN.contains(w), "{w}");
        }
    }

    /// The second real run scored verification 2/5 from the direction changes
    /// alone: the material holds the moments that went wrong, not the routine
    /// checks. Both prompts must say what the material leaves out.
    #[test]
    fn eval_prompts_say_the_material_is_not_every_request() {
        for w in ["재료에 보이지 않는다고", "꼬인 순간만"] {
            assert!(EVAL_PROMPT.contains(w), "{w}");
        }
        for w in ["does not show it", "only the moments that went wrong"] {
            assert!(EVAL_PROMPT_EN.contains(w), "{w}");
        }
    }

    use crate::writer::Engine;

    fn finished_week(monday: &str, requests: usize) -> Observed {
        let end = insight::week_end(monday, 0);
        let days = (0..7).map(|i| insight::DayObs { date: time::add_days(monday, i).unwrap(), requests: 1, ..Default::default() }).collect();
        Observed { week_start: monday.into(), through: end, week_end: end, days, requests, hours_waiting_ms: vec![0; 24], ..Default::default() }
    }

    fn week(has_week_summary: bool) -> Week {
        Week {
            monday: "2026-09-28".into(),
            observed: finished_week("2026-09-28", 40),
            days: vec![day("2026-10-04", &[("t1", "어떻게 할지 좋은 전략안내봐", true)], Some(Summary::default()))],
            has_week_summary,
            last_goal: None,
        }
    }

    fn answer() -> serde_json::Value {
        serde_json::json!({"scores": [{"item": "clarity", "score": 3, "turn": "10-04.t1", "quote": "좋은 전략안내봐", "why": "w"}], "pivots": [], "rewrites": []})
    }

    #[test]
    fn a_running_week_is_not_evaluated_and_the_model_is_not_asked() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = week(true);
        w.observed.through -= 1;
        let e = evaluate_in(dir.path(), w, true, 0, &Engine::default(), |_| panic!("asked the model")).unwrap_err();
        assert_eq!(e, "주가 끝난 뒤에 평가합니다");
        let end = insight::week_end("2026-09-28", 0);
        let o = EvalOpts { any_date: "2026-09-30".into(), engine: Engine::default(), on: true };
        assert_eq!(evaluate(&o, 0, end - 1).unwrap_err(), "주가 끝난 뒤에 평가합니다");
    }

    #[test]
    fn no_weekly_summary_no_evaluation() {
        let dir = tempfile::tempdir().unwrap();
        let e = evaluate_in(dir.path(), week(false), true, 0, &Engine::default(), |_| panic!("asked the model")).unwrap_err();
        assert_eq!(e, "평가하려면 먼저 주간 요약을 만들어야 합니다");
        assert!(load_in(dir.path(), "2026-09-28").is_none());
    }

    #[test]
    fn skips_are_saved_without_asking_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let skipped = |w: Week, on: bool| evaluate_in(dir.path(), w, on, 7, &Engine::default(), |_| panic!("asked the model")).unwrap().skipped;
        assert_eq!(skipped(week(true), false).as_deref(), Some("off"));
        let mut few = week(true);
        few.observed.requests = MIN_REQUESTS - 1;
        assert_eq!(skipped(few, true).as_deref(), Some("few_requests"));
        let mut none = week(true);
        none.days[0].summary = None;
        assert_eq!(skipped(none, true).as_deref(), Some("no_day_summaries"));
        let saved = load_in(dir.path(), "2026-09-28").unwrap();
        assert_eq!(saved.days_without_summary.len(), 7);
        assert!(saved.evaluation.is_none());
    }

    #[test]
    fn a_failed_rewrite_keeps_the_saved_evaluation() {
        let dir = tempfile::tempdir().unwrap();
        let first = evaluate_in(dir.path(), week(true), true, 1, &Engine::default(), |_| Ok(answer())).unwrap();
        let e = first.evaluation.clone().unwrap();
        assert_eq!((e.scores[0].item.as_str(), e.evaluated_at, e.evaluated_through), ("clarity", 1, insight::week_end("2026-09-28", 0)));
        assert_eq!(first.model.as_deref(), Some("default"));

        let err = evaluate_in(dir.path(), week(true), true, 2, &Engine::default(), |_| Err("Claude Code 사용 한도에 걸렸습니다".into())).unwrap_err();
        assert!(err.contains("사용 한도"));
        let saved = load_in(dir.path(), "2026-09-28").unwrap();
        assert_eq!(saved.evaluation, first.evaluation);
        assert_eq!(saved.error.as_deref(), Some("Claude Code 사용 한도에 걸렸습니다"));

        let bad = evaluate_in(dir.path(), week(true), true, 3, &Engine::default(), |_| Ok(serde_json::json!({"scores": {"clarity": 3}}))).unwrap_err();
        assert!(bad.starts_with("평가 JSON을 읽지 못했습니다"), "{bad}");
        assert_eq!(load_in(dir.path(), "2026-09-28").unwrap().evaluation, first.evaluation);

        evaluate_in(dir.path(), week(true), true, 4, &Engine::default(), |_| Ok(answer())).unwrap();
        assert_eq!(load_in(dir.path(), "2026-09-28").unwrap().error, None);
    }

    #[test]
    fn a_skip_clears_the_old_error_and_keeps_the_saved_evaluation() {
        let dir = tempfile::tempdir().unwrap();
        let made = evaluate_in(dir.path(), week(true), true, 1, &Engine::default(), |_| Ok(answer())).unwrap();
        assert!(made.evaluation.is_some());
        evaluate_in(dir.path(), week(true), true, 2, &Engine::default(), |_| Err("Claude Code 사용 한도에 걸렸습니다".into())).unwrap_err();
        assert!(load_in(dir.path(), "2026-09-28").unwrap().error.is_some());

        let skipped = evaluate_in(dir.path(), week(true), false, 3, &Engine::default(), |_| panic!("asked the model")).unwrap();
        assert_eq!((skipped.skipped.as_deref(), skipped.error.as_deref()), (Some("off"), None));
        assert_eq!(skipped.evaluation, made.evaluation);
    }

    #[test]
    fn the_material_names_last_weeks_goal_in_the_evaluations_language() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = week(true);
        w.last_goal = Some(goals::Goal { week_start: "2026-09-21".into(), key: "stop_failures".into(), label: "오류로 멈춤 줄이기".into(), baseline: 1, unit: "회".into(), ..Default::default() });
        let seen = std::cell::RefCell::new(String::new());
        evaluate_in(dir.path(), w, true, 1, &Engine { lang: Lang::En, ..Default::default() }, |text| {
            *seen.borrow_mut() = text.to_owned();
            Ok(answer())
        })
        .unwrap();
        let text = seen.into_inner();
        assert!(text.contains("Last week's goal: Fewer stops on error (1 time)"), "{text}");
    }

    #[test]
    fn material_writes_one_stop_as_a_single_time() {
        let observed = |stops: usize| {
            let mut o = Observed { requests: 30, ..Default::default() };
            o.days = vec![insight::DayObs { stop_failures: stops, ..Default::default() }];
            o
        };
        let one = observed(1);
        let m = Material { monday: "2026-09-28", observed: &one, days: &[], days_without_summary: &[], last_goal: None, last: None };
        let en = material(&m, Lang::En);
        assert!(en.contains("1 time"), "{en}");
        assert!(!en.contains("1 times"), "{en}");
        let two = observed(2);
        let m = Material { monday: "2026-09-28", observed: &two, days: &[], days_without_summary: &[], last_goal: None, last: None };
        assert!(material(&m, Lang::En).contains("2 times"));
    }

    #[test]
    fn a_dispute_marked_while_the_model_runs_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_owned();
        evaluate_in(dir.path(), week(true), true, 1, &Engine::default(), |_| {
            dispute_in(&path, "2026-09-28", "clarity", Lang::Ko).unwrap();
            Ok(answer())
        })
        .unwrap();
        let saved = load_in(dir.path(), "2026-09-28").unwrap();
        assert_eq!(saved.disputed, ["clarity"]);
        assert!(saved.evaluation.is_some());
        dispute_in(dir.path(), "2026-09-28", "clarity", Lang::Ko).unwrap();
        assert_eq!(load_in(dir.path(), "2026-09-28").unwrap().disputed.len(), 1);
        assert!(dispute_in(dir.path(), "2026-09-28", "mood", Lang::Ko).is_err());
    }

    #[test]
    fn a_finished_week_saved_before_it_ended_is_counted_again_and_its_evaluation_kept() {
        let dir = tempfile::tempdir().unwrap();
        let end = insight::week_end("2026-09-28", 0);
        let wed = end - 4 * 86_400_000;
        with_report_in(dir.path(), "2026-09-28", |r| {
            r.observed = Observed { week_start: "2026-09-28".into(), through: wed, week_end: end, ..Default::default() };
            r.evaluation = Some(Evaluation { evaluated_through: wed, ..Default::default() });
        })
        .unwrap();
        let o = observed_in(dir.path(), "2026-09-28", end, end + 5, || Ok(finished_week("2026-09-28", 40))).unwrap();
        assert_eq!(o.through, end);
        let saved = load_in(dir.path(), "2026-09-28").unwrap();
        assert_eq!(saved.observed.through, end);
        assert_eq!(saved.evaluation.unwrap().evaluated_through, wed);
        let again = observed_in(dir.path(), "2026-09-28", end, end + 9, || panic!("counted again")).unwrap();
        assert_eq!(again.requests, 40);
    }

    #[test]
    fn a_running_week_is_counted_fresh_and_never_saved() {
        let dir = tempfile::tempdir().unwrap();
        let end = insight::week_end("2026-09-28", 0);
        let o = observed_in(dir.path(), "2026-09-28", end, end - 1, || Ok(Observed { requests: 3, ..Default::default() })).unwrap();
        assert_eq!(o.requests, 3);
        assert!(load_in(dir.path(), "2026-09-28").is_none());
    }

    #[test]
    fn latest_evaluated_at_reads_only_insight_files() {
        let dir = tempfile::tempdir().unwrap();
        for (monday, at) in [("2026-09-21", 5), ("2026-09-28", 9)] {
            with_report_in(dir.path(), monday, |r| r.evaluation = Some(Evaluation { evaluated_at: at, ..Default::default() })).unwrap();
        }
        with_report_in(dir.path(), "2026-10-05", |_| ()).unwrap();
        assert_eq!(latest_evaluated_at_in(dir.path()), Some(9));
        let p = crate::mirror::saved_periods(dir.path());
        assert!(p.days.is_empty() && p.weeks.is_empty() && p.months.is_empty());
    }
}
