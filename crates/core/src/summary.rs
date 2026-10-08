//! Day and week summaries: gather material (`digest`), have Claude Code or Codex
//! (`writer`) write the summary, keep the result as JSON under the data dir. The app and the
//! CLI both call into here; neither renders anything in this module.

use crate::digest::{self, DayDigest};
use crate::lang::Lang;
use crate::writer::{self, Engine, Provider};
use crate::{paths, time};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub done: Vec<String>,
    #[serde(default)]
    pub in_progress: Vec<String>,
    #[serde(default)]
    pub next: Vec<String>,
}

/// Where a stretch of the day (or week) went. The model groups turns and names
/// the group; `minutes` is added up here from the turns' recorded time, never
/// taken from the model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimeBlock {
    pub label: String,
    #[serde(default)]
    pub project: String,
    /// Turn ids ("t3") for a day; day block ids ("09-22.T2") for a week.
    #[serde(default)]
    pub turns: Vec<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub minutes: u64,
}

/// What a day's blocker can be, for counting across days (see `health`).
pub const BLOCKER_KINDS: [&str; 7] = ["test_fail", "auth_external", "env", "misread", "review_loop", "slow", "other"];

/// Something that held the work up, with the record that shows it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Blocker {
    pub title: String,
    #[serde(default)]
    pub project: String,
    /// Turn ids for a day; day blocker ids ("09-22.B1") for a week.
    #[serde(default)]
    pub turns: Vec<String>,
    /// What in the record shows it: "12 errors, the same request 3 times".
    #[serde(default)]
    pub signal: String,
    #[serde(default)]
    pub cause: String,
    #[serde(default)]
    pub resolved: bool,
    /// What would make it shorter next time.
    #[serde(default)]
    pub fix: String,
    /// One of `BLOCKER_KINDS` as the summary wrote it. Empty on summaries
    /// written before kinds existed. Anything else is kept as written and
    /// counted as unknown, never mapped onto a listed kind.
    #[serde(default)]
    pub kind: String,
    /// Time spent in the turns it covers, added up here.
    #[serde(default)]
    pub minutes: u64,
}

/// What a day's direction change can be: the person changed a
/// direction they had set, undid what the agent changed, or said the same
/// thing again because the earlier request was read another way.
pub const PIVOT_KINDS: [&str; 3] = ["shift", "revert", "redo"];
const MAX_PIVOTS: usize = 8;
const MAX_ASKS: usize = 6;
/// A day summary's quotes are cut to this many characters.
const QUOTE_CHARS: usize = 90;

/// A request that changed the direction set earlier or what the agent made.
/// Kept only when `turn` is one of the day's turns and `quote` is part of its request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Pivot {
    #[serde(default)]
    pub turn: String,
    /// One of `PIVOT_KINDS`.
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub after: String,
    #[serde(default)]
    pub quote: String,
    /// The same request said why.
    #[serde(default)]
    pub reason_given: bool,
    /// The reason given, or for a redo how the earlier request was read. One sentence.
    #[serde(default)]
    pub note: String,
}

/// A request the weekly clarity score can rest on: a session's first request,
/// or one whose conditions were added to later. `missing` lists only what a
/// later request added.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ask {
    #[serde(default)]
    pub turn: String,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub missing: Vec<String>,
}

/// A diagram the model chose to draw, as a Mermaid flowchart. Only a plain
/// flowchart survives `keep_visuals`; anything else is dropped, not repaired.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Visual {
    pub title: String,
    pub mermaid: String,
    #[serde(default)]
    pub caption: String,
}

const MAX_VISUALS: usize = 2;

/// Keep diagrams that are a short plain flowchart: no styling, links, init
/// directives or HTML, so the page draws them in its own quiet style.
pub fn keep_visuals(v: &mut Vec<Visual>) {
    v.retain(|x| {
        let m = x.mermaid.trim();
        let first = m.lines().next().unwrap_or("").trim();
        let plain = !["click ", "classDef", "class ", "style ", "linkStyle", "%%{", "<", "javascript:", "href"]
            .iter()
            .any(|bad| m.contains(bad));
        (first.starts_with("flowchart LR") || first.starts_with("flowchart TD")) && plain && m.len() <= 2000 && !x.title.trim().is_empty()
    });
    v.truncate(MAX_VISUALS);
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis {
    #[serde(default)]
    pub observations: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub headline: String,
    #[serde(default)]
    pub projects: Vec<ProjectSummary>,
    #[serde(default)]
    pub time: Vec<TimeBlock>,
    #[serde(default)]
    pub blockers: Vec<Blocker>,
    #[serde(default)]
    pub visuals: Vec<Visual>,
    /// What this day's records say about work left open on earlier days.
    #[serde(default)]
    pub open_updates: Vec<crate::open::OpenUpdate>,
    /// Requests that changed direction, checked by `clean_pivots`.
    #[serde(default)]
    pub pivots: Vec<Pivot>,
    /// Requests the weekly evaluation's clarity score can point at.
    #[serde(default)]
    pub asks: Vec<Ask>,
    /// Summaries written before time/blockers existed carry this instead.
    #[serde(default)]
    pub analysis: Analysis,
    /// The language it was written in (`Lang::code`), set by porch after the
    /// model answered. None on summaries saved before languages existed: Korean.
    #[serde(default)]
    pub lang: Option<String>,
}

impl Summary {
    pub fn lang(&self) -> Lang {
        Lang::from_code(self.lang.as_deref())
    }
}

impl Report {
    /// The language to show this report in: its summary's, or `fallback`
    /// when there is no summary to match.
    pub fn lang_or(&self, fallback: Lang) -> Lang {
        self.summary.as_ref().map_or(fallback, Summary::lang)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub date: String,
    pub generated_at: u64,
    pub model: Option<String>,
    /// Who wrote the summary; None on reports saved before the choice existed (Claude Code).
    #[serde(default)]
    pub provider: Option<Provider>,
    pub digest: DayDigest,
    pub summary: Option<Summary>,
    /// Set when the summarizer ran but its answer could not be used.
    pub summary_error: Option<String>,
    /// Average active minutes over recent recorded days, for "longer than usual".
    #[serde(default)]
    pub usual_minutes: Option<u64>,
    /// Usage was looked for on a report saved before it was recorded (see `fill_usage`).
    #[serde(default)]
    pub usage_checked: bool,
    /// Whether turns carry the files they edited (see `fill_turn_files`):
    /// None not tried yet, Some(false) the transcripts were already gone.
    #[serde(default)]
    pub turn_files_read: Option<bool>,
    /// Pivots and asks `clean_pivots` dropped: an unknown turn, a quote not in
    /// the request, an unknown type, over the day's cap.
    #[serde(default)]
    pub dropped_pivots: usize,
}

/// Write a report whole or not at all: a temp file, then a rename. A report
/// cut off mid-write would read as missing and be rebuilt without its summary.
pub(crate) fn save_json(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    // Several screens may save the same day at once: each write gets its own temp file.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}-{n}.tmp", std::process::id()));
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn reports_dir() -> PathBuf {
    paths::data_dir().join("reports")
}

/// Where the summarizer runs. Its cwd is excluded from every digest so the
/// summary run never summarizes itself.
pub fn runner_dir() -> PathBuf {
    paths::data_dir().join("runner")
}

const SYSTEM_PROMPT: &str = r#"당신은 한 개발자의 하루 작업 기록을 읽고, 시간을 어디에 썼는지와 어디서 막혔는지를 정리하는 도우미입니다.

입력:
- 프로젝트마다 세션이 있고, 세션 안에 요청 단위 기록이 [t번호]로 있습니다. 한 요청은 사람이 요청을 보낸 때부터 다음 요청까지이고, 분은 그 요청에 실제로 쓴 시간입니다.
- 오래 걸렸거나 문제가 있었던 요청은 요청 전문, 실패한 도구와 오류, 끝난 답까지 있고, 나머지는 한 줄입니다.
- "평소"가 있으면 최근 기록된 날들의 평균입니다.

쓰는 법:
- 입력이 다른 언어여도 한국어로 씁니다. 이름과 인용한 글은 원래 표기를 씁니다.
- 기록에 있는 사실만 씁니다. 추측하지 않습니다. 쉬운 한국어로 짧게, 항목 하나는 한 문장입니다. 줄표(—)는 쓰지 않습니다. 코드 식별자, 파일 이름, 명령은 원래 표기를 씁니다.
- "(다른 사람: 이름)"이 붙은 커밋은 함께 작업하는 다른 사람의 것입니다. 이 사람이 한 일로 쓰지 않습니다.
- 글 안에 t번호를 쓰지 않습니다. t번호는 turns와 turn 칸에만 넣습니다. 글에서는 "모델 이름을 바꿔 보던 요청"처럼 무엇이었는지로 말합니다.
- 분이나 시간 숫자는 어느 글에도 쓰지 않습니다. 시간은 화면이 기록에서 계산해 보여 줍니다. 횟수(오류 6번, 같은 요청 3번)는 씁니다.
- time: 하루의 요청들을 무슨 일을 했는지 기준으로 3~6개 덩어리로 묶습니다. 덩어리 이름(label)은 "로그인 버그 고치기"처럼 한 일로 짓고, "기타"나 "개발 작업" 같은 뭉뚱그린 이름은 쓰지 않습니다. turns에는 그 덩어리에 속하는 t번호를 빠짐없이 넣습니다. 한 요청은 한 덩어리에만 넣습니다. 분은 쓰지 않습니다(프로그램이 t번호로 더합니다). note는 그 시간이 어떻게 쓰였는지 한 문장입니다(예: "절반은 테스트 실패를 쫓는 데 썼다").
- blockers: 일이 막히거나 늘어진 곳만 씁니다. 근거는 같은 오류의 반복, 같은 요청을 다시 한 것, 사람이 중단한 것, 권한 거절, 한 요청에 비해 긴 시간 같은 기록입니다. 근거가 없으면 빈 배열로 둡니다.
  - title: 무엇이 막혔는지. signal: 근거가 된 기록을 숫자로(예: "cargo test 실패 6번, 같은 요청 3번"). cause: 기록에서 보이는 원인. 원인이 기록에 없으면 "기록으로는 원인을 알 수 없음"이라고 씁니다. resolved: 그날 풀렸으면 true. fix: 다음에 이 시간을 줄일 구체적인 방법 하나. 일반론("작게 나눠라")은 쓰지 않고 이 기록에 맞는 것만 씁니다. kind: 아래 중 하나만 씁니다. test_fail(테스트·타입 검사·빌드 실패가 반복됨), auth_external(인증, 외부 서비스, 스토어 업로드처럼 이 저장소 밖에서 막힘), env(작업 폴더, 브랜치, 파일 충돌, 도구 설치 같은 작업 환경), misread(요구가 전달되지 않아 같은 요청을 다시 함), review_loop(검토에서 수정 요청이 반복됨), slow(한 작업이 오래 돌거나 끝나지 않음), other(위에 없는 것).
- projects: 프로젝트마다 두세 문장 요약과 한 일(끝난 일), 진행 중(끝났다는 근거가 없는 일), 다음 할 일(기록에 다음 단계로 언급된 일). 근거가 없으면 빈 배열입니다.
- open_updates: 입력에 "열린 일"이 있으면 그 [o번호]마다 하나씩 씁니다. 오늘 기록에서 끝난 것이 확인되면 "done", 그만두거나 필요 없어진 것이 확인되면 "dropped", 그 밖에는 "open"입니다. 확실하지 않으면 "open"입니다. 열린 일에 이미 있는 일은 진행 중이나 다음 할 일에 다시 쓰지 않습니다.
- pivots: 사람이 앞서 정한 방향이나 에이전트가 만든 결과를 바꾸게 한 요청만 씁니다. 하루 8개까지, 없으면 빈 배열입니다.
  - turn: 그 요청의 t번호 하나. type: 아래 중 하나만 씁니다. shift(사람이 앞서 정한 방향을 바꿈), revert(에이전트가 바꾼 것을 원래대로 돌림), redo(앞 요청이 다르게 읽혀 같은 지시를 다시 함).
  - before, after: 무엇에서 무엇으로 바뀌었는지 각각 짧은 명사구. quote: 그 요청 글의 일부를 한 글자도 고치지 않고 그대로, 90자까지. reason_given: 바꾸는 이유를 그 요청에서 함께 말했으면 true. note: 그 요청에서 말한 이유, redo이면 앞 요청이 어떻게 읽혔는지 한 문장. 기록에 없으면 빈 글로 둡니다.
  - "사람이 중단함" 표시가 있는 요청은 그다음 요청이 방향을 바꾼 것인지 꼭 확인합니다.
- asks: 세션의 첫 요청과, 그 뒤에 같은 일의 조건을 보탠 요청만 씁니다. 하루 6개까지. turn: t번호 하나. quote: 요청 글의 일부를 그대로, 90자까지. missing: 그 요청에 없었고 뒤의 요청에서 보태진 것만 짧은 명사구로 씁니다. 기록에 없는 일반론은 쓰지 않습니다.
- pivots와 asks에는 사람의 성격이나 능력을 쓰지 않습니다. 기록된 요청과 행동만 씁니다.
- visuals: 그림이 글보다 확실히 잘 보여 줄 때만 0~2개 그립니다. 예: 막힌 일이 어떤 시도를 거쳐 풀렸는지, 여러 단계로 이어진 작업의 흐름, 여러 부분이 서로 어떻게 이어지는지. 목록으로 충분하면 그리지 않고 빈 배열로 둡니다.
  - mermaid는 Mermaid flowchart 문법만 씁니다. 첫 줄은 "flowchart LR"(노드 4개 이하) 또는 "flowchart TD"(그보다 많을 때)입니다. 노드는 8개 이하, 노드 글은 짧은 한국어로 A["글"]처럼 큰따옴표로 감쌉니다. 화살표 글이 필요하면 A -->|"글"| B 로 씁니다. style, classDef, class, click, 링크, 색, HTML은 쓰지 않습니다.
  - title은 그림 제목, caption은 그림에서 읽어야 할 것 한 문장입니다.
- headline: 이 날의 제목. 시간이 가장 많이 간 곳이나 가장 큰 막힘이 드러나게 씁니다.
  - 문장이 아니라 핵심 키워드로 된 제목입니다. 기사 제목처럼 그날 가장 큰 일 두세 가지를 명사구로 쓰고 쉼표로 잇습니다. 무엇을(대상)과 어떻게 됐는지(결과)가 드러나게 씁니다.
  - "~한 날", "~했다", "~하며", "~에 집중" 같은 문장형 끝맺음과 설명조를 쓰지 않습니다. 명사로 끝냅니다(예: 마무리, 수정, 추가, 도입, 막힘).
  - 프로젝트 이름과 기능 이름은 핵심 키워드이니 씁니다. 다만 훅, 커밋, 세션, 서브에이전트, 컨텍스트, 프록시, 토큰, 빌드, API, CLI 같은 개발 용어와 명령 이름은 누구나 아는 말로 바꿉니다.
  - 40자 안팎으로 짧게 씁니다. 딱딱한 한자어(진행, 수행, 구현, 고도화, 착수)는 쓰지 않습니다.
  - 고쳐 쓰는 예. "결제 화면을 다듬고 주문 알림 오류를 고친 날" → "결제 화면 정리, 주문 알림 오류 수정". "로그인 기능을 새로 만들며 테스트 실패와 씨름한 하루" → "로그인 기능 추가, 테스트 실패로 막힘". "세션 현황판에 watch 기능을 추가한 하루" → "작업 상태 화면 추가".
- time의 label과 blockers의 title도 같은 기준입니다. 개발 용어 대신 무슨 일인지 말로 씁니다. 다만 막힌 곳의 signal과 cause에는 정확한 명령·오류 이름을 그대로 씁니다.

출력은 아래 모양의 JSON 하나만 냅니다. 다른 글이나 코드 블록 표시는 붙이지 않습니다.
{"headline": "", "time": [{"label": "", "project": "프로젝트 이름", "turns": ["t1"], "note": ""}], "blockers": [{"title": "", "kind": "", "project": "", "turns": ["t2"], "signal": "", "cause": "", "resolved": false, "fix": ""}], "visuals": [{"title": "", "mermaid": "flowchart LR\n  A[\"...\"] --> B[\"...\"]", "caption": ""}], "open_updates": [{"id": "o1", "status": "open"}], "projects": [{"name": "프로젝트 이름(기록의 이름 그대로)", "summary": "", "done": [], "in_progress": [], "next": []}], "pivots": [{"turn": "t3", "type": "redo", "before": "", "after": "", "quote": "", "reason_given": false, "note": ""}], "asks": [{"turn": "t1", "quote": "", "missing": []}]}"#;

/// The rule every English prompt carries, word for word: records, earlier
/// summaries and instruction files are often in another language.
#[cfg(test)]
pub(crate) const ENGLISH_EVEN_IF: &str = "Write in English even when the input is in another language";
/// The same rule in every Korean prompt.
#[cfg(test)]
pub(crate) const KOREAN_EVEN_IF: &str = "입력이 다른 언어여도 한국어로 씁니다";

const SYSTEM_PROMPT_EN: &str = r#"You read one developer's work records for a day and write down where the time went and where the work got stuck.

Input:
- Each project has sessions, and each session lists its requests as [t-numbers]. A request covers the time from when the person sent it until the next request. Its minutes are estimated from the records, with breaks over 10 minutes left out and overlapping work counted once.
- Requests that took a long time or ran into trouble include details under "Request", "Failed" and "Final reply". Other requests get one line.
- "Recent average", when present, is the average over recently recorded days.

How to write:
- Write in English even when the input is in another language. Names, code identifiers, file names, commands and quoted text stay as written.
- Write only what the records show. Do not guess. Use plain, concise English, one sentence per list item. Do not use em or en dashes (— –). Keep code identifiers, file names and commands exactly as written.
- A commit marked "(other: name)" was made by someone else working on the same code. Do not credit it to this person.
- Never put a t-number in the text. t-numbers go only in the turns and turn fields. In the text, say what the request was, such as "the request that tried renaming the model".
- Do not write minutes or hours anywhere in the text. The app works out time from the records and shows it. Counts are fine (6 errors, the same request 3 times).
- time: group the day's requests into 3 to 6 blocks by what was worked on. Name each block (label) by the work done, such as "Fixing the login bug". Do not use catch-all names like "Misc" or "Development work". Put every t-number of the block in turns, leaving none out. Each request goes in exactly one block. Do not write minutes (the program adds them up from the t-numbers). note is one sentence on how that time was spent (for example, "Half of it went to chasing a failing test").
- blockers: only where the work got stuck or dragged on. Evidence means a repeated error, the same request sent again, the person interrupting, a denied permission, or a long time on one request. If there is no evidence, leave the array empty.
  - title: what got stuck. signal: the evidence as counts (for example, "cargo test failed 6 times, same request 3 times"), never as minutes or hours. cause: the cause the records show. If the records do not show one, write "The records do not show the cause". resolved: true if it was solved that day. fix: one concrete way to spend less time on this next time. No general advice ("break it into smaller steps"); only what fits these records. kind: exactly one of these. test_fail (tests, type checks or builds failed repeatedly), auth_external (blocked outside this repo: authentication, an external service, a store upload), env (the local environment: folders, branches, file conflicts, tool installation), misread (the request was misunderstood, so the same request was sent again), review_loop (review asked for revisions again and again), slow (one task took a long time or never finished), other (none of these).
- projects: for each project, a two or three sentence summary, plus done (finished work), in_progress (work with no sign it finished) and next (work the records name as the next step). Use an empty array when there is no evidence.
- open_updates: if the input has "Open items", write one entry for each [o-number]. "done" if today's records show it finished, "dropped" if they show it was given up or no longer needed, otherwise "open". When unsure, "open". Do not list an open item again in in_progress or next.
- pivots: only requests that changed a direction the person set earlier or something the agent made. At most 8 a day; leave the array empty when there are none.
  - turn: that request's t-number. type: exactly one of these. shift (the person changed a direction they had set), revert (the person undid something the agent changed), redo (the earlier request was read another way, so the person said it again).
  - before, after: what changed from what to what, each a short noun phrase. quote: part of that request copied exactly, not a single character changed, up to 90 characters. reason_given: true if the same request said why. note: the reason that request gave, or for a redo how the earlier request was read, in one sentence. Leave it empty when the records show none.
  - After a request marked "interrupted by the person", always check whether the next request changed direction.
- asks: only a session's first request, and requests that later added conditions to the same work. At most 6 a day. turn: one t-number. quote: part of the request copied exactly, up to 90 characters. missing: only what that request lacked and a later request added, as short noun phrases. No general advice the records do not show.
- In pivots and asks, never describe the person's character or ability. Write only the requests and actions on record.
- visuals: draw 0 to 2 diagrams, only when a picture clearly shows something better than text. For example: the attempts a blocker went through before it was solved, work that ran through several steps, or how several parts connect. If a list is enough, draw nothing and leave the array empty.
  - mermaid uses Mermaid flowchart syntax only. The first line is "flowchart LR" (4 nodes or fewer) or "flowchart TD" (more). At most 8 nodes. Node text is short English in double quotes, like A["text"]. For edge text, write A -->|"text"| B. No style, classDef, class, click, links, colors or HTML.
  - title is the diagram's title; caption is one sentence explaining what it shows.
- headline: the day's title. It should show where most of the time went or the biggest blocker.
  - Keywords, not a sentence. Like a news headline, name the day's two or three biggest pieces of work as noun phrases joined by commas. Each phrase says what (the subject) and what happened (the outcome).
  - No full sentences and no framing like "A day of", "Spent the day", "Focused on". Say what happened to each piece of work when the records show it (for example: finished, fixed, added, released, still failing).
  - Keep project and feature names; they are the keywords. Replace developer jargon and command names (hook, commit, session, subagent, context, proxy, token, build, API, CLI) with words anyone knows.
  - About 60 characters. Avoid stiff words (implementation, execution, enhancement, initiative, leverage).
  - Rewrites. "A day of polishing the payment screen and fixing the order alert bug" becomes "Payment screen cleanup, order alert bug fixed". "Built the new login feature while fighting failing tests" becomes "Login feature added, tests still failing". "Added a watch feature to the session board" becomes "Work status screen added".
- time labels and blocker titles follow the same rules: say what the work was instead of using developer jargon. In a blocker's signal and cause, though, keep the exact command and error names.

Output exactly one JSON object shaped like this. Add no other text and no code fences.
{"headline": "", "time": [{"label": "", "project": "project name", "turns": ["t1"], "note": ""}], "blockers": [{"title": "", "kind": "", "project": "", "turns": ["t2"], "signal": "", "cause": "", "resolved": false, "fix": ""}], "visuals": [{"title": "", "mermaid": "flowchart LR\n  A[\"...\"] --> B[\"...\"]", "caption": ""}], "open_updates": [{"id": "o1", "status": "open"}], "projects": [{"name": "project name (as written in the records)", "summary": "", "done": [], "in_progress": [], "next": []}], "pivots": [{"turn": "t3", "type": "redo", "before": "", "after": "", "quote": "", "reason_given": false, "note": ""}], "asks": [{"turn": "t1", "quote": "", "missing": []}]}"#;

/// The day prompt for a run in `lang` (ADR 0011: one per language; change both).
fn day_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => SYSTEM_PROMPT,
        Lang::En => SYSTEM_PROMPT_EN,
    }
}

pub fn summarize(prompt: &str, engine: &Engine) -> Result<Summary, String> {
    serde_json::from_value(writer::run(engine, prompt, day_prompt(engine.lang))?).map_err(|e| match engine.lang {
        Lang::Ko => format!("요약 모양이 다릅니다: {e}"),
        Lang::En => format!("The summary response did not match the expected format: {e}"),
    })
}

pub struct DayOpts {
    pub date: String,
    pub engine: Engine,
    pub refresh: bool,
    pub no_llm: bool,
}

/// The day's records, leaving out the summarizer's own runs and excluded folders.
/// `lang` is the language of the text written onto turns (failed commands).
fn collect(date: &str, offset: i64, lang: Lang) -> Result<DayDigest, String> {
    let mut excluded = vec![runner_dir().to_string_lossy().into_owned()];
    excluded.extend(
        fs::read_to_string(paths::excluded_file())
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned),
    );
    digest::collect_day(date, offset, &digest::Options { claude_dirs: vec![paths::default_claude_dir()], excluded, lang })
        .ok_or_else(|| time::bad_date(lang))
}

