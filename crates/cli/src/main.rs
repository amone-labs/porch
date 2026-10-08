//! `porch` — command-line side of porch: board, summaries, hooks.
//!
//!   porch install   [--dry-run] [--only claude|codex] [--claude-dir DIR]... [--codex-home DIR]...
//!   porch uninstall [--dry-run] [--claude-dir DIR]... [--codex-home DIR]...
//!   porch status    [--claude-dir DIR]... [--codex-home DIR]...
//!   porch events    [--date YYYY-MM-DD] [--tail N]
//!   porch keys      [--date YYYY-MM-DD]
//!   porch now       [--json]
//!   porch watch     [--interval SECONDS]
//!   porch compare-orca
//!   porch doctor    (what this Mac has: agents, hooks, the claude and codex CLIs)
//!   porch today     [--date YYYY-MM-DD] [--provider claude|codex] [--model M] [--refresh] [--no-llm] [--json|--md]
//!   porch week      [--date any-day-in-week] [--provider claude|codex] [--model M] [--refresh] [--no-llm] [--json|--md]
//!   porch month     [--date any-day-in-month] [--provider claude|codex] [--model M] [--refresh] [--no-llm] [--json|--md]
//!   porch insight   [--date any-day-in-week] [--refresh] [--json]   (a week's observed numbers, checkpoints, goals and model evaluation; --refresh writes the evaluation of a finished week)
//!   porch statusline on|off|status|wrap <folder>|unwrap <folder> [--no-line]   (Claude's usage limits through the status line)
//!   porch usage     [--tail DAYS] [--date last-day]   (tokens and list-price cost, JSON)
//!   porch limits    (usage limits per agent account, JSON)
//!   porch digest    [--date YYYY-MM-DD]      (the text the summarizer reads)
//!   porch classify-blockers [--provider claude|codex] [--model M] (give blockers summarized before kinds existed a kind)
//!   porch suggest   [--provider claude|codex] [--model M]   (suggests changes for repeated blockers)
//!   porch suggestions [--json]        (saved suggestions and, once applied, their effect)
//!   porch mirror    [vaults | set <dir> [--vault <id>] | off]   (summaries as Markdown notes in a folder or Obsidian vault)
//!
//! `--claude-dir` / `--codex-home` replace the default search list when
//! given, which is how tests point the installer at copies instead of the
//! real config.
//!
//! `--provider` / `--model` default to the app's Settings; `--model`
//! applies to the provider in use.

use porch_core::lang::Lang;
use porch_core::event::Event;
use porch_core::install::{self, Action, Agent};
use porch_core::writer::{Engine, Provider};
use porch_core::{paths, time};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use porch_core::summary;

struct Opts {
    dry_run: bool,
    only: Option<Agent>,
    claude_dirs: Vec<PathBuf>,
    codex_homes: Vec<PathBuf>,
    date: Option<String>,
    tail: Option<usize>,
    json: bool,
    md: bool,
    no_line: bool,
    all: bool,
    positional: Vec<String>,
    provider: Option<String>,
    model: Option<String>,
    refresh: bool,
    no_llm: bool,
    vault: Option<String>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        dry_run: false,
        only: None,
        claude_dirs: vec![],
        codex_homes: vec![],
        date: None,
        tail: None,
        json: false,
        md: false,
        no_line: false,
        all: false,
        positional: vec![],
        provider: None,
        model: None,
        refresh: false,
        no_llm: false,
        vault: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--dry-run" => o.dry_run = true,
            "--only" => {
                o.only = Some(match val()?.as_str() {
                    "claude" => Agent::Claude,
                    "codex" => Agent::Codex,
                    other => return Err(format!("unknown agent {other}")),
                })
            }
            "--claude-dir" => o.claude_dirs.push(val()?.into()),
            "--codex-home" => o.codex_homes.push(val()?.into()),
            "--date" => o.date = Some(val()?),
            "--tail" | "--interval" => o.tail = Some(val()?.parse().map_err(|_| format!("{a} needs a number"))?),
            "--json" => o.json = true,
            "--md" => o.md = true,
            "--no-line" => o.no_line = true,
            "--provider" => o.provider = Some(val()?),
            "--model" => o.model = Some(val()?),
            "--vault" => o.vault = Some(val()?),
            "--refresh" => o.refresh = true,
            "--no-llm" => o.no_llm = true,
            "--all" => o.all = true,
            other if !other.starts_with("--") => o.positional.push(other.to_owned()),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(o)
}

/// (agent, hooks file) pairs the command applies to.
fn targets(o: &Opts) -> Vec<(Agent, PathBuf)> {
    let claude = if o.claude_dirs.is_empty() {
        vec![paths::default_claude_dir()]
    } else {
        o.claude_dirs.clone()
    };
    let codex = if o.codex_homes.is_empty() {
        paths::codex_homes()
    } else {
        o.codex_homes.clone()
    };
    let mut out = Vec::new();
    if o.only != Some(Agent::Codex) {
        out.extend(claude.iter().map(|d| (Agent::Claude, Agent::Claude.hooks_file(d))));
    }
    if o.only != Some(Agent::Claude) {
        out.extend(codex.iter().map(|d| (Agent::Codex, Agent::Codex.hooks_file(d))));
    }
    out
}

fn copy_hook_binary(dry_run: bool) -> Result<PathBuf, String> {
    let dest = paths::installed_hook_bin();
    let src = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("porch-hook");
    if !src.exists() {
        return Err(format!("{} not found; build it first (cargo build --release)", src.display()));
    }
    if dry_run {
        println!("would copy {} -> {}", src.display(), dest.display());
        return Ok(dest);
    }
    install::stage_hook_binary(&src).map_err(|e| e.to_string())?;
    println!("hook binary: {}", dest.display());
    Ok(dest)
}

