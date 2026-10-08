# 랜딩 페이지 카피

랜딩의 모든 블록에 들어갈 문구다. 한국어로 사실을 맞추고, 영어는 같은 사실을 옮긴다. §1–§7과 장면 데이터는 `site/src/i18n/*.ts`에서 `pnpm appendix`로 생성한다.

문체 기준: `README.md`. 앱 문구 기준: `app/src/components/ui.tsx`,
`app/src/components/DayView.tsx`, `app/src/screens/Popover.tsx`, `app/src/screens/Usage.tsx`,
`app/src/screens/settings/GeneralTab.tsx`, `app/src/format.ts`.

## 0 Nav

EN: `porch.` · How it works · Privacy · FAQ · EN/KO · [Download] (ghost)

KO: `porch.` · 작동 방식 · 개인정보 · 자주 묻는 질문 · EN/KO · [다운로드] (ghost)

## 1 Hero

| | EN | KO |
| --- | --- | --- |
| Eyebrow | {agents:amp} · MACOS | {agents:dot} · 맥용 |
| Headline | Your work with {agents}, / _summarized_ by day, week and month. | {agents:로} 한 일을 / 하루, 한 주, 한 달 단위로 정리합니다. |
| Subhead 1 | Claude Code or Codex writes it up, with time by task and where you got stuck. | 작업별 시간과 막힌 작업까지 클로드코드나 코덱스가 정리해 줍니다. |
| Condition | Summaries need Claude Code or Codex signed in. | 요약은 클로드코드나 코덱스 로그인이 필요합니다. |
| CTA | Download for Mac | 맥용 다운로드 |
| CTA aside (serif) | macOS · Apple Silicon | macOS · Apple Silicon |
| Agent link | Let your agent install it → | 에이전트에게 설치 맡기기 → |
| Hero video caption | Real app screens with demo data. Work time is estimated from records. | 예시 데이터로 찍은 실제 앱 화면입니다. 작업 시간은 기록으로 추정한 값입니다. |
| Hero video label / play / pause | Porch app screen recording / Play / Pause | Porch 앱 화면 영상 / 재생 / 일시정지 |
| English UI note |  | (none) |

Templates: `{agents…}` expands from `site/src/agents.ts`; `_word_` is the serif accent (Latin only).

## 2 Story · works (scene s2)

| | EN | KO |
| --- | --- | --- |
| Eyebrow | Any terminal | 어느 터미널에서든 |
| Headline | Sessions from any terminal, in one list. | 어느 터미널에서 시작한 세션이든 한 목록에 모입니다. |
| Body 1 | Ghostty, iTerm, VS Code or Orca: {agents} sessions land in one list. Sessions waiting for permission or an answer, and sessions stopped on an error, come first. The menu bar shows their count. | Ghostty, iTerm, VS Code, Orca 어디서 실행했든 {agents} 세션을 한곳에서 봅니다. 허락이나 답을 기다리는 세션과 오류로 멈춘 세션이 맨 위에 오고, 메뉴바에 그 수가 뜹니다. |
| Caption | Only Claude Code sessions show Stopped on error. | 오류로 멈춤은 클로드코드만 표시합니다. |

## 3 Story · time (scene s4)

| | EN | KO |
| --- | --- | --- |
| Eyebrow | Time by task | 작업별 시간 |
| Headline | See how long each task took. | 작업마다 걸린 시간을 보여 줍니다. |
| Body 1 | Claude Code or Codex groups requests into tasks; Porch adds up their time from the records, and the tasks total the day. | 클로드코드나 코덱스가 요청을 작업으로 묶고, Porch가 기록으로 시간을 더합니다. 합은 그날 작업 시간과 같습니다. |
| Caption | Estimated from the records; breaks over 10 minutes are left out. | 기록으로 추정한 시간입니다. 10분 넘게 쉰 구간은 뺍니다. |

## 4 Story · stuck (scene s5)

| | EN | KO |
| --- | --- | --- |
| Eyebrow | Stuck tasks | 막힌 작업 |
| Headline | See where you got stuck, with evidence. | 같은 오류나 요청이 반복된 작업을 찾아 줍니다. |
| Body 1 | Claude Code or Codex writes the evidence, such as the same error 12 times, the cause and one thing to change. Unresolved blockers carry over to the next day. | 같은 오류 12번 같은 근거와 원인, 다음에 줄일 방법 하나를 클로드코드나 코덱스가 씁니다. 안 풀린 막힘은 다음 날로 이어집니다. |
| Caption |  |  |

