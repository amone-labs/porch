//! Day and week reports as Markdown, for keeping or sending outside the app.
//! The PDF comes from the app printing its own report view; this module only
//! writes text. Its words follow the language passed in: the summary's own
//! (`Report::lang_or`), so one note never mixes two languages.

use crate::lang::Lang;
use crate::open::OpenItem;
use crate::summary::{Blocker, MonthReport, Report, TimeBlock, Visual, WeekReport, WeekSummary};
use crate::time;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The fixed words of an exported report.
struct Words {
    time_by_work: &'static str,
    table_head: &'static str,
    stuck: &'static str,
    solved: &'static str,
    unsolved: &'static str,
    evidence: &'static str,
    cause: &'static str,
    fix: &'static str,
    carried: &'static str,
    first_seen: &'static str,
    issue: &'static str,
    waiting: &'static str,
    done: &'static str,
    in_progress: &'static str,
    next: &'static str,
    commits_head: &'static str,
    old_analysis: &'static str,
    observations: &'static str,
    suggestions: &'static str,
    by_day: &'static str,
}

const KO: Words = Words {
    time_by_work: "작업별 시간",
    table_head: "| 일 | 프로젝트 | 시간 | 비율 |",
    stuck: "막힌 작업",
    solved: "풀림",
    unsolved: "안 풀림",
    evidence: "근거",
    cause: "원인",
    fix: "예방법",
    carried: "이날 처음 기록된 일과 닫힌 일",
    first_seen: "첫 기록",
    issue: "막힘",
    waiting: "남은 일",
    done: "끝낸 일",
    in_progress: "아직 못 끝낸 일",
    next: "다음 할 일",
    commits_head: "커밋",
    old_analysis: "작업 특징·개선점",
    observations: "작업 특징",
    suggestions: "개선 제안",
    by_day: "날마다",
};

const EN: Words = Words {
    time_by_work: "Time by task",
    table_head: "| Task | Project | Time | Share |",
    stuck: "Stuck tasks",
    solved: "Resolved",
    unsolved: "Unresolved",
    evidence: "Evidence",
    cause: "Cause",
    fix: "Next time",
    carried: "Open work: first recorded or closed on this date",
    first_seen: "first seen",
    issue: "Blocker",
    waiting: "Pending",
    done: "Done",
    in_progress: "Not finished",
    next: "Next",
    commits_head: "Commits",
    old_analysis: "Notes and suggestions",
    observations: "Notes",
    suggestions: "Suggestions",
    by_day: "By day",
};

fn words(lang: Lang) -> &'static Words {
    match lang {
        Lang::Ko => &KO,
        Lang::En => &EN,
    }
}

/// "프로젝트 3" in Korean, "3 projects" in English.
fn stat(lang: Lang, n: impl std::fmt::Display, ko: &str, en: &str) -> String {
    match lang {
        Lang::Ko => format!("{ko} {n}"),
        Lang::En => format!("{n} {en}"),
    }
}

fn minutes(min: u64, lang: Lang) -> String {
    match (min / 60, min % 60, lang) {
        (0, m, Lang::Ko) => format!("{m}분"),
        (h, 0, Lang::Ko) => format!("{h}시간"),
        (h, m, Lang::Ko) => format!("{h}시간 {m}분"),
        (0, m, Lang::En) => format!("{m}m"),
        (h, 0, Lang::En) => format!("{h}h"),
        (h, m, Lang::En) => format!("{h}h {m}m"),
    }
}

fn money(usd: f64) -> String {
    if usd >= 100.0 { format!("${usd:.0}") } else { format!("${usd:.2}") }
}

fn tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}K", n / 1000),
        1_000_000..=999_999_999 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.2}B", n as f64 / 1e9),
    }
}

