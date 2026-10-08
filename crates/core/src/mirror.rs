//! Summaries mirrored as Markdown notes into a folder the person chose,
//! usually inside an Obsidian vault. The data dir stays the source of truth;
//! a note is a copy, and the folder is porch's: every save writes the note anew,
//! edits included (people keep their own notes elsewhere and link to these).
//! See docs/decisions/0008-mirror-to-a-chosen-folder.md and 0009-porch-notes-are-rewritten.md.
use crate::lang::Lang;

use crate::summary::{MonthReport, Report, WeekReport};
use crate::{export, paths, settings, summary, time};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum Outcome {
    Written(PathBuf),
    /// Mirroring is off.
    Off,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// Before folders were told apart: note path -> hash, for whichever folder
    /// was in use. Moved under `roots` by the next write.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
    /// Mirror folder -> note path relative to it -> hash of what porch last wrote there
    /// (the notes porch wrote per folder; counted for "notes left in a previous folder").
    #[serde(default)]
    pub roots: BTreeMap<String, BTreeMap<String, String>>,
    /// "day:2026-10-02" -> "written"; the latest outcome per report.
    #[serde(default)]
    pub last: BTreeMap<String, String>,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Unix ms of the last note porch wrote.
    #[serde(default)]
    pub last_saved: Option<u64>,
}

pub fn state_path() -> PathBuf {
    paths::data_dir().join("mirror.json")
}

/// 64-bit FNV-1a, enough to tell whether a note changed since porch wrote it.
pub fn fnv64(s: &str) -> String {
    fnv64_bytes(s.as_bytes())
}

fn fnv64_bytes(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Read, change and save the state under one lock shared by the app and the CLI.
/// A state file that does not parse is treated as empty (every existing note then
/// counts as edited, so nothing is overwritten).
pub fn with_state<T>(path: &Path, f: impl FnOnce(&mut State) -> T) -> Result<T, String> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock")).map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let mut st: State = fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let out = f(&mut st);
    summary::save_json(path, &serde_json::to_string_pretty(&st).map_err(|e| e.to_string())?)?;
    Ok(out)
}

/// Create the mirror root, but never when its parent is missing
/// (a moved vault or an unplugged disk must not grow a stray folder).
pub fn ensure_root(root: &Path) -> Result<(), String> {
    if !root.exists() && !root.parent().is_some_and(Path::exists) {
        return Err(match Lang::current() {
            Lang::Ko => format!("폴더를 찾을 수 없습니다: {}", root.display()),
            Lang::En => format!("Folder not found: {}", root.display()),
        });
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())
}

/// How a mirror folder is named in the state file: resolved, so `/tmp/x` and
/// `/private/tmp/x` (or a vault reached through a link) are one folder.
fn root_key(root: &Path) -> String {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()).to_string_lossy().into_owned()
}

/// Whether `rel` under `root` passes through a symbolic link (which could lead
/// the write out of the folder the person chose).
fn through_link(root: &Path, rel: &str) -> bool {
    let mut p = root.to_path_buf();
    Path::new(rel).components().any(|c| {
        p.push(c);
        fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink())
    })
}

/// Write `text` to `root/rel`, replacing whatever is there.
pub fn write_note(state: &Path, root: &Path, rel: &str, text: &str) -> Result<Outcome, String> {
    ensure_root(root)?;
    // Subfolders are checked when settings are saved; a hand-edited settings file is checked here.
    if !Path::new(rel).components().all(|c| matches!(c, std::path::Component::Normal(_))) {
        return Err(match Lang::current() {
            Lang::Ko => format!("폴더 밖을 가리키는 경로라 쓰지 않았습니다: {rel}"),
            Lang::En => format!("Not written: the path points outside the folder: {rel}"),
        });
    }
    if through_link(root, rel) {
        return Err(match Lang::current() {
            Lang::Ko => format!("폴더 밖을 가리키는 링크가 있어 쓰지 않았습니다: {rel}"),
            Lang::En => format!("Not written: a link points outside the folder: {rel}"),
        });
    }
    let path = root.join(rel);
    let key = root_key(root);
    with_state(state, |st| -> Result<Outcome, String> {
        if !st.files.is_empty() {
            let legacy = std::mem::take(&mut st.files);
            st.roots.entry(key.clone()).or_default().extend(legacy);
        }
        if let Some(d) = path.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("md.porch-tmp");
        fs::write(&tmp, text).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        st.roots.entry(key).or_default().insert(rel.to_owned(), fnv64(text));
        Ok(Outcome::Written(path.clone()))
    })?
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Daily,
    Weekly,
    Monthly,
}