## 5 Story · delivered (scene s9)

| | EN | KO |
| --- | --- | --- |
| Eyebrow | Notes | 노트 |
| Headline | Summaries saved as Obsidian notes. | 요약이 옵시디언 노트로 쌓입니다. |
| Body 1 | Connect an Obsidian vault or a folder to save daily, weekly and monthly summaries as Markdown notes; weekly and monthly notes link the daily and weekly ones. With Daily auto-summary and Summary notifications on, a one-line summary arrives when an automatic summary finishes. | 옵시디언 볼트나 폴더를 연결하면 일·주·월 요약이 마크다운 노트로 저장되고, 주·월 노트에는 일·주 노트 링크가 들어갑니다. 매일 자동 요약과 요약 알림을 켜면 자동 요약이 끝날 때 한 줄 요약이 알림으로 옵니다. |
| Caption | Saving or regenerating a summary overwrites its Porch note, including edits you made there. Keep personal notes in a separate note and link to it. | 요약을 저장하거나 다시 만들면 그 요약의 Porch 노트를 덮어씁니다. 직접 고친 내용도 바뀌므로 개인 메모는 다른 노트에 적고 이 노트를 링크해 두세요. |

## 6 Story · suggest (scene s8)

| | EN | KO |
| --- | --- | --- |
| Eyebrow | Suggestions | 제안 |
| Headline | Suggestions for problems that keep coming back. | 반복되는 문제마다 바꿔 볼 점을 제안합니다. |
| Body 1 | For each repeated blocker, Claude Code or Codex proposes something to try, such as a CLAUDE.md line or a hook. Porch never edits your config files. Apply it yourself and Porch compares counts before and after, without calling it the cause. | 반복된 막힘마다 클로드코드나 코덱스가 CLAUDE.md 문구나 훅처럼 바꿔 볼 것을 제안합니다. Porch는 설정 파일을 고치지 않습니다. 직접 적용하면 전후 건수를 비교하되, 원인으로 단정하지 않습니다. |
| Caption | Making suggestions sends blocker records and your CLAUDE.md and AGENTS.md to the agent you picked in Settings, Claude Code or Codex. | 제안을 만들 때 막힘 기록과 CLAUDE.md·AGENTS.md 내용이 설정에서 고른 클로드코드나 코덱스로 갑니다. |

## 7 Story · usage (scene s6)

| | EN | KO |
| --- | --- | --- |
| Eyebrow | Usage | 사용량 |
| Headline | See tokens and limits in one place. | 토큰 사용량과 남은 한도를 한곳에서 봅니다. |
| Body 1 | API-equivalent cost and tokens for today, 7 or 30 days, by project and model. | 오늘·7일·30일 단위로 환산 비용과 토큰을 프로젝트별, 모델별로 봅니다. |
| Caption | API-equivalent cost uses public API prices, not your bill. Claude limits need a Pro or Max plan and a setting; Codex limits are the last values in its records. | 환산 비용은 공개 API 가격 기준이라 구독 결제액과 다릅니다. 클로드 한도는 Pro·Max 구독에서 설정을 켜야 나오고, 코덱스 한도는 기록에 남은 마지막 값입니다. |

## S Scene data

