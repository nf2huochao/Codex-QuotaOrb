# Event-Authoritative Task Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make app-server events the only source that can create or clear red/green task states, use thread polling only to restore yellow active tasks, and remove rollout logs from status decisions.

**Architecture:** Keep `TaskRegistry` as the canonical state store. Normalize live app-server requests/events into registry transitions; use `thread/list` only as a yellow-task reconciliation source; keep `RolloutWatcher` for activity/title/token fields only. All UI surfaces continue reading the same snapshot.

**Tech Stack:** Rust/Tauri, Tokio, serde_json, TypeScript/Vite, Vitest.

## Global Constraints

- Logs must never create or change `needs_action`, `running`, or `completed`.
- `thread/list` may add or refresh `running` and preserve explicit terminal completion; it must not create `needs_action`.
- Red status requires a live unresolved approval/request id.
- Blocked warnings remain an auxiliary field on yellow tasks and must not change the base status.
- Preserve existing compact/detail/web UI behavior and colors.

---

### Task 1: Remove log-driven status mutation

**Files:**
- Modify: `src-tauri/src/poller.rs:100-153,293-308`
- Modify: `src-tauri/src/rollout_watcher.rs:218-277`

**Interfaces:**
- `map_threads` returns candidates derived from explicit `active`/`running` thread status or explicit terminal completion; it never interprets waiting/approval words as red.
- `merge_rollout_tasks` merges activity/title/token fields only for matching thread ids and never writes `task.status` or adds unmatched log-only tasks.
- `RolloutWatcher::scan` may continue parsing records internally, but its inferred status is ignored by the canonical registry.

- [ ] **Step 1: Add regression tests** for a rollout containing approval-like text without an unresolved request id; assert the task remains yellow/unchanged and cannot become red.
- [ ] **Step 2: Run the focused Rust tests** and confirm the new tests fail against the current implementation.
- [ ] **Step 3: Remove rollout status assignment** from `merge_rollout_tasks` and stop `map_threads` from consulting `rollout_status` for red state.
- [ ] **Step 4: Keep rollout parsing for activity/title/token only**; discard unmatched log-only records instead of adding them as tasks.
- [ ] **Step 5: Run focused Rust tests** and confirm they pass.

### Task 2: Make thread reconciliation yellow-only

**Files:**
- Modify: `src-tauri/src/poller.rs:100-153,248-291`
- Modify: `src-tauri/src/task_registry.rs:83-127`

**Interfaces:**
- A polled candidate with `active`/`running` can add or refresh a yellow task.
- A polled candidate cannot create or replace `NeedsAction`.
- Existing red/green event states remain authoritative until a matching live event resolves or completes them.

- [ ] **Step 1: Add tests** proving an active thread with no event request id is yellow, and an active poll cannot turn a red event task into a new red task without a request id.
- [ ] **Step 2: Implement candidate filtering** so `thread/list` only emits running/completed/none reconciliation data and never `NeedsAction`.
- [ ] **Step 3: Preserve live event tasks missing from partial `thread/list` responses**, including multiple concurrent running tasks.
- [ ] **Step 4: Run `cargo test --manifest-path src-tauri/Cargo.toml`** and confirm all Rust tests pass.

### Task 3: Verify canonical counts and build

**Files:**
- Test: `src-tauri/src/task_registry.rs` and `src-tauri/src/poller.rs` tests
- Test: `src/components/FloatingIsland.test.ts`, `src/domain.test.ts`

**Interfaces:**
- `Snapshot.task_counts` is the only count source for compact island, details, and web rendering.

- [ ] **Step 1: Run `npm test`** and confirm all frontend tests pass.
- [ ] **Step 2: Run `npm run build`** and confirm TypeScript and Vite production build pass.
- [ ] **Step 3: Review `git diff --check`** and confirm no whitespace errors or accidental UI changes.
- [ ] **Step 4: Report the remaining signing-key warning separately** if packaging is requested; do not confuse it with application correctness.
