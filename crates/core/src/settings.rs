//! User settings kept under the data dir. Small, flat, and read on use.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;

pub const MODELS: &[&str] = &["sonnet", "opus", "haiku"];

/// Where a clicked summary notification opens: porch's Summary screen or the Obsidian note.
pub const NOTIFY_OPEN: &[&str] = &["porch", "obsidian"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Who writes summaries and suggestions: a `writer::Provider` id.
    #[serde(default = "claude")]
    pub provider: String,
    /// Claude Code model alias, used when the provider is Claude Code.
    #[serde(default = "default_model")]
    pub model: String,
    /// Codex model id, used when the provider is Codex; None = Codex's default.
    #[serde(default)]
    pub codex_model: Option<String>,
    /// Local "HH:MM" at which today's summary is written automatically; None = off.
    #[serde(default)]
    pub auto_summary_at: Option<String>,
    /// Set once the first-run screen has been finished or skipped.
    #[serde(default)]
    pub onboarded: bool,
    /// Monthly budget in list-price USD; None = no budget.
    #[serde(default)]
    pub monthly_budget: Option<f64>,
    /// Show list-price cost next to tokens.
    #[serde(default = "yes")]
    pub show_cost: bool,
    /// Send the app usage events of ADR 0006. On unless turned off.
    #[serde(default = "yes")]
    pub analytics: bool,
    /// Folder summaries are mirrored into as Markdown notes; None = off.
    #[serde(default)]
    pub mirror_dir: Option<String>,
    /// Obsidian vault id when mirror_dir lies in a vault (for obsidian:// links).
    #[serde(default)]
    pub obsidian_vault: Option<String>,
    /// Subfolders under mirror_dir; "" = mirror_dir itself.
    #[serde(default = "daily")]
    pub mirror_daily: String,
    #[serde(default = "weekly")]
    pub mirror_weekly: String,
    #[serde(default = "monthly")]
    pub mirror_monthly: String,
    /// Notify when an automatic summary is written.
    #[serde(default = "yes")]
    pub notify: bool,
    /// One of NOTIFY_OPEN; "obsidian" falls back to porch without a linked vault.
    #[serde(default = "porch")]
    pub notify_open: String,
    /// Write the weekly insight evaluation after the weekly summary (ADR 0012). On unless turned off.
    #[serde(default = "yes")]
    pub insight_eval: bool,
    /// "system", "ko" or "en" (`lang::SETTINGS`); see `Lang::resolve`. Anything
    /// else in the file reads as "system", so a hand-edited typo never blocks saving.
    #[serde(default = "system", deserialize_with = "language_value")]
    pub language: String,
}

fn yes() -> bool {
    true
}

fn default_model() -> String {
    "sonnet".into()
}

fn claude() -> String {
    "claude".into()
}

fn porch() -> String {
    "porch".into()
}

fn system() -> String {
    "system".into()
}

fn language_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = String::deserialize(d)?;
    Ok(if crate::lang::SETTINGS.contains(&v.as_str()) { v } else { system() })
}

fn daily() -> String {
    "daily".into()
}

fn weekly() -> String {
    "weekly".into()
}

fn monthly() -> String {
    "monthly".into()
}

/// The mirror folder is stored absolute: a relative one would mean a
/// different place to the app and to the CLI.
pub fn valid_mirror_dir(s: &str) -> bool {
    std::path::Path::new(s).is_absolute()
}

/// A relative path that stays inside the mirror folder.
pub fn valid_subfolder(s: &str) -> bool {
    !s.starts_with('/') && !s.starts_with('~') && !s.contains('\\') && s.split('/').all(|p| p != ".." && p != ".")
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            provider: claude(),
            model: default_model(),
            codex_model: None,
            auto_summary_at: None,
            onboarded: false,
            monthly_budget: None,
            show_cost: true,
            analytics: true,
            mirror_dir: None,
            obsidian_vault: None,
            mirror_daily: daily(),
            mirror_weekly: weekly(),
            mirror_monthly: monthly(),
            notify: true,
            notify_open: porch(),
            insight_eval: true,
            language: system(),
        }
    }
}

fn file() -> std::path::PathBuf {
    paths::data_dir().join("settings.json")
}

