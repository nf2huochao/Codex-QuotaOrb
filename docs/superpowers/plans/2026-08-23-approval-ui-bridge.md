# 批准状态 UI 桥接 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Windows UI Automation fallback that marks Codex desktop tasks as red when the desktop UI visibly says “等待批准”, without reading logs.

**Architecture:** Keep app-server events and hooks authoritative. During reconciliation, query a bounded Windows UI Automation helper for pending approval titles, then merge matching titles into the existing task registry as `needs_action` only when no real approval request exists.

**Tech Stack:** Rust/Tauri backend, built-in Windows PowerShell UI Automation (`System.Windows.Automation`), existing TypeScript UI and Rust task registry tests.

## Global Constraints

- Do not read rollout logs or JSONL files for status.
- Do not alter Codex approval policy or automatically approve/deny anything.
- Real app-server approval request ids remain authoritative over UI fallback results.
- UI Automation failures are non-fatal and must leave existing snapshot data intact.

---

### Task 1: Add a bounded UI Automation reader

**Files:**
- Create: `src-tauri/src/ui_approval.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/ui_approval.rs` unit tests for parsing helper output

**Interfaces:**
- Produces `pub struct UiApprovalCandidate { pub title: String, pub reason: String }`.
- Produces `pub async fn read_pending_approvals() -> Vec<UiApprovalCandidate>`.

- [ ] **Step 1: Write the failing parser tests**

Add tests proving a JSON-lines response containing `{"title":"测试审批弹窗","reason":"等待批准"}` parses, unrelated rows are ignored, and malformed output returns an empty vector.

- [ ] **Step 2: Run the focused Rust test and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml ui_approval -- --nocapture`.
Expected: FAIL because the module and parser do not exist.

- [ ] **Step 3: Implement the helper invocation**

Run built-in `powershell.exe -NoProfile -NonInteractive -Command` with a short script that loads `UIAutomationClient`/`UIAutomationTypes`, finds the top-level window named `ChatGPT`, enumerates descendant `ListItem` controls, and emits only names containing `等待批准` as compact JSON. Set a 2-second timeout, kill the child on timeout, and return an empty vector on any error.

- [ ] **Step 4: Register the module and run tests**

Add `mod ui_approval;` to `src-tauri/src/lib.rs`; run the focused test again and expect PASS.

- [ ] **Step 5: Commit the isolated reader**

Run `git add src-tauri/src/ui_approval.rs src-tauri/src/lib.rs src-tauri/Cargo.toml` and commit with `feat: add windows approval ui reader`.

### Task 2: Merge UI candidates with the authoritative task registry

**Files:**
- Modify: `src-tauri/src/poller.rs`
- Modify: `src-tauri/src/task_registry.rs`
- Test: `src-tauri/src/poller.rs` and `src-tauri/src/task_registry.rs`

**Interfaces:**
- Consumes `read_pending_approvals()` candidates.
- Produces existing `TaskStatus::NeedsAction` snapshots without changing real approval ids.

- [ ] **Step 1: Add failing merge tests**

Cover: matching title changes running to needs_action; nonmatching title stays running; a task with a real approval request id is not overwritten; candidate removal returns the task to the prior authoritative status on the next reconciliation.

- [ ] **Step 2: Run focused tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml poller::tests task_registry::tests -- --nocapture`.
Expected: FAIL for the new cases.

- [ ] **Step 3: Implement merge logic**

After normal app-server reconciliation and before snapshot counts are finalized, merge candidates by exact normalized title. Set `status=NeedsAction`, `waiting_reason="等待批准"`, and leave `approval_request_id=None` for UI-only candidates. Never replace an existing real approval id.

- [ ] **Step 4: Run backend tests**

Run `cargo test --manifest-path src-tauri/Cargo.toml`; expect all existing and new tests to pass.

- [ ] **Step 5: Commit the merge**

Run `git add src-tauri/src/poller.rs src-tauri/src/task_registry.rs` and commit with `feat: merge desktop approval state`.

### Task 3: Verify UI behavior without changing controls

**Files:**
- Modify: `web/index.html` only if a UI-only guard is needed
- Test: `src/domain.test.ts`

- [ ] **Step 1: Add normalization/count test**

Verify a UI-only `needs_action` task contributes to red count and is not rendered as yellow.

- [ ] **Step 2: Run frontend tests and build**

Run `npm test -- --runInBand` and `npm run build`; expect PASS.

- [ ] **Step 3: Perform live acceptance check**

With Codex showing “测试审批弹窗 · 等待批准”, query `/api/snapshot` and verify `task_counts.needs_action >= 1`, the matching task is red, and no automatic decision is sent.

- [ ] **Step 4: Commit verification changes**

Run `git add src/domain.test.ts web/index.html` and commit with `test: verify approval status bridge`.