/// "API-equivalent cost $23.40 · 18.2M tokens · 91% cache hits" for a stat line.
fn usage_items(u: &crate::usage::Usage, lang: Lang) -> Vec<String> {
    let mut v = Vec::new();
    if u.cost > 0.0 {
        v.push(match lang {
            Lang::Ko => format!("환산 비용 {}", money(u.cost)),
            Lang::En => format!("API-equivalent cost {}", money(u.cost)),
        });
    }
    v.push(stat(lang, tokens(u.tokens.total()), "토큰", "tokens"));
    if let Some(h) = u.cache_hit() {
        v.push(match lang {
            Lang::Ko => format!("캐시 적중률 {}%", (h * 100.0).round()),
            Lang::En => format!("{}% cache hits", (h * 100.0).round()),
        });
    }
    v
}

/// "33 commits", or "33 commits · 12 mine" when some were written by others.
fn commits_label(total: usize, mine: Option<usize>, lang: Lang) -> String {
    match (mine, lang) {
        (Some(m), Lang::Ko) if m != total => format!("커밋 {total} · 내 커밋 {m}"),
        (Some(m), Lang::En) if m != total => format!("{total} commits · {m} mine"),
        (_, lang) => stat(lang, total, "커밋", "commits"),
    }
}

fn bullets(out: &mut String, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(out, "\n**{title}**\n");
    for i in items {
        let _ = writeln!(out, "- {i}");
    }
}

/// "Time by task", "Stuck tasks" and the diagrams: the part of a report that
/// answers where the time went and where it got stuck.
fn insight(out: &mut String, time: &[TimeBlock], blockers: &[Blocker], visuals: &[Visual], lang: Lang) {
    let w = words(lang);
    if !time.is_empty() {
        let total: u64 = time.iter().map(|b| b.minutes).sum::<u64>().max(1);
        let _ = writeln!(out, "\n## {}\n", w.time_by_work);
        let _ = writeln!(out, "{}\n| --- | --- | ---: | ---: |", w.table_head);
        for b in time {
            let _ = writeln!(out, "| {} | {} | {} | {}% |", b.label, b.project, minutes(b.minutes, lang), b.minutes * 100 / total);
        }
        for b in time.iter().filter(|b| !b.note.is_empty()) {
            let _ = writeln!(out, "\n- **{}**: {}", b.label, b.note);
        }
    }
    if !blockers.is_empty() {
        let _ = writeln!(out, "\n## {}", w.stuck);
        for b in blockers {
            let state = if b.resolved { w.solved } else { w.unsolved };
            let project = if b.project.is_empty() { String::new() } else { format!(" · {}", b.project) };
            let _ = writeln!(out, "\n### {}\n\n{state} · {}{project}\n", b.title, minutes(b.minutes, lang));
            if !b.signal.is_empty() {
                let _ = writeln!(out, "- {}: {}", w.evidence, b.signal);
            }
            if !b.cause.is_empty() {
                let _ = writeln!(out, "- {}: {}", w.cause, b.cause);
            }
            if !b.fix.is_empty() {
                let _ = writeln!(out, "- {}: {}", w.fix, b.fix);
            }
        }
    }
    for v in visuals {
        let _ = writeln!(out, "\n### {}\n\n```mermaid\n{}\n```", v.title, v.mermaid.trim());
        if !v.caption.is_empty() {
            let _ = writeln!(out, "\n{}", v.caption);
        }
    }
}

/// Open work this day closed from earlier days, and what it left open.
fn carried(out: &mut String, date: &str, items: &[OpenItem], lang: Lang) {
    let w = words(lang);
    let closed: Vec<_> = items.iter().filter(|i| i.closed_on.as_deref() == Some(date) && i.closed_how.as_deref() != Some(crate::open::MANUAL)).collect();
    let opened: Vec<_> = items.iter().filter(|i| i.since == date).collect();
    if closed.is_empty() && opened.is_empty() {
        return;
    }
    let _ = writeln!(out, "\n## {}\n", w.carried);
    for i in closed {
        let _ = writeln!(out, "- [x] {} ({}, {} {} · {})", i.text, i.project, w.first_seen, i.since, crate::open::closed_label(i.closed_how.as_deref(), lang));
    }
    for i in opened {
        let kind = if i.kind == crate::open::Kind::Issue { w.issue } else { w.waiting };
        let mark = if i.closed_on.is_some() { "x" } else { " " };
        let _ = writeln!(out, "- [{mark}] {kind}: {} ({})", i.text, i.project);
    }
}