/// Give a saved report usage counted the current way (`digest::USAGE_COUNTING`):
/// reports saved before usage was recorded, or counted the old way, read the
/// transcripts again. Only usage is copied in; the rest of the saved report
/// stays as it was.
pub fn fill_usage(r: &mut Report, offset: i64) {
    let tried = r.digest.metrics.usage.is_none() && r.usage_checked;
    if r.digest.usage_counting >= digest::USAGE_COUNTING || tried {
        return;
    }
    r.usage_checked = true;
    // Only usage is copied over; the language of turn text does not matter here.
    let Ok(fresh) = collect(&r.date, offset, Lang::default()) else { return };
    recount_usage(r, &fresh);
}

/// Copy usage from a fresh read of the same day (by project name, session and
/// turn id). Only when every saved session is still there: Claude Code deletes
/// old transcripts, and a partial read would undercount. A day without usage
/// takes whatever is left. Either way the day is not tried again.
fn recount_usage(r: &mut Report, fresh: &DayDigest) {
    r.digest.usage_counting = digest::USAGE_COUNTING;
    let fresh_sessions: std::collections::HashSet<&str> =
        fresh.projects.iter().flat_map(|p| p.sessions.iter()).map(|s| s.session.as_str()).collect();
    let complete = r.digest.projects.iter().flat_map(|p| p.sessions.iter()).all(|s| fresh_sessions.contains(s.session.as_str()));
    if (fresh.projects.is_empty() && fresh.scratch_usage.is_empty()) || (r.digest.metrics.usage.is_some() && !complete) {
        return;
    }
    let turns: std::collections::HashMap<String, &digest::Turn> =
        fresh.projects.iter().flat_map(|p| p.sessions.iter().flat_map(|s| s.turns.iter())).map(|t| (t.id.clone(), t)).collect();
    for p in &mut r.digest.projects {
        if let Some(f) = fresh.projects.iter().find(|f| f.name == p.name) {
            p.metrics.usage = f.metrics.usage.clone();
        }
        for sess in &mut p.sessions {
            if let Some(f) = fresh.projects.iter().flat_map(|fp| fp.sessions.iter()).find(|f| f.session == sess.session) {
                sess.usage = f.usage.clone();
            }
        }
        for t in p.sessions.iter_mut().flat_map(|s| s.turns.iter_mut()) {
            if let Some(f) = turns.get(&t.id) {
                t.tokens = f.tokens;
                t.cost = f.cost;
                t.unpriced = f.unpriced;
                t.model = f.model.clone();
            }
        }
    }
    r.digest.metrics.usage = fresh.metrics.usage.clone();
    r.digest.scratch_usage = fresh.scratch_usage.clone();
    r.digest.agent_usage = fresh.agent_usage.clone();
    r.digest.project_usage = fresh.project_usage.clone();
}

/// Copy each turn's edited files and repo from a fresh read of the same day.
/// Turns are matched by session and start, never by id: ids shift once
/// Claude Code has cleaned up some of the day's transcripts. Returns how many
/// turns were matched.
fn copy_turn_files(saved: &mut DayDigest, fresh: &DayDigest) -> usize {
    let by_key: std::collections::HashMap<(&str, u64), &digest::Turn> = fresh
        .projects
        .iter()
        .flat_map(|p| p.sessions.iter())
        .flat_map(|s| s.turns.iter().map(move |t| ((s.session.as_str(), t.start), t)))
        .collect();
    let mut matched = 0;
    for s in saved.projects.iter_mut().flat_map(|p| p.sessions.iter_mut()) {
        let session = s.session.clone();
        for t in &mut s.turns {
            if let Some(f) = by_key.get(&(session.as_str(), t.start)) {
                t.files = f.files.clone();
                t.root = f.root.clone();
                matched += 1;
            }
        }
    }
    matched
}

/// Give a report saved before turns carried their files those files, read
/// again from the transcripts. Only files and repo are copied in; the saved
/// summary and turn ids stay as they were.
pub fn fill_turn_files(r: &mut Report, offset: i64) {
    if r.turn_files_read.is_some() {
        return;
    }
    // Only files and repos are copied over; the language of turn text does not matter here.
    let matched = match collect(&r.date, offset, Lang::default()) {
        Ok(fresh) => copy_turn_files(&mut r.digest, &fresh),
        Err(_) => 0,
    };
    r.turn_files_read = Some(matched > 0);
}

/// Saved per-model tokens are sufficient to update usage-page totals even
/// after transcripts have been removed. Keep prose and turn data unchanged:
/// a turn's `model` is only its last model, not every model it used.
fn reprice_usage(r: &mut Report) -> bool {
    let mut changed = r.digest.metrics.usage.as_mut().is_some_and(|u| u.reprice());
    for u in r.digest.agent_usage.values_mut().chain(r.digest.project_usage.values_mut()).chain(r.digest.scratch_usage.values_mut()) {
        changed |= u.reprice();
    }
    for project in &mut r.digest.projects {
        changed |= project.metrics.usage.as_mut().is_some_and(|u| u.reprice());
        for session in &mut project.sessions {
            changed |= session.usage.reprice();
        }
    }
    changed
}

/// A rewrite that failed (a usage limit, a broken answer) must not cost the
/// summary already saved: put the saved summary back before writing, and hand
/// the error to the caller only. Returns that error.
macro_rules! keep_written {
    ($report:ident, $ty:ty, $cache:expr) => {{
        let previous = ($report.summary.is_none() && $report.summary_error.is_some())
            .then(|| fs::read_to_string($cache).ok().and_then(|t| serde_json::from_str::<$ty>(&t).ok()))
            .flatten()
            .filter(|p| p.summary.is_some());
        previous.map(|p| {
            $report.summary = p.summary;
            $report.model = p.model;
            $report.provider = p.provider;
            $report.summary_error.take().unwrap_or_default()
        })
    }};
}