| Item | EN | KO |
| --- | --- | --- |
| Card date | Tue, Sep 29 | 9월 29일 (화) |
| Card title | Checkout cleanup, order alert fix | 결제 화면 정리, 주문 알림 오류 수정 |
| Card stats (template) | Work time {time} · {projects} projects · {sessions} sessions · {requests} requests · {commits} commits | 작업 시간 {time} · 프로젝트 {projects} · 세션 {sessions} · 요청 {requests} · 커밋 {commits} |
| S1/S5 section labels | Time by task / Stuck tasks | 작업별 시간 / 막힌 작업 |
| Card task 1 | Add login | 로그인 기능 추가 |
| Card task 2 | Order alert fix | 주문 알림 오류 수정 |
| S1/S5 stuck | Same error 12×, same request 3× · resolved | 같은 오류 12번, 같은 요청 3번 · 풀림 |
| Timeline projects | shop-web, shop-api, porch-site | shop-web, shop-api, porch-site |
| S2 window 1 | Ghostty · $ claude | Ghostty · $ claude |
| S2 window 2 | iTerm2 · $ codex | iTerm2 · $ codex |
| S2 window 3 | VS Code · $ claude | VS Code · $ claude |
| S2 session 1 (bright) | Checkout cleanup / Needs permission · Bash · shop-web | 결제 화면 정리 / 허락 필요 · Bash · shop-web |
| S2 session 2 (ring) | Order alert fix / Done · shop-api | 주문 알림 오류 수정 / 완료 · shop-api |
| S2 session 3 (grey) | Add login / Working · porch-site | 로그인 기능 추가 / 작업 중 · porch-site |
| S8 suggestion card | Test for orderId before touching retries · Add to CLAUDE.md · 14 days after: 2 test failures (6 in the 14 days before) | 재시도 전에 orderId를 확인하는 테스트부터 · CLAUDE.md에 추가 · 적용 후 14일: 테스트 실패 2건 (적용 전 14일 6건) |
| S9 vault / note files | Notes / porch/daily/2026-09-28.md, porch/daily/2026-09-29.md, porch/weekly/2026-W39.md | 노트 / porch/daily/2026-09-28.md, porch/daily/2026-09-29.md, porch/weekly/2026-W39.md |
| S9 note title / headline | Tue, Sep 29, 2026 / Checkout cleanup, order alert fix | 2026년 9월 29일 (화) / 결제 화면 정리, 주문 알림 오류 수정 |
| S9 note section | Time by task | 작업별 시간 |
| S9 note row 1 | Add login / 1h 14m | 로그인 기능 추가 / 1시간 14분 |
| S9 note row 2 | Order alert fix / 52m | 주문 알림 오류 수정 / 52분 |
| S5 open item 1 | Pending · Settle the refund button label | 남은 일 · 환불 버튼 문구 정하기 |
| S5 open item 2 | Blocker · Order alerts sometimes send twice | 막힘 · 주문 알림이 두 번 갈 때가 있음 |
| S6 limit 1 | 5-hour / 64% used / resets at 15:40 | 5시간 / 64% 사용 / 15:40에 다시 참 |
| S6 limit 2 | Weekly / 41% used / resets Thu 09:00 | 주간 / 41% 사용 / 목 09:00에 다시 참 |

## 8 Compare

Eyebrow: Other tools / 다른 도구

Headline (EN):
> How Porch differs from other tools.

Headline (KO):
> Porch는 다른 도구와 이렇게 다릅니다.

O/X table: one row per feature, one column per tool, Porch first. Marks and sources live in `site/src/compare.ts`
(checked 2026-10-01). A mark other than "–" needs the tool's own page to say so; X only where it says no.

| Feature (EN) | 기능 (KO) | Porch | Chit | ClaudeUsageBar | claudebill | Orca | Langfuse |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code and Codex | 클로드코드·코덱스 둘 다 | O | X | – | – | O | O |
| Sessions from any terminal in one list | 어느 터미널에서 띄운 세션이든 한 목록 | O | – | – | – | △ | – |
| Shows sessions waiting on you | 내 확인이 필요한 세션 표시 | O | – | – | – | O | – |
| Day and week summaries | 하루·주간 요약 | O | O | – | – | – | – |
| Time by task | 작업별 시간 | O | X | – | – | – | – |
| Stuck tasks with evidence | 막힌 작업과 근거 | O | – | – | – | – | – |
| Suggestions for repeated blockers, with before/after counts | 반복된 막힘에 대한 제안과 적용 전후 비교 | O | – | – | – | – | – |
| Usage and limits | 사용량·한도 | O | – | O | △ | O | △ |
| Runs and manages agents | 에이전트 실행·관리 | X | X | – | – | O | – |

Legend:
- EN: O yes · △ partly · X no (stated by the tool) · – not confirmed
- KO: O 있음 · △ 일부 · X 없음(자료에 명시) · – 확인하지 못함

Line under the table:
- EN: Orca runs your agents; Porch reads what they did. You can use both.
- KO: Orca는 에이전트를 돌리고, Porch는 그 기록을 읽습니다. 함께 쓸 수 있습니다.

