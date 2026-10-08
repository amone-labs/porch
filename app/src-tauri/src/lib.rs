//! Tauri shell: exposes porch-core to the React UI, keeps the menubar count
//! current, places the popover, and runs the daily auto-summary. No product
//! logic lives here.

mod summary_activity;
mod tray_animation;

static SUMMARY_ACTIVITY: summary_activity::Activity = summary_activity::Activity::new();

use porch_core::lang::Lang;
use porch_core::writer::Engine;
use porch_core::{activity, board, cli_link, export, goals, health, insight, insight_eval, install, limits, mirror, open, paths, schedule, settings, statusline, suggest, summary, time};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};

const TRAY_ID: &str = "main";
/// How often the menubar count is refreshed.
const TRAY_REFRESH: Duration = Duration::from_secs(3);
const POPOVER_W: f64 = 380.0;
const POPOVER_H: f64 = 520.0;

/// The shell's language, kept for the tray tooltip thread so it does not read
/// the settings file every 3 s. Set at setup and by `save_language`.
static LANG_EN: AtomicBool = AtomicBool::new(false);

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// ---------- words ----------

/// What the app shell says, in `lang`. Screens have their own dictionary (app/src/i18n).
struct Shell;

impl Shell {
    fn menu(lang: Lang) -> [&'static str; 3] {
        match lang {
            Lang::Ko => ["앱 열기", "오늘 요약", "종료"],
            Lang::En => ["Open porch", "Today's summary", "Quit"],
        }
    }

    fn tooltip(n: usize, lang: Lang) -> String {
        match lang {
            Lang::Ko => format!("나를 기다리는 세션 {n}"),
            Lang::En => format!("Waiting on you {n}"),
        }
    }

    /// "10월 3일 오늘 요약" / "Oct 3 · Today's summary".
    fn day_title(date: &str, today: &str, yesterday: &str, lang: Lang) -> String {
        let when = match (lang, date == today, date == yesterday) {
            (Lang::Ko, true, _) => "오늘 요약",
            (Lang::Ko, _, true) => "어제 요약",
            (Lang::Ko, _, _) => "요약",
            (Lang::En, true, _) => "Today's summary",
            (Lang::En, _, true) => "Yesterday's summary",
            (Lang::En, _, _) => "Summary",
        };
        match lang {
            Lang::Ko => format!("{} {when}", time::short_date(date, lang)),
            Lang::En => format!("{} · {when}", time::short_date(date, lang)),
        }
    }

    fn week_title(monday: &str, last_week: bool, lang: Lang) -> String {
        match (lang, last_week) {
            (Lang::Ko, true) => "지난주 요약".to_owned(),
            (Lang::Ko, false) => format!("{} 주간 요약", time::short_date(monday, lang)),
            (Lang::En, true) => "Last week's summary".to_owned(),
            (Lang::En, false) => format!("Week of {} · Summary", time::short_date(monday, lang)),
        }
    }

    fn month_title(month: &str, lang: Lang) -> String {
        match lang {
            Lang::Ko => format!("{}월 요약", month[5..].trim_start_matches('0')),
            Lang::En => format!("{} · Summary", time::month_title(month, lang)),
        }
    }

    fn failed(lang: Lang) -> &'static str {
        match lang {
            Lang::Ko => "요약을 만들지 못했어요",
            Lang::En => "Couldn't write the summary",
        }
    }

    fn test_notification(lang: Lang) -> (&'static str, &'static str) {
        match lang {
            Lang::Ko => ("porch 알림 시험", "요약이 끝나면 이렇게 알려 드립니다."),
            Lang::En => ("porch test notification", "This is how porch tells you a summary is ready."),
        }
    }
}

// ---------- board ----------

#[tauri::command]
async fn board_now() -> Result<board::Board, String> {
    blocking(|| Ok(board::build(time::now_ms(), &board::Sources::default()))).await
}

/// Bring a session's terminal tab to the front (ADR 0013). Errors are codes the screen words itself.
#[tauri::command]
async fn focus_session(term_program: Option<String>, term_pane: Option<String>) -> Result<(), String> {
    blocking(move || {
        let t = porch_core::focus::target(term_program.as_deref(), term_pane.as_deref()).ok_or_else(|| "not_focusable".to_owned())?;
        porch_core::focus::bring_to_front(&t).map_err(|e| e.code().to_owned())
    })
    .await
}

// ---------- summaries ----------

#[derive(Serialize)]
struct Today {
    date: String,
    week_start: String,
}

#[tauri::command]
fn today() -> Today {
    let offset = time::local_offset_secs();
    let date = time::local_date(time::now_ms(), offset);
    let week_start = summary::week_start(&date, offset).unwrap_or_else(|| date.clone());
    Today { date, week_start }
}

/// `mode`: "records" (numbers only, no model call), "generate" (use the
/// saved summary or write one), "refresh" (write a new one).
fn flags(mode: &str) -> Result<(bool, bool), String> {
    match mode {
        "records" => Ok((false, true)),
        "generate" => Ok((false, false)),
        "refresh" => Ok((true, false)),
        other => Err(format!("unknown mode {other}")),
    }
}

#[tauri::command]
async fn day_report(app: AppHandle, date: String, mode: String) -> Result<summary::Report, String> {
    let (refresh, no_llm) = flags(&mode)?;
    blocking(move || {
        let _activity = (!no_llm).then(|| SUMMARY_ACTIVITY.start());
        let offset = time::local_offset_secs();
        let r = summary::build_day(&summary::DayOpts { date: date.clone(), engine: Engine::from_settings(&settings::load()), refresh, no_llm }, offset);
        if !no_llm {
            let (headline, error) = outcome(&r, |r| r.summary_error.clone(), |r| r.summary.as_ref().map(|s| s.headline.clone()));
            let today = time::local_date(time::now_ms(), offset);
            let title = Shell::day_title(&date, &today, "", Lang::current());
            tell_if_away(&app, title, headline, error, mirror::Kind::Daily, &date);
        }
        r
    })
    .await
}

#[tauri::command]
async fn insight_week(date: String) -> Result<insight::InsightView, String> {
    blocking(move || insight::view(&date, time::local_offset_secs(), time::now_ms(), Lang::current())).await
}

/// Write (again) the evaluation of a finished week from the Insights screen.
#[tauri::command]
async fn make_insight(date: String) -> Result<insight::InsightView, String> {
    blocking(move || {
        let _activity = SUMMARY_ACTIVITY.start();
        let offset = time::local_offset_secs();
        let s = settings::load();
        insight_eval::evaluate(&insight_eval::EvalOpts { any_date: date.clone(), engine: Engine::from_settings(&s), on: s.insight_eval }, offset, time::now_ms())?;
        insight::view(&date, offset, time::now_ms(), Lang::current())
    })
    .await
}

#[tauri::command]
async fn set_goal(week: String, key: String) -> Result<insight::InsightView, String> {
    blocking(move || {
        let offset = time::local_offset_secs();
        goals::set_goal(&week, &key, offset, time::now_ms(), Lang::current())?;
        insight::view(&week, offset, time::now_ms(), Lang::current())
    })
    .await
}