fn report(path: &Path, plan: &install::Plan, dry_run: bool) {
    let verb = if dry_run { "would change" } else { "changed" };
    if plan.is_empty() {
        println!("  {}: up to date", path.display());
        return;
    }
    println!("  {}: {verb}", path.display());
    for (label, list) in [("add", &plan.added), ("update", &plan.updated), ("remove", &plan.removed)] {
        if !list.is_empty() {
            println!("    {label}: {}", list.join(", "));
        }
    }
}

fn cmd_install(o: &Opts) -> Result<(), String> {
    let bin = copy_hook_binary(o.dry_run)?;
    let mut codex_touched = false;
    for (agent, file) in targets(o) {
        let plan = install::apply_to_file(&file, Action::Install { agent, bin: &bin }, o.dry_run)
            .map_err(|e| e.to_string())?;
        codex_touched |= agent == Agent::Codex && !plan.added.is_empty();
        report(&file, &plan, o.dry_run);
    }
    if codex_touched && !o.dry_run {
        println!("\nCodex runs new hooks only after you approve them in Codex.");
    }
    Ok(())
}

fn cmd_uninstall(o: &Opts) -> Result<(), String> {
    for (_, file) in targets(o) {
        if !file.exists() {
            continue;
        }
        let plan = install::apply_to_file(&file, Action::Uninstall, o.dry_run).map_err(|e| e.to_string())?;
        report(&file, &plan, o.dry_run);
    }
    Ok(())
}

fn cmd_status(o: &Opts) -> Result<(), String> {
    let bin = paths::installed_hook_bin();
    println!("hook binary: {} ({})", bin.display(), if bin.exists() { "present" } else { "missing" });
    for (agent, file) in targets(o) {
        let plan = install::apply_to_file(&file, Action::Install { agent, bin: &bin }, true)
            .map_err(|e| e.to_string())?;
        let state = if plan.is_empty() {
            "installed".to_owned()
        } else if plan.added.len() == agent.events().len() {
            "not installed".to_owned()
        } else {
            format!("partial (missing {}, stale {})", plan.added.len(), plan.updated.len())
        };
        println!("  {} [{}]: {state}", file.display(), agent.name());
    }
    Ok(())
}

fn read_events(date: &str) -> Result<(Vec<Event>, usize), String> {
    let path = paths::events_file(date);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bad = 0;
    let events = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Event>(l).map_err(|_| bad += 1).ok())
        .collect();
    Ok((events, bad))
}

fn today(o: &Opts) -> String {
    o.date.clone().unwrap_or_else(|| time::utc_date(time::now_ms()))
}

fn cmd_events(o: &Opts) -> Result<(), String> {
    let (events, bad) = read_events(&today(o))?;
    let skip = o.tail.map(|n| events.len().saturating_sub(n)).unwrap_or(0);
    for e in &events[skip..] {
        println!(
            "{} {:6} {:20} {:10} {}",
            e.t,
            e.agent,
            e.event,
            e.tool.as_deref().unwrap_or("-"),
            e.session.as_deref().unwrap_or("-")
        );
    }
    if bad > 0 {
        eprintln!("{bad} line(s) did not parse");
    }
    Ok(())
}

/// Which payload keys each (agent, event) pair carried. This is how the
/// Codex hook format gets confirmed without storing any payload content.
fn cmd_keys(o: &Opts) -> Result<(), String> {
    let (events, _) = read_events(&today(o))?;
    let mut seen: BTreeMap<(String, String), (usize, std::collections::BTreeSet<String>)> = BTreeMap::new();
    for e in events {
        let entry = seen.entry((e.agent, e.event)).or_default();
        entry.0 += 1;
        entry.1.extend(e.keys);
    }
    for ((agent, event), (n, keys)) in seen {
        println!("{agent:6} {event:20} x{n:<5} {}", keys.into_iter().collect::<Vec<_>>().join(", "));
    }
    Ok(())
}

/// English labels match the app's state names (Settings › language).
fn state_label(s: &porch_core::state::Session, lang: Lang) -> String {
    use porch_core::state::State::*;
    let en = lang == Lang::En;
    let label = match s.state {
        Permission => if en { "Needs permission" } else { "권한 대기" },
        Question => if en { "Has a question" } else { "질문 대기" },
        Failed => if en { "Stopped on error" } else { "오류로 멈춤" },
        TurnDone => if en { "Done" } else { "차례 끝남" },
        Running => if en { "Working" } else { "실행 중" },
        Ended => if en { "Ended" } else { "세션 끝" },
    };
    match (s.state, s.pending.as_ref().and_then(|p| p.tool.as_deref())) {
        (Permission, Some(t)) => format!("{label} ({t})"),
        _ => label.to_owned(),
    }
}

fn ago(now: u64, t: u64, lang: Lang) -> String {
    let m = now.saturating_sub(t) / 60_000;
    match (m, lang) {
        (0, Lang::Ko) => "방금".to_owned(),
        (0, Lang::En) => "just now".to_owned(),
        (1..=59, Lang::Ko) => format!("{m}분 전"),
        (1..=59, Lang::En) => format!("{m}m ago"),
        (60..=1439, Lang::Ko) => format!("{}시간 전", m / 60),
        (60..=1439, Lang::En) => format!("{}h ago", m / 60),
        (_, Lang::Ko) => format!("{}일 전", m / 1440),
        (_, Lang::En) => format!("{}d ago", m / 1440),
    }
}

/// Pad by display width: Hangul is two columns wide in a terminal.
fn pad(s: &str, width: usize) -> String {
    let w: usize = s.chars().map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 }).sum();
    format!("{s}{}", " ".repeat(width.saturating_sub(w)))
}