Stamp: As of Oct 1, 2026 · Sources: (links) / 2026년 10월 1일 기준 · 출처: (링크)

## 9 Privacy

Eyebrow: Privacy / 개인정보

Headline (EN):
> No Porch account, and no Porch server for your records.

Headline (KO):
> Porch 계정도, 작업 기록을 받는 Porch 서버도 없습니다.

Three columns (source: README "개인정보", docs/privacy.md, ADR 0006 usage analytics, ADR 0007 suggestions).

| | EN | KO |
| --- | --- | --- |
| Reads | Your Claude Code and Codex session records (token counts included) and your projects' git history on this Mac. | 이 맥의 클로드코드·코덱스 세션 기록(토큰 수 포함)과 프로젝트의 git 이력을 읽습니다. |
| Stores | Records and summaries are saved on this Mac. Notes, if you turn them on, go to a folder you pick, which may sync (through iCloud, for example). | 기록과 요약은 이 맥에 저장합니다. 노트를 켜면 내가 고른 폴더에 쓰고, 그 폴더는 iCloud 등으로 동기화될 수 있습니다. |
| Sends | Summaries, suggestions, blocker classification and the weekly evaluation send what they need to the model through Claude Code (to Anthropic) or Codex (to OpenAI), whichever you pick in Settings. Suggestions, made only when you ask, also send CLAUDE.md and AGENTS.md. Anonymous usage statistics are on by default; turn them off with Send app usage events in Settings. | 요약, 제안, 막힘 유형 분류, 주간 평가에 필요한 기록을 설정에서 고른 클로드코드(Anthropic)나 코덱스(OpenAI)를 거쳐 모델로 보냅니다. 요청할 때만 만드는 제안에는 CLAUDE.md·AGENTS.md도 함께 보냅니다. 익명 사용 통계는 기본으로 켜져 있고, 설정의 사용 기록 보내기에서 끌 수 있습니다. |

Line under: EN "Porch also goes online to check for updates." / KO "업데이트 확인에도 인터넷을 씁니다."

Site line (source: ADR 0006 site analytics; short on purpose, no vendor name): EN "This site collects visit statistics; it stores a cookie only if you allow it. Change this in Cookie settings at the bottom of the page." / KO "이 사이트는 방문 통계를 모으며, 쿠키는 허용했을 때만 저장합니다. 맨 아래 쿠키 설정에서 바꿀 수 있습니다."

## 10 Setup

Eyebrow: Get started / 시작하기

Headline (EN): > Three steps.
Headline (KO): > 세 단계면 됩니다.

Steps (source: README "시작하기", Onboarding labels):

| # | EN | KO |
| --- | --- | --- |
| 1 | Download Porch.dmg and move Porch to Applications. | Porch.dmg를 받아 Porch를 응용 프로그램 폴더로 옮깁니다. |
| 2 | Open Porch and click **Turn on session detection**. Your existing settings are backed up and other tools' settings are left alone. Codex asks you to approve once on its next run. | Porch를 열고 **세션 감지 켜기**를 누릅니다. 기존 설정은 백업하고, 다른 도구의 설정은 건드리지 않습니다. 코덱스는 다음 실행 때 한 번 승인해 주세요. |
| 3 | In **Summary**, click **Make summary**. Today opens first. | **요약** 화면에서 **요약 만들기**를 누릅니다. 처음에는 오늘이 열립니다. |