#[tauri::command]
async fn clear_goal(week: String) -> Result<insight::InsightView, String> {
    blocking(move || {
        let offset = time::local_offset_secs();
        goals::clear_goal(&week, offset, Lang::current())?;
        insight::view(&week, offset, time::now_ms(), Lang::current())
    })
    .await
}

/// "Mark as wrong" on one item of a week's evaluation.
#[tauri::command]
async fn insight_feedback(week: String, item: String) -> Result<insight::InsightView, String> {
    blocking(move || {
        let offset = time::local_offset_secs();
        insight_eval::dispute(&week, &item, offset, Lang::current())?;
        insight::view(&week, offset, time::now_ms(), Lang::current())
    })
    .await
}

/// When the newest evaluation was written, for the sidebar dot; None while evaluation is off.
#[tauri::command]
async fn insight_latest() -> Result<Option<u64>, String> {
    blocking(|| Ok(if settings::load().insight_eval { insight_eval::latest_evaluated_at() } else { None })).await
}

/// Whether the weekly insight evaluation runs (ADR 0012).
#[tauri::command]
async fn save_insight_eval(on: bool) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        s.insight_eval = on;
        settings::save(&s)?;
        Ok(settings_view())
    })
    .await
}

/// The weekly summary's notification line: the headline, then the week's checkpoint count.
fn week_headline(headline: Option<String>, week_start: &str, offset: i64) -> Option<String> {
    let n = insight::observe_week(week_start, offset, time::now_ms(), Lang::current()).map(|o| insight::checkpoints(&o, Lang::current()).len()).unwrap_or(0);
    headline.map(|h| insight::week_notice(&h, n, Lang::current()))
}

#[tauri::command]
async fn week_report(app: AppHandle, date: String, mode: String) -> Result<summary::WeekReport, String> {
    let (refresh, no_llm) = flags(&mode)?;
    blocking(move || {
        let _activity = (!no_llm).then(|| SUMMARY_ACTIVITY.start());
        let offset = time::local_offset_secs();
        let r = summary::build_week(&summary::WeekOpts { any_date: date, engine: Engine::from_settings(&settings::load()), refresh, no_llm }, offset);
        if let (false, Ok(w)) = (no_llm, &r) {
            let key = w.week_start.clone();
            let (headline, error) = outcome(&r, |r| r.summary_error.clone(), |r| r.summary.as_ref().map(|s| s.headline.clone()));
            let headline = week_headline(headline, &key, offset);
            tell_if_away(&app, Shell::week_title(&key, false, Lang::current()), headline, error, mirror::Kind::Weekly, &key);
        }
        r
    })
    .await
}

#[tauri::command]
async fn month_report(app: AppHandle, date: String, mode: String) -> Result<summary::MonthReport, String> {
    let (refresh, no_llm) = flags(&mode)?;
    blocking(move || {
        let _activity = (!no_llm).then(|| SUMMARY_ACTIVITY.start());
        let offset = time::local_offset_secs();
        let r = summary::build_month(&summary::MonthOpts { any_date: date, engine: Engine::from_settings(&settings::load()), refresh, no_llm }, offset);
        if let (false, Ok(m)) = (no_llm, &r) {
            let key = m.month.clone();
            let (headline, error) = outcome(&r, |r| r.summary_error.clone(), |r| r.summary.as_ref().map(|s| s.headline.clone()));
            tell_if_away(&app, Shell::month_title(&key, Lang::current()), headline, error, mirror::Kind::Monthly, &key);
        }
        r
    })
    .await
}

#[tauri::command]
async fn list_reports() -> Result<summary::Listing, String> {
    blocking(|| Ok(summary::list_reports())).await
}

/// Sessions waiting on the person, for the sidebar count.
#[tauri::command]
async fn waiting() -> Result<usize, String> {
    blocking(|| Ok(waiting_count())).await
}

// ---------- open work ----------

/// Everything written down as open, closed items included (the screen filters).
#[tauri::command]
async fn open_items() -> Result<Vec<open::OpenItem>, String> {
    blocking(|| Ok(open::load(summary::saved_days))).await
}

#[tauri::command]
async fn close_open_item(id: String) -> Result<Vec<open::OpenItem>, String> {
    blocking(move || {
        let mut items = open::load(summary::saved_days);
        open::close(&mut items, &id, time::local_offset_secs())?;
        Ok(items)
    })
    .await
}

// ---------- export ----------

/// File stem for an exported report: "Porch-2026-09-29", "Porch-week-2026-09-21".
fn export_stem(kind: &str, date: &str) -> Result<String, String> {
    match kind {
        "day" => Ok(format!("Porch-{date}")),
        "week" => {
            let monday = summary::week_start(date, time::local_offset_secs()).ok_or_else(|| time::bad_date(Lang::current()))?;
            Ok(format!("Porch-week-{monday}"))
        }
        "month" => Ok(format!("Porch-month-{}", date.get(..7).unwrap_or(date))),
        other => Err(format!("unknown report kind {other}")),
    }
}

