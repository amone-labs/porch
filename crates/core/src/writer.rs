//! Who writes summaries and suggestions: Claude Code or Codex, chosen in
//! settings (ADR 0010). Either runs headless in the runner folder with no
//! tools, no user settings or hooks, and no saved transcript, and answers with
//! one JSON object.

use crate::lang::Lang;
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Claude,
    Codex,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Claude, Provider::Codex];

    pub fn id(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Provider> {
        Provider::ALL.into_iter().find(|p| p.id() == s)
    }

    /// The name people see.
    pub fn name(self) -> &'static str {
        match self {
            Provider::Claude => "Claude Code",
            Provider::Codex => "Codex",
        }
    }

    pub fn bin(self) -> Option<PathBuf> {
        match self {
            Provider::Claude => crate::agents::claude_bin(),
            Provider::Codex => crate::agents::codex_bin(),
        }
    }
}

/// The provider, model and language one run uses, taken from settings when
/// the run starts: a run that began in one language ends in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Engine {
    pub provider: Provider,
    /// None: the provider's own default model.
    pub model: Option<String>,
    pub lang: crate::lang::Lang,
}

impl Engine {
    pub fn from_settings(s: &Settings) -> Engine {
        let lang = crate::lang::Lang::resolve(&s.language);
        match Provider::parse(&s.provider).unwrap_or_default() {
            Provider::Claude => Engine { provider: Provider::Claude, model: Some(s.model.clone()), lang },
            Provider::Codex => Engine { provider: Provider::Codex, model: s.codex_model.clone(), lang },
        }
    }

    /// What a report keeps as its model: the name, or "default" when the provider chose.
    pub fn model_label(&self) -> String {
        self.model.clone().unwrap_or_else(|| "default".into())
    }
}

/// "Claude Code(sonnet)", "Codex (default model)": who wrote a saved summary.
/// Reports saved before the choice existed were all written by Claude Code.
pub fn written_by(provider: Option<Provider>, model: &str, lang: Lang) -> String {
    let name = provider.unwrap_or_default().name();
    match lang {
        Lang::Ko => format!("{name}({})", if model == "default" { "기본 모델" } else { model }),
        Lang::En => format!("{name} ({})", if model == "default" { "default model" } else { model }),
    }
}

/// A model Codex offers, as its own catalog lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexModel {
    pub slug: String,
    pub name: String,
}

/// The models Codex lists for people to pick, in Codex's own order: the first
/// is its default. Empty when Codex is missing or its catalog cannot be read
/// (`codex debug models` is not a documented format).
pub fn codex_models() -> Vec<CodexModel> {
    let Some(bin) = crate::agents::codex_bin() else { return vec![] };
    let Ok(out) = Command::new(&bin)
        .env("PATH", crate::agents::child_path(&bin))
        .args(["debug", "models"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return vec![];
    };
    parse_codex_models(&String::from_utf8_lossy(&out.stdout))
}

fn parse_codex_models(text: &str) -> Vec<CodexModel> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return vec![] };
    let mut listed: Vec<(i64, CodexModel)> = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| {
            let slug = m["slug"].as_str()?.to_owned();
            let name = m["display_name"].as_str().unwrap_or(&slug).to_owned();
            Some((m["priority"].as_i64().unwrap_or(i64::MAX), CodexModel { slug, name }))
        })
        .collect();
    listed.sort_by_key(|(p, _)| *p);
    listed.into_iter().map(|(_, m)| m).collect()
}

/// The words each kind of failure is told by, `[Korean, English]`, indexed by
/// `Lang as usize`. The app's analytics.ts and App.tsx read the same words
/// (ADR 0006's `failure_kind`); change them together.
const BAD_JSON: [&str; 2] = ["요약 JSON을 읽지 못했습니다", "Could not read the summary JSON"];
const LIMIT: [&str; 2] = ["사용 한도에 걸렸습니다", "hit its usage limit"];
const LOGIN: [&str; 2] = ["로그인되어 있지 않습니다", "is not logged in"];
const MISSING: [&str; 2] = ["를 찾지 못했습니다. ", " was not found. "];

