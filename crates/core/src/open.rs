//! Work left open across days: things in progress or up next (Pending) and
//! blockers that were not resolved (Blocker). A day's summary opens items; a
//! later day's summary closes them when its records show them done, or the
//! person closes one by hand. Kept in one file under the data dir.

use crate::lang::Lang;
use crate::paths;
use crate::summary::{Report, Summary};
use crate::time;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// In progress or next up: waiting to be done.
    Waiting,
    /// A blocker that was not resolved.
    Issue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenItem {
    /// "o12"; how a summary points at it.
    pub id: String,
    pub project: String,
    pub text: String,
    pub kind: Kind,
    /// The day it was first written down.
    pub since: String,
    #[serde(default)]
    pub closed_on: Option<String>,
    /// "done", "dropped" or "manual" (closed by hand). Files saved before
    /// languages existed say "끝남", "그만둠" or "직접 닫음"; they read as these codes.
    #[serde(default, deserialize_with = "closed_how_code")]
    pub closed_how: Option<String>,
}

/// `closed_how` of an item the person closed by hand.
pub const MANUAL: &str = "manual";

fn closed_how_code<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.map(|s| match s.as_str() {
        "끝남" => "done".to_owned(),
        "그만둠" => "dropped".to_owned(),
        "직접 닫음" => MANUAL.to_owned(),
        _ => s,
    }))
}

/// How an item was closed, as people read it.
pub fn closed_label(how: Option<&str>, lang: Lang) -> &'static str {
    match (how, lang) {
        (Some("dropped"), Lang::Ko) => "그만둠",
        (Some("dropped"), Lang::En) => "dropped",
        (Some(MANUAL), Lang::Ko) => "직접 닫음",
        (Some(MANUAL), Lang::En) => "closed manually",
        (_, Lang::Ko) => "끝남",
        (_, Lang::En) => "done",
    }
}

/// What a day's summary said about an item that was already open.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenUpdate {
    pub id: String,
    /// "done", "dropped" or "open".
    pub status: String,
}

fn file() -> PathBuf {
    paths::data_dir().join("open.json")
}

fn save(items: &[OpenItem]) -> Result<(), String> {
    let path = file();
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("json.{}-{}.tmp", std::process::id(), crate::time::now_ms()));
    fs::write(&tmp, serde_json::to_string(items).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// The ledger. The first time, it starts from what summaries already exist:
/// for each project only its latest summarized day counts, since earlier
/// days' open items cannot be known to be closed.
pub fn load(saved: impl FnOnce() -> Vec<Report>) -> Vec<OpenItem> {
    if let Some(items) = fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        return items;
    }
    let mut latest: std::collections::BTreeMap<String, &Report> = Default::default();
    let days = saved();
    for r in &days {
        for p in r.summary.iter().flat_map(|s| &s.projects) {
            latest.insert(p.name.clone(), r);
        }
    }
    let mut items = Vec::new();
    for (project, r) in latest {
        if let Some(s) = &r.summary {
            add_from(&mut items, &r.date, s, Some(&project));
        }
    }
    let _ = save(&items);
    items
}