/// Write the report as Markdown into Downloads and return where it went.
#[tauri::command]
async fn export_markdown(kind: String, date: String) -> Result<String, String> {
    blocking(move || {
        let offset = time::local_offset_secs();
        let engine = Engine::default();
        let text = match kind.as_str() {
            "day" => {
                let r = summary::build_day(&summary::DayOpts { date: date.clone(), engine: engine.clone(), refresh: false, no_llm: true }, offset)?;
                export::day_markdown(&r, r.lang_or(Lang::current()))
            }
            "week" => {
                let r = summary::build_week(&summary::WeekOpts { any_date: date.clone(), engine: engine.clone(), refresh: false, no_llm: true }, offset)?;
                export::week_markdown(&r, r.lang_or(Lang::current()))
            }
            "month" => {
                let r = summary::build_month(&summary::MonthOpts { any_date: date.clone(), engine: engine.clone(), refresh: false, no_llm: true }, offset)?;
                export::month_markdown(&r, r.lang_or(Lang::current()))
            }
            other => return Err(format!("unknown report kind {other}")),
        };
        let path = export::free_path(&paths::downloads_dir(), &export_stem(&kind, &date)?, "md");
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

/// Print the main window's page (its print stylesheet lays the report out on
/// paper) straight to a PDF in Downloads, with no print panel.
#[tauri::command]
async fn export_pdf(app: AppHandle, kind: String, date: String) -> Result<String, String> {
    let path = export::free_path(&paths::downloads_dir(), &export_stem(&kind, &date)?, "pdf");
    let w = app.get_webview_window("main").ok_or("main window is gone")?;
    let target = path.clone();
    #[cfg(target_os = "macos")]
    blocking(move || print_main_to_pdf(&w, &target)).await?;
    #[cfg(not(target_os = "macos"))]
    return Err("PDF export is macOS only".to_owned());
    Ok(path.to_string_lossy().into_owned())
}

/// The latest installer, opened in the browser: the way out when a self-update
/// fails. Only this one address, taken from the updater endpoint.
#[tauri::command]
fn open_download(app: AppHandle) -> Result<(), String> {
    let endpoint = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u["endpoints"][0].as_str())
        .ok_or("no update address")?
        .to_owned();
    let url = endpoint.rsplit_once('/').map(|(base, _)| format!("{base}/Porch.dmg")).ok_or("bad update address")?;
    std::process::Command::new("open").arg(url).status().map(|_| ()).map_err(|e| e.to_string())
}

/// Show a file in Finder.
#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    std::process::Command::new("open").arg("-R").arg(path).status().map(|_| ()).map_err(|e| e.to_string())
}

// AppKit's print system with the job disposition set to "save": WebKit lays
// the page out with its print media rules and AppKit writes the PDF itself,
// paginated, fonts embedded. It must run as a window-modal (asynchronous)
// operation: WKWebView gathers pages from its web process, and the blocking
// `runOperation` starves that exchange (it produced 700k blank pages).
#[cfg(target_os = "macos")]
mod pdf {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool, NSObject};
    use objc2::{class, declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
    use objc2_foundation::{NSRect, NSString, NSURL};
    use std::ffi::c_void;
    use std::path::Path;
    use std::sync::Mutex;

    pub type Done = Box<dyn FnOnce(bool) + Send>;

    declare_class!(
        struct PrintDone;

        // SAFETY: NSObject has no subclassing requirements; no Drop impl.
        unsafe impl ClassType for PrintDone {
            type Super = NSObject;
            type Mutability = mutability::InteriorMutable;
            const NAME: &'static str = "PorchPrintDone";
        }

        impl DeclaredClass for PrintDone {
            type Ivars = Mutex<Option<Done>>;
        }

        unsafe impl PrintDone {
            #[method(printOperationDidRun:success:contextInfo:)]
            fn did_run(&self, _op: *mut AnyObject, success: Bool, _ctx: *mut c_void) {
                if let Some(done) = self.ivars().lock().ok().and_then(|mut d| d.take()) {
                    done(success.as_bool());
                }
            }
        }
    );

    impl PrintDone {
        fn new(done: Done) -> Retained<Self> {
            let this = Self::alloc().set_ivars(Mutex::new(Some(done)));
            unsafe { msg_send_id![super(this), init] }
        }
    }

    /// Start printing `webview` to `path`; `done` runs on the main thread when AppKit finishes.
    /// Must be called on the main thread.
    pub unsafe fn start(webview: *mut AnyObject, window: *mut AnyObject, path: &Path, done: Done) -> Result<(), String> {
        let shared: Retained<AnyObject> = msg_send_id![class!(NSPrintInfo), sharedPrintInfo];
        let info: Retained<AnyObject> = msg_send_id![&shared, copy];
        let dict: *mut AnyObject = msg_send![&info, dictionary];
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let _: () = msg_send![dict, setObject: &*url, forKey: &*NSString::from_str("NSJobSavingURL")];
        let _: () = msg_send![&info, setJobDisposition: &*NSString::from_str("NSPrintSaveJob")];
        // NSPrintingPaginationMode (NSUInteger): 0 automatic, 1 fit.
        let _: () = msg_send![&info, setHorizontalPagination: 1usize];
        let _: () = msg_send![&info, setVerticalPagination: 0usize];
        let _: () = msg_send![&info, setVerticallyCentered: Bool::NO];
        let _: () = msg_send![&info, setTopMargin: 40.0f64];
        let _: () = msg_send![&info, setBottomMargin: 40.0f64];
        let _: () = msg_send![&info, setLeftMargin: 44.0f64];
        let _: () = msg_send![&info, setRightMargin: 44.0f64];

        let op: *mut AnyObject = msg_send![webview, printOperationWithPrintInfo: &*info];
        if op.is_null() {
            return Err(match super::Lang::current() {
                super::Lang::Ko => "인쇄를 시작하지 못했습니다".to_owned(),
                super::Lang::En => "Couldn't start printing".to_owned(),
            });
        }
        let _: () = msg_send![op, setShowsPrintPanel: Bool::NO];
        let _: () = msg_send![op, setShowsProgressPanel: Bool::NO];
        // WebKit hands back a print view with a zero frame, which prints nothing.
        let view: *mut AnyObject = msg_send![op, view];
        let bounds: NSRect = msg_send![webview, bounds];
        let _: () = msg_send![view, setFrame: bounds];

        let delegate = PrintDone::new(done);
        let _: () = msg_send![
            op,
            runOperationModalForWindow: window,
            delegate: &*delegate,
            didRunSelector: sel!(printOperationDidRun:success:contextInfo:),
            contextInfo: std::ptr::null_mut::<c_void>()
        ];
        // AppKit does not retain the delegate; it has to outlive the operation.
        // One small object per export, so it is simply kept.
        std::mem::forget(delegate);
        Ok(())
    }
}

/// Print the main window to `path` and wait for AppKit to finish writing it.
#[cfg(target_os = "macos")]
fn print_main_to_pdf(w: &tauri::WebviewWindow, path: &std::path::Path) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let target = path.to_path_buf();
    w.with_webview(move |wv| {
        let tx2 = tx.clone();
        let window = wv.ns_window().cast();
        let started = unsafe {
            pdf::start(
                wv.inner().cast(),
                window,
                &target,
                Box::new(move |ok| {
                    let _ = tx2.send(if ok {
                        Ok(())
                    } else {
                        Err(match Lang::current() {
                            Lang::Ko => "PDF를 만들지 못했습니다".to_owned(),
                            Lang::En => "Couldn't make the PDF".to_owned(),
                        })
                    });
                }),
            )
        };
        if let Err(e) = started {
            let _ = tx.send(Err(e));
        }
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(60)).map_err(|_| match Lang::current() {
        Lang::Ko => "PDF가 제때 만들어지지 않았습니다".to_owned(),
        Lang::En => "The PDF took too long to make".to_owned(),
    })??;
    if path.exists() {
        Ok(())
    } else {
        Err(match Lang::current() {
            Lang::Ko => "PDF를 만들지 못했습니다".to_owned(),
            Lang::En => "Couldn't make the PDF".to_owned(),
        })
    }
}

// ---------- projects ----------

#[tauri::command]
async fn projects() -> Result<Vec<summary::ProjectOverview>, String> {
    blocking(|| Ok(summary::projects())).await
}

#[tauri::command]
async fn project_days(root: String) -> Result<Vec<summary::ProjectDay>, String> {
    blocking(move || Ok(summary::project_days(&root))).await
}

#[tauri::command]
async fn backfill(days: u64) -> Result<usize, String> {
    blocking(move || summary::backfill(days.min(90), time::local_offset_secs())).await
}

#[tauri::command]
async fn projects_health() -> Result<Vec<health::ProjectHealth>, String> {
    blocking(|| Ok(health::all(health::DEFAULT_DAYS, time::local_offset_secs()))).await
}

#[tauri::command]
async fn classify_blockers() -> Result<usize, String> {
    blocking(|| summary::classify_old_blockers(&Engine::from_settings(&settings::load()))).await
}

// ---------- suggestions ----------

#[tauri::command]
async fn suggestions() -> Result<Vec<suggest::SuggestionView>, String> {
    blocking(|| suggest::list(time::local_offset_secs())).await
}

#[tauri::command]
async fn make_suggestions() -> Result<suggest::MakeResult, String> {
    blocking(|| suggest::make(&Engine::from_settings(&settings::load()), time::local_offset_secs())).await
}

#[tauri::command]
async fn set_suggestion_status(id: String, status: String) -> Result<(), String> {
    blocking(move || {
        let status = match status.as_str() {
            "new" => suggest::Status::New,
            "applied" => suggest::Status::Applied,
            "dismissed" => suggest::Status::Dismissed,
            other => return Err(format!("unknown status {other}")),
        };
        let offset = time::local_offset_secs();
        suggest::set_status(&suggest::store_path(), &id, status, &time::local_date(time::now_ms(), offset))
    })
    .await
}

// ---------- settings ----------

/// What this Mac has, for the first-run screen and the summary buttons.
#[derive(Serialize)]
struct Environment {
    claude_bin: Option<String>,
    codex_bin: Option<String>,
    claude_data: bool,
    codex_homes: Vec<String>,
    hooks_installed: bool,
    onboarded: bool,
}

#[tauri::command]
async fn environment() -> Result<Environment, String> {
    blocking(|| {
        Ok(Environment {
            claude_bin: porch_core::agents::claude_bin().map(|p| p.to_string_lossy().into_owned()),
            codex_bin: porch_core::agents::codex_bin().map(|p| p.to_string_lossy().into_owned()),
            claude_data: paths::default_claude_dir().join("projects").is_dir(),
            codex_homes: paths::codex_homes().iter().map(|p| p.to_string_lossy().into_owned()).collect(),
            hooks_installed: install::status(&paths::installed_hook_bin()).iter().all(|t| t.state == "installed"),
            onboarded: settings::load().onboarded,
        })
    })
    .await
}

#[tauri::command]
async fn finish_onboarding() -> Result<(), String> {
    blocking(|| {
        let mut s = settings::load();
        s.onboarded = true;
        settings::save(&s)
    })
    .await
}

#[derive(Serialize)]
struct SettingsView {
    settings: settings::Settings,
    /// The language screens use now: `settings.language` resolved against macOS.
    resolved_language: Lang,
    models: Vec<&'static str>,
    excluded: Vec<String>,
    /// Both agents, each with its hooks files, present on this Mac or not.
    agents: Vec<install::AgentStatus>,
    hook_binary: String,
    hook_binary_present: bool,
    /// Claude's usage limits through the status line: "off" | "on" | "disconnected".
    statusline: statusline::Status,
    /// Whether a status line was set before porch took the slot.
    statusline_had_original: bool,
    statusline_show_line: bool,
    mirror: mirror::Status,
    /// Projects with a status line of their own (they outrank the user setting).
    statusline_projects: Vec<statusline::ProjectLine>,
    /// The `porch` command in /usr/local/bin; "missing" when the app carries no CLI.
    cli_link: cli_link::Status,
    cli_link_path: &'static str,
}

#[tauri::command]
async fn mirror_vaults() -> Result<mirror::Vaults, String> {
    blocking(|| Ok(mirror::read_vaults(&mirror::obsidian_registry()))).await
}

#[tauri::command]
async fn save_mirror(app: AppHandle, dir: Option<String>, vault: Option<String>, daily: String, weekly: String, monthly: String, notify: bool) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        let before = s.mirror_dir.clone();
        s.mirror_dir = dir.filter(|d| !d.is_empty());
        // A folder picked by hand that lies in a vault still gets Obsidian links.
        let found = || {
            let d = s.mirror_dir.as_ref()?;
            mirror::vault_for(std::path::Path::new(d), &mirror::read_vaults(&mirror::obsidian_registry()).vaults).map(|v| v.id)
        };
        s.obsidian_vault = vault.or_else(found).filter(|_| s.mirror_dir.is_some());
        if s.obsidian_vault.is_none() && s.notify_open == "obsidian" {
            s.notify_open = "porch".into();
        }
        s.mirror_daily = daily;
        s.mirror_weekly = weekly;
        s.mirror_monthly = monthly;
        s.notify = notify;
        settings::save(&s)?;
        if s.mirror_dir.is_none() {
            mirror::clear_last_error(&mirror::state_path());
        }
        // A newly chosen folder starts with every saved summary.
        if s.mirror_dir.is_some() && s.mirror_dir != before {
            mirror_all_with_progress(&app);
        }
        Ok(settings_view())
    })
    .await
}