/// A writer error that says the agent hit its usage limit, in either language.
pub fn is_limit(e: &str) -> bool {
    LIMIT.iter().any(|w| e.contains(w))
}

fn is_bad_json(e: &str) -> bool {
    BAD_JSON.iter().any(|w| e.starts_with(w))
}

fn missing(p: Provider, lang: Lang) -> String {
    let i = lang as usize;
    match lang {
        Lang::Ko => format!("{}({}){}{}를 설치하고 한 번 로그인해 주세요.", p.name(), p.id(), MISSING[i], p.name()),
        Lang::En => format!("{} ({}){}Install {} and log in once.", p.name(), p.id(), MISSING[i], p.name()),
    }
}

/// Run the engine once, and once more when its answer is not valid JSON: now
/// and then a quote inside a string comes back unescaped, and a second answer is fine.
pub fn run(e: &Engine, prompt: &str, system_prompt: &str) -> Result<serde_json::Value, String> {
    retry_bad_json(|| run_once(e, prompt, system_prompt))
}

fn retry_bad_json(mut ask: impl FnMut() -> Result<serde_json::Value, String>) -> Result<serde_json::Value, String> {
    match ask() {
        Err(e) if is_bad_json(&e) => ask(),
        other => other,
    }
}

fn run_once(e: &Engine, prompt: &str, system_prompt: &str) -> Result<serde_json::Value, String> {
    let dir = crate::summary::runner_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let bin = e.provider.bin().ok_or_else(|| missing(e.provider, e.lang))?;
    let model = e.model.as_deref().unwrap_or("sonnet");
    let text = match e.provider {
        Provider::Claude => run_claude(&bin, &dir, prompt, model, system_prompt, e.lang)?,
        Provider::Codex => run_codex(&bin, &dir, prompt, e.model.as_deref(), system_prompt, e.lang)?,
    };
    serde_json::from_str(strip_fences(&text)).map_err(|err| format!("{}: {err}", BAD_JSON[e.lang as usize]))
}

pub fn strip_fences(s: &str) -> &str {
    let t = s.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    t.strip_suffix("```").unwrap_or(t).trim()
}

fn spawn_with_stdin(mut cmd: Command, prompt: &str, name: &str, lang: Lang) -> Result<std::process::Output, String> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match lang {
            Lang::Ko => format!("{name} 실행 실패: {e}"),
            Lang::En => format!("{name} failed to start: {e}"),
        })?;
    child.stdin.take().ok_or("stdin")?.write_all(prompt.as_bytes()).map_err(|e| e.to_string())?;
    child.wait_with_output().map_err(|e| e.to_string())
}

/// Claude Code: no tools, no setting sources (so no hooks, MCP or plugins),
/// no saved session.
fn run_claude(bin: &Path, dir: &Path, prompt: &str, model: &str, system_prompt: &str, lang: Lang) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.env("PATH", crate::agents::child_path(bin));
    cmd.current_dir(dir).args([
        "-p",
        "--no-session-persistence",
        "--tools",
        "",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--output-format",
        "json",
        "--model",
        model,
        "--system-prompt",
        system_prompt,
    ]);
    let out = spawn_with_stdin(cmd, prompt, "claude", lang)?;
    let envelope: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        let stderr = String::from_utf8_lossy(&out.stderr).chars().take(300).collect::<String>();
        match lang {
            Lang::Ko => format!("claude 응답을 읽지 못했습니다: {stderr}"),
            Lang::En => format!("Could not read the Claude Code response: {stderr}"),
        }
    })?;
    if envelope["is_error"] == true {
        let raw = envelope["result"].as_str().map(str::to_owned).unwrap_or_else(|| envelope["result"].to_string());
        return Err(explain(Provider::Claude, lang, &raw));
    }
    envelope["result"].as_str().map(str::to_owned).ok_or_else(|| match lang {
        Lang::Ko => "claude 응답에 result가 없습니다".to_owned(),
        Lang::En => "The Claude Code response contained no result".to_owned(),
    })
}