fn render(board: &porch_core::board::Board, now: u64, lang: Lang) -> String {
    use std::fmt::Write as _;
    let en = lang == Lang::En;
    let mut out = String::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let entries = || board.worktrees.iter().flat_map(|w| &w.entries);
    let waiting = entries().filter(|e| e.session.needs_you()).count();
    let total = entries().count();
    let _ = if en {
        writeln!(out, "Waiting on you {waiting} of {total}\n")
    } else {
        writeln!(out, "나를 기다리는 세션 {waiting} / 전체 {total}\n")
    };
    if board.worktrees.is_empty() {
        let _ = writeln!(out, "{}", if en { "No live sessions." } else { "떠 있는 세션이 없습니다." });
    }
    for w in &board.worktrees {
        let mut head = w.root.replacen(&home, "~", 1);
        if let Some(b) = &w.branch {
            head += &format!("  {b}");
        }
        if let Some(d) = w.dirty.filter(|&d| d > 0) {
            head += &if en { format!("  {d} changed file{}", if d == 1 { "" } else { "s" }) } else { format!("  커밋 안 한 변경 {d}") };
        }
        let _ = writeln!(out, "{head}");
        for e in &w.entries {
            let s = &e.session;
            let mut when = ago(now, s.last_event_at, lang);
            if e.stale {
                when = if en { format!("last heard {when}") } else { format!("마지막 소식 {when}") };
            }
            let n = s.subagents.len();
            let sub = match (n, en) {
                (0, _) => String::new(),
                (n, true) => format!("  {n} subagent{}", if n == 1 { "" } else { "s" }),
                (n, false) => format!("  하위 {n}"),
            };
            let _ = writeln!(
                out,
                "  {} {} {:6} {}{}  [{}]",
                pad(&state_label(s, lang), 30),
                pad(&e.title, 28),
                s.agent,
                when,
                sub,
                &s.session[..s.session.len().min(8)]
            );
        }
    }
    let c = &board.counters;
    if c.bad_lines + c.orphan_events + c.unknown_events.len() > 0 {
        let _ = if en {
            writeln!(
                out,
                "\nNote: unreadable lines {}, events without a session {}, unknown events {:?}",
                c.bad_lines, c.orphan_events, c.unknown_events
            )
        } else {
            writeln!(
                out,
                "\n참고: 읽지 못한 줄 {}, 세션 없는 사건 {}, 모르는 사건 {:?}",
                c.bad_lines, c.orphan_events, c.unknown_events
            )
        };
    }
    out
}

fn cmd_now(o: &Opts) -> Result<(), String> {
    let now = time::now_ms();
    let board = porch_core::board::build(now, &porch_core::board::Sources::default());
    if o.json {
        println!("{}", serde_json::to_string_pretty(&board).map_err(|e| e.to_string())?);
    } else {
        print!("{}", render(&board, now, Lang::current()));
    }
    Ok(())
}

/// Redraw the board every `--interval` seconds (default 2) until Ctrl-C.
fn cmd_watch(o: &Opts) -> Result<(), String> {
    let secs = o.tail.unwrap_or(2).max(1) as u64;
    let lang = Lang::current();
    let note = match lang {
        Lang::Ko => format!("{secs}초마다 새로 고침, 끝내려면 Ctrl-C"),
        Lang::En => format!("refreshes every {secs}s, Ctrl-C to quit"),
    };
    loop {
        let now = time::now_ms();
        let board = porch_core::board::build(now, &porch_core::board::Sources::default());
        let clock = {
            let local = std::process::Command::new("date").arg("+%H:%M:%S").output();
            local.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default()
        };
        // home cursor, clear screen, then draw in one write to avoid flicker
        print!("\x1b[H\x1b[2J{clock}  ({note})\n\n{}", render(&board, now, lang));
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(secs));
    }
}