#[tauri::command]
async fn save_notify(notify: bool, open: String) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        s.notify = notify;
        s.notify_open = open;
        settings::save(&s)?;
        Ok(settings_view())
    })
    .await
}

/// Write every saved summary into the folder again (notes a person edited stay).
#[tauri::command]
async fn resave_mirror(app: AppHandle) -> Result<SettingsView, String> {
    blocking(move || {
        mirror_all_with_progress(&app);
        Ok(settings_view())
    })
    .await
}

#[derive(Clone, Serialize)]
struct MirrorProgress {
    done: usize,
    total: usize,
}

/// Mirror every saved summary, telling the main window how far along it is.
fn mirror_all_with_progress(app: &AppHandle) {
    mirror::save_all(&mut |done, total| {
        let _ = app.emit_to("main", "mirror-progress", MirrorProgress { done, total });
    });
}

#[tauri::command]
async fn open_vault() -> Result<(), String> {
    blocking(|| {
        let id = settings::load().obsidian_vault.ok_or_else(|| match Lang::current() {
            Lang::Ko => "연결된 볼트가 없습니다".to_owned(),
            Lang::En => "No vault is connected".to_owned(),
        })?;
        std::process::Command::new("open").arg(mirror::vault_uri(&id)).status().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// A notification now, so a person can see that notifications reach them.
/// Clicking it opens today's summary, like the real one.
#[tauri::command]
fn test_notification(app: AppHandle) {
    let today = time::local_date(time::now_ms(), time::local_offset_secs());
    let (title, body) = Shell::test_notification(Lang::current());
    notify_and_remember(&app, title, body, mirror::Kind::Daily, &today);
}

/// macOS Settings, at porch's notification switches.
#[tauri::command]
fn open_notification_settings(app: AppHandle) -> Result<(), String> {
    let url = format!("x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={}", app.config().identifier);
    std::process::Command::new("open").arg(url).status().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_mirror_dir() -> Result<(), String> {
    blocking(|| {
        let dir = settings::load().mirror_dir.ok_or_else(|| match Lang::current() {
            Lang::Ko => "연동 폴더가 없습니다".to_owned(),
            Lang::En => "No notes folder is set".to_owned(),
        })?;
        mirror::ensure_root(std::path::Path::new(&dir))?;
        std::process::Command::new("open").arg(dir).status().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

fn settings_view() -> SettingsView {
    let bin = paths::installed_hook_bin();
    SettingsView {
        settings: settings::load(),
        resolved_language: Lang::current(),
        models: settings::MODELS.to_vec(),
        excluded: settings::excluded(),
        agents: install::agents_status(&bin),
        hook_binary: bin.to_string_lossy().into_owned(),
        hook_binary_present: bin.exists(),
        statusline: statusline::status(),
        statusline_had_original: statusline::load_saved().original.is_object(),
        statusline_show_line: statusline::load_saved().show_line,
        mirror: mirror::status(),
        statusline_projects: statusline::project_lines(time::now_ms()),
        cli_link: bundled_cli().map_or(cli_link::Status::Missing, |c| cli_link::status_at(&cli_link::link_path(), &c)),
        cli_link_path: cli_link::LINK,
    }
}

#[tauri::command]
async fn get_settings() -> Result<SettingsView, String> {
    blocking(|| Ok(settings_view())).await
}

/// The language the app writes in now ("ko" | "en"), read before every window's
/// first paint. Cheap: one small settings file.
#[tauri::command]
fn get_language() -> &'static str {
    Lang::current().code()
}

/// Save the language and tell every window and the tray at once.
#[tauri::command]
async fn save_language(app: AppHandle, language: String) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        s.language = language;
        settings::save(&s)?;
        let lang = Lang::current();
        LANG_EN.store(lang == Lang::En, Ordering::Relaxed);
        let _ = app.emit("language-changed", lang.code());
        let _ = rebuild_tray_menu(&app, lang);
        Ok(settings_view())
    })
    .await
}

/// When each agent last reported an event, at most a week back. Cheap: reads day files from their end.
#[tauri::command]
async fn last_events() -> Result<Vec<activity::LastSeen>, String> {
    blocking(|| Ok(activity::last_seen(7, time::now_ms()))).await
}

/// Models Codex lists, its default first; empty when it cannot say.
#[tauri::command]
async fn codex_models() -> Result<Vec<porch_core::writer::CodexModel>, String> {
    blocking(|| Ok(porch_core::writer::codex_models())).await
}

#[tauri::command]
async fn save_settings(provider: String, model: String, codex_model: Option<String>, auto_summary_at: Option<String>) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        s.provider = provider;
        s.model = model;
        s.codex_model = codex_model.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty());
        s.auto_summary_at = auto_summary_at.filter(|t| !t.is_empty());
        settings::save(&s)?;
        Ok(settings_view())
    })
    .await
}