/// The note's file stem: a date, an ISO week, or a month.
pub fn file_key(kind: Kind, key: &str) -> Option<String> {
    match kind {
        Kind::Daily | Kind::Monthly => Some(key.to_owned()),
        Kind::Weekly => time::iso_week(key).map(|(y, w)| format!("{y}-W{w:02}")),
    }
}

/// A YAML double-quoted scalar on one line.
fn yaml_str(s: &str) -> String {
    let one_line = s.split(['\n', '\r']).map(str::trim).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
    format!("\"{}\"", one_line.replace('\\', "\\\\").replace('"', "\\\""))
}

struct Front<'a> {
    kind: &'a str,
    date: &'a str,
    headline: &'a str,
    projects: Vec<String>,
    active_minutes: u64,
    cost: Option<f64>,
    generated_at: u64,
}

fn frontmatter(f: &Front, lang: Lang) -> String {
    let mut o = String::from("---\nporch_format: 1\n");
    let _ = writeln!(o, "type: {}\ndate: {}\nheadline: {}", f.kind, f.date, yaml_str(f.headline));
    let _ = writeln!(o, "projects: [{}]", f.projects.iter().map(|p| yaml_str(p)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(o, "active_minutes: {}", f.active_minutes);
    if let Some(c) = f.cost {
        let _ = writeln!(o, "cost_usd: {c:.2}");
    }
    let _ = writeln!(o, "generated: {}", time::utc_rfc3339(f.generated_at));
    o.push_str("tags: [porch]\n---\n\n");
    o.push_str(match lang {
        Lang::Ko => "> [!note] porch가 요약을 만들 때마다 다시 쓰는 노트입니다. 메모는 다른 노트에 쓰고 이 노트를 링크하세요.\n\n",
        Lang::En => "> [!note] porch overwrites this note whenever the summary is saved or regenerated, replacing any edits made here. Keep your own notes in a separate note and link to this one.\n\n",
    });
    o
}

fn links(title: &str, keys: &[String]) -> String {
    if keys.is_empty() {
        return String::new();
    }
    format!("\n## {title}\n\n{}\n", keys.iter().map(|k| format!("[[{k}]]")).collect::<Vec<_>>().join(" · "))
}

pub fn day_note(r: &Report, show_cost: bool, lang: Lang) -> String {
    let m = &r.digest.metrics;
    let f = Front {
        kind: "daily",
        date: &r.date,
        headline: r.summary.as_ref().map_or("", |s| s.headline.as_str()),
        projects: r.digest.projects.iter().map(|p| p.name.clone()).collect(),
        active_minutes: m.active_minutes,
        cost: m.usage.as_ref().map(|u| u.cost).filter(|_| show_cost),
        generated_at: r.generated_at,
    };
    frontmatter(&f, lang) + &export::day_markdown(r, lang)
}

fn lines_front<'a>(kind: &'a str, date: &'a str, headline: &'a str, days: &[crate::summary::DayLine], show_cost: bool, generated_at: u64) -> Front<'a> {
    let projects: std::collections::BTreeSet<String> = days.iter().flat_map(|d| d.projects.keys().cloned()).collect();
    let cost: Option<f64> = days.iter().filter_map(|d| d.cost).reduce(|a, b| a + b);
    Front { kind, date, headline, projects: projects.into_iter().collect(), active_minutes: days.iter().map(|d| d.minutes).sum(), cost: cost.filter(|_| show_cost), generated_at }
}

pub fn week_note(r: &WeekReport, show_cost: bool, lang: Lang) -> String {
    let headline = r.summary.as_ref().map_or("", |s| s.headline.as_str());
    let f = lines_front("weekly", &r.week_start, headline, &r.days, show_cost, r.generated_at);
    let days: Vec<String> = r.days.iter().filter(|d| d.headline.is_some()).map(|d| d.date.clone()).collect();
    let title = match lang {
        Lang::Ko => "일별 기록",
        Lang::En => "Daily notes",
    };
    frontmatter(&f, lang) + &export::week_markdown(r, lang) + &links(title, &days)
}