pub fn build_day(o: &DayOpts, offset: i64) -> Result<Report, String> {
    let cache = reports_dir().join(format!("{}.json", o.date));
    // A record-only report for a day still in progress goes stale; rebuild it.
    let is_today = o.date == time::local_date(time::now_ms(), offset);
    if !o.refresh && !(o.no_llm && is_today) {
        if let Ok(mut r) = fs::read_to_string(&cache).map_err(|_| ()).and_then(|t| serde_json::from_str::<Report>(&t).map_err(|_| ())) {
            if r.summary.is_some() || o.no_llm {
                // Saved before commit authors were read: add them once and keep them.
                let stale = r.digest.metrics.my_commits.is_none()
                    || (r.digest.metrics.usage.is_none() && !r.usage_checked)
                    || (r.digest.usage_counting < digest::USAGE_COUNTING && !(r.digest.metrics.usage.is_none() && r.usage_checked))
                    || r.turn_files_read.is_none();
                if stale {
                    if r.digest.metrics.my_commits.is_none() {
                        digest::fill_commit_authors(&mut r.digest);
                    }
                    fill_usage(&mut r, offset);
                    fill_turn_files(&mut r, offset);
                }
                let repriced = reprice_usage(&mut r);
                if stale || repriced {
                    if let Ok(t) = serde_json::to_string(&r) {
                        let _ = save_json(&cache, &t);
                    }
                }
                return Ok(r);
            }
        }
    }
    let digest = collect(&o.date, offset, o.engine.lang)?;
    let mut dropped_pivots = 0;
    let (summary, summary_error, model, provider) = if o.no_llm {
        // Fresh numbers for a day in progress must keep the summary already written.
        // Read it now, not before collecting: a summary may have landed meanwhile.
        match fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str::<Report>(&t).ok()) {
            Some(r) => {
                dropped_pivots = r.dropped_pivots;
                (r.summary, r.summary_error, r.model, r.provider)
            }
            None => (None, None, None, None),
        }
    } else if digest.projects.is_empty() {
        (None, None, None, None)
    } else {
        match o.engine.lang {
            Lang::Ko => eprintln!("{}로 요약하는 중… (프로젝트 {}개)", o.engine.provider.name(), digest.projects.len()),
            Lang::En => eprintln!("Summarizing with {}… ({} projects)", o.engine.provider.name(), digest.projects.len()),
        }
        let mut ledger = crate::open::load(saved_days);
        let names: Vec<&str> = digest.projects.iter().map(|p| p.name.as_str()).collect();
        let lang = o.engine.lang;
        let text = usual_text(&o.date, lang) + &digest::to_prompt_text(&digest, offset, lang) + &crate::open::prompt_text(&ledger, &o.date, &names, lang);
        let written = (Some(o.engine.model_label()), Some(o.engine.provider));
        match summarize(&text, &o.engine) {
            Ok(s) => {
                let mut s = finish_day(s, &digest, o.engine.lang);
                dropped_pivots = clean_pivots(&mut s, &digest);
                if let Err(e) = crate::open::apply(&mut ledger, &o.date, &s) {
                    match o.engine.lang {
                        Lang::Ko => eprintln!("남은 일·막힘을 저장하지 못했습니다: {e}"),
                        Lang::En => eprintln!("Could not save open items: {e}"),
                    }
                }
                (Some(s), None, written.0, written.1)
            }
            Err(e) => (None, Some(e), written.0, written.1),
        }
    };
    let usual_minutes = usual(&o.date).map(|u| u.day);
    let mut report = Report { date: o.date.clone(), generated_at: time::now_ms(), model, provider, digest, summary, summary_error, usual_minutes, usage_checked: true, turn_files_read: Some(true), dropped_pivots };
    let failed = keep_written!(report, Report, &cache);
    fs::create_dir_all(reports_dir()).map_err(|e| e.to_string())?;
    save_json(&cache, &serde_json::to_string(&report).map_err(|e| e.to_string())?)?;
    // Fresh numbers under an old summary are not a new summary: the note stays as written.
    if !o.no_llm && failed.is_none() {
        crate::mirror::save_day(&report);
    }
    report.summary_error = failed.or(report.summary_error);
    Ok(report)
}

fn minutes_of(ms: u64) -> u64 {
    (ms + 30_000) / 60_000
}