/// Monthly budget in list-price USD (None clears it) and whether cost is shown.
#[tauri::command]
async fn save_usage_settings(monthly_budget: Option<f64>, show_cost: bool) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        s.monthly_budget = monthly_budget;
        s.show_cost = show_cost;
        settings::save(&s)?;
        Ok(settings_view())
    })
    .await
}

/// Whether the main window sends usage events (ADR 0006).
#[tauri::command]
async fn save_analytics(on: bool) -> Result<SettingsView, String> {
    blocking(move || {
        let mut s = settings::load();
        s.analytics = on;
        settings::save(&s)?;
        Ok(settings_view())
    })
    .await
}

/// Turn Claude's usage limits (through the status line) on or off.
#[tauri::command]
async fn set_statusline(on: bool, show_line: bool) -> Result<SettingsView, String> {
    blocking(move || {
        if on {
            statusline::enable(&paths::installed_hook_bin(), show_line)?;
        } else {
            statusline::disable()?;
        }
        Ok(settings_view())
    })
    .await
}

/// Wrap (or unwrap) projects' own status lines; `roots` empty means every one listed.
#[tauri::command]
async fn set_project_statusline(roots: Vec<String>, on: bool) -> Result<SettingsView, String> {
    blocking(move || {
        let bin = paths::installed_hook_bin();
        let roots = if roots.is_empty() {
            statusline::project_lines(time::now_ms()).into_iter().filter(|p| p.wrapped != on).map(|p| p.root).collect()
        } else {
            roots
        };
        for r in &roots {
            if on {
                statusline::wrap_project(r, &bin)?;
            } else {
                statusline::unwrap_project(r)?;
            }
        }
        Ok(settings_view())
    })
    .await
}

#[tauri::command]
async fn usage_report(days: u64, end: Option<String>) -> Result<summary::UsageReport, String> {
    blocking(move || summary::usage_report(days, end.as_deref(), time::local_offset_secs())).await
}

#[tauri::command]
async fn usage_limits() -> Result<Vec<limits::AccountLimits>, String> {
    blocking(|| Ok(limits::all(time::now_ms()))).await
}

#[tauri::command]
async fn save_excluded(list: Vec<String>) -> Result<SettingsView, String> {
    blocking(move || {
        settings::set_excluded(&list)?;
        Ok(settings_view())
    })
    .await
}

/// The hook binary shipped inside the app bundle (Tauri externalBin puts it
/// next to the app executable).
fn bundled_hook() -> Option<std::path::PathBuf> {
    let p = std::env::current_exe().ok()?.with_file_name("porch-hook");
    p.exists().then_some(p)
}

/// The `porch` CLI shipped inside the app bundle, next to the hook.
fn bundled_cli() -> Option<std::path::PathBuf> {
    let p = std::env::current_exe().ok()?.with_file_name("porch");
    p.exists().then_some(p)
}

/// Run a shell command as administrator; macOS shows its own password prompt.
fn run_as_admin(cmd: &str) -> Result<(), String> {
    let script = format!("do shell script \"{}\" with administrator privileges", cmd.replace('\\', "\\\\").replace('"', "\\\""));
    let out = std::process::Command::new("osascript").arg("-e").arg(script).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    // -128: the person pressed Cancel.
    Err(if err.contains("-128") {
        match Lang::current() {
            Lang::Ko => "취소했습니다.".to_owned(),
            Lang::En => "Cancelled.".to_owned(),
        }
    } else {
        err.trim().to_owned()
    })
}

/// Single-quote a path for the shell.
fn sh_quote(p: &std::path::Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
}

/// Adds or removes the `porch` command in /usr/local/bin, asking for an
/// administrator only when the folder is not writable.
#[tauri::command]
async fn set_cli_link(on: bool) -> Result<SettingsView, String> {
    blocking(move || {
        let cli = bundled_cli().ok_or_else(|| match Lang::current() {
            Lang::Ko => "앱 안에서 porch 명령을 찾지 못했습니다. 앱을 다시 내려받아 주세요.".to_owned(),
            Lang::En => "The porch command is missing from the app. Download the app again.".to_owned(),
        })?;
        let link = cli_link::link_path();
        let res = if on { cli_link::link_at(&link, &cli) } else { cli_link::unlink_at(&link, &cli) };
        match res {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let (l, c) = (sh_quote(&link), sh_quote(&cli));
                let dir = sh_quote(link.parent().unwrap_or(std::path::Path::new("/usr/local/bin")));
                // Ownership was checked above: a link we may replace or remove, never someone else's file.
                run_as_admin(&if on { format!("mkdir -p {dir} && rm -f {l} && ln -s {c} {l}") } else { format!("rm -f {l}") })?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(match Lang::current() {
                    Lang::Ko => format!("{}에 다른 porch가 있어 건드리지 않았습니다.", cli_link::LINK),
                    Lang::En => format!("A different file or command already exists at {}. Porch left it unchanged.", cli_link::LINK),
                });
            }
            Err(e) => return Err(e.to_string()),
        }
        Ok(settings_view())
    })
    .await
}