pub fn month_note(r: &MonthReport, show_cost: bool, weeks: &[String], lang: Lang) -> String {
    let headline = r.summary.as_ref().map_or("", |s| s.headline.as_str());
    let f = lines_front("monthly", &r.month, headline, &r.days, show_cost, r.generated_at);
    let title = match lang {
        Lang::Ko => "주별 기록",
        Lang::En => "Weekly notes",
    };
    frontmatter(&f, lang) + &export::month_markdown(r, lang) + &links(title, weeks)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Vault {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct Vaults {
    pub vaults: Vec<Vault>,
    /// Why the list is empty, when it is.
    pub note: Option<String>,
}

pub fn obsidian_registry() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Application Support/obsidian/obsidian.json")
}

/// Obsidian's own vault list. Its format is undocumented: anything unexpected
/// gives an empty list with the reason, never a guess.
pub fn read_vaults(registry: &Path) -> Vaults {
    let empty = |why: &str| Vaults { vaults: vec![], note: Some(why.to_owned()) };
    let en = Lang::current() == Lang::En;
    let Ok(text) = fs::read_to_string(registry) else { return empty(if en { "No Obsidian vault list found" } else { "Obsidian 볼트 목록이 없습니다" }) };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return empty(if en { "Could not read the Obsidian vault list" } else { "Obsidian 볼트 목록을 읽지 못했습니다" });
    };
    let Some(map) = v.get("vaults").and_then(|x| x.as_object()) else {
        return empty(if en { "The Obsidian vault list is not in the expected format" } else { "Obsidian 볼트 목록의 형식이 예상과 다릅니다" });
    };
    let mut vaults: Vec<Vault> = map
        .iter()
        .filter_map(|(id, e)| {
            let path = PathBuf::from(e.get("path")?.as_str()?);
            let name = path.file_name()?.to_string_lossy().into_owned();
            path.is_dir().then(|| Vault { id: id.clone(), name, path })
        })
        .collect();
    vaults.sort_by(|a, b| a.name.cmp(&b.name));
    let note = vaults.is_empty().then(|| if en { "No Obsidian vault found" } else { "Obsidian 볼트를 찾지 못했습니다" }.to_owned());
    Vaults { vaults, note }
}

fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `obsidian://open?vault=<id>`: the vault itself.
pub fn vault_uri(vault_id: &str) -> String {
    format!("obsidian://open?vault={}", pct(vault_id))
}

/// `obsidian://open?vault=<id>&file=<note path in the vault, no .md>`.
pub fn obsidian_uri(vault_id: &str, vault_path: &Path, note: &Path) -> Option<String> {
    let rel = note.strip_prefix(vault_path).ok()?.with_extension("");
    Some(format!("obsidian://open?vault={}&file={}", pct(vault_id), pct(&rel.to_string_lossy())))
}

fn record(state: &Path, period_key: &str, result: Result<Outcome, String>) {
    let _ = with_state(state, |st| match result {
        Ok(Outcome::Written(_)) => {
            st.last.insert(period_key.to_owned(), "written".into());
            st.last_error = None;
            st.last_saved = Some(time::now_ms());
        }
        Ok(Outcome::Off) => {}
        Err(e) => st.last_error = Some(e),
    });
}

/// Notes porch wrote that are still in the folder.
pub fn count_notes(st: &State, root: &Path) -> usize {
    let files = st.roots.get(&root_key(root)).unwrap_or(&st.files);
    files.keys().filter(|rel| root.join(rel).is_file()).count()
}

/// Folders other than `current` that still hold notes porch wrote, with how many.
pub fn elsewhere(st: &State, current: &Path) -> Vec<(String, usize)> {
    st.roots
        .keys()
        .filter(|r| **r != root_key(current))
        .map(|r| (r.clone(), count_notes(st, Path::new(r))))
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// Where today's day, week and month notes go, relative to the mirror folder.
pub fn samples(subs: [&str; 3], today: &str) -> Vec<String> {
    let week = summary::week_start(today, 0).and_then(|m| file_key(Kind::Weekly, &m)).unwrap_or_default();
    let month = today.get(..7).unwrap_or_default();
    vec![rel(subs[0], today), rel(subs[1], &week), rel(subs[2], month)]
}

/// The vault a folder lies in, if any.
pub fn vault_for(dir: &Path, vaults: &[Vault]) -> Option<Vault> {
    vaults.iter().find(|v| dir.starts_with(&v.path)).cloned()
}

fn rel(sub: &str, stem: &str) -> String {
    if sub.is_empty() { format!("{stem}.md") } else { format!("{}/{stem}.md", sub.trim_end_matches('/')) }
}

pub fn save_day_into(state: &Path, root: &Path, sub: &str, r: &Report, show_cost: bool) {
    if r.summary.is_none() || r.summary_error.is_some() {
        return;
    }
    record(state, &format!("day:{}", r.date), write_note(state, root, &rel(sub, &r.date), &day_note(r, show_cost, r.lang_or(Lang::current()))));
}

pub fn save_week_into(state: &Path, root: &Path, sub: &str, r: &WeekReport, show_cost: bool) {
    let Some(stem) = file_key(Kind::Weekly, &r.week_start) else { return };
    if r.summary.is_none() || r.summary_error.is_some() {
        return;
    }
    record(state, &format!("week:{}", r.week_start), write_note(state, root, &rel(sub, &stem), &week_note(r, show_cost, r.lang_or(Lang::current()))));
}

pub fn save_month_into(state: &Path, root: &Path, sub: &str, r: &MonthReport, show_cost: bool, weeks: &[String]) {
    if r.summary.is_none() || r.summary_error.is_some() {
        return;
    }
    record(state, &format!("month:{}", r.month), write_note(state, root, &rel(sub, &r.month), &month_note(r, show_cost, weeks, r.lang_or(Lang::current()))));
}

/// ISO keys of the saved week summaries whose Monday falls in `month`.
fn weeks_in(month: &str) -> Vec<String> {
    (0..6)
        .filter_map(|i| time::add_days(&format!("{month}-01"), i * 7))
        .filter_map(|d| summary::week_start(&d, 0))
        .filter(|m| m.starts_with(month))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|m| summary::load_week(m).is_some_and(|w| w.summary.is_some()))
        .filter_map(|m| file_key(Kind::Weekly, &m))
        .collect()
}