/// Codex: no config.toml, rules, hooks or plugins, no web search, no project
/// docs, a read-only sandbox, no saved session. Codex has no switch for its
/// shell tool; the sandbox keeps it from changing anything. The last message
/// goes to a file of its own, so runs at the same time do not mix.
fn run_codex(bin: &Path, dir: &Path, prompt: &str, model: Option<&str>, system_prompt: &str, lang: Lang) -> Result<String, String> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let last = dir.join(format!("codex-{}-{n}.txt", std::process::id()));
    // A JSON string literal is a valid TOML basic string.
    let instructions = format!("developer_instructions={}", serde_json::to_string(system_prompt).map_err(|e| e.to_string())?);
    let mut cmd = Command::new(bin);
    cmd.env("PATH", crate::agents::child_path(bin));
    cmd.current_dir(dir).args(["exec", "--skip-git-repo-check", "--ephemeral", "--ignore-user-config", "--ignore-rules"]);
    cmd.args(["--disable", "hooks", "--disable", "plugins", "-s", "read-only"]);
    cmd.args(["-c", "web_search=\"disabled\"", "-c", "project_doc_max_bytes=0", "-c", &instructions]);
    cmd.arg("-C").arg(dir).arg("-o").arg(&last);
    if let Some(m) = model {
        cmd.args(["-m", m]);
    }
    cmd.arg("-");
    let out = spawn_with_stdin(cmd, prompt, "codex", lang);
    let text = fs::read_to_string(&last).ok();
    let _ = fs::remove_file(&last);
    let out = out?;
    match text.filter(|t| !t.trim().is_empty()) {
        Some(t) if out.status.success() => Ok(t),
        _ => {
            let e = codex_error(&String::from_utf8_lossy(&out.stderr));
            let empty = match lang {
                Lang::Ko => "답이 비어 있습니다",
                Lang::En => "The summary agent returned an empty response",
            };
            Err(explain(Provider::Codex, lang, if e.is_empty() { empty } else { &e }))
        }
    }
}

/// The line Codex marks as the error, else the end of what it printed.
fn codex_error(stderr: &str) -> String {
    match stderr.lines().rev().find_map(|l| l.trim().strip_prefix("ERROR:")) {
        Some(e) => e.trim().to_owned(),
        None => {
            let t = stderr.trim();
            let start = t.char_indices().rev().nth(299).map_or(0, |(i, _)| i);
            t[start..].to_owned()
        }
    }
}