/// Installs or removes our hooks for one agent, or every agent when `agent` is None.
#[tauri::command]
async fn set_hooks(install_them: bool, agent: Option<String>) -> Result<SettingsView, String> {
    blocking(move || {
        let only = match agent.as_deref() {
            None => None,
            Some(name) => Some(install::Agent::from_name(name).ok_or_else(|| match Lang::current() {
                Lang::Ko => format!("알 수 없는 에이전트: {name}"),
                Lang::En => format!("Unknown agent: {name}"),
            })?),
        };
        let bin = paths::installed_hook_bin();
        if install_them {
            if let Some(src) = bundled_hook() {
                install::stage_hook_binary(&src).map_err(|e| e.to_string())?;
            }
            if !bin.exists() {
                return Err(match Lang::current() {
                    Lang::Ko => "앱 안에서 훅 프로그램을 찾지 못했습니다. 앱을 다시 내려받아 주세요.".to_owned(),
                    Lang::En => "The hook program is missing from the app. Download the app again.".to_owned(),
                });
            }
        }
        for (agent, file) in install::targets_for(only) {
            if !install_them && !file.exists() {
                continue;
            }
            let action = if install_them { install::Action::Install { agent, bin: &bin } } else { install::Action::Uninstall };
            install::apply_to_file(&file, action, false).map_err(|e| e.to_string())?;
        }
        Ok(settings_view())
    })
    .await
}

// ---------- windows ----------

fn show_main(app: &AppHandle, screen: Option<&str>) {
    if let Some(p) = app.get_webview_window("popover") {
        let _ = p.hide();
    }
    if let Some(w) = app.get_webview_window("main") {
        // A visible main window participates in Dock and Command-Tab.
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        if let Some(s) = screen {
            let _ = w.emit("navigate", s);
        }
    }
}

#[tauri::command]
fn open_main(app: AppHandle, screen: Option<String>) {
    show_main(&app, screen.as_deref());
}

// Where a menubar click landed, in Cocoa's global coordinates (points, origin
// at the bottom-left of the primary display). The tray event carries a
// position too, but in pixels of the display that holds the icon, which
// cannot be mapped back reliably once more than one display is attached.
#[cfg(target_os = "macos")]
fn click_point() -> (f64, f64) {
    let p = unsafe { objc2_app_kit::NSEvent::mouseLocation() };
    (p.x, p.y)
}

/// Bottom-left origin of the popover in Cocoa points: centered under the
/// click, kept 8pt inside the sides of the work area, its top just under the
/// menu bar. `work` is the display's visible frame as (x, y, width, height).
#[cfg(any(target_os = "macos", test))]
fn popover_origin(click_x: f64, work: (f64, f64, f64, f64)) -> (f64, f64) {
    const EDGE: f64 = 8.0;
    const UNDER_MENU_BAR: f64 = 4.0;
    let (wx, wy, ww, wh) = work;
    let min_x = wx + EDGE;
    let max_x = (wx + ww - POPOVER_W - EDGE).max(min_x);
    ((click_x - POPOVER_W / 2.0).clamp(min_x, max_x), wy + wh - POPOVER_H - UNDER_MENU_BAR)
}

// Hang the popover from the top of the work area of the display that was
// clicked. The click, the screen frames and the window frame are all Cocoa
// points; nothing here is in Tauri's physical pixels, so displays with
// different scale factors never mix units.
//
// The display is found by first putting the window under the click and asking
// AppKit which screen it is on. `NSScreen::screens` is not walked: on current
// macOS it can be a Swift-backed array whose `count` is typed differently from
// objc2's declaration, which panics in debug builds.
#[cfg(target_os = "macos")]
fn place_popover(w: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
    let Ok(ptr) = w.ns_window() else { return };
    let window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    // Tray callbacks run on the main thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let (cx, cy) = click_point();
    let size = NSSize::new(POPOVER_W, POPOVER_H);
    window.setFrame_display(NSRect::new(NSPoint::new(cx - POPOVER_W / 2.0, cy - POPOVER_H), size), false);
    let holds_click = |s: &NSScreen| {
        let f = s.frame();
        (f.origin.x..=f.origin.x + f.size.width).contains(&cx) && (f.origin.y..=f.origin.y + f.size.height).contains(&cy)
    };
    let work = window
        .screen()
        .filter(|s| holds_click(s))
        .or_else(|| NSScreen::mainScreen(mtm))
        .map(|s| s.visibleFrame());
    let Some(v) = work else { return };
    let (x, y) = popover_origin(cx, (v.origin.x, v.origin.y, v.size.width, v.size.height));
    window.setFrame_display(NSRect::new(NSPoint::new(x, y), size), false);
}

fn toggle_popover(app: &AppHandle) {
    let Some(w) = app.get_webview_window("popover") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    #[cfg(target_os = "macos")]
    place_popover(&w);
    let _ = w.show();
    let _ = w.set_focus();
    let _ = w.emit("popover-shown", ());
}

fn build_popover(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "popover", WebviewUrl::default())
        .title("porch")
        .inner_size(POPOVER_W, POPOVER_H)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .shadow(true)
        .build()?;
    Ok(())
}

// ---------- tray ----------

fn waiting_count() -> usize {
    let b = board::build(time::now_ms(), &board::Sources { with_git: false, with_detail: false, ..Default::default() });
    b.worktrees.iter().flat_map(|w| &w.entries).filter(|e| e.session.needs_you()).count()
}

fn tray_menu(app: &AppHandle, lang: Lang) -> tauri::Result<Menu<Wry>> {
    let [open, today, quit] = Shell::menu(lang);
    let open = MenuItem::with_id(app, "open", open, true, None::<&str>)?;
    let today = MenuItem::with_id(app, "today", today, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &today, &PredefinedMenuItem::separator(app)?, &quit])
}