pub fn save_day(r: &Report) {
    let s = settings::load();
    if let Some(dir) = &s.mirror_dir {
        save_day_into(&state_path(), Path::new(dir), &s.mirror_daily, r, s.show_cost);
    }
}

pub fn save_week(r: &WeekReport) {
    let s = settings::load();
    if let Some(dir) = &s.mirror_dir {
        save_week_into(&state_path(), Path::new(dir), &s.mirror_weekly, r, s.show_cost);
    }
}

pub fn save_month(r: &MonthReport) {
    let s = settings::load();
    if let Some(dir) = &s.mirror_dir {
        save_month_into(&state_path(), Path::new(dir), &s.mirror_monthly, r, s.show_cost, &weeks_in(&r.month));
    }
}

pub fn note_path(kind: Kind, key: &str) -> Option<PathBuf> {
    let s = settings::load();
    let sub = match kind {
        Kind::Daily => &s.mirror_daily,
        Kind::Weekly => &s.mirror_weekly,
        Kind::Monthly => &s.mirror_monthly,
    };
    Some(Path::new(s.mirror_dir.as_ref()?).join(rel(sub, &file_key(kind, key)?)))
}

/// The link a clicked notification opens: the Obsidian note only when the
/// person chose that and the vault is still there; otherwise None (porch).
pub fn click_uri(notify_open: &str, uri: Option<String>) -> Option<String> {
    uri.filter(|_| notify_open == "obsidian")
}

pub fn open_uri(kind: Kind, key: &str) -> Option<String> {
    let id = settings::load().obsidian_vault?;
    let vault = read_vaults(&obsidian_registry()).vaults.into_iter().find(|v| v.id == id)?;
    obsidian_uri(&id, &vault.path, &note_path(kind, key)?)
}

/// Forget the last failure, e.g. when the folder is disconnected.
pub fn clear_last_error(state: &Path) {
    let _ = with_state(state, |st| st.last_error = None);
}

/// Days, weeks (their Mondays) and months ("YYYY-MM"), by key.
#[derive(Debug, PartialEq)]
pub struct Periods {
    pub days: Vec<String>,
    /// Mondays, this week first.
    pub weeks: Vec<String>,
    /// "YYYY-MM", this month first.
    pub months: Vec<String>,
}

/// Every summary saved in `reports`, oldest first.
pub fn saved_periods(reports: &Path) -> Periods {
    let mut p = Periods { days: vec![], weeks: vec![], months: vec![] };
    for e in fs::read_dir(reports).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".json") else { continue };
        if let Some(m) = stem.strip_prefix("week-") {
            p.weeks.push(m.to_owned());
        } else if let Some(m) = stem.strip_prefix("month-") {
            p.months.push(m.to_owned());
        } else if stem.len() == 10 && time::add_days(stem, 0).is_some() {
            p.days.push(stem.to_owned());
        }
    }
    p.days.sort();
    p.weeks.sort();
    p.months.sort();
    p
}

/// Mirror every saved summary into the folder (a newly chosen one starts
/// complete). `progress(done, total)` follows along; edited notes are kept.
pub fn save_all(progress: &mut dyn FnMut(usize, usize)) {
    let p = saved_periods(&summary::reports_dir());
    let total = p.days.len() + p.weeks.len() + p.months.len();
    let mut done = 0;
    let mut step = |progress: &mut dyn FnMut(usize, usize)| {
        done += 1;
        progress(done, total);
    };
    progress(0, total);
    for d in &p.days {
        if let Some(r) = summary::load_day(d) {
            save_day(&r);
        }
        step(progress);
    }
    for w in &p.weeks {
        if let Some(r) = summary::load_week(w) {
            save_week(&r);
        }
        step(progress);
    }
    for m in &p.months {
        if let Some(r) = summary::load_month(m) {
            save_month(&r);
        }
        step(progress);
    }
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub dir: Option<String>,
    pub vault: Option<Vault>,
    /// A vault was chosen but Obsidian no longer lists it, or the folder's parent is gone.
    pub vault_missing: bool,
    pub last_error: Option<String>,
    /// Notes porch wrote that are still in the folder.
    pub notes: usize,
    /// Unix ms of the last note written.
    pub last_saved: Option<u64>,
    /// Where today's day, week and month notes go, relative to the vault (or the folder).
    pub samples: Vec<String>,
    /// Folders used before that still hold notes porch wrote.
    pub elsewhere: Vec<Elsewhere>,
}