/// Take turn ids out of prose the model wrote despite being told not to:
/// "t12", "09-22.T3", and a particle glued to one ("t7에서") go; the brackets
/// and commas they leave behind are tidied up.
pub(crate) fn strip_ids(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let alnum = |i: usize| c.get(i).is_some_and(|x| x.is_ascii_alphanumeric());
    let hangul = |x: char| ('\u{AC00}'..='\u{D7A3}').contains(&x);
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        let boundary = i == 0 || !alnum(i - 1);
        // optional "MM-DD." prefix
        let mut j = i;
        if boundary && j + 6 <= c.len() && c[j].is_ascii_digit() && c[j + 1].is_ascii_digit() && c[j + 2] == '-' && c[j + 3].is_ascii_digit() && c[j + 4].is_ascii_digit() && c[j + 5] == '.' {
            j += 6;
        }
        if boundary && j < c.len() && matches!(c[j], 't' | 'T' | 'B') && c.get(j + 1).is_some_and(|x| x.is_ascii_digit()) {
            let mut k = j + 1;
            while k < c.len() && c[k].is_ascii_digit() {
                k += 1;
            }
            if !alnum(k) {
                while k < c.len() && hangul(c[k]) {
                    k += 1;
                }
                i = k;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    let mut t = out;
    loop {
        let before = t.clone();
        for (a, b) in [("(, ", "("), (", )", ")"), ("(,", "("), (",)", ")"), (", ,", ","), ("()", ""), ("( )", ""), ("  ", " "), (" ,", ",")] {
            t = t.replace(a, b);
        }
        if t == before {
            break;
        }
    }
    t.trim().trim_start_matches([',', ' ']).trim_end_matches([',', ' ']).to_owned()
}

/// Runs of whitespace as one space, trimmed: how a quote is matched against a request.
pub(crate) fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A quote as kept: the quotation marks and ellipses the model put around it
/// come off, whitespace is squashed, and it is cut to `max` characters. None
/// when nothing is left or it is not word for word part of `request`.
pub(crate) fn checked_quote(quote: &str, request: &str, max: usize) -> Option<String> {
    let q = quote.trim_matches(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’' | '「' | '」' | '…'));
    let q = squash(q.trim_start_matches("...").trim_end_matches("..."));
    (!q.is_empty() && squash(request).contains(&q)).then(|| q.chars().take(max).collect())
}

/// Keep the pivots and asks that point at one of the day's turns and quote its
/// request word for word, up to the day's caps. Returns how
/// many were dropped.
pub fn clean_pivots(s: &mut Summary, d: &DayDigest) -> usize {
    let requests: BTreeMap<&str, &str> =
        d.projects.iter().flat_map(|p| p.sessions.iter()).flat_map(|x| x.turns.iter()).map(|t| (t.id.as_str(), t.prompt.as_str())).collect();
    let before = s.pivots.len() + s.asks.len();
    s.pivots.retain_mut(|p| {
        let quote = requests.get(p.turn.as_str()).and_then(|r| checked_quote(&p.quote, r, QUOTE_CHARS));
        match quote.filter(|_| PIVOT_KINDS.contains(&p.kind.as_str())) {
            Some(q) => {
                p.quote = q;
                p.before = strip_ids(&p.before);
                p.after = strip_ids(&p.after);
                p.note = strip_ids(&p.note);
                true
            }
            None => false,
        }
    });
    s.asks.retain_mut(|a| match requests.get(a.turn.as_str()).and_then(|r| checked_quote(&a.quote, r, QUOTE_CHARS)) {
        Some(q) => {
            a.quote = q;
            a.missing = a.missing.iter().map(|m| strip_ids(m)).filter(|m| !m.is_empty()).collect();
            true
        }
        None => false,
    });
    s.pivots.truncate(MAX_PIVOTS);
    s.asks.truncate(MAX_ASKS);
    before - s.pivots.len() - s.asks.len()
}

fn clean_prose(time: &mut [TimeBlock], blockers: &mut [Blocker]) {
    for b in time {
        b.note = strip_ids(&b.note);
    }
    for b in blockers {
        b.title = strip_ids(&b.title);
        b.signal = strip_ids(&b.signal);
        b.cause = strip_ids(&b.cause);
        b.fix = strip_ids(&b.fix);
    }
}

/// Fill in minutes from the turns the model pointed at. A turn counts toward one
/// time block only; turns the model left out become one last block, so the
/// blocks always add up to the day. Ids the model made up are dropped.
pub fn add_day_minutes(s: &mut Summary, d: &DayDigest, lang: Lang) {
    let turns: BTreeMap<&str, (u64, &str)> = d
        .projects
        .iter()
        .flat_map(|p| p.sessions.iter().flat_map(move |s| s.turns.iter().map(move |t| (t.id.as_str(), (t.active_ms, p.name.as_str())))))
        .collect();
    let mut used = std::collections::BTreeSet::new();
    for b in &mut s.time {
        b.turns.retain(|id| turns.contains_key(id.as_str()) && used.insert(id.clone()));
        b.minutes = minutes_of(b.turns.iter().map(|id| turns[id.as_str()].0).sum());
    }
    let rest: Vec<String> = turns.keys().filter(|id| !used.contains(**id)).map(|id| id.to_string()).collect();
    let rest_ms: u64 = rest.iter().map(|id| turns[id.as_str()].0).sum();
    if minutes_of(rest_ms) > 0 {
        let label = match lang {
            Lang::Ko => "묶이지 않은 요청",
            Lang::En => "Ungrouped requests",
        };
        s.time.push(TimeBlock { label: label.into(), turns: rest, minutes: minutes_of(rest_ms), ..Default::default() });
    }
    s.time.retain(|b| b.minutes > 0);
    s.time.sort_by_key(|b| std::cmp::Reverse(b.minutes));
    for b in &mut s.blockers {
        b.turns.retain(|id| turns.contains_key(id.as_str()));
        b.turns.dedup();
        b.minutes = minutes_of(b.turns.iter().map(|id| turns[id.as_str()].0).sum());
    }
    s.blockers.sort_by_key(|b| std::cmp::Reverse(b.minutes));
}

struct Usual {
    days: usize,
    day: u64,
    projects: BTreeMap<String, (u64, usize)>,
}

/// Recorded days in the two weeks before `date`, averaged.
fn usual(date: &str) -> Option<Usual> {
    let past: Vec<Report> = saved_days().into_iter().filter(|r| r.date.as_str() < date).rev().take(14).collect();
    if past.len() < 3 {
        return None;
    }
    let mut projects: BTreeMap<String, (u64, usize)> = BTreeMap::new();
    for r in &past {
        for p in &r.digest.projects {
            let e = projects.entry(p.name.clone()).or_default();
            e.0 += p.metrics.active_minutes;
            e.1 += 1;
        }
    }
    let day = past.iter().map(|r| r.digest.metrics.active_minutes).sum::<u64>() / past.len() as u64;
    Some(Usual { days: past.len(), day, projects })
}

fn usual_text(date: &str, lang: Lang) -> String {
    usual(date).map(|u| usual_line(&u, lang)).unwrap_or_default()
}

/// "Usual" as the day prompt names it.
fn usual_line(u: &Usual, lang: Lang) -> String {
    let mut out = match lang {
        Lang::Ko => format!("평소(최근 기록된 {}일): 하루 평균 {}분", u.days, u.day),
        Lang::En => format!("Recent average (last {} recorded days): {} min a day", u.days, u.day),
    };
    for (name, (min, days)) in &u.projects {
        let avg = min / *days as u64;
        let _ = match lang {
            Lang::Ko => write!(out, ", {name}은 일한 날 평균 {avg}분({days}일)"),
            Lang::En => write!(out, ", {name}: average {avg} min per recorded workday ({days} days)"),
        };
    }
    out + "\n"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeekProject {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub shipped: Vec<String>,
    #[serde(default)]
    pub ongoing: Vec<String>,
    #[serde(default)]
    pub next: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeekSummary {
    pub headline: String,
    #[serde(default)]
    pub projects: Vec<WeekProject>,
    #[serde(default)]
    pub time: Vec<TimeBlock>,
    #[serde(default)]
    pub blockers: Vec<Blocker>,
    #[serde(default)]
    pub visuals: Vec<Visual>,
    #[serde(default)]
    pub analysis: Analysis,
    /// The language it was written in (`Lang::code`), set by porch after the
    /// model answered. None on summaries saved before languages existed: Korean.
    #[serde(default)]
    pub lang: Option<String>,
}

impl WeekSummary {
    pub fn lang(&self) -> Lang {
        Lang::from_code(self.lang.as_deref())
    }
}

impl WeekReport {
    /// The language to show this week in: its summary's, or `fallback`.
    pub fn lang_or(&self, fallback: Lang) -> Lang {
        self.summary.as_ref().map_or(fallback, WeekSummary::lang)
    }
}

impl MonthReport {
    /// The language to show this month in: its summary's, or `fallback`.
    pub fn lang_or(&self, fallback: Lang) -> Lang {
        self.summary.as_ref().map_or(fallback, WeekSummary::lang)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayLine {
    pub date: String,
    pub minutes: u64,
    pub commits: usize,
    #[serde(default)]
    pub my_commits: Option<usize>,
    /// List-price cost and tokens of the day, when known.
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub tokens: Option<u64>,
    pub headline: Option<String>,
    /// project -> active minutes
    pub projects: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeekReport {
    pub week_start: String,
    pub generated_at: u64,
    pub model: Option<String>,
    /// Who wrote the summary; None on reports saved before the choice existed (Claude Code).
    #[serde(default)]
    pub provider: Option<Provider>,
    pub days: Vec<DayLine>,
    pub summary: Option<WeekSummary>,
    pub summary_error: Option<String>,
}

const WEEK_SYSTEM_PROMPT: &str = r#"당신은 한 개발자의 한 주 작업 기록을 읽고, 한 주의 시간이 어디로 갔는지와 무엇이 반복해서 막았는지를 정리하는 도우미입니다.

입력은 날짜별로, 그날의 시간 덩어리와 요약입니다. 시간 덩어리는 [MM-DD.T번호] 또는 [MM-DD.P번호](그날 요약에 덩어리가 없어 프로젝트 단위로 잡은 것)와 분으로, 막힌 곳은 [MM-DD.B번호]로 있습니다. 요약이 없는 날은 수치·커밋·세션 제목만 있습니다.

쓰는 법:
- 입력이 다른 언어여도 한국어로 씁니다(앞서 쓴 하루 요약이 다른 언어일 수 있습니다). 이름과 인용한 글은 원래 표기를 씁니다.
- 기록에 있는 사실만 씁니다. 추측하지 않습니다. 쉬운 한국어로 짧게, 항목 하나는 한 문장입니다. 줄표(—)는 쓰지 않습니다.
- 글 안에 [MM-DD.T번호] 같은 번호를 쓰지 않습니다. 번호는 turns 칸에만 넣습니다.
- 분이나 시간 숫자는 어느 글에도 쓰지 않습니다. 시간은 화면이 기록에서 계산해 보여 줍니다. 날짜와 횟수는 씁니다.
- time: 날마다의 시간 덩어리를 한 주 기준으로 3~6개 일로 다시 묶습니다. turns에는 속하는 [MM-DD.T번호]와 [MM-DD.P번호]를 빠짐없이 넣고, 하나는 한 곳에만 넣습니다. 분은 쓰지 않습니다(프로그램이 더합니다). note는 한 주 동안 그 시간이 어떻게 흘렀는지 한 문장입니다(예: "화·수에 몰렸고 금요일엔 손대지 않았다").
- blockers: 한 주에 두 번 이상 나왔거나 가장 시간을 많이 잡아먹은 막힘만 씁니다. turns에는 해당하는 [MM-DD.B번호]를 넣습니다. signal은 며칠, 몇 번인지. cause는 기록에서 보이는 원인. resolved는 주 안에 풀렸으면 true. fix는 이 기록에 맞는 구체적인 방법 하나입니다. 없으면 빈 배열입니다.
- projects: 프로젝트마다 두세 문장 요약과 나간 것(끝나서 결과가 남은 일), 이어지는 것(주 안에 끝나지 않은 일), 다음 주(기록에서 다음 단계로 언급된 일). 근거가 없으면 빈 배열입니다.
- visuals: 그림이 글보다 확실히 잘 보여 줄 때만 0~2개 그립니다. 예: 막힌 일이 어떤 시도를 거쳐 풀렸는지, 여러 단계로 이어진 작업의 흐름, 여러 부분이 서로 어떻게 이어지는지. 목록으로 충분하면 그리지 않고 빈 배열로 둡니다.
  - mermaid는 Mermaid flowchart 문법만 씁니다. 첫 줄은 "flowchart LR"(노드 4개 이하) 또는 "flowchart TD"(그보다 많을 때)입니다. 노드는 8개 이하, 노드 글은 짧은 한국어로 A["글"]처럼 큰따옴표로 감쌉니다. 화살표 글이 필요하면 A -->|"글"| B 로 씁니다. style, classDef, class, click, 링크, 색, HTML은 쓰지 않습니다.
  - title은 그림 제목, caption은 그림에서 읽어야 할 것 한 문장입니다.
- headline: 이 주의 제목. 시간이 가장 많이 간 곳이나 반복된 막힘이 드러나게 씁니다.
  - 문장이 아니라 핵심 키워드 제목입니다. 이 주에 가장 큰 일 두세 가지를 "무엇, 어떻게 됐는지" 명사구로 쓰고 쉼표로 잇습니다. "~한 주", "~했다" 같은 끝맺음은 쓰지 않고 명사로 끝냅니다. 프로젝트·기능 이름은 쓰고 개발 용어는 쉬운 말로 바꿉니다. 40자 안팎. 예: "결제 개편 마무리, 알림 기능 도입, 배포 자동화 막힘".
- time의 label과 blockers의 title도 같은 기준입니다.

출력은 아래 모양의 JSON 하나만 냅니다. 다른 글이나 코드 블록 표시는 붙이지 않습니다.
{"headline": "", "time": [{"label": "", "project": "", "turns": ["09-22.T1"], "note": ""}], "blockers": [{"title": "", "project": "", "turns": ["09-22.B1"], "signal": "", "cause": "", "resolved": false, "fix": ""}], "visuals": [{"title": "", "mermaid": "flowchart LR\n  A[\"...\"] --> B[\"...\"]", "caption": ""}], "projects": [{"name": "프로젝트 이름", "summary": "", "shipped": [], "ongoing": [], "next": []}]}"#;

const WEEK_SYSTEM_PROMPT_EN: &str = r#"You read one developer's work records for a week and write down where the week's time went and what kept getting in the way.

The input is organized by date, with each day's time blocks and summary. Each time block includes its minutes and an id: [MM-DD.T-number], or [MM-DD.P-number] for a project on a day whose summary has no time blocks. Blocker ids use [MM-DD.B-number]. Days without summaries include only recorded metrics, commits and session titles.

How to write:
- Write in English even when the input is in another language (day summaries written earlier may be). Names, code identifiers and quoted text stay as written.
- Write only what the records show. Do not guess. Use plain, short English, one sentence per item. Do not use em or en dashes (— –).
- Never put an id like [MM-DD.T1] in the text. Ids go only in the turns fields.
- Do not write minutes or hours anywhere in the text. The app works out time from the records and shows it. Dates and counts are fine.
- time: regroup the days' time blocks into 3 to 6 pieces of work for the week. Put every [MM-DD.T-number] and [MM-DD.P-number] that belongs in turns, leaving none out; each goes in exactly one place. Do not write minutes (the program adds them up). note is one sentence on how that time was spread over the week (for example, "Mostly Tuesday and Wednesday, nothing recorded on Friday").
- blockers: include only blockers that came up at least twice during the week or took the most time. Put the matching [MM-DD.B-number] ids in turns. signal states how many times each blocker occurred and on how many days. cause states the cause shown in the records. resolved is true if it was resolved within the week. fix suggests one specific change to try next time, based on these records. If no blockers qualify, leave the array empty.
- projects: for each project, a two or three sentence summary, plus shipped (work finished with a result, not necessarily released), ongoing (work with no recorded finish within the week) and next (what the records name as the next step for next week). Use an empty array when there is no evidence.
- visuals: draw 0 to 2 diagrams, only when a picture clearly shows something better than text. For example: the attempts a blocker went through before it was solved, work that ran through several steps, or how several parts connect. If a list is enough, draw nothing and leave the array empty.
  - mermaid uses Mermaid flowchart syntax only. The first line is "flowchart LR" (4 nodes or fewer) or "flowchart TD" (more). At most 8 nodes. Node text is short English in double quotes, like A["text"]. For edge text, write A -->|"text"| B. No style, classDef, class, click, links, colors or HTML.
  - title is the diagram's title; caption is one sentence explaining what it shows.
- headline: the week's title. It should show where most of the time went or a blocker that kept coming back.
  - Keywords, not a sentence. Name the week's two or three biggest pieces of work in noun phrases separated by commas. Each phrase names the work and, when the records show it, the outcome. No framing like "A week of" and no full sentences. Keep project and feature names; replace developer jargon with plain words. About 60 characters. Example: "Payments rewrite finished, notifications added, deploy automation still blocked".
- time labels and blocker titles follow the same rules.

Output exactly one JSON object shaped like this. Add no other text and no code fences.
{"headline": "", "time": [{"label": "", "project": "", "turns": ["09-22.T1"], "note": ""}], "blockers": [{"title": "", "project": "", "turns": ["09-22.B1"], "signal": "", "cause": "", "resolved": false, "fix": ""}], "visuals": [{"title": "", "mermaid": "flowchart LR\n  A[\"...\"] --> B[\"...\"]", "caption": ""}], "projects": [{"name": "project name", "summary": "", "shipped": [], "ongoing": [], "next": []}]}"#;

/// The week prompt for a run in `lang` (ADR 0011: one per language; change both).
fn week_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => WEEK_SYSTEM_PROMPT,
        Lang::En => WEEK_SYSTEM_PROMPT_EN,
    }
}

/// Monday of the week containing `date` (local calendar).
pub fn week_start(date: &str, offset: i64) -> Option<String> {
    let (start, _) = time::local_day_window(date, offset)?;
    // 1970-01-01 was a Thursday: days since epoch + 3, mod 7 == 0 on Mondays.
    let local_days = (start as i64 / 1000 + offset).div_euclid(86_400);
    let back = (local_days + 3).rem_euclid(7) as u64;
    Some(time::local_date(start - back * 86_400_000, offset))
}

fn compact_day(d: &DayDigest, lang: Lang) -> String {
    let en = lang == Lang::En;
    let mut out = String::new();
    for p in &d.projects {
        let m = &p.metrics;
        let _ = if en {
            writeln!(out, "- {}: estimated work time {} min, {} requests, {} commits", p.name, m.active_minutes, m.prompts, m.commits)
        } else {
            writeln!(out, "- {}: 작업 {}분, 요청 {}, 커밋 {}", p.name, m.active_minutes, m.prompts, m.commits)
        };
        for c in &p.commits {
            let _ = writeln!(out, "  - {} {}", if en { "commit:" } else { "커밋:" }, c.subject);
        }
        for s in &p.sessions {
            if let Some(t) = &s.title {
                let _ = writeln!(out, "  - {} {t}", if en { "session:" } else { "세션:" });
            }
            for r in s.recaps.iter().take(3) {
                let _ = writeln!(out, "  - {} {r}", if en { "summary:" } else { "요약:" });
            }
        }
    }
    out
}

/// "09-22", the day part of the ids a week summary points at.
fn day_tag(date: &str) -> &str {
    date.get(5..).unwrap_or(date)
}

/// A day's time as blocks a week can point at: the summary's own blocks, or,
/// for a day summarized before they existed (or not at all), one per project.
fn day_blocks(r: &Report, lang: Lang) -> Vec<(String, String, String, u64, String)> {
    let tag = day_tag(&r.date);
    match r.summary.as_ref().filter(|s| !s.time.is_empty()) {
        Some(s) => s
            .time
            .iter()
            .enumerate()
            .map(|(i, b)| (format!("{tag}.T{}", i + 1), b.label.clone(), b.project.clone(), b.minutes, b.note.clone()))
            .collect(),
        None => {
            // Projects worked side by side each count the shared minutes; scale
            // them down so the day's blocks add up to the day, as turns do.
            let day = r.digest.metrics.active_minutes;
            let sum: u64 = r.digest.projects.iter().map(|p| p.metrics.active_minutes).sum();
            let scale = |m: u64| if sum > day && sum > 0 { (m * day + sum / 2) / sum } else { m };
            r.digest
                .projects
                .iter()
                .enumerate()
                .filter(|(_, p)| scale(p.metrics.active_minutes) > 0)
                .map(|(i, p)| (format!("{tag}.P{}", i + 1), match lang {
                    Lang::Ko => format!("{} 작업", p.name),
                    Lang::En => format!("{} work", p.name),
                }, p.name.clone(), scale(p.metrics.active_minutes), String::new()))
                .collect()
        }
    }
}

fn day_text(r: &Report, lang: Lang) -> String {
    let en = lang == Lang::En;
    let mut out = String::new();
    for (id, label, project, minutes, note) in day_blocks(r, lang) {
        let note = if note.is_empty() { String::new() } else { format!(": {note}") };
        let _ = if en {
            writeln!(out, "- Time [{id}] {label} ({project}) {minutes} min{note}")
        } else {
            writeln!(out, "- 시간 [{id}] {label} ({project}) {minutes}분{note}")
        };
    }
    out + &match &r.summary {
        Some(s) => {
            let tag = day_tag(&r.date);
            let mut out = format!("{} {}\n", if en { "Summary:" } else { "요약:" }, s.headline);
            for (i, b) in s.blockers.iter().enumerate() {
                let _ = if en {
                    let done = if b.resolved { "resolved" } else { "unresolved" };
                    writeln!(out, "- Blocker [{tag}.B{}] {} ({}) {} min, {done}: {} / cause: {}", i + 1, b.title, b.project, b.minutes, b.signal, b.cause)
                } else {
                    let done = if b.resolved { "풀림" } else { "안 풀림" };
                    writeln!(out, "- 막힘 [{tag}.B{}] {} ({}) {}분, {done}: {} / 원인: {}", i + 1, b.title, b.project, b.minutes, b.signal, b.cause)
                };
            }
            for p in &s.projects {
                let _ = writeln!(out, "- {}: {}", p.name, p.summary);
                let (done, next) = if en { ("done", "next") } else { ("한 일", "다음") };
                for x in p.done.iter().map(|x| (done, x)).chain(p.next.iter().map(|x| (next, x))) {
                    let _ = writeln!(out, "  - {}: {}", x.0, x.1);
                }
            }
            for c in r.digest.projects.iter().flat_map(|p| &p.commits) {
                let _ = writeln!(out, "  - {} {}", if en { "commit:" } else { "커밋:" }, c.subject);
            }
            out
        }
        None => compact_day(&r.digest, lang),
    }
}

/// A week summary standing in for its days in the month's material.
fn week_in_month(text: &mut String, monday: &str, ws: &WeekSummary, lang: Lang, blocks: &mut BTreeMap<String, u64>, stuck: &mut BTreeMap<String, u64>) {
    let en = lang == Lang::En;
    let wtag = format!("W{}", day_tag(monday));
    let _ = if en {
        writeln!(text, "\n## Week of {monday} (weekly summary)\nSummary: {}", ws.headline)
    } else {
        writeln!(text, "\n## 주 {monday} (주간 요약)\n요약: {}", ws.headline)
    };
    for (i, b) in ws.time.iter().enumerate() {
        let id = format!("{wtag}.T{}", i + 1);
        let _ = if en {
            writeln!(text, "- Time [{id}] {} ({}) {} min: {}", b.label, b.project, b.minutes, b.note)
        } else {
            writeln!(text, "- 시간 [{id}] {} ({}) {}분: {}", b.label, b.project, b.minutes, b.note)
        };
        blocks.insert(id, b.minutes);
    }
    for (i, b) in ws.blockers.iter().enumerate() {
        let id = format!("{wtag}.B{}", i + 1);
        let _ = if en {
            let done = if b.resolved { "resolved" } else { "unresolved" };
            writeln!(text, "- Blocker [{id}] {} ({}) {} min, {done}: {} / cause: {}", b.title, b.project, b.minutes, b.signal, b.cause)
        } else {
            let done = if b.resolved { "풀림" } else { "안 풀림" };
            writeln!(text, "- 막힘 [{id}] {} ({}) {}분, {done}: {} / 원인: {}", b.title, b.project, b.minutes, b.signal, b.cause)
        };
        stuck.insert(id, b.minutes);
    }
    let (shipped, ongoing) = if en { ("done", "open") } else { ("나간 것", "이어지는 것") };
    for p in &ws.projects {
        let _ = writeln!(text, "- {}: {}", p.name, p.summary);
        for x in p.shipped.iter().map(|x| (shipped, x)).chain(p.ongoing.iter().map(|x| (ongoing, x))) {
            let _ = writeln!(text, "  - {}: {}", x.0, x.1);
        }
    }
}

/// What porch does to a day summary the model wrote: minutes from the turns,
/// ids out of the prose, plain diagrams only, and the language it was written
/// in. Anything the model put in `lang` is replaced.
fn finish_day(mut s: Summary, d: &DayDigest, lang: Lang) -> Summary {
    add_day_minutes(&mut s, d, lang);
    clean_prose(&mut s.time, &mut s.blockers);
    keep_visuals(&mut s.visuals);
    s.lang = Some(lang.code().into());
    s
}

/// The same for a week or a month, over the block and blocker ids it read.
fn finish_period(mut s: WeekSummary, blocks: &BTreeMap<String, u64>, stuck: &BTreeMap<String, u64>, lang: Lang) -> WeekSummary {
    add_minutes(&mut s, blocks, stuck, lang);
    clean_prose(&mut s.time, &mut s.blockers);
    keep_visuals(&mut s.visuals);
    s.lang = Some(lang.code().into());
    s
}

/// A day's block and blocker ids ("09-22.T1", "09-22.P1", "09-22.B1") with their minutes.
fn day_refs(r: &Report, blocks: &mut BTreeMap<String, u64>, stuck: &mut BTreeMap<String, u64>) {
    // Labels are unused here: only ids and minutes.
    for (id, _, _, minutes, _) in day_blocks(r, Lang::default()) {
        blocks.insert(id, minutes);
    }
    if let Some(ds) = &r.summary {
        let tag = day_tag(&r.date);
        for (i, b) in ds.blockers.iter().enumerate() {
            stuck.insert(format!("{tag}.B{}", i + 1), b.minutes);
        }
    }
}

/// Sum the minutes each time block and blocker points at; a block id counts
/// once, and unclaimed ones become one last block so the total holds.
fn add_minutes(s: &mut WeekSummary, blocks: &BTreeMap<String, u64>, stuck: &BTreeMap<String, u64>, lang: Lang) {
    let mut used = std::collections::BTreeSet::new();
    for b in &mut s.time {
        b.turns.retain(|id| blocks.contains_key(id) && used.insert(id.clone()));
        b.minutes = b.turns.iter().map(|id| blocks[id]).sum();
    }
    let rest: Vec<String> = blocks.keys().filter(|id| !used.contains(*id)).cloned().collect();
    let rest_min: u64 = rest.iter().map(|id| blocks[id]).sum();
    if rest_min > 0 {
        let label = match lang {
            Lang::Ko => "묶이지 않은 일",
            Lang::En => "Ungrouped work",
        };
        s.time.push(TimeBlock { label: label.into(), turns: rest, minutes: rest_min, ..Default::default() });
    }
    s.time.retain(|b| b.minutes > 0);
    s.time.sort_by_key(|b| std::cmp::Reverse(b.minutes));
    for b in &mut s.blockers {
        b.turns.retain(|id| stuck.contains_key(id));
        b.turns.dedup();
        b.minutes = b.turns.iter().map(|id| stuck[id]).sum();
    }
    s.blockers.sort_by_key(|b| std::cmp::Reverse(b.minutes));
}

pub struct WeekOpts {
    pub any_date: String,
    pub engine: Engine,
    pub refresh: bool,
    pub no_llm: bool,
}

pub fn build_week(o: &WeekOpts, offset: i64) -> Result<WeekReport, String> {
    let monday = week_start(&o.any_date, offset).ok_or_else(|| time::bad_date(o.engine.lang))?;
    let cache = reports_dir().join(format!("week-{monday}.json"));
    let saved = fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str::<WeekReport>(&t).ok());
    let (start, _) = time::local_day_window(&monday, offset).ok_or("date")?;
    let today = time::local_date(time::now_ms(), offset);
    let mut days = Vec::new();
    let mut day_reports: Vec<Report> = Vec::new();
    let lang = o.engine.lang;
    let mut text = match lang {
        Lang::Ko => format!("주 시작: {monday}\n"),
        Lang::En => format!("Week starting: {monday}\n"),
    };
    for i in 0..7 {
        let date = time::local_date(start + i * 86_400_000, offset);
        if date > today {
            break;
        }
        // Reuse a saved day report; never call the summarizer per day here.
        let r = build_day(
            &DayOpts { date: date.clone(), engine: o.engine.clone(), refresh: false, no_llm: true },
            offset,
        )?;
        if r.digest.projects.is_empty() {
            continue;
        }
        let _ = write!(text, "\n## {date}\n{}", day_text(&r, lang));
        days.push(DayLine {
            date: date.clone(),
            minutes: r.digest.metrics.active_minutes,
            commits: r.digest.metrics.commits,
            my_commits: r.digest.metrics.my_commits,
            cost: r.digest.metrics.usage.as_ref().map(|u| u.cost),
            tokens: r.digest.metrics.usage.as_ref().map(|u| u.tokens.total()),
            headline: r.summary.as_ref().map(|s| s.headline.clone()),
            projects: r.digest.projects.iter().map(|p| (p.name.clone(), p.metrics.active_minutes)).collect(),
        });
        day_reports.push(r);
    }
    // A saved week keeps its summary, but its days are read fresh: a day
    // summarized after the week was written should show its line here too.
    if !o.refresh {
        if let Some(mut r) = saved {
            if r.summary.is_some() || o.no_llm {
                r.days = days;
                return Ok(r);
            }
        }
    }
    let (summary, summary_error, model) = if o.no_llm || days.is_empty() {
        (None, None, None)
    } else {
        match o.engine.lang {
            Lang::Ko => eprintln!("{}로 주간 요약을 쓰는 중… ({}일)", o.engine.provider.name(), days.len()),
            Lang::En => eprintln!("Writing the weekly summary with {}… ({} days)", o.engine.provider.name(), days.len()),
        }
        let model = Some(o.engine.model_label());
        let (mut blocks, mut stuck) = (BTreeMap::new(), BTreeMap::new());
        for r in &day_reports {
            day_refs(r, &mut blocks, &mut stuck);
        }
        match writer::run(&o.engine, &text, week_prompt(o.engine.lang)) {
            Ok(v) => match serde_json::from_value::<WeekSummary>(v) {
                Ok(s) => (Some(finish_period(s, &blocks, &stuck, o.engine.lang)), None, model),
                Err(e) => {
                    let error = match o.engine.lang {
                        Lang::Ko => format!("주간 요약 JSON을 읽지 못했습니다: {e}"),
                        Lang::En => format!("Could not read the weekly summary JSON: {e}"),
                    };
                    (None, Some(error), model)
                }
            },
            Err(e) => (None, Some(e), model),
        }
    };
    let mut report = WeekReport { week_start: monday, generated_at: time::now_ms(), provider: model.as_ref().map(|_| o.engine.provider), model, days, summary, summary_error };
    let failed = keep_written!(report, WeekReport, &cache);
    fs::create_dir_all(reports_dir()).map_err(|e| e.to_string())?;
    save_json(&cache, &serde_json::to_string(&report).map_err(|e| e.to_string())?)?;
    if failed.is_none() {
        crate::mirror::save_week(&report);
    }
    report.summary_error = failed.or(report.summary_error);
    Ok(report)
}

// ---------- month ----------

const MONTH_SYSTEM_PROMPT: &str = r#"당신은 한 개발자의 한 달 작업 기록을 읽고, 그 달의 시간이 어디로 갔는지와 무엇이 반복해서 막았는지를 정리하는 도우미입니다.

입력은 주별로 있습니다. 주간 요약이 있는 주는 그 요약이, 없는 주는 날짜별 요약이나 수치가 있습니다. 시간 덩어리는 [W09-21.T번호], [09-22.T번호], [09-22.P번호] 같은 번호와 분으로, 막힌 곳은 [W09-21.B번호], [09-22.B번호]로 있습니다.

쓰는 법:
- 입력이 다른 언어여도 한국어로 씁니다(앞서 쓴 주간·하루 요약이 다른 언어일 수 있습니다). 이름과 인용한 글은 원래 표기를 씁니다.
- 기록에 있는 사실만 씁니다. 추측하지 않습니다. 쉬운 한국어로 짧게, 항목 하나는 한 문장입니다. 줄표(—)와 쌍점은 쓰지 않습니다.
- 글 안에 번호를 쓰지 않습니다. 번호는 turns 칸에만 넣습니다. 분이나 시간 숫자도 글에 쓰지 않습니다(프로그램이 더합니다). 날짜와 횟수는 씁니다.
- time: 한 달의 덩어리를 3~6개 일로 다시 묶습니다. turns에는 속하는 번호를 빠짐없이 넣고, 하나는 한 곳에만 넣습니다. note는 한 달 동안 그 시간이 어떻게 흘렀는지 한 문장입니다(예: "첫 두 주에 몰렸고 마지막 주엔 손대지 않았다").
- blockers: 두 주 이상에 걸쳐 나왔거나 가장 시간을 많이 잡아먹은 막힘만 씁니다. turns에는 해당 번호를 넣습니다. signal은 몇 주, 며칠, 몇 번인지. cause는 기록에서 보이는 원인. resolved는 달 안에 풀렸으면 true. fix는 이 기록에 맞는 구체적인 방법 하나입니다. 없으면 빈 배열입니다.
- projects: 프로젝트마다 두세 문장 요약과 나간 것(끝나서 결과가 남은 일), 이어지는 것(달 안에 끝나지 않은 일), 다음 달(기록에서 다음 단계로 언급된 일). 근거가 없으면 빈 배열입니다.
- visuals: 그림이 글보다 확실히 잘 보여 줄 때만 0~2개. Mermaid flowchart만, 첫 줄은 "flowchart LR"(노드 4개 이하) 또는 "flowchart TD", 노드 8개 이하, 노드 글은 A["글"]처럼 큰따옴표, style·classDef·class·click·링크·색·HTML 금지.
- headline: 이 달의 핵심 키워드 제목. 가장 큰 일 두세 가지를 "무엇, 어떻게 됐는지" 명사구로 쓰고 쉼표로 잇습니다. "~한 달", "~했다" 같은 끝맺음은 쓰지 않고 명사로 끝냅니다. 프로젝트·기능 이름은 쓰고 개발 용어는 쉬운 말로 바꿉니다. 40자 안팎.
- time의 label과 blockers의 title도 같은 기준입니다.

출력은 아래 모양의 JSON 하나만 냅니다. 다른 글이나 코드 블록 표시는 붙이지 않습니다.
{"headline": "", "time": [{"label": "", "project": "", "turns": ["W09-21.T1"], "note": ""}], "blockers": [{"title": "", "project": "", "turns": ["W09-21.B1"], "signal": "", "cause": "", "resolved": false, "fix": ""}], "visuals": [], "projects": [{"name": "프로젝트 이름", "summary": "", "shipped": [], "ongoing": [], "next": []}]}"#;

#[derive(Debug, Serialize, Deserialize)]
pub struct MonthReport {
    /// "2026-09"
    pub month: String,
    pub generated_at: u64,
    pub model: Option<String>,
    /// Who wrote the summary; None on reports saved before the choice existed (Claude Code).
    #[serde(default)]
    pub provider: Option<Provider>,
    /// The month's worked days, as the week report lists them.
    pub days: Vec<DayLine>,
    pub summary: Option<WeekSummary>,
    pub summary_error: Option<String>,
}

const MONTH_SYSTEM_PROMPT_EN: &str = r#"You read one developer's work records for a month and write down where the month's time went and what kept getting in the way.

The input is organized by week. Each week includes its saved weekly summary when there is one; otherwise it includes daily summaries or recorded metrics by date. Time blocks include their minutes and ids such as [W09-21.T-number], [09-22.T-number] or [09-22.P-number]. Blocker ids use [W09-21.B-number] or [09-22.B-number].

How to write:
- Write in English even when the input is in another language (week and day summaries written earlier may be). Names, code identifiers and quoted text stay as written.
- Write only what the records show. Do not guess. Use plain, short English, one sentence per item. Do not use em or en dashes (— –) or colons in the text.
- Never put an id in the text. Ids go only in the turns fields. Do not write minutes or hours in the text either (the program adds them up). Dates and counts are fine.
- time: regroup the month's blocks into 3 to 6 pieces of work. Put every id that belongs in turns, leaving none out; each goes in exactly one place. note is one sentence on how that time was spread over the month (for example, "Mostly the first two weeks, nothing recorded in the last").
- blockers: include only blockers that occurred in at least two weeks or took the most time. Put the matching ids in turns. signal states how many times each blocker occurred and across how many days and weeks. cause states the cause shown in the records. resolved is true if it was resolved within the month. fix suggests one specific change to try next time, based on these records. If no blockers qualify, leave the array empty.
- projects: for each project, a two or three sentence summary, plus shipped (work finished with a result, not necessarily released), ongoing (work with no recorded finish within the month) and next (what the records name as the next step for next month). Use an empty array when there is no evidence.
- visuals: 0 to 2, only when a picture clearly shows something better than text. Mermaid flowchart only; the first line is "flowchart LR" (4 nodes or fewer) or "flowchart TD"; at most 8 nodes; node text in double quotes, like A["text"]; no style, classDef, class, click, links, colors or HTML.
- headline: the month's title in keywords. Name the month's two or three biggest pieces of work in noun phrases separated by commas. Each phrase names the work and, when the records show it, the outcome. No framing like "A month of" and no full sentences. Keep project and feature names; replace developer jargon with plain words. About 60 characters.
- time labels and blocker titles follow the same rules.

Output exactly one JSON object shaped like this. Add no other text and no code fences.
{"headline": "", "time": [{"label": "", "project": "", "turns": ["W09-21.T1"], "note": ""}], "blockers": [{"title": "", "project": "", "turns": ["W09-21.B1"], "signal": "", "cause": "", "resolved": false, "fix": ""}], "visuals": [], "projects": [{"name": "project name", "summary": "", "shipped": [], "ongoing": [], "next": []}]}"#;

/// The month prompt for a run in `lang` (ADR 0011: one per language; change both).
fn month_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => MONTH_SYSTEM_PROMPT,
        Lang::En => MONTH_SYSTEM_PROMPT_EN,
    }
}

pub struct MonthOpts {
    /// Any date in the month, "YYYY-MM-DD".
    pub any_date: String,
    pub engine: Engine,
    pub refresh: bool,
    pub no_llm: bool,
}

/// A month from its weeks. A week written up and lying wholly inside the
/// month goes in as its summary; the others go in day by day, only the
/// month's own days, so the month's blocks add up to the month.
pub fn build_month(o: &MonthOpts, offset: i64) -> Result<MonthReport, String> {
    let month = o.any_date.get(..7).filter(|m| m.len() == 7).ok_or_else(|| time::bad_date(o.engine.lang))?.to_owned();
    let cache = reports_dir().join(format!("month-{month}.json"));
    let saved = fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str::<MonthReport>(&t).ok());
    let today = time::local_date(time::now_ms(), offset);
    let first = format!("{month}-01");

    let mut days: Vec<DayLine> = Vec::new();
    let mut blocks: BTreeMap<String, u64> = BTreeMap::new();
    let mut stuck: BTreeMap<String, u64> = BTreeMap::new();
    let lang = o.engine.lang;
    let mut text = match lang {
        Lang::Ko => format!("달: {month}\n"),
        Lang::En => format!("Month: {month}\n"),
    };
    let mut monday = week_start(&first, offset).ok_or("date")?;
    loop {
        if monday.as_str() > today.as_str() || !(monday.starts_with(&month) || monday < first) {
            break;
        }
        let week = build_week(&WeekOpts { any_date: monday.clone(), engine: o.engine.clone(), refresh: false, no_llm: true }, offset)?;
        let in_month: Vec<&DayLine> = week.days.iter().filter(|d| d.date.starts_with(&month)).collect();
        let whole = week.days.iter().all(|d| d.date.starts_with(&month)) && monday.starts_with(&month);
        // Only a week summary with time blocks can stand in for its days;
        // one written before blocks existed would drop the week's time.
        match week.summary.as_ref().filter(|ws| whole && !in_month.is_empty() && !ws.time.is_empty()) {
            Some(ws) => week_in_month(&mut text, &monday, ws, lang, &mut blocks, &mut stuck),
            None => {
                let _ = match lang {
                    Lang::Ko => writeln!(text, "\n## 주 {monday} (날짜별)"),
                    Lang::En => writeln!(text, "\n## Week of {monday} (by day)"),
                };
                for d in &in_month {
                    let r = build_day(&DayOpts { date: d.date.clone(), engine: o.engine.clone(), refresh: false, no_llm: true }, offset)?;
                    let _ = write!(text, "\n### {}\n{}", d.date, day_text(&r, lang));
                    day_refs(&r, &mut blocks, &mut stuck);
                }
            }
        }
        days.extend(in_month.into_iter().cloned());
        let next = time::local_day_window(&monday, offset).map(|(s, _)| s + 7 * 86_400_000).ok_or("date")?;
        monday = time::local_date(next, offset);
    }
    if !o.refresh {
        if let Some(mut r) = saved {
            if r.summary.is_some() || o.no_llm {
                r.days = days;
                return Ok(r);
            }
        }
    }
    let (summary, summary_error, model) = if o.no_llm || days.is_empty() {
        (None, None, None)
    } else {
        match o.engine.lang {
            Lang::Ko => eprintln!("{}로 월간 요약을 쓰는 중… ({}일)", o.engine.provider.name(), days.len()),
            Lang::En => eprintln!("Writing the monthly summary with {}… ({} days)", o.engine.provider.name(), days.len()),
        }
        let model = Some(o.engine.model_label());
        match writer::run(&o.engine, &text, month_prompt(o.engine.lang)) {
            Ok(v) => match serde_json::from_value::<WeekSummary>(v) {
                Ok(s) => (Some(finish_period(s, &blocks, &stuck, o.engine.lang)), None, model),
                Err(e) => {
                    let error = match o.engine.lang {
                        Lang::Ko => format!("월간 요약 JSON을 읽지 못했습니다: {e}"),
                        Lang::En => format!("Could not read the monthly summary JSON: {e}"),
                    };
                    (None, Some(error), model)
                }
            },
            Err(e) => (None, Some(e), model),
        }
    };
    let mut report = MonthReport { month, generated_at: time::now_ms(), provider: model.as_ref().map(|_| o.engine.provider), model, days, summary, summary_error };
    let failed = keep_written!(report, MonthReport, &cache);
    fs::create_dir_all(reports_dir()).map_err(|e| e.to_string())?;
    save_json(&cache, &serde_json::to_string(&report).map_err(|e| e.to_string())?)?;
    if failed.is_none() {
        crate::mirror::save_month(&report);
    }
    report.summary_error = failed.or(report.summary_error);
    Ok(report)
}

// ---------- usage over days ----------

#[derive(Debug, Clone, Serialize)]
pub struct UsageDay {
    pub date: String,
    pub usage: crate::usage::Usage,
    pub minutes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageGroup {
    pub name: String,
    pub usage: crate::usage::Usage,
    /// Cost and tokens per step of the span: per day over several days, per
    /// hour for one day (tokens per hour are not recorded, so empty then).
    pub series_cost: Vec<f64>,
    pub series_tokens: Vec<u64>,
}

/// Tokens and list-price cost for `days` days ending at `end` (included).
#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    /// Last day of the range.
    pub end: String,
    pub days: Vec<UsageDay>,
    pub total: crate::usage::Usage,
    pub active_minutes: u64,
    pub by_project: Vec<UsageGroup>,
    pub by_model: Vec<UsageGroup>,
    pub by_agent: Vec<UsageGroup>,
    /// Days in range whose transcripts are gone, so their usage is unknown.
    pub unknown_days: Vec<String>,
    /// This calendar month so far, for the budget.
    pub month_cost: f64,
    pub month_tokens: u64,
    pub monthly_budget: Option<f64>,
    pub prices_as_of: &'static str,
}

type Series = BTreeMap<String, (Vec<f64>, Vec<u64>)>;

fn group(map: BTreeMap<String, crate::usage::Usage>, series: &mut Series) -> Vec<UsageGroup> {
    let mut v: Vec<UsageGroup> = map
        .into_iter()
        .map(|(name, usage)| {
            let (series_cost, series_tokens) = series.remove(&name).unwrap_or_default();
            UsageGroup { name, usage, series_cost, series_tokens }
        })
        .collect();
    v.sort_by(|a, b| b.usage.cost.total_cmp(&a.usage.cost).then(b.usage.tokens.total().cmp(&a.usage.tokens.total())));
    v
}

/// `end` defaults to today; a later date is clamped to today. The budget
/// always covers this calendar month, whatever range is shown.
/// The usage page's row for sessions run in scratch folders.
/// Sessions in scratch folders spend but belong to no project: their own row.
fn scratch_group(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => "임시 폴더",
        Lang::En => "Temporary folders",
    }
}

fn agent_label(agent: &str) -> &str {
    match agent {
        "codex" => "Codex",
        "claude" => "Claude Code",
        other => other,
    }
}

pub fn usage_report(days: u64, end: Option<&str>, offset: i64) -> Result<UsageReport, String> {
    let lang = Lang::current();
    use crate::usage::Usage;
    let today = time::local_date(time::now_ms(), offset);
    let end = match end {
        Some(e) if e < today.as_str() => e.to_owned(),
        _ => today.clone(),
    };
    let (end_start, _) = time::local_day_window(&end, offset).ok_or("date")?;
    let mut dates: Vec<String> = (0..days.max(1)).rev().map(|b| time::local_date(end_start.saturating_sub(b * 86_400_000) + 3_600_000, offset)).collect();
    dates.dedup();
    let month = today[..7].to_owned();
    let settings = crate::settings::load();
    // With a budget, the month so far may reach further back than the range.
    let first = format!("{month}-01");
    let mut all_dates = dates.clone();
    if settings.monthly_budget.is_some() {
        let (s, _) = time::local_day_window(&first, offset).ok_or("date")?;
        let mut d = s;
        loop {
            let date = time::local_date(d, offset);
            if date > today {
                break;
            }
            if !all_dates.contains(&date) {
                all_dates.push(date);
            }
            d += 86_400_000;
        }
    }
    let (mut total, mut by_project, mut by_model, mut by_agent) = (Usage::default(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let (mut out_days, mut unknown, mut minutes) = (Vec::new(), Vec::new(), 0u64);
    // Per group: cost and tokens at each day of the range (or each hour for one day).
    let steps = if days > 1 { dates.len() } else { 24 };
    let (mut s_project, mut s_model, mut s_agent): (Series, Series, Series) = Default::default();
    let bump = |m: &mut Series, name: &str, i: usize, cost: f64, toks: u64| {
        let e = m.entry(name.to_owned()).or_insert_with(|| (vec![0.0; steps], if days > 1 { vec![0; steps] } else { Vec::new() }));
        if i < e.0.len() {
            e.0[i] += cost;
        }
        if i < e.1.len() {
            e.1[i] += toks;
        }
    };
    let hours = |u: &Usage| -> Vec<f64> { if u.by_hour.len() == 24 { u.by_hour.clone() } else { vec![0.0; 24] } };
    let add_group = |groups: &mut BTreeMap<String, Usage>, series: &mut Series, name: &str, di: usize, u: &Usage| {
        groups.entry(name.to_owned()).or_default().merge(u);
        if days > 1 {
            bump(series, name, di, u.cost, u.tokens.total());
        } else {
            for (h, c) in hours(u).into_iter().enumerate() {
                bump(series, name, h, c, 0);
            }
        }
    };
    let (mut month_cost, mut month_tokens) = (0.0, 0u64);
    for date in &all_dates {
        let r = build_day(&DayOpts { date: date.clone(), engine: Engine::default(), refresh: false, no_llm: true }, offset)?;
        let Some(u) = r.digest.metrics.usage.clone() else {
            if dates.contains(date) && !r.digest.projects.is_empty() {
                unknown.push(date.clone());
            }
            continue;
        };
        if date.starts_with(&month) {
            month_cost += u.cost;
            month_tokens += u.tokens.total();
        }
        if !dates.contains(date) {
            continue;
        }
        total.merge(&u);
        minutes += r.digest.metrics.active_minutes;
        let di = dates.iter().position(|d| d == date).unwrap_or(0);
        for (name, pu) in &r.digest.project_usage {
            add_group(&mut by_project, &mut s_project, name, di, pu);
        }
        for p in &r.digest.projects {
            // Days saved before day totals were kept: take them from the projects.
            if r.digest.project_usage.is_empty() {
                if let Some(pu) = &p.metrics.usage {
                    add_group(&mut by_project, &mut s_project, &p.name, di, pu);
                }
            }
            // Days saved before agent totals were kept: add up their sessions.
            if r.digest.agent_usage.is_empty() {
                for s in &p.sessions {
                    add_group(&mut by_agent, &mut s_agent, agent_label(&s.agent), di, &s.usage);
                }
            }
        }
        for (agent, au) in &r.digest.agent_usage {
            add_group(&mut by_agent, &mut s_agent, agent_label(agent), di, au);
        }
        // Scratch-folder sessions are spent but belong to no project: their own row.
        for su in r.digest.scratch_usage.values() {
            add_group(&mut by_project, &mut s_project, scratch_group(lang), di, su);
        }
        for (m, mu) in &u.by_model {
            let name = crate::usage::short_model(m);
            let e: &mut Usage = by_model.entry(name.clone()).or_default();
            e.tokens.add(&mu.tokens);
            match mu.cost {
                Some(c) => e.cost += c,
                None => e.unpriced += mu.tokens.total(),
            }
            // Hours are not kept per model, so a single day has no model trend.
            if days > 1 {
                bump(&mut s_model, &name, di, mu.cost.unwrap_or(0.0), mu.tokens.total());
            }
        }
        out_days.push(UsageDay { date: date.clone(), usage: u, minutes: r.digest.metrics.active_minutes });
    }
    // Summed over several days, hours add up the same hour of each day.
    if days > 1 {
        total.by_hour = Vec::new();
    }
    Ok(UsageReport {
        end,
        days: out_days,
        total,
        active_minutes: minutes,
        by_project: group(by_project, &mut s_project),
        by_model: group(by_model, &mut s_model),
        by_agent: group(by_agent, &mut s_agent),
        unknown_days: unknown,
        month_cost,
        month_tokens,
        monthly_budget: settings.monthly_budget,
        prices_as_of: crate::usage::PRICES_AS_OF,
    })
}

/// One entry per saved day or week, newest first; what the calendar needs.
#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub days: Vec<ListedDay>,
    pub weeks: Vec<ListedWeek>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListedDay {
    pub date: String,
    pub headline: Option<String>,
    pub minutes: u64,
    pub commits: usize,
    /// Items first written down this day that are still open now.
    pub waiting: usize,
    pub issues: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListedWeek {
    pub week_start: String,
    pub headline: Option<String>,
}

pub fn list_reports() -> Listing {
    let ledger = crate::open::load(saved_days);
    let still_open = |date: &str, kind: crate::open::Kind| ledger.iter().filter(|i| i.since == date && i.kind == kind && i.closed_on.is_none()).count();
    let mut days = Vec::new();
    let mut weeks = Vec::new();
    for f in fs::read_dir(reports_dir()).into_iter().flatten().flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".json") else { continue };
        let Ok(text) = fs::read_to_string(f.path()) else { continue };
        if let Some(ws) = stem.strip_prefix("week-") {
            if let Ok(w) = serde_json::from_str::<WeekReport>(&text) {
                weeks.push(ListedWeek { week_start: ws.to_owned(), headline: w.summary.map(|s| s.headline) });
            }
        } else if let Ok(r) = serde_json::from_str::<Report>(&text) {
            // Week builds save record-only reports for idle days too; those are not "recorded".
            if r.digest.projects.is_empty() {
                continue;
            }
            days.push(ListedDay {
                date: stem.to_owned(),
                headline: r.summary.map(|s| s.headline),
                minutes: r.digest.metrics.active_minutes,
                commits: r.digest.metrics.commits,
                waiting: still_open(stem, crate::open::Kind::Waiting),
                issues: still_open(stem, crate::open::Kind::Issue),
            });
        }
    }
    days.sort_by(|a, b| b.date.cmp(&a.date));
    weeks.sort_by(|a, b| b.week_start.cmp(&a.week_start));
    Listing { days, weeks }
}

/// A saved day report, without generating anything.
pub fn load_day(date: &str) -> Option<Report> {
    serde_json::from_str(&fs::read_to_string(reports_dir().join(format!("{date}.json"))).ok()?).ok()
}

/// A saved week report, without generating anything.
pub fn load_week(week_start: &str) -> Option<WeekReport> {
    serde_json::from_str(&fs::read_to_string(reports_dir().join(format!("week-{week_start}.json"))).ok()?).ok()
}

/// A saved month report, without generating anything.
pub fn load_month(month: &str) -> Option<MonthReport> {
    load_month_in(&reports_dir(), month)
}

fn load_month_in(dir: &std::path::Path, month: &str) -> Option<MonthReport> {
    serde_json::from_str(&fs::read_to_string(dir.join(format!("month-{month}.json"))).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::{ProjectDigest, SessionDigest, Turn};
    use crate::lang::Lang;

    fn two_turn_day() -> DayDigest {
        let turn = |id: &str, ms: u64| Turn { id: id.into(), active_ms: ms, ..Default::default() };
        DayDigest {
            projects: vec![ProjectDigest {
                name: "p".into(),
                sessions: vec![SessionDigest { turns: vec![turn("t1", 600_000), turn("t2", 300_000)], ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn a_finished_day_carries_its_language_and_names_unclaimed_turns_in_it() {
        let model = Summary {
            headline: "h".into(),
            lang: Some("fr".into()), // the model's own "lang" is never kept
            time: vec![TimeBlock { label: "Login".into(), turns: vec!["t1".into(), "t9".into()], ..Default::default() }],
            ..Default::default()
        };
        let en = finish_day(model.clone(), &two_turn_day(), Lang::En);
        assert_eq!(en.lang.as_deref(), Some("en"));
        assert_eq!(en.time.iter().map(|b| b.minutes).sum::<u64>(), 15);
        assert!(en.time.iter().any(|b| b.label == "Ungrouped requests" && b.turns == ["t2"]), "{:?}", en.time);
        let ko = finish_day(model, &two_turn_day(), Lang::Ko);
        assert_eq!(ko.lang.as_deref(), Some("ko"));
        assert!(ko.time.iter().any(|b| b.label == "묶이지 않은 요청"));
    }

    #[test]
    fn a_finished_period_carries_its_language_and_names_unclaimed_work_in_it() {
        let blocks: BTreeMap<String, u64> = [("09-22.T1".to_owned(), 30), ("09-23.T1".to_owned(), 20)].into();
        let stuck = BTreeMap::new();
        let model = WeekSummary {
            headline: "h".into(),
            time: vec![TimeBlock { label: "A".into(), turns: vec!["09-22.T1".into(), "made-up".into()], ..Default::default() }],
            ..Default::default()
        };
        let en = finish_period(model.clone(), &blocks, &stuck, Lang::En);
        assert_eq!(en.lang.as_deref(), Some("en"));
        assert_eq!(en.time.iter().map(|b| b.minutes).sum::<u64>(), 50);
        assert!(en.time.iter().any(|b| b.label == "Ungrouped work"));
        assert!(finish_period(model, &blocks, &stuck, Lang::Ko).time.iter().any(|b| b.label == "묶이지 않은 일"));
    }

    #[test]
    fn the_usual_line_in_both_languages() {
        let u = Usual { days: 9, day: 240, projects: [("porch".to_owned(), (300, 3))].into() };
        assert_eq!(usual_line(&u, Lang::Ko), "평소(최근 기록된 9일): 하루 평균 240분, porch은 일한 날 평균 100분(3일)\n");
        assert_eq!(usual_line(&u, Lang::En), "Recent average (last 9 recorded days): 240 min a day, porch: average 100 min per recorded workday (3 days)\n");
    }

    fn day_report_with(summary: Option<Summary>) -> Report {
        Report {
            date: "2026-09-22".into(),
            generated_at: 0,
            model: None,
            provider: None,
            digest: DayDigest {
                metrics: crate::digest::Metrics { active_minutes: 90, ..Default::default() },
                projects: vec![ProjectDigest { name: "porch".into(), metrics: crate::digest::Metrics { active_minutes: 90, prompts: 4, commits: 2, ..Default::default() }, ..Default::default() }],
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

    #[test]
    fn lang_or_prefers_the_summary_language() {
        let r = day_report_with(Some(Summary { lang: Some("ko".into()), ..Default::default() }));
        assert_eq!(r.lang_or(Lang::En), Lang::Ko);
        assert_eq!(day_report_with(None).lang_or(Lang::En), Lang::En);
        let old = day_report_with(Some(Summary::default())); // saved before languages: Korean
        assert_eq!(old.lang_or(Lang::En), Lang::Ko);
    }

    #[test]
    fn week_material_labels_are_english_around_korean_summaries() {
        let ko_summary = Summary {
            headline: "트레이 고침".into(),
            time: vec![TimeBlock { label: "트레이".into(), project: "porch".into(), minutes: 90, ..Default::default() }],
            blockers: vec![Blocker { title: "테스트 실패".into(), project: "porch".into(), signal: "3번".into(), cause: "원인".into(), minutes: 30, ..Default::default() }],
            ..Default::default()
        };
        let en = day_text(&day_report_with(Some(ko_summary)), Lang::En);
        let labels: String = en.lines().map(|l| l.split(['[', ':']).next().unwrap_or("")).collect();
        assert!(!crate::lang::has_hangul(&labels), "{en}");
        for w in ["- Time [09-22.T1]", "Summary: 트레이 고침", "- Blocker [09-22.B1]", "unresolved"] {
            assert!(en.contains(w), "{w}\n{en}");
        }
        // A day with no summary: one block per project, named in the language.
        let bare = day_text(&day_report_with(None), Lang::En);
        assert!(bare.contains("- Time [09-22.P1] porch work (porch) 90 min"), "{bare}");
        assert!(!crate::lang::has_hangul(&bare), "{bare}");
        assert!(day_text(&day_report_with(None), Lang::Ko).contains("- 시간 [09-22.P1] porch 작업 (porch) 90분"));
    }

    #[test]
    fn a_week_summary_goes_into_the_month_in_the_language() {
        let ws = WeekSummary {
            headline: "Tray fixed".into(),
            time: vec![TimeBlock { label: "Tray".into(), project: "porch".into(), minutes: 90, note: "Mostly Monday".into(), ..Default::default() }],
            blockers: vec![Blocker { title: "Tests".into(), project: "porch".into(), minutes: 30, resolved: true, signal: "3 times".into(), cause: "flaky".into(), ..Default::default() }],
            projects: vec![WeekProject { name: "porch".into(), summary: "s".into(), shipped: vec!["tray".into()], ..Default::default() }],
            ..Default::default()
        };
        let (mut text, mut blocks, mut stuck) = (String::new(), BTreeMap::new(), BTreeMap::new());
        week_in_month(&mut text, "2026-09-21", &ws, Lang::En, &mut blocks, &mut stuck);
        assert!(!crate::lang::has_hangul(&text), "{text}");
        assert!(text.contains("## Week of 2026-09-21 (weekly summary)") && text.contains("- Time [W09-21.T1] Tray (porch) 90 min: Mostly Monday"), "{text}");
        assert!(text.contains("- Blocker [W09-21.B1] Tests (porch) 30 min, resolved: 3 times / cause: flaky"), "{text}");
        assert_eq!((blocks["W09-21.T1"], stuck["W09-21.B1"]), (90, 30));
        let mut ko = String::new();
        week_in_month(&mut ko, "2026-09-21", &ws, Lang::Ko, &mut BTreeMap::new(), &mut BTreeMap::new());
        assert!(ko.contains("## 주 2026-09-21 (주간 요약)") && ko.contains("- 막힘 [W09-21.B1] Tests (porch) 30분, 풀림: 3 times / 원인: flaky"));
    }

    #[test]
    fn scratch_folders_are_named_in_the_language() {
        assert_eq!((scratch_group(Lang::Ko), scratch_group(Lang::En)), ("임시 폴더", "Temporary folders"));
    }

    #[test]
    fn a_failed_rewrite_keeps_the_saved_language() {
        let t = tempfile::tempdir().unwrap();
        let cache = t.path().join("week-2026-09-28.json");
        let week = |summary: Option<WeekSummary>, error: Option<&str>| WeekReport {
            week_start: "2026-09-28".into(),
            generated_at: 0,
            model: Some("sonnet".into()),
            provider: None,
            days: vec![],
            summary,
            summary_error: error.map(str::to_owned),
        };
        let ko = WeekSummary { headline: "지난 요약".into(), lang: Some("ko".into()), ..Default::default() };
        std::fs::write(&cache, serde_json::to_string(&week(Some(ko), None)).unwrap()).unwrap();
        let mut r = week(None, Some("Codex hit its usage limit. x"));
        assert!(keep_written!(r, WeekReport, &cache).is_some());
        assert_eq!(r.summary.unwrap().lang(), Lang::Ko);
    }

    #[test]
    fn a_summary_saved_without_a_language_reads_as_korean() {
        let s: Summary = serde_json::from_str(r#"{"headline":"h"}"#).unwrap();
        assert_eq!((s.lang.as_deref(), s.lang()), (None, Lang::Ko));
        let w: WeekSummary = serde_json::from_str(r#"{"headline":"h","lang":"en"}"#).unwrap();
        assert_eq!(w.lang(), Lang::En);
        // A records-only refresh copies the summary whole; its language goes with it.
        let back: Summary = serde_json::from_str(&serde_json::to_string(&Summary { lang: Some("ko".into()), ..s }).unwrap()).unwrap();
        assert_eq!(back.lang.as_deref(), Some("ko"));
    }

    #[test]
    fn a_failed_rewrite_keeps_the_saved_summary() {
        let t = tempfile::tempdir().unwrap();
        let cache = t.path().join("week-2026-09-28.json");
        let week = |summary: Option<&str>, error: Option<&str>, model: &str| WeekReport {
            week_start: "2026-09-28".into(),
            generated_at: 0,
            model: Some(model.into()),
            provider: None,
            days: vec![],
            summary: summary.map(|h| WeekSummary { headline: h.into(), ..Default::default() }),
            summary_error: error.map(str::to_owned),
        };
        std::fs::write(&cache, serde_json::to_string(&week(Some("old"), None, "sonnet")).unwrap()).unwrap();

        let mut r = week(None, Some("Codex 사용 한도에 걸렸습니다."), "default");
        let failed = keep_written!(r, WeekReport, &cache);
        assert_eq!(failed.as_deref(), Some("Codex 사용 한도에 걸렸습니다."));
        assert_eq!((r.summary.unwrap().headline.as_str(), r.model.as_deref(), r.summary_error), ("old", Some("sonnet"), None));

        // Nothing saved before: the error stays on the report.
        let mut r = week(None, Some("x"), "default");
        assert!(keep_written!(r, WeekReport, &t.path().join("none.json")).is_none());
        assert_eq!(r.summary_error.as_deref(), Some("x"));

        // A rewrite that worked replaces the old one.
        let mut r = week(Some("new"), None, "default");
        assert!(keep_written!(r, WeekReport, &cache).is_none());
        assert_eq!(r.summary.unwrap().headline, "new");
    }

    #[test]
    fn load_month_reads_the_saved_month() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("reports");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("month-2026-09.json"), r#"{"month":"2026-09","generated_at":0,"model":null,"days":[],"summary":null,"summary_error":null}"#).unwrap();
        assert_eq!(load_month_in(&dir, "2026-09").map(|m| m.month).as_deref(), Some("2026-09"));
        assert!(load_month_in(&dir, "2026-08").is_none());
    }

    fn usage_day(sessions: &[(&str, u64)], scratch: u64) -> DayDigest {
        let usage = |input: u64| {
            let mut u = crate::usage::Usage::default();
            u.record("claude-opus-5", &crate::usage::Tokens { input, ..Default::default() }, Some(9));
            u
        };
        let total: u64 = sessions.iter().map(|(_, n)| n).sum::<u64>() + scratch;
        let mut d = DayDigest {
            projects: vec![digest::ProjectDigest {
                name: "app".into(),
                metrics: digest::Metrics { usage: Some(usage(sessions.iter().map(|(_, n)| n).sum())), ..Default::default() },
                sessions: sessions.iter().map(|(id, n)| digest::SessionDigest { session: (*id).into(), usage: usage(*n), ..Default::default() }).collect(),
                ..Default::default()
            }],
            metrics: digest::Metrics { usage: Some(usage(total)), ..Default::default() },
            ..Default::default()
        };
        if scratch > 0 {
            d.scratch_usage.insert("claude".into(), usage(scratch));
        }
        d
    }

    #[test]
    fn saved_usage_is_counted_again_when_every_session_is_still_there() {
        let mut saved = report_with("2026-09-13", vec![]);
        saved.digest = usage_day(&[("a", 500), ("b", 300)], 0);
        let mut fresh = usage_day(&[("a", 400), ("b", 300), ("c", 9)], 20);
        fresh.usage_counting = digest::USAGE_COUNTING;
        fresh.agent_usage.insert("codex".into(), fresh.projects[0].sessions[2].usage.clone());
        fresh.project_usage.insert("app".into(), fresh.projects[0].metrics.usage.clone().unwrap());
        fresh.project_usage.insert("missed".into(), fresh.projects[0].sessions[2].usage.clone());
        recount_usage(&mut saved, &fresh);
        assert_eq!(saved.digest.metrics.usage.as_ref().unwrap().tokens.input, 729);
        assert_eq!(saved.digest.projects[0].metrics.usage.as_ref().unwrap().tokens.input, 709);
        assert_eq!(saved.digest.projects[0].sessions[0].usage.tokens.input, 400);
        assert_eq!(saved.digest.scratch_usage["claude"].tokens.input, 20);
        // A session the saved report never listed still counts for its agent.
        assert_eq!(saved.digest.agent_usage["codex"].tokens.input, 9);
        // So does a project it never listed; the saved project list stays as written.
        assert_eq!(saved.digest.project_usage["missed"].tokens.input, 9);
        assert_eq!(saved.digest.projects.len(), 1);
        assert_eq!(saved.digest.usage_counting, digest::USAGE_COUNTING);
        assert_eq!(saved.summary.as_ref().unwrap().headline, "h", "the summary stays");
    }

    #[test]
    fn saved_usage_stays_when_a_session_file_is_gone() {
        let mut saved = report_with("2026-08-01", vec![]);
        saved.digest = usage_day(&[("a", 500), ("b", 300)], 0);
        let fresh = usage_day(&[("b", 300)], 0);
        recount_usage(&mut saved, &fresh);
        assert_eq!(saved.digest.metrics.usage.as_ref().unwrap().tokens.input, 800);
        assert_eq!(saved.digest.usage_counting, digest::USAGE_COUNTING, "not tried again: the file will not come back");
    }

    #[test]
    fn saved_report_reprices_all_usage_groups_without_changing_summary() {
        let mut report = report_with("2026-09-01", vec![]);
        let mut usage = crate::usage::Usage::default();
        usage.record("deepseek/deepseek-v4-flash", &crate::usage::Tokens {
            input: 1_000_000, ..Default::default()
        }, Some(9));
        usage.cost = 0.0;
        usage.unpriced = 1_000_000;
        usage.by_model.values_mut().for_each(|m| m.cost = None);
        report.digest.metrics.usage = Some(usage.clone());
        report.digest.projects.push(digest::ProjectDigest {
            metrics: digest::Metrics { usage: Some(usage.clone()), ..Default::default() },
            sessions: vec![digest::SessionDigest { usage: usage.clone(), ..Default::default() }],
            ..Default::default()
        });
        report.digest.agent_usage.insert("claude".into(), usage.clone());
        report.digest.project_usage.insert("app".into(), usage.clone());
        report.digest.scratch_usage.insert("claude".into(), usage);
        assert!(reprice_usage(&mut report));
        assert_eq!(report.summary.as_ref().unwrap().headline, "h");
        for usage in [report.digest.metrics.usage.as_ref().unwrap(),
            report.digest.projects[0].metrics.usage.as_ref().unwrap(),
            &report.digest.projects[0].sessions[0].usage,
            &report.digest.agent_usage["claude"], &report.digest.project_usage["app"],
            &report.digest.scratch_usage["claude"]] {
            assert!((usage.cost - 0.04186).abs() < 1e-12);
            assert_eq!(usage.unpriced, 0);
        }
        assert!(!reprice_usage(&mut report));
    }

    #[test]
    fn monday_of_week() {
        // 2026-09-28 is a Monday, 2026-09-27 a Sunday.
        assert_eq!(week_start("2026-09-28", 9 * 3600).as_deref(), Some("2026-09-28"));
        assert_eq!(week_start("2026-09-27", 9 * 3600).as_deref(), Some("2026-09-21"));
        assert_eq!(week_start("2026-10-04", 9 * 3600).as_deref(), Some("2026-09-28"));
        assert_eq!(week_start("2026-09-30", -5 * 3600).as_deref(), Some("2026-09-28"));
    }

    #[test]
    fn ids_leave_the_prose() {
        assert_eq!(strip_ids("같은 요청 4번(t10, t12, t13, t15), 2분 안에"), "같은 요청 4번, 2분 안에");
        assert_eq!(strip_ids("t24 38분, 도구 39회"), "38분, 도구 39회");
        assert_eq!(strip_ids("테스트 실패 확인 (t5)"), "테스트 실패 확인");
        assert_eq!(strip_ids("t7에서 22분 사용, 도구 오류 1번"), "22분 사용, 도구 오류 1번");
        assert_eq!(strip_ids("09-22.B1, 09-24.B2와 같은 문제"), "같은 문제");
        assert_eq!(strip_ids("cargo test 실패 6번"), "cargo test 실패 6번");
    }

    #[test]
    fn blockers_without_kind_still_read() {
        let b: Blocker = serde_json::from_str(r#"{"title":"x","resolved":false}"#).unwrap();
        assert_eq!(b.kind, "");
    }

    #[test]
    fn day_prompt_names_every_blocker_kind() {
        for p in [SYSTEM_PROMPT, SYSTEM_PROMPT_EN] {
            for k in BLOCKER_KINDS {
                assert!(p.contains(k), "{k}");
            }
            assert!(p.contains(r#""kind": """#));
        }
    }

    #[test]
    fn english_day_prompt_has_no_korean_and_asks_for_the_same_json() {
        assert!(!crate::lang::has_hangul(SYSTEM_PROMPT_EN));
        assert_eq!(crate::lang::example_keys(SYSTEM_PROMPT), crate::lang::example_keys(SYSTEM_PROMPT_EN));
        assert_eq!((day_prompt(Lang::Ko), day_prompt(Lang::En)), (SYSTEM_PROMPT, SYSTEM_PROMPT_EN));
    }

    #[test]
    fn english_week_and_month_prompts_match_the_korean_ones() {
        for (ko, en) in [(WEEK_SYSTEM_PROMPT, WEEK_SYSTEM_PROMPT_EN), (MONTH_SYSTEM_PROMPT, MONTH_SYSTEM_PROMPT_EN)] {
            assert!(!crate::lang::has_hangul(en));
            assert_eq!(crate::lang::example_keys(ko), crate::lang::example_keys(en));
            for w in ["flowchart LR", "flowchart TD", "turns"] {
                assert!(en.contains(w), "{w}");
            }
        }
        assert_eq!((week_prompt(Lang::En), month_prompt(Lang::En)), (WEEK_SYSTEM_PROMPT_EN, MONTH_SYSTEM_PROMPT_EN));
        assert_eq!((week_prompt(Lang::Ko), month_prompt(Lang::Ko)), (WEEK_SYSTEM_PROMPT, MONTH_SYSTEM_PROMPT));
    }

    #[test]
    fn prompts_say_to_keep_their_language_whatever_the_input() {
        for p in [SYSTEM_PROMPT_EN, WEEK_SYSTEM_PROMPT_EN, MONTH_SYSTEM_PROMPT_EN] {
            assert!(p.contains(ENGLISH_EVEN_IF), "{}", &p[..80]);
        }
        for p in [SYSTEM_PROMPT, WEEK_SYSTEM_PROMPT, MONTH_SYSTEM_PROMPT] {
            assert!(p.contains(KOREAN_EVEN_IF), "{}", &p[..80]);
        }
    }

    #[test]
    fn english_day_prompt_names_the_labels_english_material_uses() {
        for w in ["\"Recent average\"", "\"Open items\"", "(other: name)", "Request", "Failed", "Final reply", "The records do not show the cause"] {
            assert!(SYSTEM_PROMPT_EN.contains(w), "{w}");
        }
    }

    #[test]
    fn visuals_keep_only_plain_flowcharts() {
        let v = |m: &str| Visual { title: "t".into(), mermaid: m.into(), caption: String::new() };
        let mut all = vec![
            v("flowchart LR\n  A[\"시도\"] --> B[\"해결\"]"),
            v("graph LR\n A --> B"),
            v("flowchart TD\n A --> B\n click A \"https://x\""),
            v("flowchart TD\n A --> B\n style A fill:#f00"),
            v("flowchart TD\n A[\"<img src=x>\"]"),
        ];
        keep_visuals(&mut all);
        assert_eq!(all.len(), 1);
        assert!(all[0].mermaid.contains("시도"));
    }

    #[test]
    fn day_minutes_come_from_turns() {
        use crate::digest::{ProjectDigest, SessionDigest, Turn};
        let turn = |id: &str, min: u64| Turn { id: id.into(), active_ms: min * 60_000, prompt: "p".into(), ..Default::default() };
        let d = DayDigest {
            projects: vec![ProjectDigest {
                name: "app".into(),
                sessions: vec![SessionDigest { turns: vec![turn("t1", 30), turn("t2", 10), turn("t3", 5)], ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut s = Summary {
            headline: "h".into(),
            time: vec![
                TimeBlock { label: "a".into(), turns: vec!["t1".into(), "t9".into()], minutes: 999, ..Default::default() },
                TimeBlock { label: "b".into(), turns: vec!["t1".into(), "t2".into()], ..Default::default() },
            ],
            blockers: vec![Blocker { title: "x".into(), turns: vec!["t2".into(), "t3".into()], ..Default::default() }],
            ..Default::default()
        };
        add_day_minutes(&mut s, &d, Lang::Ko);
        // t9 is made up and dropped, t1 counts once, t3 lands in the leftover block.
        let got: Vec<(&str, u64)> = s.time.iter().map(|b| (b.label.as_str(), b.minutes)).collect();
        assert_eq!(got, [("a", 30), ("b", 10), ("묶이지 않은 요청", 5)]);
        assert_eq!(s.blockers[0].minutes, 15);
    }

    #[test]
    fn compact_day_of_empty_digest_is_empty() {
        let d = DayDigest::default();
        assert!(compact_day(&d, Lang::Ko).is_empty());
    }

    #[test]
    fn turn_files_are_copied_by_session_and_start() {
        use crate::digest::{ProjectDigest, SessionDigest, Turn};
        let day = |turns: Vec<Turn>| DayDigest {
            projects: vec![ProjectDigest { sessions: vec![SessionDigest { session: "s1".into(), turns, ..Default::default() }], ..Default::default() }],
            ..Default::default()
        };
        let t = |id: &str, start: u64| Turn { id: id.into(), start, ..Default::default() };
        // Saved before a transcript was cleaned up: the fresh read numbers the same turn t1, not t3.
        let mut saved = day(vec![t("t2", 100), t("t3", 200)]);
        let mut fresh_turn = t("t1", 200);
        fresh_turn.files = vec!["/r/b/x.rs".into()];
        fresh_turn.root = Some("/r/b".into());
        let fresh = day(vec![fresh_turn]);
        assert_eq!(copy_turn_files(&mut saved, &fresh), 1);
        let turns = &saved.projects[0].sessions[0].turns;
        assert_eq!((turns[0].root.as_deref(), turns[1].root.as_deref()), (None, Some("/r/b")));
        assert_eq!(turns[1].id, "t3");
        assert_eq!(turns[1].files, ["/r/b/x.rs"]);
    }

    #[test]
    fn old_reports_read_without_turn_files() {
        let r: Report = serde_json::from_str(r#"{"date":"2026-09-01","generated_at":0,"model":null,"digest":{"date":"2026-09-01","start":0,"end":0,"projects":[],"metrics":{"sessions":0,"prompts":0,"active_minutes":0,"tool_calls":0,"tool_errors":0,"denials":0,"files_touched":0,"commits":0,"insertions":0,"deletions":0,"by_hour":[]}},"summary":null,"summary_error":null}"#).unwrap();
        assert_eq!(r.turn_files_read, None);
    }

    fn report_with(date: &str, blockers: Vec<Blocker>) -> Report {
        Report {
            date: date.into(),
            generated_at: 0,
            model: None,
            provider: None,
            digest: DayDigest { date: date.into(), ..Default::default() },
            summary: Some(Summary { headline: "h".into(), blockers, ..Default::default() }),
            summary_error: None,
            usual_minutes: None,
            usage_checked: true,
            turn_files_read: Some(true),
            dropped_pivots: 0,
        }
    }

    fn blocker_titled(title: &str, kind: &str) -> Blocker {
        Blocker { title: title.into(), kind: kind.into(), signal: "같은 요청 3번".into(), ..Default::default() }
    }

    #[test]
    fn only_blockers_without_a_kind_are_numbered() {
        let reports = [
            report_with("2026-09-01", vec![blocker_titled("a", ""), blocker_titled("b", "env")]),
            report_with("2026-09-02", vec![blocker_titled("c", "")]),
        ];
        let refs = unclassified(&reports);
        assert_eq!(refs, [(0, 0), (1, 0)]);
        let text = classify_prompt(&reports, &refs);
        assert!(text.contains("[B1] a") && text.contains("[B2] c") && !text.contains("] b"), "{text}");
    }

    #[test]
    fn kinds_land_on_the_numbered_blockers_only() {
        let mut reports = [
            report_with("2026-09-01", vec![blocker_titled("a", ""), blocker_titled("b", "env")]),
            report_with("2026-09-02", vec![blocker_titled("c", ""), blocker_titled("d", "")]),
            report_with("2026-09-03", vec![blocker_titled("e", "")]),
        ];
        let refs = unclassified(&reports);
        // B9 is made up; B3 has no answer; B4 gets a kind outside the list, kept as written.
        let answer = serde_json::json!({"B1": "misread", "B2": " test_fail ", "B4": "flaky", "B9": "env"});
        let changed = apply_kinds(&mut reports, &refs, &answer);
        let kinds = |r: &Report| r.summary.as_ref().unwrap().blockers.iter().map(|b| b.kind.clone()).collect::<Vec<_>>();
        assert_eq!(kinds(&reports[0]), ["misread", "env"]);
        assert_eq!(kinds(&reports[1]), ["test_fail", ""]);
        assert_eq!(kinds(&reports[2]), ["flaky"]);
        assert_eq!(changed, [0, 1, 2].into_iter().collect());
    }

    #[test]
    fn classify_prompt_names_every_blocker_kind() {
        for k in BLOCKER_KINDS {
            assert!(CLASSIFY_PROMPT.contains(&format!("{k}(")), "{k}");
            assert!(CLASSIFY_PROMPT_EN.contains(&format!("{k} (")), "{k}");
        }
        assert!(!crate::lang::has_hangul(CLASSIFY_PROMPT_EN));
        assert_eq!((classify_prompt_for(Lang::Ko), classify_prompt_for(Lang::En)), (CLASSIFY_PROMPT, CLASSIFY_PROMPT_EN));
    }

    fn day_with(turns: &[(&str, &str)]) -> DayDigest {
        let turns = turns.iter().map(|(id, p)| crate::digest::Turn { id: (*id).into(), prompt: (*p).into(), ..Default::default() }).collect();
        DayDigest {
            projects: vec![ProjectDigest { name: "porch".into(), sessions: vec![crate::digest::SessionDigest { turns, ..Default::default() }], ..Default::default() }],
            ..Default::default()
        }
    }

    fn pivot(turn: &str, kind: &str, quote: &str) -> Pivot {
        Pivot { turn: turn.into(), kind: kind.into(), quote: quote.into(), ..Default::default() }
    }

    #[test]
    fn clean_pivots_keeps_only_quotes_from_the_turn() {
        let d = day_with(&[("t1", "어떻게 할지 좋은 전략안내봐"), ("t9", "요청 다시 쓰기는   결정 변경 이력 아래로\n내리라고")]);
        let mut s = Summary {
            pivots: vec![
                pivot("t9", "redo", "“요청 다시 쓰기는 결정 변경 이력 아래로 내리라고”"),
                pivot("t7", "redo", "요청 다시 쓰기는"),
                pivot("t9", "redo", "섹션을 아래로 옮겨"),
                pivot("t9", "change", "요청 다시 쓰기는"),
                pivot("t9", "shift", ""),
            ],
            asks: vec![
                Ask { turn: "t1".into(), quote: "좋은 전략안내봐…".into(), missing: vec!["원하는 화면 형태".into(), " ".into()] },
                Ask { turn: "t1".into(), quote: "전략을 짜줘".into(), missing: vec![] },
            ],
            ..Default::default()
        };
        assert_eq!(clean_pivots(&mut s, &d), 5);
        assert_eq!(s.pivots.len(), 1);
        assert_eq!(s.pivots[0].quote, "요청 다시 쓰기는 결정 변경 이력 아래로 내리라고");
        assert_eq!(s.asks.len(), 1);
        assert_eq!(s.asks[0].quote, "좋은 전략안내봐");
        assert_eq!(s.asks[0].missing, ["원하는 화면 형태"]);
    }

    #[test]
    fn clean_pivots_caps_a_day_and_cuts_long_quotes() {
        let long = "가".repeat(120);
        let d = day_with(&[("t1", long.as_str())]);
        let mut s = Summary { pivots: (0..10).map(|_| pivot("t1", "shift", &long)).collect(), ..Default::default() };
        assert_eq!(clean_pivots(&mut s, &d), 2);
        assert_eq!(s.pivots.len(), 8);
        assert_eq!(s.pivots[0].quote.chars().count(), 90);
    }

    #[test]
    fn summaries_saved_before_pivots_read_with_none() {
        let s: Summary = serde_json::from_str(r#"{"headline": "h"}"#).unwrap();
        assert!(s.pivots.is_empty() && s.asks.is_empty());
        let p: Pivot = serde_json::from_str(r#"{"turn": "t1", "type": "redo", "quote": "q"}"#).unwrap();
        assert_eq!((p.kind.as_str(), p.note.as_str()), ("redo", ""));
    }

    #[test]
    fn day_prompts_ask_for_pivots_and_asks() {
        for p in [SYSTEM_PROMPT, SYSTEM_PROMPT_EN] {
            for w in ["pivots", "asks", "shift", "revert", "redo", "reason_given", "missing", "\"note\""] {
                assert!(p.contains(w), "{w}");
            }
        }
    }
}

// ---------- projects across days ----------

#[derive(Debug, Clone, Serialize)]
pub struct ProjectOverview {
    pub name: String,
    pub root: String,
    pub days: usize,
    pub minutes: u64,
    pub commits: usize,
    pub first_date: String,
    pub last_date: String,
    /// The latest day's one-paragraph summary, when that day was summarized.
    pub latest_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDay {
    pub date: String,
    pub minutes: u64,
    pub sessions: usize,
    pub commits: Vec<String>,
    pub summary: Option<String>,
    pub done: Vec<String>,
    pub next: Vec<String>,
}

/// Every saved day with work in it, oldest first.
pub fn saved_days() -> Vec<Report> {
    let mut out: Vec<Report> = fs::read_dir(reports_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|f| {
            let n = f.file_name().to_string_lossy().into_owned();
            n.ends_with(".json") && !n.starts_with("week-")
        })
        .filter_map(|f| serde_json::from_str::<Report>(&fs::read_to_string(f.path()).ok()?).ok())
        .filter(|r| !r.digest.projects.is_empty())
        .collect();
    out.sort_by(|a, b| a.date.cmp(&b.date));
    out
}

/// Every project seen in saved day reports, most recently worked on first.
pub fn projects() -> Vec<ProjectOverview> {
    let mut by_root: BTreeMap<String, ProjectOverview> = BTreeMap::new();
    for r in saved_days() {
        for p in &r.digest.projects {
            let summary = r.summary.as_ref().and_then(|s| s.projects.iter().find(|x| x.name == p.name)).map(|x| x.summary.clone());
            let o = by_root.entry(p.root.clone()).or_insert_with(|| ProjectOverview {
                name: p.name.clone(),
                root: p.root.clone(),
                days: 0,
                minutes: 0,
                commits: 0,
                first_date: r.date.clone(),
                last_date: r.date.clone(),
                latest_summary: None,
            });
            o.days += 1;
            o.minutes += p.metrics.active_minutes;
            o.commits += p.commits.len();
            o.last_date = r.date.clone();
            if summary.is_some() {
                o.latest_summary = summary;
            }
        }
    }
    let mut v: Vec<ProjectOverview> = by_root.into_values().collect();
    v.sort_by(|a, b| b.last_date.cmp(&a.last_date).then(b.minutes.cmp(&a.minutes)));
    v
}

/// One project's days, newest first.
pub fn project_days(root: &str) -> Vec<ProjectDay> {
    let mut out: Vec<ProjectDay> = saved_days()
        .into_iter()
        .filter_map(|r| {
            let p = r.digest.projects.iter().find(|p| p.root == root)?;
            let ps = r.summary.as_ref().and_then(|s| s.projects.iter().find(|x| x.name == p.name));
            Some(ProjectDay {
                date: r.date.clone(),
                minutes: p.metrics.active_minutes,
                sessions: p.sessions.len(),
                commits: p.commits.iter().map(|c| c.subject.clone()).collect(),
                summary: ps.map(|x| x.summary.clone()),
                done: ps.map(|x| x.done.clone()).unwrap_or_default(),
                next: ps.map(|x| x.next.clone()).unwrap_or_default(),
            })
        })
        .collect();
    out.reverse();
    out
}

/// Save record-only reports for past days that have none yet, so projects and
/// the calendar reach further back. Never calls the summarizer. Returns how
/// many days were added.
pub fn backfill(days: u64, offset: i64) -> Result<usize, String> {
    let today = time::local_date(time::now_ms(), offset);
    let (start, _) = time::local_day_window(&today, offset).ok_or("date")?;
    let mut added = 0;
    // New days reach back `days`; turn files are filled twice as far, so both
    // windows the health checks compare are turn-attributed while the transcripts last.
    for back in 1..=days.saturating_mul(2) {
        let date = time::local_date(start - back * 86_400_000, offset);
        let path = reports_dir().join(format!("{date}.json"));
        if path.exists() {
            // Days saved before turns carried their files get them now, while the transcripts last.
            if let Some(mut r) = load_day(&date).filter(|r| r.turn_files_read.is_none()) {
                fill_turn_files(&mut r, offset);
                if let Ok(t) = serde_json::to_string(&r) {
                    let _ = save_json(&path, &t);
                }
            }
            continue;
        }
        if back > days {
            continue;
        }
        let r = build_day(&DayOpts { date, engine: Engine::default(), refresh: false, no_llm: true }, offset)?;
        if !r.digest.projects.is_empty() {
            added += 1;
        }
    }
    Ok(added)
}

// ---------- kinds for blockers summarized before kinds existed ----------

const CLASSIFY_PROMPT: &str = r#"당신은 개발 작업 중 막힌 곳의 기록을 읽고 유형을 하나씩 붙이는 도우미입니다.

입력은 막힌 곳마다 한 줄입니다: [B번호] 제목 / 근거 / 원인.
각 B번호에 아래 중 하나만 붙입니다. test_fail(테스트·타입 검사·빌드 실패가 반복됨), auth_external(인증, 외부 서비스, 스토어 업로드처럼 이 저장소 밖에서 막힘), env(작업 폴더, 브랜치, 파일 충돌, 도구 설치 같은 작업 환경), misread(요구가 전달되지 않아 같은 요청을 다시 함), review_loop(검토에서 수정 요청이 반복됨), slow(한 작업이 오래 돌거나 끝나지 않음), other(위에 없는 것).
기록에 있는 것만 보고 고릅니다. 애매하면 other입니다.

출력은 {"B1": "misread", "B2": "env"} 모양의 JSON 하나만 냅니다. 다른 글이나 코드 블록 표시는 붙이지 않습니다."#;

const CLASSIFY_PROMPT_EN: &str = r#"You read records of where development work got stuck and classify each blocker.

The input has one line per blocker: [B-number] title / signal / cause.
Give each B-number exactly one category. test_fail (tests, type checks or builds failed repeatedly), auth_external (blocked outside this repo: authentication, an external service, a store upload), env (the local environment: folders, branches, file conflicts, tool installation), misread (the request was misunderstood, so the same request was sent again), review_loop (review asked for revisions again and again), slow (one task took a long time or never finished), other (none of these).
Choose only from what the record shows. When unsure, other.

Output exactly one JSON object shaped like {"B1": "misread", "B2": "env"}. Add no other text and no code fences."#;

/// The classify prompt for a run in `lang` (ADR 0011: one per language; change both).
fn classify_prompt_for(lang: Lang) -> &'static str {
    match lang {
        Lang::Ko => CLASSIFY_PROMPT,
        Lang::En => CLASSIFY_PROMPT_EN,
    }
}

/// Blockers without a kind, as (report index, blocker index); the n-th is "B{n}".
fn unclassified(reports: &[Report]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (ri, r) in reports.iter().enumerate() {
        for (bi, b) in r.summary.iter().flat_map(|s| s.blockers.iter()).enumerate() {
            if b.kind.trim().is_empty() {
                out.push((ri, bi));
            }
        }
    }
    out
}

/// What the classifier reads: titles, signals and causes only, never prompts.
fn classify_prompt(reports: &[Report], refs: &[(usize, usize)]) -> String {
    let mut out = String::new();
    for (n, &(ri, bi)) in refs.iter().enumerate() {
        let b = &reports[ri].summary.as_ref().expect("numbered from a summary").blockers[bi];
        let _ = writeln!(out, "[B{}] {} / {} / {}", n + 1, b.title, b.signal, b.cause);
    }
    out
}

/// Write the classifier's answer onto the numbered blockers. Ids it made up and
/// blockers it left out stay as they were; a kind outside the list is kept as
/// written (health counts it as unknown). Returns the reports that changed.
fn apply_kinds(reports: &mut [Report], refs: &[(usize, usize)], answer: &serde_json::Value) -> std::collections::BTreeSet<usize> {
    let mut changed = std::collections::BTreeSet::new();
    for (n, &(ri, bi)) in refs.iter().enumerate() {
        let Some(kind) = answer[format!("B{}", n + 1)].as_str().map(str::trim).filter(|k| !k.is_empty()) else { continue };
        if let Some(b) = reports[ri].summary.as_mut().and_then(|s| s.blockers.get_mut(bi)) {
            b.kind = kind.to_owned();
            changed.insert(ri);
        }
    }
    changed
}

/// Give blockers from summaries written before kinds existed a kind, with one
/// Claude Code call over their titles, signals and causes. Nothing else in the
/// saved reports changes. Returns how many blockers got a kind.
pub fn classify_old_blockers(engine: &Engine) -> Result<usize, String> {
    let mut reports = saved_days();
    let refs = unclassified(&reports);
    if refs.is_empty() {
        return Ok(0);
    }
    let answer = writer::run(engine, &classify_prompt(&reports, &refs), classify_prompt_for(engine.lang))?;
    let changed = apply_kinds(&mut reports, &refs, &answer);
    for &ri in &changed {
        let r = &reports[ri];
        let text = serde_json::to_string(r).map_err(|e| e.to_string())?;
        save_json(&reports_dir().join(format!("{}.json", r.date)), &text)?;
    }
    Ok(refs
        .iter()
        .filter(|&&(ri, bi)| reports[ri].summary.as_ref().is_some_and(|s| !s.blockers[bi].kind.is_empty()))
        .count())
}