pub fn day_markdown(r: &Report, lang: Lang) -> String {
    day_markdown_with(r, &crate::open::load(crate::summary::saved_days), lang)
}

/// `day_markdown` over the open items given, so tests never read the data dir.
fn day_markdown_with(r: &Report, items: &[OpenItem], lang: Lang) -> String {
    let w = words(lang);
    let d = &r.digest;
    let m = &d.metrics;
    let mut out = String::new();
    let _ = writeln!(out, "# {}", time::long_date(&r.date, lang));
    if let Some(s) = &r.summary {
        let _ = writeln!(out, "\n{}", s.headline);
    }
    let mut stats = vec![
        minutes(m.active_minutes, lang),
        stat(lang, d.projects.len(), "프로젝트", "projects"),
        stat(lang, m.sessions, "세션", "sessions"),
        stat(lang, m.prompts, "요청", "requests"),
        commits_label(m.commits, m.my_commits, lang),
    ];
    if m.commits > 0 {
        stats.push(format!("+{} −{}", m.insertions, m.deletions));
    }
    if let Some(u) = r.usual_minutes {
        stats.push(match lang {
            Lang::Ko => format!("최근 평균 {}", minutes(u, lang)),
            Lang::En => format!("recent average {}", minutes(u, lang)),
        });
    }
    if let Some(u) = &m.usage {
        stats.extend(usage_items(u, lang));
    }
    let _ = writeln!(out, "\n{}", stats.join(" · "));
    if let Some(s) = &r.summary {
        insight(&mut out, &s.time, &s.blockers, &s.visuals, lang);
        carried(&mut out, &r.date, items, lang);
    }

    for p in &d.projects {
        let pm = &p.metrics;
        let _ = writeln!(out, "\n## {}\n", p.name);
        let _ = writeln!(out, "{} · {} · {}", minutes(pm.active_minutes, lang), stat(lang, pm.sessions, "세션", "sessions"), commits_label(pm.commits, pm.my_commits, lang));
        if let Some(ps) = r.summary.as_ref().and_then(|s| s.projects.iter().find(|x| x.name == p.name)) {
            let _ = writeln!(out, "\n{}", ps.summary);
            bullets(&mut out, w.done, &ps.done);
            bullets(&mut out, w.in_progress, &ps.in_progress);
            bullets(&mut out, w.next, &ps.next);
        }
        if !p.commits.is_empty() {
            let _ = writeln!(out, "\n**{}**\n", w.commits_head);
            for c in &p.commits {
                let repo = c.repo.as_deref().map(|r| format!("{r} ")).unwrap_or_default();
                let who = c.author.as_deref().filter(|_| c.mine == Some(false)).map(|a| format!(" · {a}")).unwrap_or_default();
                let _ = writeln!(out, "- `{repo}{}` {} (+{} −{}){who}", c.hash, c.subject, c.insertions, c.deletions);
            }
        }
    }

    if let Some(s) = &r.summary {
        if !s.analysis.observations.is_empty() || !s.analysis.suggestions.is_empty() {
            let _ = writeln!(out, "\n## {}\n", w.old_analysis);
            let _ = match lang {
                Lang::Ko => writeln!(out, "도구 호출 {} · 오류 {} · 권한 거절 {}", m.tool_calls, m.tool_errors, m.denials),
                Lang::En => writeln!(out, "{} tool calls · {} tool errors · {} permission denials", m.tool_calls, m.tool_errors, m.denials),
            };
            bullets(&mut out, w.observations, &s.analysis.observations);
            bullets(&mut out, w.suggestions, &s.analysis.suggestions);
        }
    }
    footer(&mut out, r.model.as_deref(), r.provider, lang);
    out
}