Agent install block (id `#agent-install`, target of the hero's secondary link):
- EN label: Or paste this into Claude Code or Codex.
- KO label: 또는 클로드코드나 코덱스에 붙여 넣으세요.
- Code block (copy button, mono), one per language:
  - EN: `Install and open Porch on this Mac. The installer is https://getporch.pages.dev/Porch.dmg — open it and move Porch.app to the Applications folder.`
  - KO: `이 맥에 Porch를 설치하고 실행해 줘. 설치 파일은 https://getporch.pages.dev/Porch.dmg 이고, 열어서 Porch.app을 응용 프로그램 폴더로 옮기면 돼.`

Requirements line: EN "Apple Silicon Mac. Written summaries need Claude Code or Codex installed and signed in." / KO "Apple Silicon 맥. 글로 된 요약에는 클로드코드나 코덱스 설치와 로그인이 필요합니다."

## 11 FAQ

Headline: FAQ / 자주 묻는 질문

| Q (KO) | A (KO) | Source |
| --- | --- | --- |
| 에이전트가 느려지지 않나요? | 훅은 사건 이름, 시각, 도구 이름만 적고 바로 끝납니다. 어떤 경우에도 에이전트를 멈추거나 오류를 내지 않습니다. | AGENTS.md hard rules |
| 클로드코드 설정을 바꾸나요? | 세션 감지를 켜면 훅을 추가합니다. 기존 설정은 백업하고, 다른 도구가 넣은 항목은 건드리지 않습니다. | README, install.rs |
| 대화 내용이 밖으로 나가나요? | 요약과 제안을 만들 때만 갑니다. 요약에는 필요한 기록 일부가, 제안에는 막힘 기록과 CLAUDE.md·AGENTS.md 내용이 이 맥에서 고른 클로드코드나 코덱스를 거쳐 모델로 갑니다. 작업 현황과 사용량은 보내지 않습니다. | README, ADR 0007 |
| 코덱스만 써도 되나요? | 됩니다. 작업 시간, 타임라인, 사용량, 작업 현황을 볼 수 있고, 설정에서 고르면 요약도 코덱스가 씁니다. | README |
| 환산 비용은 실제로 낸 돈인가요? | 아닙니다. 토큰에 공개 API 가격을 곱한 값이며 실제 구독 결제액이 아닙니다. 일과 프로젝트를 비교하는 기준으로 씁니다. 가격을 모르는 모델은 토큰만 셉니다. | README |
| 인텔 맥에서도 되나요? | 지금은 Apple Silicon 맥만 지원합니다. | README |
| 특정 폴더는 빼고 싶어요. | 설정의 제외 폴더에 넣으면 그 아래 세션은 작업 현황, 기록, 요약 어디에도 들어가지 않습니다. 임시 폴더는 요약에서 기본으로 빠집니다. | GeneralTab.tsx (제외 폴더 설명) |

| Q (EN) | A (EN) |
| --- | --- |
| Will Porch slow down my agent? | The hook records the event name, time and tool name, then exits. It never stops your agent or makes it fail. |
| Does Porch change my Claude Code settings? | Turning on session detection adds hooks. Porch backs up your settings and leaves entries from other tools alone. |
| Does conversation content leave my Mac? | Only when you make a summary or suggestions. Summaries send the excerpts they need; suggestions send blocker records and your CLAUDE.md and AGENTS.md files, through the Claude Code or Codex on this Mac, whichever you picked. Session status and usage stay on your Mac. |
| Can I use Porch with Codex alone? | Yes. You get work time, the timeline, usage and session status, and Codex can write the summaries: pick it in Settings. |
| Is API-equivalent cost what I actually paid? | No. It is token counts × public API prices, not your subscription bill. Use it to compare tasks and projects. For models with unknown prices, Porch counts tokens only. |
| Does Porch work on Intel Macs? | Porch currently supports Apple Silicon Macs only. |
| Can I leave a folder out? | Add it to excluded folders in Settings. Sessions under it are left out of session status, history and summaries. Temporary folders are left out of summaries by default. |

## 12 Agents

Eyebrow: Agents / 에이전트

Headline (EN):
> Supported features for each agent.

Headline (KO):
> 에이전트별 지원 기능을 확인합니다.

Table. Columns come from `site/src/agents.ts`; one column per supported agent.

| 기능 (KO) | Feature (EN) | 클로드코드 (KO / EN) | 코덱스 (KO / EN) | Source |
| --- | --- | --- | --- | --- |
| 허락 필요 | Needs permission | 됨 / Supported | 됨 / Supported | states.md, data-sources.md |
| 질문 있음 | Has a question | 됨 / Supported | 확인 필요 / Not yet verified | states.md |
| 오류로 멈춤 | Stopped on error | 됨 / Supported | 안 됨 (해당 훅 없음) / Not supported (no matching hook) | states.md |
| 세션 끝 확인과 목록 숨김 | Session end and hiding | 세션 목록에서 프로세스가 살아 있는지 확인. 끝난 세션은 바로 숨김 / Checks whether the session's process is still alive. Ended sessions are hidden at once | 끝났는지 확인하지 못함. 12시간 넘게 사건이 없으면 숨김 / Can't confirm the end. Hidden after more than 12 hours without events | states.md |
| 작업 시간, 타임라인 | Work time, timeline | 됨 / Supported | 됨 / Supported | README |
| 글로 된 요약 | Written summary | 설정에서 고르면 클로드코드가 씀 (설치·로그인 필요) / Written by Claude Code when picked in Settings (installed and signed in) | 설정에서 고르면 코덱스가 씀 (설치·로그인 필요). 어느 쪽이든 두 에이전트 기록을 함께 요약 / Written by Codex when picked in Settings (installed and signed in). Either agent summarizes both agents' records | README, ADR 0005 |
| 토큰, 환산 비용 | Tokens, API-equivalent cost | 됨. 하위 에이전트 포함. 가격을 모르는 모델은 토큰만 / Supported, subagents included. Tokens only for models with unknown prices | 됨. 가격을 모르는 모델은 토큰만 / Supported. Tokens only for models with unknown prices | README, ui.md |
| 5시간·주간 한도 | 5-hour and weekly limits | Pro·Max 구독 필요. 설정에서 켠 뒤 클로드코드에서 다음 답이 오면 나타남. 상태 표시줄을 따로 둔 프로젝트는 따로 연결 / Needs Pro or Max. Appears after Claude Code's next reply once turned on in Settings. Projects with their own status line need a separate connection | 코덱스 기록에 남은 마지막 값 / Last values in Codex's records | README, ui.md |
| 설치 | Setup | 훅 추가 / Adds hooks | 훅 추가, 다음 실행 때 한 번 승인 / Adds hooks; approve once on the next run | README, data-sources.md |

## 13 Close and footer

Close — two cards side by side:
- Left card h2: EN "Porch summarizes today's work, estimated time by task and stuck tasks. Written summaries need Claude Code or Codex installed and signed in." / KO "오늘 한 일과 작업마다 걸린 추정 시간, 반복된 오류를 요약합니다. 글로 된 요약에는 클로드코드나 코덱스 설치와 로그인이 필요합니다."
- Right card h3: EN "Porch for Mac" / KO "맥용 Porch"
- Primary CTA: Download for Mac / 맥용 다운로드, with serif aside "(Apple Silicon)"

Footer:
- Left: `porch.` wordmark, one line: EN "Porch is a Mac app for looking back on your work with Claude Code and Codex." / KO "Porch는 클로드코드와 코덱스로 한 일을 돌아보는 맥 앱입니다."
- Links (EN): Get started · Privacy · Privacy Policy · FAQ · Release notes · EN/KO · Cookie settings
- Links (KO): 시작하기 · 개인정보 · 개인정보처리방침 · 자주 묻는 질문 · 변경 기록 · EN/KO · 쿠키 설정
- Privacy Policy / 개인정보처리방침 opens /privacy/ or /ko/privacy/; its text lives in `site/src/i18n/policy.ts`.
- Release notes / 변경 기록 appears only once it is published in both languages.
- Cookie settings / 쿠키 설정 appears only in a build with the PostHog key (ADR 0006); it brings the consent banner back.
- © 2026

## 14 Consent banner

Floating card in the bottom-right corner, shown when the build has the analytics key and no choice is stored yet
(ADR 0006); full width on phones. Title, what each
choice does, where to change it and a link to the privacy policy. Two buttons of the same style, Decline first;
no per-category choice.

| | EN | KO |
| --- | --- | --- |
| Title | Visit statistics and cookies | 방문 통계와 쿠키 |
| Body | This site collects visit statistics. Before you choose, or if you decline, visits are counted without a cookie; if you allow it, a cookie lets a return visit on another day count as the same visitor. | 이 사이트는 방문 통계를 모읍니다. 고르기 전이나 거부한 뒤에는 쿠키 없이 세고, 허용하면 쿠키를 저장해 다른 날 다시 방문해도 같은 방문자로 셉니다. |
| Note (+ policy link) | Change this any time in Cookie settings at the bottom of the page. Privacy Policy | 맨 아래 쿠키 설정에서 언제든 바꿀 수 있습니다. 개인정보처리방침 |
| Buttons | Decline / Allow | 거부 / 허용 |
| Region label | Cookie choice | 쿠키 선택 |
