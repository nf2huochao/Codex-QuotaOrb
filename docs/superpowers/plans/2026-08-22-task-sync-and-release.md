# 任务同步与发布能力 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make task counts identical in every view, add explicit approval actions, improve offline visibility, record a compact usage history, and make signed releases fail clearly when secrets are missing.

**Architecture:** Extend the shared snapshot model with approval metadata and capped usage points. The Codex client stores pending JSON-RPC approval request IDs and responds only after an explicit user action. Desktop and LAN web clients call the same approval command/API and render the same snapshot-derived counts and freshness state.

**Tech Stack:** Rust/Tauri, Axum LAN API, TypeScript DOM UI, static mobile web, GitHub Actions, Vitest, Cargo tests, Playwright.

## Global Constraints

- Never auto-approve or auto-decline a request.
- Accept only `accept` or `decline` decisions from the UI/API.
- Keep signing private keys out of the repository, logs, and chat.
- Use one snapshot task list as the source for summary counts and details.
- Cap history to 48 points and do not add a remote database.

---

### Task 1: Shared task metadata, counts, and history

**Files:** `src/domain.ts`, `src/components/FloatingIsland.ts`, `src/components/DetailsPanel.ts`, `src/components/TaskList.ts`, `web/index.html`, Rust domain/store/poller literals and tests.

- [ ] Add approval reason/request ID and capped usage points to the snapshot model.
- [ ] Add one shared task-count helper and use it for island and details labels.
- [ ] Render a compact history card in desktop and web details.
- [ ] Add unit tests for count parity and history rendering.

### Task 2: Approval request round trip

**Files:** `src-tauri/src/codex_client.rs`, `src-tauri/src/codex_protocol.rs`, `src-tauri/src/poller.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/lan_server.rs`, UI files and tests.

- [ ] Store server request IDs when approval requests arrive.
- [ ] Respond with `{ "decision": "accept" | "decline" }` only after a button action.
- [ ] Expose desktop and LAN endpoints/commands with authorization.
- [ ] Show task name, reason, approve, and decline controls.
- [ ] Test parsing, routing, authorization, and UI actions.

### Task 3: Offline state and signed release guardrails

**Files:** `web/index.html`, `src-tauri/src/lib.rs`, `.github/workflows/release.yml`, `docs/release-signing.md`.

- [ ] Make offline/reconnecting/last-success states explicit while retaining cached data.
- [ ] Add a release preflight that fails with a clear message when signing secrets are absent.
- [ ] Document one-time local key generation and GitHub Secret names without storing secret values.

### Task 4: Verification

- [ ] Run `npm.cmd test`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --quiet`.
- [ ] Run `npm.cmd run test:ui`.
- [ ] Run `npm.cmd run build` and `git diff --check`.
