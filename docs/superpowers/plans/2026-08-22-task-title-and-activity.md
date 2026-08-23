# Task Title and Activity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep Codex conversation titles stable while showing the current user task and status as separate lines in desktop and web task lists.

**Architecture:** Add an optional `activity` field to the shared task summary. The app-server supplies the conversation title, RolloutWatcher supplies the first user sentence, and merge logic combines them without overwriting the title. Both renderers consume the same fields.

**Tech Stack:** Rust/Tauri, TypeScript, Vitest, Playwright.

## Global Constraints

- Keep the existing ball → island → details interaction unchanged.
- Keep approval/rejection and acknowledgement buttons in the existing task row.
- Do not display generated context tags as conversation titles.

### Task 1: Separate title and rollout activity

**Files:**
- Modify: `src-tauri/src/domain.rs`, `src-tauri/src/rollout_watcher.rs`, `src-tauri/src/poller.rs`, `src-tauri/src/lan_server.rs`, `src-tauri/src/snapshot_store.rs`
- Test: Rust unit tests in `src-tauri/src/rollout_watcher.rs` and `src-tauri/src/poller.rs`

- [ ] Add `activity: Option<String>` to `TaskSummary` and initialize it in all constructors.
- [ ] Store the rollout first user sentence as `activity`, not `title`.
- [ ] Keep the app-server conversation title in `map_threads`; only use `Codex 对话` when no clean title exists.
- [ ] Merge rollout activity into an existing task without changing its title.
- [ ] Treat `<codex_...>` and other generated context blocks as non-title content.
- [ ] Add tests covering title preservation and activity extraction.

### Task 2: Render the three-level task row

**Files:**
- Modify: `src/domain.ts`, `src/components/TaskList.ts`, `src/components/DetailsPanel.ts`, `web/index.html`
- Test: `src/components/FloatingIsland.test.ts`, `tests/ui/pairing.spec.ts`

- [ ] Normalize `activity` from both camelCase and snake_case payloads.
- [ ] Include `activity` in task signatures so row content updates without full-page flicker.
- [ ] Render title, current content, and status in that order; keep waiting reasons and action buttons.
- [ ] Add focused tests for the shared snapshot and the web task row.

### Task 3: Verify

- [ ] Run `cargo test` in `src-tauri`.
- [ ] Run `npm test -- --run` and `npm run build`.
- [ ] Run `npx playwright test`.
- [ ] Run `git diff --check`.
