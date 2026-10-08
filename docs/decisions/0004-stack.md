# 0004. Tauri 2 + Rust + React

- Status: accepted (confirmed 2026-09-29)
- Date: 2026-09-27

## Context

The hardest part is catching sessions accurately, and the app shell has nothing to do with that risk. Two requirements decide the stack.

- The hook binary runs every time an agent uses a tool, so it must be a small native program that finishes within 10 ms.
- Placing a menu bar popover is fiddly. Coordinate units that disagree across displays are a problem already met in another Tauri app.

## Decision

- Hook and core logic: Rust (`crates/hook`, `crates/core`). The two share the event format.
- App shell: Tauri 2. The popover's position is computed in one unit, AppKit coordinates (Cocoa points).
- Screens: React + TypeScript + Vite + Tailwind.
- Storage: SQLite (`rusqlite`).

## Alternatives

Swift + SwiftUI `MenuBarExtra`. The app would be much lighter (Chit is 777 KB) and the menu bar more natural, but it is a new language. With the hook and core logic in Rust, swapping only the shell for Swift stays possible.

## Confirmed

Confirmed once the summary screens were going into the same app ([ADR 0005](0005-model-written-summaries.md)) and it was time to add screens. Versions follow a combination proven in another Tauri app (Tauri 2.11, React 18, Vite 5, Tailwind 3, TypeScript 5.6).