/// Say what the agent reported in words people can act on, keeping its own text.
/// The app's analytics.ts sorts failures by these words (ADR 0006); keep them in step.
fn explain(p: Provider, lang: Lang, raw: &str) -> String {
    let lower = raw.to_lowercase();
    let i = lang as usize;
    if ["usage limit", "hit your limit", "limit reached", "rate limit"].iter().any(|k| lower.contains(k)) {
        match lang {
            Lang::Ko => format!("{} {}. 설정 > 일반 > 요약에서 다른 에이전트로 바꾸면 바로 다시 만들 수 있습니다. ({raw})", p.name(), LIMIT[i]),
            Lang::En => format!("{} {}. If the other summary agent is installed and signed in, pick it in Settings and try again. ({raw})", p.name(), LIMIT[i]),
        }
    } else if ["not logged in", "/login", "codex login", "unauthorized"].iter().any(|k| lower.contains(k)) {
        match lang {
            Lang::Ko => format!("{}에 {}. 터미널에서 {}를 한 번 실행해 로그인해 주세요. ({raw})", p.name(), LOGIN[i], p.id()),
            Lang::En => format!("{} {}. Run {} once in a terminal to log in. ({raw})", p.name(), LOGIN[i], p.id()),
        }
    } else {
        match lang {
            Lang::Ko => format!("{} 오류: {raw}", p.id()),
            Lang::En => format!("{} error: {raw}", p.id()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn failures_read_the_same_in_both_languages() {
        for lang in [Lang::Ko, Lang::En] {
            let limit = explain(Provider::Codex, lang, "You've hit your usage limit");
            assert!(is_limit(&limit), "{limit}");
            let login = explain(Provider::Claude, lang, "Not logged in · Please run /login");
            assert!(!is_limit(&login) && login.contains(LOGIN[lang as usize]), "{login}");
            let other = explain(Provider::Codex, lang, "something broke");
            assert_eq!(other, "codex error: something broke".replace("error", if lang == Lang::Ko { "오류" } else { "error" }));
            assert!(missing(Provider::Claude, lang).contains(MISSING[lang as usize]));
        }
        assert!(explain(Provider::Claude, Lang::Ko, "usage limit reached").starts_with("Claude Code 사용 한도에 걸렸습니다"));
        assert!(explain(Provider::Claude, Lang::En, "usage limit reached").starts_with("Claude Code hit its usage limit"));
        assert_eq!(missing(Provider::Codex, Lang::Ko), "Codex(codex)를 찾지 못했습니다. Codex를 설치하고 한 번 로그인해 주세요.");
    }

    #[test]
    fn a_broken_answer_is_asked_once_more_in_either_language() {
        for lang in [Lang::Ko, Lang::En] {
            let mut n = 0;
            let r = retry_bad_json(|| {
                n += 1;
                if n == 1 { Err(format!("{}: x", BAD_JSON[lang as usize])) } else { Ok(serde_json::json!({"ok": true})) }
            });
            assert!(r.is_ok() && n == 2);
            let mut n = 0;
            let r = retry_bad_json(|| {
                n += 1;
                Err(explain(Provider::Codex, lang, "usage limit reached"))
            });
            assert!(r.is_err() && n == 1, "a limit is not asked again");
        }
    }

    #[test]
    fn written_by_names_the_default_model_in_the_language() {
        assert_eq!(written_by(Some(Provider::Codex), "default", Lang::Ko), "Codex(기본 모델)");
        assert_eq!(written_by(Some(Provider::Codex), "default", Lang::En), "Codex (default model)");
        assert_eq!(written_by(None, "sonnet", Lang::En), "Claude Code (sonnet)");
    }

    /// A stand-in `codex` that records its arguments and answers like the real one.
    fn fake_codex(dir: &Path, body: &str) -> PathBuf {
        let bin = dir.join("codex");
        let script = format!(
            "#!/bin/sh\necho \"$@\" > \"{}\"\nout=\"\"\nwhile [ $# -gt 0 ]; do [ \"$1\" = \"-o\" ] && out=\"$2\"; shift; done\ncat > /dev/null\n{body}\n",
            dir.join("args").display()
        );
        fs::write(&bin, script).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    #[test]
    fn codex_answer_is_the_last_message() {
        let t = tempfile::tempdir().unwrap();
        let bin = fake_codex(t.path(), r#"printf '%s' '```json
{"ok": 1}
```' > "$out""#);
        let text = run_codex(&bin, t.path(), "material", Some("gpt-x"), "system \"quoted\"\nline", Lang::Ko).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(strip_fences(&text)).unwrap()["ok"], 1);
        let args = fs::read_to_string(t.path().join("args")).unwrap();
        for flag in ["--ephemeral", "--ignore-user-config", "--disable hooks", "--disable plugins", "-s read-only", "-m gpt-x"] {
            assert!(args.contains(flag), "{flag} missing from {args}");
        }
        // The answer file is not left behind.
        assert!(!fs::read_dir(t.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("codex-")));
    }

    #[test]
    fn codex_default_model_passes_no_model() {
        let t = tempfile::tempdir().unwrap();
        let bin = fake_codex(t.path(), r#"printf '{}' > "$out""#);
        run_codex(&bin, t.path(), "x", None, "s", Lang::Ko).unwrap();
        assert!(!fs::read_to_string(t.path().join("args")).unwrap().contains(" -m "));
    }

    #[test]
    fn codex_usage_limit_says_switch() {
        let t = tempfile::tempdir().unwrap();
        let bin = fake_codex(
            t.path(),
            "echo 'ERROR: You’ve hit your usage limit. Try again at 3:46 PM.' >&2\necho 'ERROR: You’ve hit your usage limit. Try again at 3:46 PM.' >&2\nexit 1",
        );
        let e = run_codex(&bin, t.path(), "x", None, "s", Lang::Ko).unwrap_err();
        assert!(e.starts_with("Codex 사용 한도에 걸렸습니다"), "{e}");
        assert!(e.contains("3:46 PM"), "{e}");
    }

    #[test]
    fn codex_failure_without_error_line_keeps_the_tail() {
        let t = tempfile::tempdir().unwrap();
        let bin = fake_codex(t.path(), "echo 'something broke' >&2\nexit 2");
        assert_eq!(run_codex(&bin, t.path(), "x", None, "s", Lang::Ko).unwrap_err(), "codex 오류: something broke");
    }

    #[test]
    fn claude_limit_is_explained() {
        let e = explain(Provider::Claude, Lang::Ko, "Claude AI usage limit reached|1791000000");
        assert!(e.starts_with("Claude Code 사용 한도에 걸렸습니다"), "{e}");
    }

    #[test]
    fn bad_json_is_asked_once_more() {
        let mut n = 0;
        let r = retry_bad_json(|| {
            n += 1;
            if n == 1 { Err(format!("{}: x", BAD_JSON[0])) } else { Ok(serde_json::json!({"ok": true})) }
        });
        assert!(r.is_ok());
        assert_eq!(n, 2);
        let mut m = 0;
        assert!(retry_bad_json(|| {
            m += 1;
            Err("codex 오류: x".into())
        })
        .is_err());
        assert_eq!(m, 1);
    }

    #[test]
    fn old_reports_were_written_by_claude_code() {
        assert_eq!(written_by(None, "sonnet", Lang::Ko), "Claude Code(sonnet)");
        assert_eq!(written_by(Some(Provider::Codex), "default", Lang::Ko), "Codex(기본 모델)");
    }

    #[test]
    fn codex_catalog_keeps_listed_models_in_order() {
        let text = r#"{"models":[
            {"slug":"b","display_name":"B","visibility":"list","priority":2},
            {"slug":"hidden","display_name":"H","visibility":"hide","priority":0},
            {"slug":"a","display_name":"A","visibility":"list","priority":1}]}"#;
        let slugs: Vec<String> = parse_codex_models(text).into_iter().map(|m| m.slug).collect();
        assert_eq!(slugs, ["a", "b"]);
        assert!(parse_codex_models("not json").is_empty());
    }

    #[test]
    fn engine_follows_settings() {
        // Not "system": the test must not depend on this Mac's language.
        let mut s = Settings { language: "ko".into(), ..Default::default() };
        assert_eq!(Engine::from_settings(&s), Engine { provider: Provider::Claude, model: Some("sonnet".into()), lang: crate::lang::Lang::Ko });
        s.provider = "codex".into();
        assert_eq!(Engine::from_settings(&s).model_label(), "default");
        s.codex_model = Some("gpt-x".into());
        assert_eq!(Engine::from_settings(&s), Engine { provider: Provider::Codex, model: Some("gpt-x".into()), lang: crate::lang::Lang::Ko });
    }

    #[test]
    fn an_engine_takes_the_language_from_settings() {
        let s = Settings { language: "en".into(), ..Default::default() };
        assert_eq!(Engine::from_settings(&s).lang, crate::lang::Lang::En);
        let s = Settings { language: "ko".into(), ..Default::default() };
        assert_eq!(Engine::from_settings(&s).lang, crate::lang::Lang::Ko);
    }
}