/// "3h 50m · 5 recorded days · 3 projects · 33 commits" under a week or month title.
fn period_stats(total: u64, days: usize, projects: usize, commits: usize, mine: Option<usize>, lang: Lang) -> String {
    match lang {
        Lang::Ko => format!("{} · 기록된 날 {days} · 프로젝트 {projects} · {}", minutes(total, lang), commits_label(commits, mine, lang)),
        Lang::En => format!("{} · {days} recorded days · {projects} projects · {}", minutes(total, lang), commits_label(commits, mine, lang)),
    }
}

pub fn month_markdown(r: &MonthReport, lang: Lang) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}", time::month_title(&r.month, lang));
    if let Some(s) = &r.summary {
        let _ = writeln!(out, "\n{}", s.headline);
    }
    let total: u64 = r.days.iter().map(|d| d.minutes).sum();
    let commits: usize = r.days.iter().map(|d| d.commits).sum();
    let mine: Option<usize> = r.days.iter().map(|d| d.my_commits).sum();
    let projects: std::collections::BTreeSet<&String> = r.days.iter().flat_map(|d| d.projects.keys()).collect();
    let _ = writeln!(out, "\n{}", period_stats(total, r.days.len(), projects.len(), commits, mine, lang));
    if let Some(s) = &r.summary {
        insight(&mut out, &s.time, &s.blockers, &s.visuals, lang);
        projects_md(&mut out, s, lang);
    }
    footer(&mut out, r.model.as_deref(), r.provider, lang);
    out
}

fn projects_md(out: &mut String, s: &WeekSummary, lang: Lang) {
    let w = words(lang);
    for p in &s.projects {
        let _ = writeln!(out, "\n## {}\n\n{}", p.name, p.summary);
        bullets(out, w.done, &p.shipped);
        bullets(out, w.in_progress, &p.ongoing);
        bullets(out, w.next, &p.next);
    }
}

pub fn week_markdown(r: &WeekReport, lang: Lang) -> String {
    let w = words(lang);
    let mut out = String::new();
    let total: u64 = r.days.iter().map(|d| d.minutes).sum();
    let commits: usize = r.days.iter().map(|d| d.commits).sum();
    let mine: Option<usize> = r.days.iter().map(|d| d.my_commits).sum();
    let _ = match lang {
        Lang::Ko => writeln!(out, "# {} 주", time::long_date(&r.week_start, lang)),
        Lang::En => writeln!(out, "# Week of {}", time::long_date(&r.week_start, lang)),
    };
    if let Some(s) = &r.summary {
        let _ = writeln!(out, "\n{}", s.headline);
    }
    let projects: std::collections::BTreeSet<&String> = r.days.iter().flat_map(|d| d.projects.keys()).collect();
    let _ = writeln!(out, "\n{}", period_stats(total, r.days.len(), projects.len(), commits, mine, lang));

    let _ = writeln!(out, "\n## {}\n", w.by_day);
    for d in r.days.iter().filter(|d| d.minutes > 0 || d.commits > 0) {
        let head = d.headline.as_deref().map(|h| format!(" · {h}")).unwrap_or_default();
        let _ = writeln!(out, "- {} · {} · {}{head}", time::long_date(&d.date, lang), minutes(d.minutes, lang), commits_label(d.commits, d.my_commits, lang));
    }

    if let Some(s) = &r.summary {
        insight(&mut out, &s.time, &s.blockers, &s.visuals, lang);
        projects_md(&mut out, s, lang);
        if !s.analysis.observations.is_empty() || !s.analysis.suggestions.is_empty() {
            let _ = writeln!(out, "\n## {}", w.old_analysis);
            bullets(&mut out, w.observations, &s.analysis.observations);
            bullets(&mut out, w.suggestions, &s.analysis.suggestions);
        }
    }
    footer(&mut out, r.model.as_deref(), r.provider, lang);
    out
}