/// Put the tray menu back, in `lang`, after the language changed.
fn rebuild_tray_menu(app: &AppHandle, lang: Lang) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(tray_menu(app, lang)?))?;
    }
    Ok(())
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let lang = Lang::current();
    LANG_EN.store(lang == Lang::En, Ordering::Relaxed);
    let menu = tray_menu(app, lang)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Porch")
        .menu(&menu)
        // Left click opens the popover; right click shows the menu.
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_popover(tray.app_handle());
            }
        })
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app, Some("summary")),
            "today" => show_main(app, Some("today")),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    tray_animation::start(app, TRAY_ID)?;
    let handle = app.clone();
    std::thread::spawn(move || loop {
        let n = waiting_count();
        if let Some(tray) = handle.tray_by_id(TRAY_ID) {
            let _ = tray.set_title(if n > 0 { Some(n.to_string()) } else { None });
            let lang = if LANG_EN.load(Ordering::Relaxed) { Lang::En } else { Lang::Ko };
            let _ = tray.set_tooltip(Some(Shell::tooltip(n, lang)));
        }
        std::thread::sleep(TRAY_REFRESH);
    });
    Ok(())
}

// ---------- daily auto-summary ----------

/// The main window turns this into `summary_created` (ADR 0006); the error
/// stays on this Mac, only its kind goes out.
#[derive(Clone, Serialize)]
struct AutoSummaryDone {
    kind: &'static str,
    ok: bool,
    duration_ms: u64,
    writer: &'static str,
    error: Option<String>,
}

/// At the configured local time, write the owed day, week and month summaries
/// (latest of each, catching up after sleep). A period tried once is not tried
/// again while the app runs, whatever came of it.
fn start_auto_summary(app: AppHandle) {
    std::thread::spawn(move || {
        let mut tried: std::collections::HashSet<String> = std::collections::HashSet::new();
        loop {
            std::thread::sleep(Duration::from_secs(30));
            let s = settings::load();
            let Some(at) = s.auto_summary_at.clone() else { continue };
            let offset = time::local_offset_secs();
            let saved = |p: &schedule::Period| match p {
                schedule::Period::Day(d) => summary::load_day(d).is_some_and(|r| r.summary.is_some()),
                schedule::Period::Week(m) => summary::load_week(m).is_some_and(|r| r.summary.is_some()),
                schedule::Period::Month(m) => summary::load_month(m).is_some_and(|r| r.summary.is_some()),
            };
            let Some(p) = schedule::due(time::now_ms(), offset, &at, &saved).into_iter().find(|p| !tried.contains(&p.key())) else { continue };
            tried.insert(p.key());
            run_period(&app, &p, &Engine::from_settings(&s), offset, s.notify);
        }
    });
}

/// Write the week's model evaluation (ADR 0012) right after its weekly summary
/// and its notification. A failure stays in the insight file; nothing else is told.
fn evaluate_week(app: &AppHandle, monday: &str, engine: &Engine, offset: i64) {
    let on = settings::load().insight_eval;
    let _ = insight_eval::evaluate(&insight_eval::EvalOpts { any_date: monday.to_owned(), engine: engine.clone(), on }, offset, time::now_ms());
    let _ = app.emit_to("main", "insight-updated", ());
}

fn run_period(app: &AppHandle, p: &schedule::Period, engine: &Engine, offset: i64, notify: bool) {
    let _activity = SUMMARY_ACTIVITY.start();
    let started = time::now_ms();
    let lang = Lang::current();
    let mut evaluate_after: Option<String> = None;
    let (headline, error, kind, key, title) = match p {
        schedule::Period::Day(d) => {
            let r = summary::build_day(&summary::DayOpts { date: d.clone(), engine: engine.clone(), refresh: false, no_llm: false }, offset);
            let error = match &r {
                Ok(r) => r.summary_error.clone(),
                Err(e) => Some(e.clone()),
            };
            let today = time::local_date(time::now_ms(), offset);
            let yesterday = time::add_days(&today, -1).unwrap_or_default();
            let title = Shell::day_title(d, &today, &yesterday, lang);
            (r.ok().and_then(|r| r.summary.map(|s| s.headline)), error, mirror::Kind::Daily, d.clone(), title)
        }
        schedule::Period::Week(m) => {
            let r = summary::build_week(&summary::WeekOpts { any_date: m.clone(), engine: engine.clone(), refresh: false, no_llm: false }, offset);
            let error = match &r {
                Ok(r) => r.summary_error.clone(),
                Err(e) => Some(e.clone()),
            };
            // The evaluation follows a written weekly summary only.
            if r.as_ref().is_ok_and(|w| w.summary.is_some()) {
                evaluate_after = Some(m.clone());
            }
            (week_headline(r.ok().and_then(|r| r.summary.map(|s| s.headline)), m, offset), error, mirror::Kind::Weekly, m.clone(), Shell::week_title(m, true, lang))
        }
        schedule::Period::Month(m) => {
            let r = summary::build_month(&summary::MonthOpts { any_date: format!("{m}-01"), engine: engine.clone(), refresh: false, no_llm: false }, offset);
            let error = match &r {
                Ok(r) => r.summary_error.clone(),
                Err(e) => Some(e.clone()),
            };
            let title = Shell::month_title(m, lang);
            (r.ok().and_then(|r| r.summary.map(|s| s.headline)), error, mirror::Kind::Monthly, m.clone(), title)
        }
    };
    // A period nothing was worked on has no summary to count.
    if headline.is_some() || error.is_some() {
        let done = AutoSummaryDone {
            kind: kind_name(kind),
            ok: error.is_none(),
            duration_ms: time::now_ms().saturating_sub(started),
            writer: engine.provider.id(),
            error: error.clone(),
        };
        let _ = app.emit_to("main", "auto-summary-done", done);
    }
    if notify {
        tell_done(app, title, headline, error, kind, &key);
    }
    // After the notification, under the same activity guard: the menubar icon keeps turning.
    if let Some(m) = evaluate_after {
        evaluate_week(app, &m, engine, offset);
    }
}

/// Closing the window only hides it, so the app runs for days on one
/// `app_opened`. Tell the main window each time the local date turns; it sends
/// `app_active` (ADR 0006).
fn start_day_ping(app: AppHandle) {
    std::thread::spawn(move || {
        let mut day = time::local_date(time::now_ms(), time::local_offset_secs());
        loop {
            std::thread::sleep(Duration::from_secs(60));
            let now = time::local_date(time::now_ms(), time::local_offset_secs());
            if now != day {
                day = now;
                let _ = app.emit_to("main", "day-changed", ());
            }
        }
    });
}

/// The macOS notification for a finished summary: its headline, or that it
/// failed (naming a usage limit, since switching the agent gets it now).
fn tell_done(app: &AppHandle, title: String, headline: Option<String>, error: Option<String>, kind: mirror::Kind, key: &str) {
    let (title, body) = if let Some(e) = error {
        // A usage limit is worth saying: switching the writer in Settings gets the summary now.
        // Its first sentence names the agent ("Codex 사용 한도에 걸렸습니다", "Codex hit its usage limit").
        let body = if porch_core::writer::is_limit(&e) { format!("{title}: {}", e.split(". ").next().unwrap_or(&e)) } else { title };
        (Shell::failed(Lang::current()).to_owned(), body)
    } else if let Some(h) = headline {
        (title, h)
    } else {
        return; // nothing worked on: no summary, no notification
    };
    notify_and_remember(app, &title, &body, kind, key);
}