#[derive(Debug, Serialize)]
pub struct Elsewhere {
    pub dir: String,
    pub notes: usize,
}

pub fn status() -> Status {
    let s = settings::load();
    let vault = s.obsidian_vault.as_ref().and_then(|id| read_vaults(&obsidian_registry()).vaults.into_iter().find(|v| &v.id == id));
    let parent_gone = s.mirror_dir.as_ref().is_some_and(|d| !Path::new(d).exists() && !Path::new(d).parent().is_some_and(Path::exists));
    let st: State = fs::read_to_string(state_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let notes = s.mirror_dir.as_ref().map_or(0, |d| count_notes(&st, Path::new(d)));
    let today = time::local_date(time::now_ms(), time::local_offset_secs());
    // Shown inside the vault, the folder's own name leads: "porch/daily/…".
    let prefix = match (&vault, &s.mirror_dir) {
        (Some(v), Some(d)) => Path::new(d).strip_prefix(&v.path).ok().map(|p| p.to_string_lossy().into_owned()).filter(|p| !p.is_empty()),
        _ => None,
    };
    let samples = samples([&s.mirror_daily, &s.mirror_weekly, &s.mirror_monthly], &today)
        .into_iter()
        .map(|p| prefix.as_ref().map_or(p.clone(), |pre| format!("{pre}/{p}")))
        .collect();
    let others = s.mirror_dir.as_ref().map_or(vec![], |d| elsewhere(&st, Path::new(d)).into_iter().map(|(dir, notes)| Elsewhere { dir, notes }).collect());
    Status {
        vault_missing: (s.obsidian_vault.is_some() && vault.is_none()) || parent_gone,
        dir: s.mirror_dir,
        vault,
        last_error: st.last_error,
        notes,
        last_saved: st.last_saved,
        samples,
        elsewhere: others,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::DayDigest;
    use crate::summary::{DayLine, MonthReport, Report, Summary, WeekReport, WeekSummary};
    use crate::lang::Lang;

    #[test]
    fn an_english_note_says_its_note_line_in_english() {
        let mut r = day("2026-10-02", "Landing done");
        if let Some(s) = r.summary.as_mut() {
            s.lang = Some("en".into());
        }
        let n = day_note(&r, true, Lang::En);
        assert!(n.contains("> [!note] porch overwrites this note whenever the summary is saved or regenerated"), "{n}");
        assert!(n.contains("# Fri, Oct 2, 2026"), "{n}");
    }

    fn day(date: &str, headline: &str) -> Report {
        Report {
            date: date.into(),
            generated_at: 0,
            model: Some("sonnet".into()),
            provider: None,
            digest: DayDigest { date: date.into(), ..Default::default() },
            summary: Some(Summary { headline: headline.into(), ..Default::default() }),
            summary_error: None,
            usual_minutes: None,
            usage_checked: false,
            turn_files_read: None,
            dropped_pivots: 0,
        }
    }

    fn front(note: &str) -> String {
        note.strip_prefix("---\n").unwrap().split_once("\n---\n").unwrap().0.to_owned()
    }

    #[test]
    fn day_note_has_frontmatter_then_body() {
        let n = day_note(&day("2026-10-02", "랜딩 완성"), true, Lang::Ko);
        assert!(n.contains("\n---\n\n> [!note] porch가 요약을 만들 때마다 다시 쓰는 노트입니다."));
        let f = front(&n);
        assert!(f.starts_with("porch_format: 1\ntype: daily\ndate: 2026-10-02\n"));
        assert!(f.contains("headline: \"랜딩 완성\"\n"));
        assert!(f.contains("tags: [porch]"));
        assert!(n.contains("# 2026년 10월 2일"));
    }

    #[test]
    fn headline_with_yaml_specials_stays_one_quoted_line() {
        let n = day_note(&day("2026-10-02", "a: \"b\" # c\nd\\e"), true, Lang::Ko);
        assert!(front(&n).contains("headline: \"a: \\\"b\\\" # c d\\\\e\"\n"));
    }

    #[test]
    fn cost_is_left_out_when_hidden() {
        let mut r = day("2026-10-02", "x");
        r.digest.metrics.usage = Some(crate::usage::Usage { cost: 23.4, ..Default::default() });
        assert!(front(&day_note(&r, true, Lang::Ko)).contains("cost_usd: 23.40"));
        assert!(!front(&day_note(&r, false, Lang::Ko)).contains("cost_usd"));
    }

    #[test]
    fn week_note_links_only_days_with_a_summary() {
        let line = |d: &str, h: Option<&str>| DayLine { date: d.into(), minutes: 10, commits: 0, my_commits: None, cost: None, tokens: None, headline: h.map(Into::into), projects: Default::default() };
        let w = WeekReport { week_start: "2026-09-28".into(), generated_at: 0, model: None, provider: None, days: vec![line("2026-09-28", Some("a")), line("2026-09-29", None)], summary: Some(WeekSummary { headline: "w".into(), ..Default::default() }), summary_error: None };
        let n = week_note(&w, true, Lang::Ko);
        assert!(front(&n).contains("type: weekly\ndate: 2026-09-28\n"));
        assert!(n.contains("[[2026-09-28]]"));
        assert!(!n.contains("[[2026-09-29]]"));
    }

    #[test]
    fn month_note_links_given_weeks() {
        let m = MonthReport { month: "2026-10".into(), generated_at: 0, model: None, provider: None, days: vec![], summary: Some(WeekSummary { headline: "m".into(), ..Default::default() }), summary_error: None };
        let n = month_note(&m, true, &["2026-W40".into()], Lang::Ko);
        assert!(front(&n).contains("type: monthly\ndate: 2026-10\n"));
        assert!(n.contains("[[2026-W40]]"));
    }

    #[test]
    fn file_keys() {
        assert_eq!(file_key(Kind::Daily, "2026-10-02").as_deref(), Some("2026-10-02"));
        assert_eq!(file_key(Kind::Weekly, "2026-09-28").as_deref(), Some("2026-W40"));
        assert_eq!(file_key(Kind::Monthly, "2026-10").as_deref(), Some("2026-10"));
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let state = t.path().join("data/mirror.json");
        let root = t.path().join("vault/porch");
        std::fs::create_dir_all(t.path().join("vault")).unwrap();
        (t, state, root)
    }

    #[test]
    fn writes_then_overwrites_untouched_note() {
        let (_t, state, root) = setup();
        assert_eq!(write_note(&state, &root, "daily/2026-10-02.md", "one").unwrap(), Outcome::Written(root.join("daily/2026-10-02.md")));
        assert_eq!(write_note(&state, &root, "daily/2026-10-02.md", "two").unwrap(), Outcome::Written(root.join("daily/2026-10-02.md")));
        assert_eq!(std::fs::read_to_string(root.join("daily/2026-10-02.md")).unwrap(), "two");
    }

    #[test]
    fn rewrites_a_note_a_person_edited() {
        let (_t, state, root) = setup();
        write_note(&state, &root, "daily/d.md", "porch text").unwrap();
        std::fs::write(root.join("daily/d.md"), "porch text\n\nmy note").unwrap();
        assert_eq!(write_note(&state, &root, "daily/d.md", "new").unwrap(), Outcome::Written(root.join("daily/d.md")));
        assert_eq!(std::fs::read_to_string(root.join("daily/d.md")).unwrap(), "new");
    }

    #[test]
    fn rewrites_a_deleted_note() {
        let (_t, state, root) = setup();
        write_note(&state, &root, "daily/d.md", "a").unwrap();
        std::fs::remove_file(root.join("daily/d.md")).unwrap();
        assert!(matches!(write_note(&state, &root, "daily/d.md", "b").unwrap(), Outcome::Written(_)));
    }

    #[test]
    fn never_creates_a_root_whose_parent_is_gone() {
        let (t, state, _) = setup();
        let gone = t.path().join("unplugged-disk/vault/porch");
        assert!(write_note(&state, &gone, "daily/d.md", "a").is_err());
        assert!(!t.path().join("unplugged-disk").exists());
    }

    #[test]
    fn a_file_already_in_the_folder_is_replaced() {
        let (_t, state, root) = setup();
        std::fs::create_dir_all(root.join("daily")).unwrap();
        std::fs::write(root.join("daily/d.md"), "someone else's").unwrap();
        assert!(matches!(write_note(&state, &root, "daily/d.md", "porch").unwrap(), Outcome::Written(_)));
    }

    #[test]
    fn a_broken_state_file_does_not_stop_writing() {
        let (_t, state, root) = setup();
        write_note(&state, &root, "daily/d.md", "a").unwrap();
        std::fs::write(&state, "{not json").unwrap();
        assert!(matches!(write_note(&state, &root, "daily/d.md", "b").unwrap(), Outcome::Written(_)));
    }

    #[test]
    fn reads_vaults_and_drops_missing_paths() {
        let t = tempfile::tempdir().unwrap();
        let v1 = t.path().join("Obsidian Vault");
        std::fs::create_dir_all(&v1).unwrap();
        let reg = t.path().join("obsidian.json");
        std::fs::write(&reg, serde_json::json!({"vaults": {
            "abc": {"path": v1, "ts": 1, "open": true},
            "def": {"path": t.path().join("gone"), "ts": 2}
        }}).to_string()).unwrap();
        let v = read_vaults(&reg);
        assert_eq!(v.vaults, vec![Vault { id: "abc".into(), name: "Obsidian Vault".into(), path: v1 }]);
        assert_eq!(v.note, None);
    }

    #[test]
    fn missing_or_unknown_registry_gives_a_reason() {
        let t = tempfile::tempdir().unwrap();
        assert!(read_vaults(&t.path().join("none.json")).note.is_some());
        let reg = t.path().join("obsidian.json");
        std::fs::write(&reg, r#"{"vaults": []}"#).unwrap();
        let v = read_vaults(&reg);
        assert!(v.vaults.is_empty() && v.note.is_some());
    }

    #[test]
    fn uri_is_percent_encoded_and_relative_to_the_vault() {
        let vault = Path::new("/Users/me/Library/Mobile Documents/iCloud~md~obsidian/Documents/내 볼트");
        let note = vault.join("porch/weekly/2026-W40.md");
        assert_eq!(
            obsidian_uri("abc 1", vault, &note).as_deref(),
            Some("obsidian://open?vault=abc%201&file=porch%2Fweekly%2F2026-W40")
        );
        assert_eq!(obsidian_uri("abc", vault, Path::new("/elsewhere/x.md")), None);
    }

    #[test]
    fn save_into_records_the_outcome_by_period() {
        let (_t, state, root) = setup();
        let r = day("2026-10-02", "h");
        save_day_into(&state, &root, "daily", &r, true);
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        assert_eq!(st.last.get("day:2026-10-02").map(String::as_str), Some("written"));
        assert!(root.join("daily/2026-10-02.md").exists());
    }

    #[test]
    fn report_without_summary_is_not_mirrored() {
        let (_t, state, root) = setup();
        let mut r = day("2026-10-02", "h");
        r.summary = None;
        save_day_into(&state, &root, "daily", &r, true);
        assert!(!root.join("daily/2026-10-02.md").exists());
    }

    #[test]
    fn a_write_error_is_recorded_not_raised() {
        let (t, state, _) = setup();
        save_day_into(&state, &t.path().join("gone/porch"), "daily", &day("2026-10-02", "h"), true);
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        let e = st.last_error.unwrap();
        assert!(e.contains("폴더를 찾을 수 없습니다") || e.contains("Folder not found"), "{e}");
    }

    #[test]
    fn ensure_root_makes_the_folder_only_under_an_existing_parent() {
        let (t, _, root) = setup();
        ensure_root(&root).unwrap();
        assert!(root.is_dir());
        assert!(ensure_root(&t.path().join("unplugged-disk/vault/porch")).is_err());
        assert!(!t.path().join("unplugged-disk").exists());
    }

    #[test]
    fn obsidian_link_only_when_chosen() {
        let uri = || Some("obsidian://open?vault=a&file=b".to_owned());
        assert_eq!(click_uri("obsidian", uri()), uri());
        assert_eq!(click_uri("porch", uri()), None);
        assert_eq!(click_uri("obsidian", None), None);
    }

    #[test]
    fn a_written_note_stamps_the_last_save_and_counts() {
        let (_t, state, root) = setup();
        save_day_into(&state, &root, "daily", &day("2026-10-02", "h"), true);
        save_day_into(&state, &root, "daily", &day("2026-10-01", "h"), true);
        std::fs::remove_file(root.join("daily/2026-10-01.md")).unwrap();
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        assert!(st.last_saved.is_some());
        assert_eq!(count_notes(&st, &root), 1);
    }

    #[test]
    fn samples_show_where_todays_notes_go() {
        assert_eq!(
            samples(["daily", "Journal/Weekly", ""], "2026-10-03"),
            vec!["daily/2026-10-03.md".to_owned(), "Journal/Weekly/2026-W40.md".to_owned(), "2026-10.md".to_owned()]
        );
    }

    #[test]
    fn a_folder_inside_a_vault_belongs_to_it() {
        let v = Vault { id: "abc".into(), name: "Notes".into(), path: PathBuf::from("/Users/me/Notes") };
        assert_eq!(vault_for(Path::new("/Users/me/Notes/porch"), std::slice::from_ref(&v)), Some(v.clone()));
        assert_eq!(vault_for(Path::new("/Users/me/Notes"), std::slice::from_ref(&v)), Some(v.clone()));
        assert_eq!(vault_for(Path::new("/Users/me/NotesOther"), std::slice::from_ref(&v)), None);
    }

    #[test]
    fn vault_link_is_encoded() {
        assert_eq!(vault_uri("abc 1"), "obsidian://open?vault=abc%201");
    }

    #[test]
    fn each_folder_keeps_its_own_edit_record() {
        let (t, state, a) = setup();
        let b = t.path().join("vault/other");
        write_note(&state, &a, "daily/d.md", "a1").unwrap();
        write_note(&state, &b, "daily/d.md", "b1").unwrap();
        assert!(matches!(write_note(&state, &a, "daily/d.md", "a2").unwrap(), Outcome::Written(_)));
        assert_eq!(std::fs::read_to_string(a.join("daily/d.md")).unwrap(), "a2");
    }

    #[test]
    fn old_records_belong_to_the_folder_written_next() {
        let (_t, state, root) = setup();
        std::fs::create_dir_all(root.join("daily")).unwrap();
        std::fs::write(root.join("daily/d.md"), "x").unwrap();
        std::fs::create_dir_all(state.parent().unwrap()).unwrap();
        std::fs::write(&state, serde_json::json!({ "files": { "daily/d.md": fnv64("x") } }).to_string()).unwrap();
        assert!(matches!(write_note(&state, &root, "daily/d.md", "y").unwrap(), Outcome::Written(_)));
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        assert!(st.files.is_empty() && st.roots.len() == 1);
    }

    #[test]
    fn notes_left_in_other_folders_are_counted() {
        let (t, state, a) = setup();
        let b = t.path().join("vault/other");
        write_note(&state, &a, "daily/d.md", "a").unwrap();
        write_note(&state, &a, "daily/e.md", "a").unwrap();
        write_note(&state, &b, "daily/d.md", "b").unwrap();
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        let a = std::fs::canonicalize(&a).unwrap();
        assert_eq!(elsewhere(&st, &b), vec![(a.to_string_lossy().into_owned(), 2)]);
        assert_eq!(count_notes(&st, &b), 1);
    }

    #[test]
    fn a_note_that_is_not_text_is_replaced() {
        let (_t, state, root) = setup();
        write_note(&state, &root, "daily/d.md", "a").unwrap();
        std::fs::write(root.join("daily/d.md"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(write_note(&state, &root, "daily/d.md", "b").unwrap(), Outcome::Written(_)));
    }

    #[test]
    fn a_folder_in_the_notes_place_is_not_replaced() {
        let (_t, state, root) = setup();
        std::fs::create_dir_all(root.join("daily/d.md")).unwrap();
        assert!(write_note(&state, &root, "daily/d.md", "b").is_err());
        assert!(root.join("daily/d.md").is_dir());
    }

    #[test]
    fn a_linked_subfolder_cannot_lead_outside() {
        let (t, state, root) = setup();
        let outside = t.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("daily")).unwrap();
        assert!(write_note(&state, &root, "daily/d.md", "a").is_err());
        assert!(!outside.join("d.md").exists());
    }

    #[test]
    fn saved_reports_are_listed_by_kind() {
        let t = tempfile::tempdir().unwrap();
        for f in ["2026-10-01.json", "2026-09-30.json", "week-2026-09-28.json", "week-2026-09-21.html", "month-2026-09.json", "notes.json"] {
            std::fs::write(t.path().join(f), "{}").unwrap();
        }
        let p = saved_periods(t.path());
        assert_eq!(p.days, vec!["2026-09-30".to_owned(), "2026-10-01".to_owned()]);
        assert_eq!(p.weeks, vec!["2026-09-28".to_owned()]);
        assert_eq!(p.months, vec!["2026-09".to_owned()]);
    }

    #[test]
    fn two_spellings_of_one_folder_share_a_record() {
        let (t, state, root) = setup();
        let alias = t.path().join("alias");
        std::os::unix::fs::symlink(t.path().join("vault"), &alias).unwrap();
        write_note(&state, &root, "daily/d.md", "a").unwrap();
        assert!(matches!(write_note(&state, &alias.join("porch"), "daily/d.md", "b").unwrap(), Outcome::Written(_)));
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        assert_eq!(st.roots.len(), 1);
    }

    #[test]
    fn a_note_path_may_not_climb_out_of_the_folder() {
        let (t, state, root) = setup();
        for rel in ["../escaped.md", "daily/../../escaped.md", "/tmp/escaped.md"] {
            assert!(write_note(&state, &root, rel, "a").is_err(), "{rel}");
        }
        assert!(!t.path().join("vault/escaped.md").exists());
    }

    #[test]
    fn disconnecting_clears_the_last_error() {
        let (t, state, _) = setup();
        save_day_into(&state, &t.path().join("gone/porch"), "daily", &day("2026-10-02", "h"), true);
        clear_last_error(&state);
        let st: State = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
        assert_eq!(st.last_error, None);
    }
}