/// Put our state next to Orca's for every Orca pane our hook has seen.
/// Orca observes the same agents through its own hooks, so a disagreement is
/// a lead worth checking by eye; agreement is not proof.
fn cmd_compare_orca(_o: &Opts) -> Result<(), String> {
    use porch_core::state::State::*;
    let out = std::process::Command::new("orca")
        .args(["worktree", "ps", "--json"])
        .output()
        .map_err(|e| format!("could not run orca: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("orca output: {e}"))?;
    let mut orca: BTreeMap<String, (String, String)> = BTreeMap::new();
    for w in v["result"]["worktrees"].as_array().into_iter().flatten() {
        for a in w["agents"].as_array().into_iter().flatten() {
            if let Some(pane) = a["paneKey"].as_str() {
                orca.insert(
                    pane.to_owned(),
                    (a["agentType"].as_str().unwrap_or("?").to_owned(), a["state"].as_str().unwrap_or("?").to_owned()),
                );
            }
        }
    }
    let now = time::now_ms();
    let lang = Lang::current();
    let board = porch_core::board::build(now, &porch_core::board::Sources { with_git: false, ..Default::default() });
    let (mut agree, mut differ, mut ours_only) = (0, 0, 0);
    let mut matched = std::collections::HashSet::new();
    for e in board.worktrees.iter().flat_map(|w| &w.entries) {
        let s = &e.session;
        let Some(pane) = s.term_pane.as_deref().filter(|_| s.term_program.as_deref() == Some("Orca")) else { continue };
        let Some((_, ostate)) = orca.get(pane) else {
            ours_only += 1;
            println!("  ?  {} {:24} orca: (pane not listed)", pad(&state_label(s, lang), 28), e.title);
            continue;
        };
        matched.insert(pane.to_owned());
        let same = matches!(
            (s.state, ostate.as_str()),
            (Running, "working") | (TurnDone | Failed | Ended, "done" | "idle") | (Permission | Question, "waiting" | "blocked")
        );
        if same { agree += 1 } else { differ += 1 }
        println!("  {} {} {:24} orca: {ostate}", if same { "=" } else { "≠" }, pad(&state_label(s, lang), 28), e.title);
    }
    let orca_only = orca.keys().filter(|p| !matched.contains(*p)).count();
    match lang {
        Lang::Ko => println!(
            "\n일치 {agree}, 불일치 {differ}, 우리만 본 탭 {ours_only}, Orca만 본 탭 {orca_only} (훅 설치 전에 시작한 세션은 탭 식별값이 없어 Orca 쪽에만 잡힌다)"
        ),
        Lang::En => println!(
            "\nAgree {agree}, differ {differ}, tabs only we saw {ours_only}, tabs only Orca saw {orca_only} (sessions started before the hook was installed have no tab id, so only Orca sees them)"
        ),
    }
    Ok(())
}

fn local_today(o: &Opts, offset: i64) -> String {
    o.date.clone().unwrap_or_else(|| time::local_date(time::now_ms(), offset))
}

fn bullet_list(out: &mut String, title: &str, items: &[String]) {
    use std::fmt::Write as _;
    if !items.is_empty() {
        let _ = writeln!(out, "  {title}");
        for i in items {
            let _ = writeln!(out, "    - {i}");
        }
    }
}

fn hours(min: u64, lang: Lang) -> String {
    match (min / 60, min % 60, lang) {
        (0, m, Lang::Ko) => format!("{m}분"),
        (h, 0, Lang::Ko) => format!("{h}시간"),
        (h, m, Lang::Ko) => format!("{h}시간 {m}분"),
        (0, m, Lang::En) => format!("{m}m"),
        (h, 0, Lang::En) => format!("{h}h"),
        (h, m, Lang::En) => format!("{h}h {m}m"),
    }
}

fn cmd_today(o: &Opts) -> Result<(), String> {
    use std::fmt::Write as _;
    let offset = time::local_offset_secs();
    let r = summary::build_day(
        &summary::DayOpts {
            date: local_today(o, offset),
            engine: engine(o)?,
            refresh: o.refresh,
            no_llm: o.no_llm,
        },
        offset,
    )?;
    if o.json {
        println!("{}", serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?);
        return Ok(());
    }
    if o.md {
        print!("{}", porch_core::export::day_markdown(&r, r.lang_or(porch_core::lang::Lang::current())));
        return Ok(());
    }
    let lang = Lang::current();
    let en = lang == Lang::En;
    let m = &r.digest.metrics;
    let mut out = String::new();
    let _ = writeln!(out, "{}", r.date);
    if let Some(s) = &r.summary {
        let _ = writeln!(out, "{}", s.headline);
    }
    let (active, projects) = (hours(m.active_minutes, lang), r.digest.projects.len());
    let _ = if en {
        writeln!(out, "Work time {active} · {projects} projects · {} sessions · {} requests · {} commits (+{} -{})", m.sessions, m.prompts, m.commits, m.insertions, m.deletions)
    } else {
        writeln!(out, "작업 {active} · 프로젝트 {projects} · 세션 {} · 요청 {} · 커밋 {} (+{} -{})", m.sessions, m.prompts, m.commits, m.insertions, m.deletions)
    };
    if let Some(e) = &r.summary_error {
        let _ = writeln!(out, "\n{}: {e}", if en { "Summary failed" } else { "요약 실패" });
    }
    for p in &r.digest.projects {
        let _ = if en {
            writeln!(out, "\n■ {}  {} · {} commits", p.name, hours(p.metrics.active_minutes, lang), p.metrics.commits)
        } else {
            writeln!(out, "\n■ {}  {} · 커밋 {}", p.name, hours(p.metrics.active_minutes, lang), p.metrics.commits)
        };
        if let Some(ps) = r.summary.as_ref().and_then(|s| s.projects.iter().find(|x| x.name == p.name)) {
            let _ = writeln!(out, "  {}", ps.summary);
            bullet_list(&mut out, if en { "Done" } else { "끝낸 일" }, &ps.done);
            bullet_list(&mut out, if en { "Not finished" } else { "아직 못 끝낸 일" }, &ps.in_progress);
            bullet_list(&mut out, if en { "Next" } else { "다음 할 일" }, &ps.next);
        }
    }
    if let Some(s) = &r.summary {
        let _ = writeln!(out, "\n■ {}", if en { "Notes and suggestions" } else { "작업 특징·개선점" });
        bullet_list(&mut out, if en { "Notes" } else { "작업 특징" }, &s.analysis.observations);
        bullet_list(&mut out, if en { "Suggestions" } else { "개선 제안" }, &s.analysis.suggestions);
    }
    print!("{out}");
    Ok(())
}

fn cmd_statusline(o: &Opts) -> Result<(), String> {
    use porch_core::statusline;
    let bin = paths::installed_hook_bin();
    match (o.positional.first().map(String::as_str), o.positional.get(1)) {
        (Some("on"), _) => statusline::enable(&bin, !o.no_line)?,
        (Some("off"), _) => statusline::disable()?,
        (Some("wrap"), Some(root)) => statusline::wrap_project(root, &bin)?,
        (Some("unwrap"), Some(root)) => statusline::unwrap_project(root)?,
        _ => {}
    }
    println!("{:?}", statusline::status());
    for p in statusline::project_lines(time::now_ms()) {
        println!("  {} wrapped={} disconnected={} :: {}", p.root, p.wrapped, p.disconnected, p.command);
    }
    Ok(())
}

fn cmd_month(o: &Opts) -> Result<(), String> {
    let offset = time::local_offset_secs();
    let r = summary::build_month(
        &summary::MonthOpts {
            any_date: local_today(o, offset),
            engine: engine(o)?,
            refresh: o.refresh,
            no_llm: o.no_llm,
        },
        offset,
    )?;
    if o.json {
        println!("{}", serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?);
    } else {
        print!("{}", porch_core::export::month_markdown(&r, r.lang_or(porch_core::lang::Lang::current())));
    }
    Ok(())
}

fn cmd_mirror(o: &Opts) -> Result<(), String> {
    use porch_core::{mirror, settings};
    let en = Lang::current() == Lang::En;
    match o.positional.first().map(String::as_str) {
        None => {
            let st = mirror::status();
            if o.json {
                println!("{}", serde_json::to_string_pretty(&st).map_err(|e| e.to_string())?);
            } else {
                match &st.dir {
                    None => println!("{}", if en { "Off" } else { "꺼짐" }),
                    Some(d) => println!("{d}{}", st.vault.as_ref().map(|v| format!(" (Obsidian: {})", v.name)).unwrap_or_default()),
                }
                if st.vault_missing {
                    println!("{}", if en { "Vault or folder not found" } else { "볼트나 폴더를 찾을 수 없습니다" });
                }
                if let Some(e) = &st.last_error {
                    println!("{}: {e}", if en { "Last save failed" } else { "마지막 저장 실패" });
                }
                for e in &st.elsewhere {
                    if en {
                        println!("{} notes are left in the earlier folder {}", e.notes, e.dir);
                    } else {
                        println!("이전 폴더 {}에 노트 {}개가 남아 있습니다", e.dir, e.notes);
                    }
                }
            }
            Ok(())
        }
        Some("vaults") => {
            let v = mirror::read_vaults(&mirror::obsidian_registry());
            for x in &v.vaults {
                println!("{}  {}  {}", x.id, x.name, x.path.display());
            }
            if let Some(n) = v.note {
                println!("{n}");
            }
            Ok(())
        }
        Some("set") => {
            let given = o.positional.get(1).ok_or("usage: porch mirror set <dir> [--vault <id>]")?;
            let dir = std::path::absolute(given).map_err(|e| e.to_string())?.to_string_lossy().into_owned();
            let mut s = settings::load();
            s.mirror_dir = Some(dir.clone());
            s.obsidian_vault = o.vault.clone();
            settings::save(&s)?;
            let mut total = 0;
            mirror::save_all(&mut |_, t| total = t);
            if en {
                println!("Saving to {dir} ({total} summaries)");
            } else {
                println!("{dir}에 저장합니다 (요약 {total}개)");
            }
            Ok(())
        }
        Some("off") => {
            let mut s = settings::load();
            s.mirror_dir = None;
            s.obsidian_vault = None;
            settings::save(&s)?;
            mirror::clear_last_error(&mirror::state_path());
            println!("{}", if en { "Turned off" } else { "껐습니다" });
            Ok(())
        }
        Some(other) => Err(format!("unknown mirror command {other}")),
    }
}

fn cmd_insight(o: &Opts) -> Result<(), String> {
    use porch_core::insight;
    use std::fmt::Write as _;
    let offset = time::local_offset_secs();
    let lang = Lang::current();
    // --refresh: write the evaluation first; the numbers still print when it fails.
    let failed = if o.refresh {
        let s = porch_core::settings::load();
        let opts = porch_core::insight_eval::EvalOpts { any_date: local_today(o, offset), engine: engine(o)?, on: s.insight_eval };
        porch_core::insight_eval::evaluate(&opts, offset, time::now_ms()).err()
    } else {
        None
    };
    let v = insight::view(&local_today(o, offset), offset, time::now_ms(), lang)?;
    if o.json {
        println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
        return failed.map_or(Ok(()), Err);
    }
    let ob = &v.observed;
    let mut out = String::new();
    let until = if ob.through < ob.week_end {
        let through = time::short_date(&time::local_date(ob.through, offset), lang);
        let hour = time::local_hour(ob.through, offset);
        match lang {
            Lang::Ko => format!(" · {through} {hour}시까지"),
            Lang::En => format!(" · through {through} {hour}h"),
        }
    } else {
        String::new()
    };
    let _ = match lang {
        Lang::Ko => writeln!(out, "{} 주 · 요청 {} · 세션 {}{until}", ob.week_start, ob.requests, ob.sessions),
        Lang::En => writeln!(
            out,
            "Week of {} · {} request{} · {} session{}{until}",
            time::short_date(&ob.week_start, lang),
            ob.requests,
            if ob.requests == 1 { "" } else { "s" },
            ob.sessions,
            if ob.sessions == 1 { "" } else { "s" }
        ),
    };
    let total = ob.running_ms() + ob.waiting_ms();
    let share = (ob.waiting_ms() * 100).checked_div(total).unwrap_or(0);
    let _ = match lang {
        Lang::Ko => writeln!(
            out,
            "실행 {} · 멈춰 있던 시간 {}({share}%) · 오류로 멈춤 {}회 · 도구 오류 {}/{}",
            insight::hm(ob.running_ms(), lang),
            insight::hm(ob.waiting_ms(), lang),
            ob.stop_failures(),
            ob.tool_error_total(),
            ob.tool_calls()
        ),
        Lang::En => writeln!(
            out,
            "Working time {} · Idle time {} ({share}%) · Stopped on error {} · Tool errors {}/{}",
            insight::hm(ob.running_ms(), lang),
            insight::hm(ob.waiting_ms(), lang),
            ob.stop_failures(),
            ob.tool_error_total(),
            ob.tool_calls()
        ),
    };
    for d in &ob.days {
        let _ = match lang {
            Lang::Ko => writeln!(
                out,
                "  {}  실행 중 {:>6} · 차례 끝남·질문 대기 {:>6} · 오류로 멈춤 {:>6} · 권한 대기 {:>6}",
                d.date,
                insight::hm(d.running_ms, lang),
                insight::hm(d.turn_done_ms + d.question_ms, lang),
                insight::hm(d.failed_ms, lang),
                insight::hm(d.permission_ms, lang)
            ),
            Lang::En => writeln!(
                out,
                "  {:>6}  Working {:>6} · Done + Has a question {:>6} · Stopped on error {:>6} · Needs permission {:>6}",
                time::short_date(&d.date, lang),
                insight::hm(d.running_ms, lang),
                insight::hm(d.turn_done_ms + d.question_ms, lang),
                insight::hm(d.failed_ms, lang),
                insight::hm(d.permission_ms, lang)
            ),
        };
    }
    if !v.checkpoints.is_empty() {
        let _ = writeln!(out, "\n{}", match lang { Lang::Ko => "체크포인트", Lang::En => "Checkpoints" });
        for c in &v.checkpoints {
            let _ = writeln!(out, "  {} · {}", c.title, c.details.join(" · "));
        }
    }
    if let Some(from) = &ob.recorded_from {
        let _ = writeln!(out, "\n{}", match lang {
            Lang::Ko => format!("기록은 {}부터 있습니다", time::short_date(from, lang)),
            Lang::En => format!("Records start on {}", time::short_date(from, lang)),
        });
    }
    let _ = writeln!(
        out,
        "\n{}",
        match lang {
            Lang::Ko => "멈춰 있던 시간에는 권한 대기·질문 대기·오류로 멈춤·차례 끝남이 포함됩니다. 멈춰 있던 시간에 들어가는 네 상태(권한 대기·질문 대기·오류로 멈춤·차례 끝남)는 한 번에 최대 10분까지 셉니다. 세션별 합계이므로 겹치는 시간도 중복 집계합니다.".to_owned(),
            Lang::En => "Idle time includes time in Needs permission, Has a question, Stopped on error and Done. Each stretch in one of these states counts for up to 10 minutes. Time is added up separately for each session, so overlapping time counts more than once.".to_owned(),
        }
    );
    out.push_str(&insight_eval_text(&v, lang));
    print!("{out}");
    failed.map_or(Ok(()), Err)
}

/// The goal and evaluation part of `porch insight`.
fn insight_eval_text(v: &porch_core::insight::InsightView, lang: Lang) -> String {
    use porch_core::{goals, insight_eval};
    use std::fmt::Write as _;
    let en = lang == Lang::En;
    let mut out = String::new();
    // The goal's name and unit are re-derived in the CLI's language; the stored ones are the fallback.
    let label_unit = |g: &goals::Goal| goals::describe(&g.key, lang).unwrap_or_else(|| (g.label.clone(), g.unit.clone()));
    let value = |g: &goals::Goal, n: u64| goals::format_value(&g.key, n, &label_unit(g).1, lang);
    if let Some(l) = &v.last_goal {
        let (g, r) = (&l.goal, &l.result);
        let line = match (r.after, r.so_far) {
            (Some(a), _) => format!("{} → {}", value(g, r.before), value(g, a)),
            (None, Some(n)) if en => format!("in progress, {} so far", value(g, n)),
            (None, Some(n)) => format!("주가 아직 끝나지 않음 · 지금까지 {}", value(g, n)),
            (None, None) => {
                let why = match (r.reason, en) {
                    (Some("in_progress"), false) => "주가 아직 끝나지 않음",
                    (Some("in_progress"), true) => "the week hasn't ended",
                    (Some("few_requests"), false) => "요청이 20개 미만",
                    (Some("few_requests"), true) => "fewer than 20 requests",
                    (Some("no_evaluation"), false) => "이 주의 평가가 없음",
                    (Some("no_evaluation"), true) => "no evaluation for this week",
                    (_, false) => "두 기간의 길이가 다름",
                    (_, true) => "the two periods differ in length",
                };
                format!("{} · {}", if en { "Can't compare" } else { "비교할 수 없음" }, why)
            }
        };
        let _ = writeln!(out, "\n{} {} · {line}", if en { "Last week's goal:" } else { "지난주 목표:" }, label_unit(g).0);
    }
    if v.eval_on {
        let _ = writeln!(out, "\n{}", if en { "Evaluation (written by the model)" } else { "평가 (모델이 씀)" });
        match &v.evaluation {
            _ if !v.finished => {
                let _ = writeln!(out, "  {}", if en { "Written after the week ends" } else { "주가 끝난 뒤에 평가합니다" });
            }
            None => {
                let why = match (v.skipped.as_deref(), en) {
                    (Some("off"), false) => "이 주가 끝났을 때 평가가 꺼져 있었습니다",
                    (Some("off"), true) => "Evaluation was off when this week ended",
                    (Some("few_requests"), false) => "이 주는 평가하기에 요청이 적습니다",
                    (Some("few_requests"), true) => "Too few requests this week to evaluate",
                    (Some("no_day_summaries"), false) => "하루 요약이 있는 날이 없어 평가하지 않았습니다",
                    (Some("no_day_summaries"), true) => "Not evaluated: no day this week has a daily summary",
                    (_, false) => "아직 평가하지 않았습니다 · porch insight --refresh",
                    (_, true) => "Not evaluated yet · porch insight --refresh",
                };
                let _ = writeln!(out, "  {why}");
            }
            Some(e) => {
                for c in insight_eval::rubric(lang) {
                    match e.scores.iter().find(|s| s.item == c.item) {
                        Some(s) => {
                            let _ = writeln!(out, "  {} {}/5 · {}", c.name, s.score, s.why);
                            if !s.quote.is_empty() {
                                let _ = writeln!(out, "    \"{}\" ({})", s.quote, s.turn);
                            }
                        }
                        None => {
                            let _ = writeln!(out, "  {} · {}", c.name, if en { "No evidence to judge" } else { "판단할 근거 없음" });
                        }
                    }
                }
                for p in &e.pivots {
                    let moved = if p.pivot.before.is_empty() && p.pivot.after.is_empty() { String::new() } else { format!("{} → {} · ", p.pivot.before, p.pivot.after) };
                    let _ = writeln!(out, "  {} {moved}\"{}\"", time::short_date(&p.date, lang), p.pivot.quote);
                }
                for w in &e.rewrites {
                    let _ = writeln!(out, "  {} \"{}\"\n    → {}", time::short_date(&w.date, lang), w.quote, w.rewritten);
                }
            }
        }
        if let Some(err) = &v.error {
            let _ = writeln!(out, "  {} {err}", if en { "Couldn't make the evaluation:" } else { "평가를 만들지 못했습니다:" });
        }
    }
    if v.finished {
        let _ = writeln!(out, "\n{}", if en { "Next week's goal" } else { "다음 주 목표" });
        match &v.goal {
            Some(g) => {
                let _ = writeln!(out, "  {} · {}", label_unit(g).0, value(g, g.baseline));
            }
            None => {
                for c in &v.candidates {
                    let _ = writeln!(out, "  - {} ({})", c.label, goals::format_value(&c.key, c.value, &c.unit, lang));
                }
            }
        }
    }
    out
}

fn cmd_week(o: &Opts) -> Result<(), String> {
    use std::fmt::Write as _;
    let offset = time::local_offset_secs();
    let r = summary::build_week(
        &summary::WeekOpts {
            any_date: local_today(o, offset),
            engine: engine(o)?,
            refresh: o.refresh,
            no_llm: o.no_llm,
        },
        offset,
    )?;
    if o.json {
        println!("{}", serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?);
        return Ok(());
    }
    if o.md {
        print!("{}", porch_core::export::week_markdown(&r, r.lang_or(porch_core::lang::Lang::current())));
        return Ok(());
    }
    let lang = Lang::current();
    let en = lang == Lang::En;
    let mut out = String::new();
    let _ = if en { writeln!(out, "Week of {}", r.week_start) } else { writeln!(out, "{} 주", r.week_start) };
    if let Some(s) = &r.summary {
        let _ = writeln!(out, "{}", s.headline);
    }
    for d in &r.days {
        let _ = if en {
            writeln!(out, "  {}  {} · {} commits", d.date, hours(d.minutes, lang), d.commits)
        } else {
            writeln!(out, "  {}  {} · 커밋 {}", d.date, hours(d.minutes, lang), d.commits)
        };
    }
    if let Some(e) = &r.summary_error {
        let _ = writeln!(out, "\n{}: {e}", if en { "Summary failed" } else { "요약 실패" });
    }
    if let Some(s) = &r.summary {
        for p in &s.projects {
            let _ = writeln!(out, "\n■ {}\n  {}", p.name, p.summary);
            bullet_list(&mut out, if en { "Done" } else { "끝낸 일" }, &p.shipped);
            bullet_list(&mut out, if en { "Not finished" } else { "아직 못 끝낸 일" }, &p.ongoing);
            bullet_list(&mut out, if en { "Next" } else { "다음 할 일" }, &p.next);
        }
        let _ = writeln!(out, "\n■ {}", if en { "Notes and suggestions" } else { "작업 특징·개선점" });
        bullet_list(&mut out, if en { "Notes" } else { "작업 특징" }, &s.analysis.observations);
        bullet_list(&mut out, if en { "Suggestions" } else { "개선 제안" }, &s.analysis.suggestions);
    }
    print!("{out}");
    Ok(())
}

fn cmd_digest(o: &Opts) -> Result<(), String> {
    let offset = time::local_offset_secs();
    let date = local_today(o, offset);
    let d = porch_core::digest::collect_day(
        &date,
        offset,
        &porch_core::digest::Options {
            claude_dirs: vec![paths::default_claude_dir()],
            excluded: vec![summary::runner_dir().to_string_lossy().into_owned()],
            lang: porch_core::lang::Lang::current(),
        },
    )
    .ok_or_else(|| time::bad_date(porch_core::lang::Lang::current()))?;
    print!("{}", porch_core::digest::to_prompt_text(&d, offset, porch_core::lang::Lang::current()));
    Ok(())
}

/// The writer from Settings, with `--provider` / `--model` on top.
fn engine(o: &Opts) -> Result<Engine, String> {
    let mut s = porch_core::settings::load();
    if let Some(p) = &o.provider {
        Provider::parse(p).ok_or_else(|| format!("unknown provider {p} (claude or codex)"))?;
        s.provider = p.clone();
    }
    let mut e = Engine::from_settings(&s);
    if let Some(m) = &o.model {
        e.model = Some(m.clone());
    }
    Ok(e)
}

fn cmd_doctor(_o: &Opts) -> Result<(), String> {
    let lang = Lang::current();
    let en = lang == Lang::En;
    let found = |yes: bool| match (yes, en) {
        (true, true) => "found",
        (false, true) => "missing",
        (true, false) => "있음",
        (false, false) => "없음",
    };
    let claude_dir = paths::default_claude_dir();
    println!(
        "{}: {} ({})",
        if en { "Claude Code data" } else { "클로드코드 데이터" },
        claude_dir.display(),
        found(claude_dir.join("projects").is_dir())
    );
    for p in Provider::ALL {
        match (p.bin(), en) {
            (Some(b), true) => println!("{} command: {}", p.id(), b.display()),
            (Some(b), false) => println!("{} 명령: {}", p.id(), b.display()),
            (None, true) => println!("{} command: not found ({} can't write summaries)", p.id(), p.name()),
            (None, false) => println!("{} 명령: 못 찾음 ({}로 요약을 만들 수 없습니다)", p.id(), p.name()),
        }
    }
    let e = engine(_o)?;
    println!(
        "{}: {}",
        if en { "Summary agent" } else { "요약을 쓰는 에이전트" },
        porch_core::writer::written_by(Some(e.provider), &e.model_label(), lang)
    );
    let homes = paths::codex_homes();
    if en {
        println!("Codex config folders: {}", homes.len());
    } else {
        println!("코덱스 설정 폴더: {}곳", homes.len());
    }
    for h in &homes {
        println!("  {}", h.display());
    }
    let bin = paths::installed_hook_bin();
    println!("{}: {} ({})", if en { "Hook program" } else { "훅 프로그램" }, bin.display(), found(bin.exists()));
    for t in install::status(&bin) {
        println!("  [{}] {} {}", t.agent, t.state, t.file);
    }
    Ok(())
}

/// An applied suggestion's effect, worded for its state. A comparison of equal
/// windows, never a cause.
fn effect_line(e: &porch_core::health::Effect, lang: Lang) -> String {
    use porch_core::health::EffectState;
    match (e.state, lang) {
        (EffectState::Measured, Lang::Ko) => format!("효과: 적용 전 {}일 {}건 → 적용 후 {}일 {}건 (전후 비교일 뿐 원인은 아닙니다)", e.days, e.before.items, e.days, e.after.items),
        (EffectState::Measured, Lang::En) => format!("Before/after counts: {} in the {} days before you applied the suggestion, {} in the {} days after. This comparison does not show whether the suggestion caused a change.", e.before.items, e.days, e.after.items, e.days),
        (EffectState::Early, Lang::Ko) => format!("효과: 적용 {}일째, 판단하기엔 이른 시점", e.days),
        (EffectState::Early, Lang::En) => format!("Before/after counts: you applied the suggestion {} days ago. Not enough time has passed to compare.", e.days),
        (EffectState::NoRecords, Lang::Ko) => "효과: 적용한 뒤 기록이 없음".to_owned(),
        (EffectState::NoRecords, Lang::En) => "Before/after counts: no records since you applied the suggestion".to_owned(),
        (EffectState::Uncomparable, Lang::Ko) => "효과: 앞뒤 기록을 다른 방식으로 세서 비교하지 않음".to_owned(),
        (EffectState::Uncomparable, Lang::En) => "Before/after counts: unavailable because the records before and after were counted differently".to_owned(),
    }
}

const USAGE: &str = "\
usage: porch <command> [options]

  now                 sessions and which ones wait on you (--json)
  watch               the same, redrawn every --interval seconds
  today, week, month  summary (--date, --refresh, --no-llm, --json, --md)
  insight             a week's numbers, checkpoints, goals and model evaluation (--date, --refresh, --json)
  usage               tokens and cost, JSON (--tail DAYS, --date last-day)
  limits              usage limits per agent account, JSON
  suggest             suggestions for repeated blockers
  suggestions         saved suggestions and their effect (--json)
  mirror              summary notes folder (vaults | set <dir> | off)
  doctor              what this Mac has: agents, hooks, the claude and codex CLIs
  install, uninstall  agent hooks (--dry-run, --only claude|codex)
  status              whether our hooks are installed
  statusline          Claude limits through the status line (on | off)
  digest, events, keys, compare-orca, classify-blockers: debugging

Summaries use the agent picked in the app, or --provider claude|codex.
";

fn main() -> ExitCode {
    paths::migrate_legacy_data_dir();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    if matches!(cmd.as_str(), "help" | "-h" | "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let result = parse(rest).and_then(|o| match cmd.as_str() {
        "install" => cmd_install(&o),
        "uninstall" => cmd_uninstall(&o),
        "status" => cmd_status(&o),
        "events" => cmd_events(&o),
        "keys" => cmd_keys(&o),
        "now" => cmd_now(&o),
        "watch" => cmd_watch(&o),
        "compare-orca" => cmd_compare_orca(&o),
        "today" => cmd_today(&o),
        "month" => cmd_month(&o),
        "statusline" => cmd_statusline(&o),
        "usage" => {
            let days = o.tail.unwrap_or(7) as u64;
            println!("{}", serde_json::to_string_pretty(&summary::usage_report(days, o.date.as_deref(), time::local_offset_secs())?).map_err(|e| e.to_string())?);
            Ok(())
        }
        "limits" => {
            println!("{}", serde_json::to_string_pretty(&porch_core::limits::all(time::now_ms())).map_err(|e| e.to_string())?);
            Ok(())
        }
        "week" => cmd_week(&o),
        "insight" => cmd_insight(&o),
        "mirror" => cmd_mirror(&o),
        "digest" => cmd_digest(&o),
        "classify-blockers" => {
            let e = engine(&o)?;
            match e.lang {
                Lang::Ko => eprintln!("{}로 예전 막힘에 유형을 붙이는 중…", e.provider.name()),
                Lang::En => eprintln!("Classifying older blockers with {}…", e.provider.name()),
            }
            let n = summary::classify_old_blockers(&e)?;
            match e.lang {
                Lang::Ko => println!("막힘 {n}건에 유형을 붙였습니다"),
                Lang::En => println!("Classified {n} blockers"),
            }
            Ok(())
        }
        "suggest" => {
            let e = engine(&o)?;
            match e.lang {
                Lang::Ko => eprintln!("{}로 제안을 만드는 중…", e.provider.name()),
                Lang::En => eprintln!("Writing suggestions with {}…", e.provider.name()),
            }
            let r = porch_core::suggest::make(&e, time::local_offset_secs())?;
            match e.lang {
                Lang::Ko => println!("제안 {}개 (근거가 맞지 않아 버린 것 {}개)", r.added, r.dropped),
                Lang::En => println!("{} suggestions ({} left out because their evidence did not match the records)", r.added, r.dropped),
            }
            Ok(())
        }
        "suggestions" => {
            let list = porch_core::suggest::list(time::local_offset_secs())?;
            if o.json {
                println!("{}", serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?);
            } else {
                for v in &list {
                    let s = &v.suggestion;
                    println!("{} [{:?}] {} · {} · {}\n  {}\n  {}", s.id, s.status, s.scope, s.title, v.target_label, s.why, s.text);
                    if let Some(e) = v.effect {
                        println!("  {}", effect_line(&e, Lang::current()));
                    }
                }
            }
            Ok(())
        }
        "doctor" => cmd_doctor(&o),
        other => Err(format!("unknown command {other}")),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use porch_core::lang::Lang;

    #[test]
    fn durations_in_both_languages() {
        assert_eq!(hours(125, Lang::Ko), "2시간 5분");
        assert_eq!(hours(125, Lang::En), "2h 5m");
        assert_eq!(hours(45, Lang::En), "45m");
    }

    #[test]
    fn time_since_in_both_languages() {
        let now = 3 * 86_400_000;
        assert_eq!(ago(now, now, Lang::En), "just now");
        assert_eq!(ago(now, now - 5 * 60_000, Lang::Ko), "5분 전");
        assert_eq!(ago(now, now - 5 * 60_000, Lang::En), "5m ago");
        assert_eq!(ago(now, now - 2 * 3_600_000, Lang::En), "2h ago");
        assert_eq!(ago(now, 0, Lang::En), "3d ago");
    }
}
