// English for the fixtures' own text in the English landing video: summaries, task names, blockers,
// suggestions. The app draws its labels, dates and counts from its own dictionaries (app/src/i18n) once
// mock.ts answers get_language with "en"; nothing here may translate the app's interface.
// Only entries the shot screens use are kept; after a fixture change, look for Hangul left on the
// English shots and add what is missing here.

const EXACT: Record<string, string> = {
  "결제 화면 정리, 주문 알림 오류 수정": "Checkout cleanup, order alert fix",
  "로그인 기능 추가": "Add login",
  "이메일 인증과 실패 잠금": "Email verification and lockout",
  "주문 알림 오류 수정": "Order alert fix",
  "중복 알림 원인 추적": "Tracing duplicate alerts",
  "알림 큐 테스트": "Alert queue tests",
  "재시도 큐": "Retry queue",
  "랜딩 페이지 정리": "Landing page cleanup",
  "문구와 스크린샷": "Copy and screenshots",
  "결제 화면 정리": "Checkout cleanup",
  "버튼 문구": "Button labels",
  "주문 알림이 두 번 감": "Order alerts sent twice",
  "같은 오류 12번, 같은 요청 3번": "Same error 12×, same request 3×",
  "재시도할 때 orderId 없이 핸들러를 다시 불렀습니다.": "Retries called the handler again without an orderId.",
  "재시도 전에 orderId를 확인하는 테스트를 먼저 둡니다.": "Add a test that checks orderId before retrying.",
  "끝남": "Done",
  "주문 알림이 두 번 갈 때가 있음": "Order alerts sometimes send twice",
  "환불 버튼 문구 정하기": "Settle the refund button label",
  "로그인에 이메일 인증과 실패 잠금을 넣고, 결제 화면 버튼 문구를 정리했습니다.": "Added email verification and lockout to sign-in, and cleaned up the checkout button labels.",
  "이메일 인증 추가": "Email verification",
  "다섯 번 실패하면 잠금": "Lock after five failures",
  "결제 버튼 문구 정리": "Checkout button labels",
  "인증 메일 다국어": "Localize verification mail",
  "주문 알림이 두 번 가던 원인을 재시도 로직에서 찾아 막고, 알림 큐 테스트를 추가했습니다.": "Found the duplicate alerts in the retry logic, stopped them, and added alert queue tests.",
  "중복 알림 차단": "Stop duplicate alerts",
  "재시도 큐 테스트": "Retry queue tests",
  "알림 지연 측정": "Measure alert latency",
  "히어로 문구를 줄이고 요약 화면 스크린샷을 새로 찍었습니다.": "Shortened the hero copy and retook the summary screenshots.",
  "히어로 문구 정리": "Hero copy",
  "스크린샷 갱신": "New screenshots",
  "같은 오류를 두 요청에 걸쳐 12번 만났습니다.": "Hit the same error 12 times across two requests.",
  "알림 핸들러에 orderId가 없을 때를 먼저 확인하는 테스트를 두면 같은 오류를 줄일 수 있습니다.": "A test for a missing orderId in the alert handler would cut this error.",
  "알림 지연 측정 스크립트 만들어 줘": "Write a script to measure alert latency",
  "인증 메일 다국어 초안": "Draft localized verification mail",
  "테스트 명령은 묻지 않고 실행": "Run test commands without asking",
  "세 프로젝트에서 pnpm test 허락 요청이 30일 동안 41번 있었고, 모두 허락했습니다.": "pnpm test asked for permission 41 times in 30 days across three projects, and you allowed every one.",
  "로컬 Redis를 테스트 전에 띄우는 훅": "A hook that starts local Redis before tests",
  "작업 환경 막힘 3번 중 2번이 로컬 Redis가 꺼져 있던 경우였습니다.": "Two of three environment blockers were local Redis being down.",
  "작업 환경": "Environment",
  "재시도 전에 orderId를 확인하는 테스트부터": "Test for orderId before touching retries",
  "- 알림·결제 핸들러를 고칠 때는 orderId가 없는 경우의 테스트를 먼저 추가하고, 그 테스트가 실패하는 것을 확인한 뒤 고친다.": "- When changing alert or payment handlers, first add a test for a missing orderId, watch it fail, then fix.",
  "재시도 큐 테스트가 간헐적으로 실패": "Retry queue test fails intermittently",
  "결제 대행사 샌드박스 401": "Payment provider sandbox 401",
  "테스트 실패": "Test failures",
  "주문 상태 전이 테스트 실패": "Order state transition test failed",
  "알림 큐 순서 테스트 실패": "Alert queue order test failed",
  "로컬 Redis가 안 뜸": "Local Redis not running",
  "Node 버전 불일치": "Node version mismatch",
  "인증·외부 서비스": "Auth & external services",
  "알림 재시도에 지수 백오프를 넣었습니다.": "Added exponential backoff to alert retries.",
  "재시도 백오프": "Retry backoff",
};

/** Health-check lines the fixtures carry pre-rendered in Korean (core writes them in the run's language). */
const RULES: [RegExp, (...m: string[]) => string][] = [
  [/^테스트 실패: (\d+)일 막힘, (.+) \(이전 30일 (\d+)일\)$/, (_, n, m, b) => `Test failures: blocked on ${n} days, ${m} (previous 30 days: ${b})`],
  [/^(.+) (\d+)일 실패$/, (_, c, n) => `${c} failed on ${n} days`],
  [/^작업 시간의 ([\d.]+%)가 막힘에 걸린 요청 \(이전 30일 ([\d.]+%)\)$/, (_, a, b) => `${a} of work time went to blocked requests (previous 30 days: ${b})`],
  [/^결제 대행사 샌드박스 인증이 가끔 실패: 풀렸다는 기록이 없음 \(([\d-]+)부터, (\d+)일\)$/, (_, d, n) => `Payment sandbox auth fails now and then: no record of a fix (since ${d}, ${n} days)`],
  [/(\d+)분/g, (_, m) => `${m}m`],
];

/** Fixture terms inside a line the app composed ("Latest: …", "Aims to reduce: …"), longest first. */
const FRAGMENTS = Object.keys(EXACT).sort((a, b) => b.length - a.length);

export function toEnglish(text: string): string {
  const lead = text.match(/^\s*/)![0];
  const trail = text.match(/\s*$/)![0];
  let t = text.trim();
  if (!/[가-힣]/.test(t)) return text;
  if (t in EXACT) return lead + EXACT[t] + trail;
  for (const [re, fn] of RULES) {
    if (!/[가-힣]/.test(t)) break;
    t = t.replace(re, fn as (...a: string[]) => string);
  }
  for (const k of FRAGMENTS) {
    if (!/[가-힣]/.test(t)) break;
    if (t.includes(k)) t = t.split(k).join(EXACT[k]);
  }
  return lead + t + trail;
}

/** Translate every text node now and whenever React re-renders. */
export function englishOverlay(root: Node = document.body): void {
  const fix = (n: Node) => {
    if (n.nodeType === Node.TEXT_NODE) {
      const next = toEnglish(n.textContent ?? "");
      if (next !== n.textContent) n.textContent = next;
      return;
    }
    const w = document.createTreeWalker(n, NodeFilter.SHOW_TEXT);
    for (let t = w.nextNode(); t; t = w.nextNode()) fix(t);
  };
  fix(root);
  new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === "characterData") fix(r.target);
      for (const added of r.addedNodes) fix(added);
    }
  }).observe(root, { subtree: true, childList: true, characterData: true });
}