fn footer(out: &mut String, model: Option<&str>, provider: Option<crate::writer::Provider>, lang: Lang) {
    let _ = writeln!(out, "\n---\n");
    let _ = match (model, lang) {
        (Some(m), Lang::Ko) => writeln!(
            out,
            "porch · 요약은 {}가 작업 기록을 읽고 썼습니다. 작업 시간은 기록으로 계산한 추정치입니다. 10분 넘게 쉰 구간은 빼고, 동시에 돌린 작업은 두 번 세지 않습니다.",
            crate::writer::written_by(provider, m, lang)
        ),
        (Some(m), Lang::En) => writeln!(
            out,
            "porch · {} wrote this summary from the work records. Work time is estimated from the records: breaks over 10 minutes are left out, and overlapping work counts once.",
            crate::writer::written_by(provider, m, lang)
        ),
        (None, Lang::Ko) => writeln!(out, "porch · 작업 시간은 기록으로 계산한 추정치입니다. 10분 넘게 쉰 구간은 빼고, 동시에 돌린 작업은 두 번 세지 않습니다."),
        (None, Lang::En) => writeln!(out, "porch · Work time is estimated from the records: breaks over 10 minutes are left out, and overlapping work counts once."),
    };
}

/// `dir/stem.ext`, or `dir/stem 2.ext`, `stem 3.ext`… when taken, as Finder names copies.
pub fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} {n}.{ext}")))
        .find(|p| !p.exists())
        .expect("some name is free")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::{DayDigest, Metrics, ProjectDigest};
    use crate::lang::has_hangul;
    use crate::summary::{Blocker, Summary, TimeBlock};

    fn english_day() -> Report {
        Report {
            date: "2026-10-02".into(),
            generated_at: 0,
            model: Some("default".into()),
            provider: None,
            digest: DayDigest {
                date: "2026-10-02".into(),
                metrics: Metrics { active_minutes: 80, sessions: 2, prompts: 5, commits: 1, ..Default::default() },
                projects: vec![ProjectDigest { name: "porch".into(), metrics: Metrics { active_minutes: 80, sessions: 2, commits: 1, ..Default::default() }, ..Default::default() }],
                ..Default::default()
            },
            summary: Some(Summary {
                headline: "Tray fixed".into(),
                lang: Some("en".into()),
                time: vec![TimeBlock { label: "Tray".into(), project: "porch".into(), minutes: 60, note: "Mostly morning".into(), ..Default::default() }],
                blockers: vec![Blocker { title: "Tests".into(), minutes: 20, signal: "3 times".into(), cause: "flaky".into(), fix: "pin seed".into(), ..Default::default() }],
                ..Default::default()
            }),
            summary_error: None,
            usual_minutes: Some(70),
            usage_checked: true,
            turn_files_read: Some(true),
            dropped_pivots: 0,
        }
    }

    #[test]
    fn an_english_day_has_no_korean_of_its_own() {
        let md = day_markdown_with(&english_day(), &[], Lang::En);
        assert!(!has_hangul(&md), "{md}");
        for w in ["# Fri, Oct 2, 2026", "## Time by task", "## Stuck tasks", "Unresolved", "Evidence: 3 times", "Next time: pin seed", "2 sessions", "Claude Code (default model) wrote this summary", "Work time is estimated"] {
            assert!(md.contains(w), "{w}\n{md}");
        }
        let ko = day_markdown_with(&english_day(), &[], Lang::Ko);
        assert!(ko.contains("## 작업별 시간") && ko.contains("세션 2"), "{ko}");
    }

    #[test]
    fn long_date_has_weekday() {
        assert_eq!(time::long_date_ko("2026-09-29"), "2026년 9월 29일 (화)");
        assert_eq!(time::long_date_ko("2026-09-27"), "2026년 9월 27일 (일)");
    }

    #[test]
    fn free_path_counts_up() {
        let dir = std::env::temp_dir().join(format!("porch-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = free_path(&dir, "porch-2026-09-29", "md");
        assert!(a.ends_with("porch-2026-09-29.md"));
        std::fs::write(&a, "").unwrap();
        assert!(free_path(&dir, "porch-2026-09-29", "md").ends_with("porch-2026-09-29 2.md"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