fn next_id(items: &[OpenItem]) -> String {
    let n = items.iter().filter_map(|i| i.id.strip_prefix('o')?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("o{}", n + 1)
}

/// Open items from one day's summary; `only` limits it to one project.
fn add_from(items: &mut Vec<OpenItem>, date: &str, s: &Summary, only: Option<&str>) {
    let push = |items: &mut Vec<OpenItem>, project: &str, text: &str, kind: Kind| {
        if only.is_some_and(|o| o != project) || text.trim().is_empty() {
            return;
        }
        let id = next_id(items);
        items.push(OpenItem { id, project: project.into(), text: text.trim().into(), kind, since: date.into(), closed_on: None, closed_how: None });
    };
    for p in &s.projects {
        for t in p.in_progress.iter().chain(&p.next) {
            push(items, &p.name, t, Kind::Waiting);
        }
    }
    for b in s.blockers.iter().filter(|b| !b.resolved) {
        push(items, &b.project, &b.title, Kind::Issue);
    }
}

/// What the summarizer is shown: items still open from before `date`, for
/// the projects worked on that day.
pub fn prompt_text(items: &[OpenItem], date: &str, projects: &[&str], lang: Lang) -> String {
    let open: Vec<&OpenItem> = items
        .iter()
        .filter(|i| i.since.as_str() < date && i.closed_on.as_deref().is_none_or(|c| c >= date) && projects.contains(&i.project.as_str()))
        .collect();
    if open.is_empty() {
        return String::new();
    }
    let mut out = String::from(match lang {
        Lang::Ko => "\n## 열린 일 (이전 날에서 이어짐)\n",
        Lang::En => "\n## Open items (carried over from earlier days)\n",
    });
    for i in open {
        let line = match (lang, i.kind) {
            (Lang::Ko, Kind::Issue) => format!("- [{}] {} · 이슈 · {}부터: {}\n", i.id, i.project, i.since, i.text),
            (Lang::Ko, Kind::Waiting) => format!("- [{}] {} · 대기 · {}부터: {}\n", i.id, i.project, i.since, i.text),
            (Lang::En, Kind::Issue) => format!("- [{}] {} · blocker · since {}: {}\n", i.id, i.project, i.since, i.text),
            (Lang::En, Kind::Waiting) => format!("- [{}] {} · pending · since {}: {}\n", i.id, i.project, i.since, i.text),
        };
        out.push_str(&line);
    }
    out
}

/// Record what a day's summary did to the ledger. Safe to run again for the
/// same day: that day's earlier effect is undone first.
pub fn apply(items: &mut Vec<OpenItem>, date: &str, s: &Summary) -> Result<(), String> {
    apply_to(items, date, s);
    save(items)
}

fn apply_to(items: &mut Vec<OpenItem>, date: &str, s: &Summary) {
    items.retain(|i| i.since != date);
    for i in items.iter_mut().filter(|i| i.closed_on.as_deref() == Some(date) && i.closed_how.as_deref() != Some(MANUAL)) {
        i.closed_on = None;
        i.closed_how = None;
    }
    for u in &s.open_updates {
        let how = match u.status.as_str() {
            "done" | "dropped" => u.status.as_str(),
            _ => continue,
        };
        if let Some(i) = items.iter_mut().find(|i| i.id == u.id && i.since.as_str() < date && i.closed_on.is_none()) {
            i.closed_on = Some(date.into());
            i.closed_how = Some(how.into());
        }
    }
    add_from(items, date, s, None);
}

/// Close one item by hand, today.
pub fn close(items: &mut [OpenItem], id: &str, offset: i64) -> Result<(), String> {
    let i = items.iter_mut().find(|i| i.id == id).ok_or_else(|| match Lang::current() {
        Lang::Ko => "그 일을 찾지 못했습니다".to_owned(),
        Lang::En => "Could not find that item".to_owned(),
    })?;
    i.closed_on = Some(time::local_date(time::now_ms(), offset));
    i.closed_how = Some(MANUAL.into());
    save(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::{Blocker, ProjectSummary};
    use crate::lang::Lang;

    #[test]
    fn open_items_are_shown_with_labels_in_the_language() {
        let items = vec![OpenItem { id: "o1".into(), project: "p".into(), text: "ship it".into(), kind: Kind::Issue, since: "2026-09-20".into(), closed_on: None, closed_how: None }];
        let en = prompt_text(&items, "2026-09-22", &["p"], Lang::En);
        assert_eq!(en, "\n## Open items (carried over from earlier days)\n- [o1] p · blocker · since 2026-09-20: ship it\n");
        let ko = prompt_text(&items, "2026-09-22", &["p"], Lang::Ko);
        assert_eq!(ko, "\n## 열린 일 (이전 날에서 이어짐)\n- [o1] p · 이슈 · 2026-09-20부터: ship it\n");
    }

    #[test]
    fn old_closed_how_reads_as_codes_and_manual_stays_closed() {
        let old = r#"[
            {"id":"o1","project":"p","text":"a","kind":"waiting","since":"2026-09-20","closed_on":"2026-09-22","closed_how":"직접 닫음"},
            {"id":"o2","project":"p","text":"b","kind":"issue","since":"2026-09-20","closed_on":"2026-09-21","closed_how":"끝남"},
            {"id":"o3","project":"p","text":"c","kind":"issue","since":"2026-09-20","closed_on":"2026-09-21","closed_how":"그만둠"}
        ]"#;
        let mut items: Vec<OpenItem> = serde_json::from_str(old).unwrap();
        let hows: Vec<_> = items.iter().map(|i| i.closed_how.clone().unwrap()).collect();
        assert_eq!(hows, ["manual", "done", "dropped"]);
        // Summarizing the 22nd again undoes that day's closings, never one made by hand.
        apply_to(&mut items, "2026-09-22", &Summary::default());
        assert_eq!(items[0].closed_how.as_deref(), Some(MANUAL));
    }

    #[test]
    fn closings_are_saved_as_codes_and_shown_in_the_language() {
        let mut items = vec![];
        apply_to(&mut items, "2026-09-21", &day(&["로그인 고치기"], &[], None, &[]));
        let id = items[0].id.clone();
        apply_to(&mut items, "2026-09-22", &day(&[], &[], None, &[(id.as_str(), "done")]));
        assert_eq!(items[0].closed_how.as_deref(), Some("done"));
        assert_eq!(closed_label(Some("done"), Lang::Ko), "끝남");
        assert_eq!(closed_label(Some("dropped"), Lang::En), "dropped");
        assert_eq!(closed_label(Some(MANUAL), Lang::En), "closed manually");
        assert_eq!(closed_label(None, Lang::En), "done");
    }

    fn day(in_progress: &[&str], next: &[&str], issue: Option<&str>, updates: &[(&str, &str)]) -> Summary {
        Summary {
            headline: "h".into(),
            projects: vec![ProjectSummary {
                name: "app".into(),
                in_progress: in_progress.iter().map(|s| s.to_string()).collect(),
                next: next.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }],
            blockers: issue
                .map(|t| vec![Blocker { title: t.into(), project: "app".into(), resolved: false, ..Default::default() }])
                .unwrap_or_default(),
            open_updates: updates.iter().map(|(id, st)| OpenUpdate { id: id.to_string(), status: st.to_string() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn items_open_close_and_reapply() {
        let mut items = Vec::new();
        apply_to(&mut items, "2026-09-21", &day(&["로그인 고치기"], &["배포"], Some("테스트가 계속 실패"), &[]));
        assert_eq!(items.len(), 3);
        assert_eq!(items.iter().filter(|i| i.kind == Kind::Issue).count(), 1);

        // Next day: login done, deploy still open, a new item.
        apply_to(&mut items, "2026-09-22", &day(&["문서"], &[], None, &[("o1", "done"), ("o2", "open")]));
        let open: Vec<&str> = items.iter().filter(|i| i.closed_on.is_none()).map(|i| i.text.as_str()).collect();
        assert_eq!(open, ["배포", "테스트가 계속 실패", "문서"]);
        assert!(prompt_text(&items, "2026-09-23", &["app"], Lang::Ko).contains("[o2]"));
        assert!(prompt_text(&items, "2026-09-23", &["other"], Lang::Ko).is_empty());

        // Redoing 9/22 differently undoes its first version.
        apply_to(&mut items, "2026-09-22", &day(&[], &[], None, &[]));
        let open: Vec<&str> = items.iter().filter(|i| i.closed_on.is_none()).map(|i| i.text.as_str()).collect();
        assert_eq!(open, ["로그인 고치기", "배포", "테스트가 계속 실패"]);
    }
}
