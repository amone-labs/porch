// The privacy policy page (/privacy/, /ko/privacy/). Facts come from ADR 0006 (app usage events) and
// ADR 0006 (site visit events, installer requests). Keep both languages saying the same thing.
import { OPERATOR } from "../site";
import type { Locale } from "./types";

export const POLICY_EMAIL = "dev.bearjb@gmail.com";

export interface Policy {
  title: string;
  intro: string;
  sections: { heading: string; items: string[] }[];
  effective: string;
  home: string;
}

export const policy: Record<Locale, Policy> = {
  ko: {
    title: "개인정보처리방침",
    intro:
      `${OPERATOR}(이하 운영자)는 Porch 웹사이트(getporch.pages.dev)와 Porch 맥 앱에서 아래와 같이 개인정보를 처리합니다. Porch에는 계정이 없고, 이름이나 이메일 같은 정보를 받지 않습니다.`,
    sections: [
      {
        heading: "1. 처리하는 정보와 목적",
        items: [
          "웹사이트 방문 통계: 본 페이지와 섹션, 일부 클릭, 유입 경로, 브라우저·운영체제·기기 종류, 화면 크기, 머문 시간, IP 주소로 추정한 나라·도시. 사이트를 개선하는 데 씁니다.",
          "설치 파일 요청: Porch.dmg를 받을 때 브라우저인지 명령줄 도구인지, 운영체제, 유입 페이지, 나라. 다운로드 수를 세는 데 쓰며, IP 주소는 보내지 않습니다.",
          "앱 사용 통계: 연 화면, 켠 기능, 요약 성공 여부, 앱 버전, 운영체제. 앱을 개선하는 데 씁니다. 경로, 프로젝트 이름, 요청·요약 내용, 토큰, 비용은 보내지 않습니다.",
          "대화 기록과 요약은 사용자의 맥에만 저장하며 운영자는 받지 않습니다. 요약과 제안을 만들 때는 사용자가 고른 클로드코드나 코덱스가 사용자의 계정으로 Anthropic이나 OpenAI에 보냅니다.",
        ],
      },
      {
        heading: "2. 보유 기간과 파기",
        items: ["통계는 수집한 날부터 1년 동안 보관하며, 기간이 지나면 지체 없이 삭제됩니다."],
      },
      {
        heading: "3. 처리 위탁과 국외 이전",
        items: [
          "PostHog, Inc.(미국, privacy@posthog.com): 웹사이트·앱 사용 통계의 저장과 분석. 이용할 때마다 네트워크로 전송하며, 보유 기간은 2항과 같습니다.",
          "Cloudflare, Inc.(미국, privacyquestions@cloudflare.com): 웹사이트 제공과 설치 파일 요청 처리. 접속할 때 네트워크로 전송합니다.",
          "Amazon Web Services(서울 리전): 설치 파일과 업데이트 배포.",
          "앱 사용 통계는 앱 설정에서 끌 수 있습니다. 웹사이트 방문 통계는 쿠키를 거부해도 쿠키 없이 집계되므로, 원하지 않으면 브라우저에서 추적을 차단해 주세요.",
        ],
      },
      {
        heading: "4. 쿠키",
        items: [
          "허용하면 분석 쿠키(ph_로 시작)를 저장해 다른 날 다시 온 방문을 같은 방문자로 셉니다. 고르기 전이나 거부하면 쿠키 없이 셉니다. 사이트 맨 아래 쿠키 설정이나 브라우저 설정에서 언제든 바꿀 수 있습니다.",
        ],
      },
      {
        heading: "5. 정보주체의 권리",
        items: [
          `개인정보의 열람, 정정, 삭제, 처리 정지를 ${POLICY_EMAIL}로 요청할 수 있으며, 지체 없이 처리합니다. 통계에는 이름이나 연락처가 없어 특정인의 기록을 찾지 못할 수 있습니다.`,
        ],
      },
      {
        heading: "6. 안전성 확보 조치",
        items: ["통계는 HTTPS로 암호화해 전송하고, 통계를 볼 수 있는 사람을 운영자로 제한합니다."],
      },
      {
        heading: "7. 개인정보 보호책임자",
        items: [`${OPERATOR} · ${POLICY_EMAIL}`],
      },
      {
        heading: "8. 권익침해 구제",
        items: [
          "개인정보분쟁조정위원회 1833-6972 (www.kopico.go.kr)",
          "개인정보침해신고센터 118 (privacy.kisa.or.kr)",
          "대검찰청 1301 (www.spo.go.kr)",
          "경찰청 182 (ecrm.police.go.kr)",
        ],
      },
      {
        heading: "9. 변경",
        items: ["이 방침이 바뀌면 이 페이지에 알립니다."],
      },
    ],
    effective: "시행일: 2026년 9월 30일",
    home: "Porch 홈으로 가기 →",
  },
  en: {
    title: "Privacy Policy",
    intro:
      `${OPERATOR} (the operator) handles personal information on the Porch website (getporch.pages.dev) and in the Porch Mac app as described below. Porch has no accounts and does not ask for your name or email.`,
    sections: [
      {
        heading: "1. What is processed and why",
        items: [
          "Website visit statistics: pages and sections viewed, some clicks, referrer, browser, operating system and device type, screen size, time on page, and country and city estimated from your IP address. Used to improve the site.",
          "Installer requests: when Porch.dmg is requested, whether a browser or a command-line tool asked, the operating system, the referring page and the country. Used to count downloads; no IP address is sent.",
          "App usage statistics: screens opened, features turned on, whether summaries succeeded, app version and operating system. Used to improve the app. Paths, project names, request and summary content, tokens and cost are never sent.",
          "Conversation records and summaries stay on your Mac; the operator never receives them. To make summaries and suggestions, the Claude Code or Codex you pick sends them to Anthropic or OpenAI under your own account.",
        ],
      },
      {
        heading: "2. Retention and deletion",
        items: ["Statistics are kept for 1 year from collection and deleted without delay after that."],
      },
      {
        heading: "3. Processors and transfers abroad",
        items: [
          "PostHog, Inc. (USA, privacy@posthog.com): stores and analyses website and app usage statistics, sent over the network as you use them, kept as in section 2.",
          "Cloudflare, Inc. (USA, privacyquestions@cloudflare.com): serves the website and handles installer requests, sent over the network as you visit.",
          "Amazon Web Services (Seoul region): delivers the installer and updates.",
          "App usage statistics can be turned off in the app's Settings. Website visit statistics are counted without a cookie even if you decline cookies; to stop them, block tracking in your browser.",
        ],
      },
      {
        heading: "4. Cookies",
        items: [
          "If you allow it, an analytics cookie (starting with ph_) is stored so visits on different days count as the same visitor. Before you choose, or if you decline, visits are counted without a cookie. Change this any time in Cookie settings at the bottom of the site or in your browser.",
        ],
      },
      {
        heading: "5. Your rights",
        items: [
          `You can ask to see, correct, delete or stop the processing of your personal information at ${POLICY_EMAIL}, and the request is handled without delay. Statistics hold no names or contact details, so a specific person's records may not be identifiable.`,
        ],
      },
      {
        heading: "6. Security",
        items: ["Statistics are sent encrypted over HTTPS, and only the operator can view them."],
      },
      {
        heading: "7. Privacy officer",
        items: [`${OPERATOR} · ${POLICY_EMAIL}`],
      },
      {
        heading: "8. Remedies (Korea)",
        items: [
          "Personal Information Dispute Mediation Committee 1833-6972 (www.kopico.go.kr)",
          "Personal Information Infringement Report Center 118 (privacy.kisa.or.kr)",
          "Supreme Prosecutors' Office 1301 (www.spo.go.kr)",
          "Korean National Police Agency 182 (ecrm.police.go.kr)",
        ],
      },
      {
        heading: "9. Changes",
        items: ["Changes to this policy are posted on this page."],
      },
    ],
    effective: "Effective September 30, 2026",
    home: "Go to Porch home →",
  },
};
