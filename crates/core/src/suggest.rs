//! Suggestions: Claude Code reads where each project kept getting held up and
//! proposes one concrete change per pattern, with the records behind it. porch
//! never writes the person's own config; it keeps the suggestions, whether they
//! were applied, and counts the target before and after (see `health::effect`).

use crate::lang::Lang;
use crate::health::{self, Count, ProjectHealth};
use crate::summary::Report;
use crate::{open, paths, summary, time};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Applied,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestion {
    /// "s12"; never reused.
    pub id: String,
    pub created: String,
    /// A repo root, or "common".
    pub scope: String,
    pub title: String,
    pub why: String,
    /// "claude_md", "hook", "permission" or "habit".
    pub action: String,
    /// What to paste, as written.
    pub text: String,
    /// "kind:env" or "command:pnpm --filter @x/api test".
    pub target: String,
    pub evidence: Vec<String>,
    pub status: Status,
    #[serde(default)]
    pub applied_on: Option<String>,
    /// The target in the 30 days up to `created`.
    #[serde(default)]
    pub baseline: Count,
}

pub fn store_path() -> PathBuf {
    paths::data_dir().join("suggestions.json")
}

/// The saved suggestions; none when the file does not exist yet. A file that
/// does not parse is an error, never an empty list, so it is not overwritten.
pub fn read_store(path: &Path) -> Result<Vec<Suggestion>, String> {
    match fs::read_to_string(path) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| match Lang::current() {
            Lang::Ko => format!("제안 파일을 읽지 못했습니다 ({}): {e}", path.display()),
            Lang::En => format!("Could not read the suggestions file ({}): {e}", path.display()),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// Read, change and save the suggestions under one lock shared by the app and
/// the CLI, so a write never drops another one made in between.
pub fn with_store<T>(path: &Path, f: impl FnOnce(&mut Vec<Suggestion>) -> Result<T, String>) -> Result<T, String> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock")).map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let mut list = read_store(path)?;
    let out = f(&mut list)?;
    let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    summary::save_json(path, &text)?;
    Ok(out)
}

fn next_id(list: &[Suggestion]) -> u64 {
    list.iter().filter_map(|s| s.id.strip_prefix('s')?.parse::<u64>().ok()).max().map_or(1, |n| n + 1)
}

/// Fresh suggestions replace the ones still `new`; applied and dismissed ones
/// stay. Ids continue from the highest one in `current`, removed ones included.
pub fn merge(current: Vec<Suggestion>, fresh: Vec<Suggestion>) -> Vec<Suggestion> {
    let start = next_id(&current);
    let mut out: Vec<Suggestion> = current.into_iter().filter(|s| s.status != Status::New).collect();
    for (n, mut s) in (start..).zip(fresh) {
        s.id = format!("s{n}");
        out.push(s);
    }
    out
}

/// Mark a suggestion applied (from `today`), dismissed, or new again.
pub fn set_status(path: &Path, id: &str, status: Status, today: &str) -> Result<(), String> {
    with_store(path, |list| {
        let s = list.iter_mut().find(|s| s.id == id).ok_or_else(|| match Lang::current() {
            Lang::Ko => "이미 바뀐 제안입니다. 다시 불러와 주세요.".to_owned(),
            Lang::En => "That suggestion has already changed. Reload and try again.".to_owned(),
        })?;
        s.status = status;
        s.applied_on = (status == Status::Applied).then(|| today.to_owned());
        Ok(())
    })
}

pub const ACTIONS: [&str; 4] = ["claude_md", "hook", "permission", "habit"];
const MAX_PER_SCOPE: usize = 3;
const MAX_PROJECTS: usize = 8;
const DOC_CHARS: usize = 8000;

pub struct BlockerLine {
    pub date: String,
    pub title: String,
    pub kind: String,
    pub signal: String,
    pub cause: String,
    pub fix: String,
}

pub struct ProjectMaterial {
    pub root: String,
    pub name: String,
    pub checks: Vec<String>,
    pub blockers: Vec<BlockerLine>,
    /// "kind:<kind>" and "command:<command>" a suggestion for this project may aim at.
    pub targets: Vec<String>,
    /// Dates a suggestion for this project may cite.
    pub dates: BTreeSet<String>,
    /// (file name, contents cut to 8000 chars) for CLAUDE.md and AGENTS.md.
    pub docs: Vec<(String, String)>,
}

pub struct CommonMaterial {
    pub claude_md: Option<String>,
    /// (kind, blockers, projects) for kinds seen in two projects or more.
    pub kinds: Vec<(String, usize, usize)>,
    pub targets: Vec<String>,
    pub dates: BTreeSet<String>,
}

pub struct Material {
    pub projects: Vec<ProjectMaterial>,
    pub common: CommonMaterial,
    /// Each previous suggestion with its measured effect, when applied, so
    /// `material_text` stays a pure rendering.
    pub previous: Vec<(Suggestion, Option<health::Effect>)>,
}

pub struct Draft {
    pub scope: String,
    pub title: String,
    pub why: String,
    pub action: String,
    pub text: String,
    pub target: String,
    pub evidence: Vec<String>,
}

fn cut(s: &str) -> String {
    s.chars().take(DOC_CHARS).collect()
}

/// The effect line the suggester reads for a previous applied suggestion. Two
/// windows counted differently cannot be compared, so nothing is written for
/// them rather than something wrong.
fn effect_line(e: &health::Effect, lang: Lang) -> Option<String> {
    match (e.state, lang) {
        (health::EffectState::Measured, Lang::Ko) => Some(format!(" · 효과: 적용 전 {}일 {} → 적용 후 {}일 {}", e.days, e.before.items, e.days, e.after.items)),
        (health::EffectState::Measured, Lang::En) => Some(format!(" · before/after counts: {} in the {} days before the suggestion was applied, {} in the {} days after; this does not show whether the suggestion caused the change", e.before.items, e.days, e.after.items, e.days)),
        (health::EffectState::Early, Lang::Ko) => Some(" · 효과: 아직 이르다".to_owned()),
        (health::EffectState::Early, Lang::En) => Some(" · before/after counts: not enough time has passed to compare".to_owned()),
        (health::EffectState::NoRecords, Lang::Ko) => Some(" · 효과: 적용 후 기록 없음".to_owned()),
        (health::EffectState::NoRecords, Lang::En) => Some(" · before/after counts: no records since the suggestion was applied".to_owned()),
        (health::EffectState::Uncomparable, _) => None,
    }
}

/// What the suggester reads: health's checks and the blockers behind them, with
/// their signal, cause and the day's fix, plus the projects' own instructions.
/// Prompts, answers and conversations are never part of it.
pub fn material(
    reports: &[Report],
    health: &[ProjectHealth],
    previous: &[Suggestion],
    docs: &dyn Fn(&str) -> Vec<(String, String)>,
    home_claude_md: Option<String>,
    today: &str,
    offset: i64,
) -> Material {
    let mut text_of: BTreeMap<(&str, &str), &crate::summary::Blocker> = BTreeMap::new();
    for r in reports {
        for b in r.summary.iter().flat_map(|s| &s.blockers) {
            text_of.insert((r.date.as_str(), b.title.as_str()), b);
        }
    }
    let mut ranked: Vec<&ProjectHealth> = health.iter().filter(|p| !p.root.is_empty() && (!p.kinds.is_empty() || !p.repeated_errors.is_empty() || !p.checks.is_empty())).collect();
    ranked.sort_by_key(|p| std::cmp::Reverse(p.blocked_minutes));
    ranked.truncate(MAX_PROJECTS);

    let mut projects = Vec::new();
    let mut kind_projects: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut kind_dates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in ranked {
        let mut blockers = Vec::new();
        let mut dates = BTreeSet::new();
        let mut targets = Vec::new();
        for k in &p.kinds {
            if k.kind != "unclassified" && k.kind != "unknown" {
                targets.push(format!("kind:{}", k.kind));
                let e = kind_projects.entry(k.kind.clone()).or_default();
                e.0 += k.blockers;
                e.1 += 1;
            }
            for ev in &k.evidence {
                dates.insert(ev.date.clone());
                kind_dates.entry(k.kind.clone()).or_default().insert(ev.date.clone());
                let full = text_of.get(&(ev.date.as_str(), ev.label.as_str()));
                blockers.push(BlockerLine {
                    date: ev.date.clone(),
                    title: ev.label.clone(),
                    kind: k.kind.clone(),
                    signal: full.map(|b| b.signal.clone()).unwrap_or_default(),
                    cause: full.map(|b| b.cause.clone()).unwrap_or_default(),
                    fix: full.map(|b| b.fix.clone()).unwrap_or_default(),
                });
            }
        }
        for e in &p.repeated_errors {
            targets.push(format!("command:{}", e.command));
            dates.extend(e.evidence.iter().map(|x| x.date.clone()));
        }
        for c in &p.checks {
            dates.extend(c.evidence.iter().map(|x| x.date.clone()));
        }
        blockers.sort_by(|a, b| a.date.cmp(&b.date));
        projects.push(ProjectMaterial {
            root: p.root.clone(),
            name: p.name.clone(),
            checks: p.checks.iter().map(|c| c.title.clone()).collect(),
            blockers,
            targets,
            dates,
            docs: docs(&p.root).into_iter().map(|(n, t)| (n, cut(&t))).collect(),
        });
    }
    let shared: Vec<(String, usize, usize)> = kind_projects.into_iter().filter(|(_, (_, n))| *n >= 2).map(|(k, (b, n))| (k, b, n)).collect();
    let common = CommonMaterial {
        targets: shared.iter().map(|(k, _, _)| format!("kind:{k}")).collect(),
        dates: shared.iter().flat_map(|(k, _, _)| kind_dates.get(k).cloned().unwrap_or_default()).collect(),
        kinds: shared,
        claude_md: home_claude_md.map(|t| cut(&t)),
    };
    let previous = previous
        .iter()
        .map(|s| {
            let effect = s
                .applied_on
                .as_deref()
                .filter(|_| s.status == Status::Applied)
                .map(|on| health::effect(reports, &s.scope, &s.target, on, today, offset));
            (s.clone(), effect)
        })
        .collect();
    Material { projects, common, previous }
}

/// The material as the suggester reads it.
pub fn material_text(m: &Material, lang: Lang) -> String {
    let en = lang == Lang::En;
    let mut out = String::new();
    for p in &m.projects {
        let _ = if en {
            writeln!(out, "## Project {} (scope: {})\nTargets: {}", p.name, p.root, p.targets.join(", "))
        } else {
            writeln!(out, "## 프로젝트 {} (scope: {})\n대상: {}", p.name, p.root, p.targets.join(", "))
        };
        if !p.checks.is_empty() {
            let _ = writeln!(out, "{}", if en { "Checks:" } else { "점검:" });
            for c in &p.checks {
                let _ = writeln!(out, "- {c}");
            }
        }
        let _ = writeln!(out, "{}", if en { "Blockers:" } else { "막힘:" });
        for b in &p.blockers {
            let _ = if en {
                writeln!(out, "- {} [{}] {} / signal: {} / cause: {} / fix that day: {}", b.date, b.kind, b.title, b.signal, b.cause, b.fix)
            } else {
                writeln!(out, "- {} [{}] {} / 근거: {} / 원인: {} / 그날 제안: {}", b.date, b.kind, b.title, b.signal, b.cause, b.fix)
            };
        }
        for (name, text) in &p.docs {
            let _ = writeln!(out, "{name} {}:\n{text}", if en { "contents" } else { "내용" });
        }
        out.push('\n');
    }
    let _ = if en {
        writeln!(out, "## Common (scope: common)\nTargets: {}", m.common.targets.join(", "))
    } else {
        writeln!(out, "## 공통 (scope: common)\n대상: {}", m.common.targets.join(", "))
    };
    for (k, b, n) in &m.common.kinds {
        let _ = if en {
            writeln!(out, "- {}: {b} blockers, {n} projects", health::kind_label(k, lang))
        } else {
            writeln!(out, "- {}: 막힘 {b}건, 프로젝트 {n}곳", health::kind_label(k, lang))
        };
    }
    if let Some(t) = &m.common.claude_md {
        let _ = writeln!(out, "~/.claude/CLAUDE.md {}:\n{t}", if en { "contents" } else { "내용" });
    }
    let shown: Vec<&(Suggestion, Option<health::Effect>)> = m.previous.iter().filter(|(s, _)| s.status != Status::New).collect();
    if !shown.is_empty() {
        let _ = writeln!(out, "\n## {}", if en { "Previous suggestions" } else { "이전 제안" });
        for (s, effect) in shown {
            let status = match s.status {
                Status::Applied => format!("applied {}", s.applied_on.as_deref().unwrap_or("")),
                _ => "dismissed".to_owned(),
            };
            let _ = write!(out, "- [{status}] ({}) {} · {} {}", s.scope, s.title, if en { "target" } else { "대상" }, s.target);
            if let Some(line) = effect.as_ref().and_then(|e| effect_line(e, lang)) {
                let _ = write!(out, "{line}");
            }
            out.push('\n');
        }
    }
    out
}

/// Keep the suggestions whose scope, action, target and dates all come from the
/// material, at most three per scope. Returns them and how many were dropped.
pub fn validate(answer: &serde_json::Value, m: &Material) -> (Vec<Draft>, usize) {
    let Some(items) = answer["suggestions"].as_array() else { return (Vec::new(), 0) };
    let mut out: Vec<Draft> = Vec::new();
    let mut dropped = 0;
    for it in items {
        let s = |k: &str| it[k].as_str().map(str::trim).unwrap_or("").to_owned();
        let (scope, target, action) = (s("scope"), s("target"), s("action"));
        let allowed = if scope == "common" {
            Some((&m.common.targets, &m.common.dates))
        } else {
            m.projects.iter().find(|p| p.root == scope).map(|p| (&p.targets, &p.dates))
        };
        let Some((targets, dates)) = allowed else {
            dropped += 1;
            continue;
        };
        let mut evidence: Vec<String> = Vec::new();
        for d in it["evidence"].as_array().into_iter().flatten().filter_map(|d| d.as_str()) {
            if dates.contains(d) && !evidence.iter().any(|e| e == d) {
                evidence.push(d.to_owned());
            }
        }
        let full = !s("title").is_empty() && !s("text").is_empty() && !s("why").is_empty();
        if !full || !ACTIONS.contains(&action.as_str()) || !targets.contains(&target) || evidence.is_empty() || out.iter().filter(|d| d.scope == scope).count() >= MAX_PER_SCOPE {
            dropped += 1;
            continue;
        }
        out.push(Draft { title: s("title"), why: s("why"), text: s("text"), scope, target, action, evidence });
    }
    (out, dropped)
}

#[derive(Debug, Serialize)]
pub struct MakeResult {
    pub added: usize,
    /// Suggestions the model wrote whose scope, target, action or dates were not in the material.
    pub dropped: usize,
}

#[derive(Debug, Serialize)]
pub struct SuggestionView {
    #[serde(flatten)]
    pub suggestion: Suggestion,
    pub target_label: String,
    /// Only for applied suggestions.
    pub effect: Option<health::Effect>,
}

pub fn target_label(target: &str, lang: Lang) -> String {
    match target.split_once(':') {
        Some(("kind", k)) => health::kind_label(k, lang).to_owned(),
        Some(("command", c)) => match lang {
            Lang::Ko => format!("{c} 실패"),
            Lang::En => format!("{c} failing"),
        },
        _ => target.to_owned(),
    }
}

/// The steps of `make` after the material is built, with the model call passed
/// in so they can be tested without Claude Code.
pub fn make_from(m: Material, ask: impl FnOnce(&str) -> Result<serde_json::Value, String>, reports: &[Report], today: &str, offset: i64, path: &Path, lang: Lang) -> Result<MakeResult, String> {
    if m.projects.is_empty() {
        return Ok(MakeResult { added: 0, dropped: 0 });
    }
    let answer = ask(&material_text(&m, lang))?;
    // An answer without a suggestions array is a broken shape, not "nothing to
    // suggest": refuse it so a merge never wipes the new ones already saved.
    if !answer["suggestions"].is_array() {
        return Err(match lang {
            Lang::Ko => "제안 모양이 다릅니다: suggestions 배열이 없습니다".into(),
            Lang::En => "The suggestions response is missing the suggestions array".into(),
        });
    }
    let (drafts, dropped) = validate(&answer, &m);
    let from = health::days_before(today, health::DEFAULT_DAYS - 1, offset);
    let fresh: Vec<Suggestion> = drafts
        .into_iter()
        .map(|d| Suggestion {
            id: String::new(),
            created: today.to_owned(),
            baseline: health::target_count(reports, &d.scope, &d.target, &from, today),
            scope: d.scope,
            title: d.title,
            why: d.why,
            action: d.action,
            text: d.text,
            target: d.target,
            evidence: d.evidence,
            status: Status::New,
            applied_on: None,
        })
        .collect();
    let added = fresh.len();
    // The model ran without the lock; merge into the list as it is now.
    with_store(path, |list| {
        *list = merge(std::mem::take(list), fresh);
        Ok(())
    })?;
    Ok(MakeResult { added, dropped })
}

fn read_docs(root: &str) -> Vec<(String, String)> {
    ["CLAUDE.md", "AGENTS.md"].iter().filter_map(|n| Some((n.to_string(), fs::read_to_string(Path::new(root).join(n)).ok()?))).collect()
}

/// Ask the chosen writer for suggestions over the last 30 days and save them.
pub fn make(engine: &crate::writer::Engine, offset: i64) -> Result<MakeResult, String> {
    let today = time::local_date(time::now_ms(), offset);
    let reports = summary::saved_days();
    let issues = open::load(summary::saved_days);
    let lang = engine.lang;
    let health = health::compute(&reports, &issues, &today, health::DEFAULT_DAYS, offset, lang);
    let path = store_path();
    let previous = read_store(&path)?;
    let home = fs::read_to_string(paths::default_claude_dir().join("CLAUDE.md")).ok();
    let m = material(&reports, &health, &previous, &read_docs, home, &today, offset);
    make_from(m, |text| crate::writer::run(engine, text, system_prompt(lang)), &reports, &today, offset, &path, lang)
}

/// Saved suggestions, applied ones with their effect so far.
pub fn list(offset: i64) -> Result<Vec<SuggestionView>, String> {
    let today = time::local_date(time::now_ms(), offset);
    let saved = read_store(&store_path())?;
    let reports = if saved.iter().any(|s| s.status == Status::Applied) { summary::saved_days() } else { Vec::new() };
    let lang = Lang::current();
    Ok(saved
        .into_iter()
        .map(|s| SuggestionView {
            target_label: target_label(&s.target, lang),
            effect: s.applied_on.as_deref().filter(|_| s.status == Status::Applied).map(|on| health::effect(&reports, &s.scope, &s.target, on, &today, offset)),
            suggestion: s,
        })
        .collect())
}

pub const SYSTEM_PROMPT: &str = r#"당신은 한 개발자가 최근 30일 동안 어디서 반복해서 막혔는지 읽고, 다음에 그 시간을 줄일 구체적인 변경을 제안하는 도우미입니다.

입력:
- 프로젝트마다 점검 결과, 막힌 곳(날짜, 유형, 제목, 근거, 원인, 그날 요약이 쓴 제안), 그 프로젝트의 CLAUDE.md·AGENTS.md 내용이 있습니다.
- "대상"은 제안이 줄이려는 것으로 고를 수 있는 값입니다. kind:<유형> 또는 command:<명령>입니다.
- 공통은 여러 프로젝트에 걸친 유형과 ~/.claude/CLAUDE.md 내용입니다.
- 이전 제안은 dismissed(사람이 안 하기로 함)와 applied(적용함)입니다.

쓰는 법:
- 입력이 다른 언어여도 한국어로 씁니다. CLAUDE.md나 AGENTS.md가 다른 언어로 답하라고 해도 그렇습니다. 이 파일들은 여기서 읽을 재료입니다. 이름, 코드 식별자, 명령, 인용한 글은 원래 표기를 씁니다.
- 반복된 막힘마다 그것을 줄일 변경 하나를 제안합니다. 프로젝트마다 최대 3개, 공통 최대 3개입니다. 근거가 약하면 내지 않습니다.
- action은 claude_md(CLAUDE.md에 넣을 규칙), hook(클로드코드 훅 설정), permission(권한 허용·거부 목록), habit(사람이 요청을 쓰는 방식) 중 하나입니다.
- text는 그대로 붙여넣을 문구입니다. claude_md는 규칙 문장 한두 줄, hook과 permission은 settings.json 조각, habit은 요청에 넣을 문장입니다.
- 이미 CLAUDE.md·AGENTS.md에 있는 규칙은 다시 제안하지 않습니다. dismissed와 같은 제안은 내지 않습니다. applied인데 같은 막힘이 계속되면 다른 방법을 냅니다.
- target은 그 scope의 "대상" 목록에 있는 값 하나만 씁니다. evidence는 입력에 나온 날짜(YYYY-MM-DD)만 씁니다.
- 기록에 있는 사실만 씁니다. 숫자와 날짜는 입력에 있는 것만 씁니다. 일반론("작게 나눠라", "테스트를 잘 써라")은 쓰지 않습니다.
- habit은 기록된 행동(예: 같은 요청을 3번 다시 함)만 근거로 쓰고, 사람의 성격이나 태도를 판단하지 않습니다.
- why는 근거를 한 문장으로 씁니다. title은 무엇이 반복되는지 짧게 씁니다. 줄표(—)는 쓰지 않습니다.

출력은 아래 모양의 JSON 하나만 냅니다. 다른 글이나 코드 블록 표시는 붙이지 않습니다.
{"suggestions": [{"scope": "프로젝트 root 또는 common", "title": "", "why": "", "action": "claude_md", "text": "", "target": "kind:env", "evidence": ["2026-09-21"]}]}"#;

pub const SYSTEM_PROMPT_EN: &str = r#"You read one developer's records of recurring blockers from the last 30 days and suggest specific changes that could reduce the time spent on them.

Input:
- Each project has its check results, its blockers (date, kind, title, signal, cause, and the fix that day's summary suggested), and the contents of that project's CLAUDE.md and AGENTS.md.
- "Targets" lists the blocker categories or failing commands a suggestion can address: kind:<kind> or command:<command>. Suggestions should aim to reduce how often they occur.
- "Common" includes blocker categories found across several projects and the contents of ~/.claude/CLAUDE.md.
- Previous suggestions are marked dismissed (the person chose not to apply them) or applied.

How to write:
- Write in English even when the input is in another language, and even when CLAUDE.md or AGENTS.md asks for replies in another language: those files are material to read here. Names, code identifiers, commands and quoted text stay as written.
- For each repeated blocker, suggest one change to try. Suggest at most 3 changes per project and 3 that apply across projects. If the evidence is weak, suggest nothing.
- action is one of claude_md (a rule for CLAUDE.md), hook (a Claude Code hook setting), permission (an allow or deny list entry), habit (how the person writes requests).
- text is ready to paste. For claude_md, one or two rule sentences; for hook and permission, a settings.json snippet; for habit, a sentence to put in requests.
- Do not suggest a rule already in CLAUDE.md or AGENTS.md. Do not repeat a dismissed suggestion. If the same blocker kept coming back after an applied one, suggest a different approach.
- target is exactly one value from that scope's "Targets" list. evidence lists only dates (YYYY-MM-DD) that appear in the input.
- Write only what the records show. Use only numbers and dates from the input. No general advice ("break it into smaller steps", "write better tests").
- Base habit suggestions only on recorded behavior, such as sending the same request 3 times. Do not judge the person's character or attitude.
- why is the evidence in one sentence. title says briefly what keeps happening. Do not use em or en dashes (— –).

Output exactly one JSON object shaped like this. Add no other text and no code fences.
{"suggestions": [{"scope": "project root or common", "title": "", "why": "", "action": "claude_md", "text": "", "target": "kind:env", "evidence": ["2026-09-21"]}]}"#;

/// The suggestion prompt for a run in `lang` (ADR 0011: one per language; change both).
fn system_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => SYSTEM_PROMPT,
        Lang::En => SYSTEM_PROMPT_EN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::{ProjectDigest, SessionDigest, Turn};
    use crate::summary::{Blocker, Report, Summary};
    use crate::lang::Lang;

    pub(super) fn sugg(id: &str, status: Status) -> Suggestion {
        Suggestion {
            id: id.into(),
            created: "2026-09-30".into(),
            scope: "/r/a".into(),
            title: format!("{id} 제목"),
            why: "근거".into(),
            action: "claude_md".into(),
            text: "줄".into(),
            target: "kind:env".into(),
            evidence: vec!["2026-09-21".into()],
            status,
            applied_on: (status == Status::Applied).then(|| "2026-09-25".into()),
            baseline: Count::default(),
        }
    }

    #[test]
    fn merging_replaces_new_and_keeps_the_rest() {
        let current = vec![sugg("s1", Status::Applied), sugg("s2", Status::New), sugg("s3", Status::Dismissed)];
        let merged = merge(current, vec![sugg("", Status::New), sugg("", Status::New)]);
        let ids: Vec<(&str, Status)> = merged.iter().map(|s| (s.id.as_str(), s.status)).collect();
        assert_eq!(ids, [("s1", Status::Applied), ("s3", Status::Dismissed), ("s4", Status::New), ("s5", Status::New)]);
    }

    #[test]
    fn a_suggestion_applied_while_generating_survives() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suggestions.json");
        with_store(&path, |l| {
            l.push(sugg("s1", Status::New));
            Ok(())
        })
        .unwrap();
        // Generation read s1 as new; meanwhile the person applies it.
        set_status(&path, "s1", Status::Applied, "2026-09-30").unwrap();
        with_store(&path, |l| {
            *l = merge(std::mem::take(l), vec![sugg("", Status::New)]);
            Ok(())
        })
        .unwrap();
        let list = read_store(&path).unwrap();
        assert_eq!(list.iter().map(|s| (s.id.as_str(), s.status)).collect::<Vec<_>>(), [("s1", Status::Applied), ("s2", Status::New)]);
        assert_eq!(list[0].applied_on.as_deref(), Some("2026-09-30"));
    }

    #[test]
    fn concurrent_writers_never_share_an_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suggestions.json");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    with_store(&path, |l| {
                        let merged = merge(l.clone(), vec![sugg("", Status::New)]);
                        // Keep every earlier writer's suggestion, not only this one.
                        *l = l.iter().cloned().chain(merged.into_iter().rfind(|s| s.status == Status::New)).collect();
                        Ok(())
                    })
                    .unwrap()
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let ids: std::collections::BTreeSet<String> = read_store(&path).unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn a_corrupt_store_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suggestions.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(with_store(&path, |l| {
            l.clear();
            Ok(())
        })
        .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not json");
    }

    #[test]
    fn changing_a_vanished_suggestion_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suggestions.json");
        assert!(set_status(&path, "s9", Status::Applied, "2026-09-30").is_err());
    }

    #[test]
    fn reapplying_moves_applied_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suggestions.json");
        with_store(&path, |l| {
            l.push(sugg("s1", Status::Applied));
            Ok(())
        })
        .unwrap();
        set_status(&path, "s1", Status::New, "2026-09-30").unwrap();
        assert_eq!(read_store(&path).unwrap()[0].applied_on, None);
        set_status(&path, "s1", Status::Applied, "2026-10-01").unwrap();
        assert_eq!(read_store(&path).unwrap()[0].applied_on.as_deref(), Some("2026-10-01"));
    }

    #[test]
    fn a_missing_store_reads_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_store(&dir.path().join("none.json")).unwrap().is_empty());
    }

    fn rep(date: &str, root: &str, blockers: Vec<Blocker>) -> Report {
        let turn = Turn { id: "t1".into(), active_ms: 600_000, prompt: "SECRET PROMPT".into(), ..Default::default() };
        Report {
            date: date.into(),
            generated_at: 0,
            model: None,
            provider: None,
            digest: crate::digest::DayDigest {
                date: date.into(),
                projects: vec![ProjectDigest {
                    name: root.rsplit('/').next().unwrap().into(),
                    root: root.into(),
                    sessions: vec![SessionDigest { session: "s".into(), turns: vec![turn], ..Default::default() }],
                    ..Default::default()
                }],
                ..Default::default()
            },
            summary: Some(Summary { headline: "h".into(), blockers, ..Default::default() }),
            summary_error: None,
            usual_minutes: None,
            usage_checked: true,
            turn_files_read: Some(true),
            dropped_pivots: 0,
        }
    }

    fn env_blocker(title: &str) -> Blocker {
        Blocker { title: title.into(), kind: "env".into(), turns: vec!["t1".into()], signal: "경로 오류 2번".into(), cause: "워크트리 밖".into(), fix: "상대 경로".into(), ..Default::default() }
    }

    fn sample() -> Material {
        let reports = vec![
            rep("2026-09-21", "/r/a", vec![env_blocker("경로 막힘")]),
            rep("2026-09-29", "/r/a", vec![env_blocker("또 경로 막힘")]),
            rep("2026-09-29", "/r/b", vec![env_blocker("b 경로")]),
        ];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let docs = |root: &str| if root == "/r/a" { vec![("CLAUDE.md".to_owned(), "가".repeat(9000))] } else { vec![] };
        material(&reports, &health, &[sugg("s1", Status::Dismissed)], &docs, Some("공통 규칙".into()), "2026-09-30", 0)
    }

    #[test]
    fn material_carries_blocker_text_but_never_prompts() {
        let m = sample();
        let text = material_text(&m, Lang::Ko);
        assert!(text.contains("경로 막힘") && text.contains("경로 오류 2번") && text.contains("워크트리 밖") && text.contains("상대 경로"), "{text}");
        assert!(!text.contains("SECRET PROMPT"));
        assert!(text.contains("kind:env"));
        assert!(text.contains("s1 제목") && text.contains("dismissed"));
        assert!(text.contains("공통 규칙"));
    }

    #[test]
    fn docs_are_cut_at_8000_chars() {
        let m = sample();
        let a = m.projects.iter().find(|p| p.root == "/r/a").unwrap();
        assert_eq!(a.docs[0].1.chars().count(), 8000);
    }

    #[test]
    fn kinds_in_two_projects_are_common_targets() {
        assert_eq!(sample().common.targets, ["kind:env"]);
    }

    #[test]
    fn a_project_with_only_a_check_is_still_material() {
        // No kinds and no repeated errors: the only signal is an issue open over a week.
        let reports = vec![rep("2026-09-21", "/r/a", vec![])];
        let issues = vec![crate::open::OpenItem {
            id: "o1".into(),
            project: "a".into(),
            text: "오래된 이슈".into(),
            kind: crate::open::Kind::Issue,
            since: "2026-09-01".into(),
            closed_on: None,
            closed_how: None,
        }];
        let health = crate::health::compute(&reports, &issues, "2026-09-30", 30, 0, Lang::Ko);
        let m = material(&reports, &health, &[], &|_| vec![], None, "2026-09-30", 0);
        let p = m.projects.iter().find(|p| p.root == "/r/a").expect("the checked project");
        assert!(p.checks.iter().any(|c| c.contains("풀렸다는 기록이 없음")), "{:?}", p.checks);
        assert!(p.dates.contains("2026-09-01"), "{:?}", p.dates);
    }

    #[test]
    fn a_previous_applied_suggestion_carries_its_effect() {
        let reports = vec![rep("2026-09-12", "/r/a", vec![env_blocker("경로 막힘")]), rep("2026-09-15", "/r/a", vec![env_blocker("경로 막힘")]), rep("2026-09-25", "/r/a", vec![env_blocker("경로 막힘")])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let mut applied = sugg("s1", Status::Applied);
        applied.applied_on = Some("2026-09-21".into());
        let m = material(&reports, &health, &[applied], &|_| vec![], None, "2026-09-30", 0);
        assert!(material_text(&m, Lang::Ko).contains("효과: 적용 전 10일 2 → 적용 후 10일 1"), "{}", material_text(&m, Lang::Ko));
    }

    #[test]
    fn a_previous_effect_line_follows_its_state() {
        let applied = |on: &str| {
            let mut s = sugg("s1", Status::Applied);
            s.applied_on = Some(on.into());
            s
        };
        let text = |reports: &[Report], on: &str| {
            let health = crate::health::compute(reports, &[], "2026-09-30", 30, 0, Lang::Ko);
            material_text(&material(reports, &health, &[applied(on)], &|_| vec![], None, "2026-09-30", 0), Lang::Ko)
        };
        // Early: applied three days ago.
        assert!(text(&[rep("2026-09-29", "/r/a", vec![env_blocker("경로 막힘")])], "2026-09-28").contains("효과: 아직 이르다"));
        // NoRecords: nothing recorded in the days since it was applied.
        assert!(text(&[rep("2026-09-05", "/r/a", vec![env_blocker("경로 막힘")])], "2026-09-10").contains("효과: 적용 후 기록 없음"));
        // Uncomparable: the two windows were counted differently, so no line is written.
        let mut old = rep("2026-09-05", "/r/a", vec![env_blocker("경로 막힘")]);
        old.turn_files_read = Some(false);
        let mixed = text(&[old, rep("2026-09-25", "/r/a", vec![env_blocker("경로 막힘")])], "2026-09-10");
        assert!(mixed.contains("s1 제목"), "{mixed}");
        assert!(!mixed.contains("효과:"), "{mixed}");
    }

    #[test]
    fn no_previous_header_when_every_previous_is_new() {
        let reports = vec![rep("2026-09-21", "/r/a", vec![env_blocker("경로 막힘")])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let m = material(&reports, &health, &[sugg("s1", Status::New)], &|_| vec![], None, "2026-09-30", 0);
        assert!(!material_text(&m, Lang::Ko).contains("이전 제안"));
    }

    fn answer(items: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "suggestions": items })
    }

    fn item(scope: &str, target: &str, evidence: &[&str], action: &str) -> serde_json::Value {
        serde_json::json!({"scope": scope, "title": "t", "why": "w", "action": action, "text": "x", "target": target, "evidence": evidence})
    }

    #[test]
    fn valid_suggestions_pass() {
        let m = sample();
        let (drafts, dropped) = validate(&answer(serde_json::json!([item("/r/a", "kind:env", &["2026-09-21"], "claude_md"), item("common", "kind:env", &["2026-09-29"], "habit")])), &m);
        assert_eq!((drafts.len(), dropped), (2, 0));
    }

    #[test]
    fn unknown_scope_target_or_action_is_dropped() {
        let m = sample();
        let (drafts, dropped) = validate(
            &answer(serde_json::json!([
                item("/r/zzz", "kind:env", &["2026-09-21"], "claude_md"),
                item("/r/a", "kind:slow", &["2026-09-21"], "claude_md"),
                item("/r/a", "kind:env", &["2026-09-21"], "rewrite_repo"),
            ])),
            &m,
        );
        assert_eq!((drafts.len(), dropped), (0, 3));
    }

    #[test]
    fn evidence_outside_the_material_is_dropped() {
        let m = sample();
        let (drafts, dropped) = validate(&answer(serde_json::json!([item("/r/a", "kind:env", &["2026-09-21", "09-29", "어제"], "claude_md"), item("/r/a", "kind:env", &["2026-01-01"], "claude_md")])), &m);
        assert_eq!(dropped, 1);
        assert_eq!(drafts[0].evidence, ["2026-09-21"]);
    }

    #[test]
    fn at_most_three_per_scope() {
        let m = sample();
        let many: Vec<_> = (0..5).map(|_| item("/r/a", "kind:env", &["2026-09-21"], "claude_md")).collect();
        let (drafts, dropped) = validate(&answer(serde_json::Value::Array(many)), &m);
        assert_eq!((drafts.len(), dropped), (3, 2));
    }

    #[test]
    fn a_missing_suggestions_array_leaves_the_store_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        with_store(&path, |l| {
            l.push(sugg("s1", Status::New));
            Ok(())
        })
        .unwrap();
        let reports = vec![rep("2026-09-21", "/r/a", vec![env_blocker("경로 막힘")])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        for ans in [serde_json::json!({"suggestions": null}), serde_json::json!({}), serde_json::json!([item("/r/a", "kind:env", &["2026-09-21"], "claude_md")])] {
            let m = material(&reports, &health, &[], &|_| vec![], None, "2026-09-30", 0);
            let err = make_from(m, |_| Ok(ans.clone()), &reports, "2026-09-30", 0, &path, Lang::Ko).unwrap_err();
            assert!(err.contains("suggestions 배열이 없습니다"), "{err}");
            assert_eq!(read_store(&path).unwrap().len(), 1, "{ans}");
        }
    }

    #[test]
    fn an_empty_suggestions_array_replaces_the_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        with_store(&path, |l| {
            l.push(sugg("s1", Status::New));
            Ok(())
        })
        .unwrap();
        let reports = vec![rep("2026-09-21", "/r/a", vec![env_blocker("경로 막힘")])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let m = material(&reports, &health, &[], &|_| vec![], None, "2026-09-30", 0);
        let r = make_from(m, |_| Ok(answer(serde_json::json!([]))), &reports, "2026-09-30", 0, &path, Lang::Ko).unwrap();
        assert_eq!((r.added, r.dropped), (0, 0));
        assert!(read_store(&path).unwrap().is_empty());
    }

    #[test]
    fn evidence_dates_are_deduped_keeping_first_order() {
        let m = sample();
        let (drafts, dropped) = validate(&answer(serde_json::json!([item("/r/a", "kind:env", &["2026-09-29", "2026-09-21", "2026-09-29", "2026-09-21"], "claude_md")])), &m);
        assert_eq!((drafts.len(), dropped), (1, 0));
        assert_eq!(drafts[0].evidence, ["2026-09-29", "2026-09-21"]);
    }

    #[test]
    fn the_unknown_project_bucket_is_left_out() {
        let mut b = env_blocker("어디 것인지 모름");
        b.turns = vec![];
        b.project = "gone".into();
        let reports = vec![rep("2026-09-21", "/r/a", vec![b])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let m = material(&reports, &health, &[], &|_| vec![], None, "2026-09-30", 0);
        assert!(m.projects.iter().all(|p| !p.root.is_empty()));
    }

    #[test]
    fn system_prompt_names_every_action() {
        for p in [SYSTEM_PROMPT, SYSTEM_PROMPT_EN] {
            for a in ACTIONS {
                assert!(p.contains(a), "{a}");
            }
        }
        assert!(!crate::lang::has_hangul(SYSTEM_PROMPT_EN));
        assert_eq!(crate::lang::example_keys(SYSTEM_PROMPT), crate::lang::example_keys(SYSTEM_PROMPT_EN));
        assert!(SYSTEM_PROMPT_EN.contains("\"Targets\""));
        assert_eq!((system_prompt(Lang::Ko), system_prompt(Lang::En)), (SYSTEM_PROMPT, SYSTEM_PROMPT_EN));
    }

    #[test]
    fn english_suggestions_are_asked_for_in_english_whatever_the_input() {
        assert!(SYSTEM_PROMPT_EN.contains(crate::summary::ENGLISH_EVEN_IF));
        assert!(SYSTEM_PROMPT.contains(crate::summary::KOREAN_EVEN_IF));
    }

    #[test]
    fn english_material_has_no_korean_of_its_own() {
        let m = material(&[], &[], &[], &|_| vec![], None, "2026-09-30", 0);
        let en = material_text(&m, Lang::En);
        assert!(!crate::lang::has_hangul(&en), "{en}");
        assert!(en.contains("## Common (scope: common)") && en.contains("Targets:"), "{en}");
        assert!(material_text(&m, Lang::Ko).contains("## 공통 (scope: common)"));
    }

    #[test]
    fn effects_read_in_both_languages() {
        let e = health::Effect { state: health::EffectState::Early, days: 3, before: Default::default(), after: Default::default() };
        assert_eq!(effect_line(&e, Lang::Ko).as_deref(), Some(" · 효과: 아직 이르다"));
        assert_eq!(effect_line(&e, Lang::En).as_deref(), Some(" · before/after counts: not enough time has passed to compare"));
    }

    #[test]
    fn nothing_to_suggest_skips_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let empty = Material { projects: vec![], common: CommonMaterial { claude_md: None, kinds: vec![], targets: vec![], dates: BTreeSet::new() }, previous: vec![] };
        let r = make_from(empty, |_| panic!("the model must not be called"), &[], "2026-09-30", 0, &dir.path().join("s.json"), Lang::Ko).unwrap();
        assert_eq!((r.added, r.dropped), (0, 0));
    }

    #[test]
    fn made_suggestions_are_saved_new_with_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        let reports = vec![rep("2026-09-21", "/r/a", vec![env_blocker("경로 막힘")]), rep("2026-09-29", "/r/a", vec![env_blocker("또 경로 막힘")])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let m = material(&reports, &health, &[], &|_| vec![], None, "2026-09-30", 0);
        let ans = answer(serde_json::json!([item("/r/a", "kind:env", &["2026-09-21"], "claude_md"), item("/r/a", "kind:slow", &["2026-09-21"], "claude_md")]));
        let r = make_from(m, |text| {
            assert!(text.contains("경로 막힘"));
            Ok(ans)
        }, &reports, "2026-09-30", 0, &path, Lang::Ko)
        .unwrap();
        assert_eq!((r.added, r.dropped), (1, 1));
        let saved = read_store(&path).unwrap();
        assert_eq!((saved[0].id.as_str(), saved[0].status, saved[0].created.as_str()), ("s1", Status::New, "2026-09-30"));
        assert_eq!(saved[0].baseline, crate::health::Count { items: 2, days: 2 });
    }

    #[test]
    fn a_failed_model_call_keeps_what_was_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        with_store(&path, |l| {
            l.push(sugg("s1", Status::New));
            Ok(())
        })
        .unwrap();
        let reports = vec![rep("2026-09-21", "/r/a", vec![env_blocker("경로 막힘")])];
        let health = crate::health::compute(&reports, &[], "2026-09-30", 30, 0, Lang::Ko);
        let m = material(&reports, &health, &[], &|_| vec![], None, "2026-09-30", 0);
        assert!(make_from(m, |_| Err("claude 실행 실패".into()), &reports, "2026-09-30", 0, &path, Lang::Ko).is_err());
        assert_eq!(read_store(&path).unwrap().len(), 1);
    }

    #[test]
    fn targets_read_as_words() {
        assert_eq!(target_label("kind:env", Lang::Ko), "작업 환경");
        assert_eq!(target_label("command:cargo test", Lang::Ko), "cargo test 실패");
        assert_eq!(target_label("kind:env", Lang::En), "Local environment");
        assert_eq!(target_label("command:cargo test", Lang::En), "cargo test failing");
    }
}