pub fn load() -> Settings {
    fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(s: &Settings) -> Result<(), String> {
    if !crate::lang::SETTINGS.contains(&s.language.as_str()) {
        return Err(format!("unknown language {}", s.language));
    }
    if crate::writer::Provider::parse(&s.provider).is_none() {
        return Err(format!("unknown provider {}", s.provider));
    }
    if !MODELS.contains(&s.model.as_str()) {
        return Err(format!("unknown model {}", s.model));
    }
    if s.codex_model.as_deref().is_some_and(|m| m.trim().is_empty() || m.chars().any(char::is_whitespace)) {
        return Err("Codex model must be one word, or unset for its default".into());
    }
    if let Some(t) = &s.auto_summary_at {
        let ok = t.len() == 5
            && t.as_bytes()[2] == b':'
            && t[..2].parse::<u8>().is_ok_and(|h| h < 24)
            && t[3..].parse::<u8>().is_ok_and(|m| m < 60);
        if !ok {
            return Err(format!("time must be HH:MM, got {t}"));
        }
    }
    if s.monthly_budget.is_some_and(|b| !b.is_finite() || b <= 0.0) {
        return Err("budget must be a positive amount".into());
    }
    if !NOTIFY_OPEN.contains(&s.notify_open.as_str()) {
        return Err(format!("unknown notification target {}", s.notify_open));
    }
    if let Some(d) = s.mirror_dir.as_deref().filter(|d| !valid_mirror_dir(d)) {
        return Err(format!("folder must be an absolute path, got {d}"));
    }
    for sub in [&s.mirror_daily, &s.mirror_weekly, &s.mirror_monthly] {
        if !valid_subfolder(sub) {
            return Err(format!("folder must stay inside the mirror folder, got {sub}"));
        }
    }
    fs::create_dir_all(paths::data_dir()).map_err(|e| e.to_string())?;
    fs::write(file(), serde_json::to_string_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Path prefixes excluded from the board, hooks and summaries.
pub fn excluded() -> Vec<String> {
    fs::read_to_string(paths::excluded_file())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn set_excluded(list: &[String]) -> Result<(), String> {
    fs::create_dir_all(paths::data_dir()).map_err(|e| e.to_string())?;
    let home = std::env::var("HOME").unwrap_or_default();
    let expand = |l: &str| match l.strip_prefix("~/") {
        Some(rest) if !home.is_empty() => format!("{home}/{rest}"),
        _ => l.to_owned(),
    };
    let mut body: String = list
        .iter()
        .map(|l| expand(l.trim()))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    body.push('\n');
    fs::write(paths::excluded_file(), body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_language_in_the_file_reads_as_system() {
        // A hand-edited typo must not block every later save from the app.
        let s: Settings = serde_json::from_str(r#"{"provider":"claude","language":"eng"}"#).unwrap();
        assert_eq!(s.language, "system");
        let s: Settings = serde_json::from_str(r#"{"provider":"claude","language":"en"}"#).unwrap();
        assert_eq!(s.language, "en");
    }

    #[test]
    fn language_defaults_to_system_and_rejects_others() {
        let s: Settings = serde_json::from_str(r#"{"provider":"claude"}"#).unwrap();
        assert_eq!(s.language, "system");
        assert_eq!(Settings::default().language, "system");
        // Refused before anything is written.
        let bad = Settings { language: "fr".into(), ..Default::default() };
        assert!(save(&bad).unwrap_err().contains("language"));
    }

    #[test]
    fn subfolders() {
        for ok in ["daily", "Journal/Daily", "", "주간"] {
            assert!(valid_subfolder(ok), "{ok}");
        }
        for bad in ["../x", "a/../b", "/tmp", "~/x", "a\\b"] {
            assert!(!valid_subfolder(bad), "{bad}");
        }
    }

    #[test]
    fn old_settings_files_get_mirror_defaults() {
        let s: Settings = serde_json::from_str(r#"{"model":"sonnet"}"#).unwrap();
        assert_eq!((s.mirror_dir, s.mirror_daily.as_str(), s.notify), (None, "daily", true));
    }

    #[test]
    fn notification_opens_porch_by_default() {
        let s: Settings = serde_json::from_str(r#"{"model":"sonnet"}"#).unwrap();
        assert_eq!(s.notify_open, "porch");
        assert!(NOTIFY_OPEN.contains(&"obsidian"));
    }

    #[test]
    fn old_settings_files_write_with_claude_code() {
        let s: Settings = serde_json::from_str(r#"{"model":"opus"}"#).unwrap();
        assert_eq!((s.provider.as_str(), s.model.as_str(), s.codex_model), ("claude", "opus", None));
    }

    #[test]
    fn mirror_folder_must_be_absolute() {
        assert!(valid_mirror_dir("/Users/me/Notes/porch"));
        assert!(!valid_mirror_dir("."));
        assert!(!valid_mirror_dir("notes/porch"));
        assert!(!valid_mirror_dir("~/notes"));
    }

    #[test]
    fn insight_evaluation_is_on_unless_turned_off() {
        assert!(serde_json::from_str::<Settings>("{}").unwrap().insight_eval);
        assert!(Settings::default().insight_eval);
    }
}