/// A summary asked for in the window that finished while the person looked
/// elsewhere gets the same notification as an automatic one.
fn tell_if_away(app: &AppHandle, title: String, headline: Option<String>, error: Option<String>, kind: mirror::Kind, key: &str) {
    let focused = app.get_webview_window("main").is_some_and(|w| w.is_focused().unwrap_or(false));
    if !focused && settings::load().notify {
        tell_done(app, title, headline, error, kind, key);
    }
}

fn outcome<T>(r: &Result<T, String>, error: impl Fn(&T) -> Option<String>, headline: impl Fn(&T) -> Option<String>) -> (Option<String>, Option<String>) {
    match r {
        Ok(v) => (headline(v), error(v)),
        Err(e) => (None, Some(e.clone())),
    }
}

#[derive(Clone, Serialize)]
struct OpenReport {
    kind: &'static str,
    key: String,
}

fn kind_name(k: mirror::Kind) -> &'static str {
    match k {
        mirror::Kind::Daily => "day",
        mirror::Kind::Weekly => "week",
        mirror::Kind::Monthly => "month",
    }
}

#[derive(Clone, Serialize)]
struct NotificationOpened {
    kind: &'static str,
    /// "obsidian" | "app"
    target: &'static str,
}

/// Open the report in the main window, or the Obsidian note when the person chose that.
/// The main window reports `notification_opened` (ADR 0006).
fn open_notified(app: &AppHandle, kind: mirror::Kind, key: &str) {
    let target = if let Some(uri) = mirror::click_uri(&settings::load().notify_open, mirror::open_uri(kind, key)) {
        let _ = std::process::Command::new("open").arg(uri).status();
        "obsidian"
    } else {
        show_main(app, None);
        let _ = app.emit_to("main", "open-report", OpenReport { kind: kind_name(kind), key: key.to_owned() });
        "app"
    };
    let _ = app.emit_to("main", "notification-opened", NotificationOpened { kind: kind_name(kind), target });
}

/// Post the notification and wait on its own thread for the click.
/// tauri-plugin-notification cannot report clicks on desktop, so macOS goes
/// through mac-notification-sys directly; a dismissed notification ends the wait.
#[cfg(target_os = "macos")]
fn notify_and_remember(app: &AppHandle, title: &str, body: &str, kind: mirror::Kind, key: &str) {
    let (app, title, body, key) = (app.clone(), title.to_owned(), body.to_owned(), key.to_owned());
    std::thread::spawn(move || {
        // Fails harmlessly when already set (once per process).
        let _ = mac_notification_sys::set_application(&app.config().identifier);
        let clicked = mac_notification_sys::Notification::new()
            .title(&title)
            .message(&body)
            .wait_for_click(true)
            .send()
            .is_ok_and(|r| r == mac_notification_sys::NotificationResponse::Click);
        if clicked {
            let h = app.clone();
            let _ = app.run_on_main_thread(move || open_notified(&h, kind, &key));
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn notify_and_remember(app: &AppHandle, title: &str, body: &str, _kind: mirror::Kind, _key: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            mirror_vaults,
            save_mirror,
            save_notify,
            test_notification,
            open_notification_settings,
            resave_mirror,
            open_vault,
            open_mirror_dir,
            board_now,
            focus_session,
            today,
            day_report,
            insight_week,
            make_insight, set_goal, clear_goal, insight_feedback, insight_latest,
            week_report,
            month_report,
            list_reports,
            projects,
            project_days,
            backfill,
            projects_health,
            classify_blockers,
            suggestions,
            make_suggestions,
            set_suggestion_status,
            get_settings,
            get_language,
            save_language,
            save_settings,
            codex_models,
            save_excluded,
            set_hooks,
            set_cli_link,
            last_events,
            open_main,
            environment,
            finish_onboarding,
            export_markdown,
            export_pdf,
            open_items,
            close_open_item,
            waiting,
            open_download,
            save_usage_settings,
            save_analytics,
            save_insight_eval,
            set_statusline,
            set_project_statusline,
            usage_report,
            usage_limits,
            reveal
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            paths::migrate_legacy_data_dir();
            // Stay in the menubar until the main window is opened.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            // Hooks already installed keep running the binary this app ships:
            // refresh the staged copy when the app was updated.
            if paths::installed_hook_bin().exists() {
                if let Some(src) = bundled_hook() {
                    let _ = install::stage_hook_binary(&src);
                }
            }
            build_popover(app.handle())?;
            setup_tray(app.handle())?;
            start_auto_summary(app.handle().clone());
            start_day_ping(app.handle().clone());
            // A month of older days needs its usage read once (a minute or two);
            // do it in the background so the usage screen opens at once.
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(20));
                let _ = summary::usage_report(30, None, time::local_offset_secs());
            });
            // An accessory app is not activated on launch, so its window opens
            // behind others and WebKit skips painting it. Bring it forward.
            show_main(app.handle(), None);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the main window hides it; the app keeps running in the menubar.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.hide().is_ok() && window.label() == "main" {
                    #[cfg(target_os = "macos")]
                    let _ = window.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
            // The popover behaves like a menu: clicking elsewhere dismisses it.
            WindowEvent::Focused(false) if window.label() == "popover" => {
                let _ = window.hide();
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building porch")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                show_main(_app, None);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_words_in_both_languages() {
        assert_eq!(Shell::menu(Lang::Ko), ["앱 열기", "오늘 요약", "종료"]);
        assert_eq!(Shell::tooltip(2, Lang::En), "Waiting on you 2");
        assert_eq!(Shell::day_title("2026-10-03", "2026-10-03", "2026-10-02", Lang::Ko), "10월 3일 오늘 요약");
        assert_eq!(Shell::day_title("2026-10-03", "2026-10-03", "2026-10-02", Lang::En), "Oct 3 · Today's summary");
        assert_eq!(Shell::day_title("2026-10-02", "2026-10-03", "2026-10-02", Lang::En), "Oct 2 · Yesterday's summary");
        assert_eq!(Shell::week_title("2026-09-28", false, Lang::Ko), "9월 28일 주간 요약");
        assert_eq!(Shell::month_title("2026-09", Lang::En), "September 2026 · Summary");
    }

    #[test]
    fn popover_hangs_under_the_click_inside_the_work_area() {
        // A 1440×875 work area whose top sits under a 25pt menu bar on a 900pt-high display.
        let work = (0.0, 0.0, 1440.0, 875.0);
        let top = 875.0 - POPOVER_H - 4.0;
        assert_eq!(popover_origin(700.0, work), (700.0 - POPOVER_W / 2.0, top));
        assert_eq!(popover_origin(10.0, work), (8.0, top));
        assert_eq!(popover_origin(1435.0, work), (1440.0 - POPOVER_W - 8.0, top));
        // A second display to the left, at negative x and offset vertically.
        let left = (-1920.0, 120.0, 1920.0, 1055.0);
        assert_eq!(popover_origin(-100.0, left), (-POPOVER_W - 8.0, 120.0 + 1055.0 - POPOVER_H - 4.0));
    }
}
