# Codex 统一任务同步与小时额度趋势实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立后端统一任务快照，可靠同步批准状态和任务数量，并把趋势改为当天 24 个整点的额度变化。

**Architecture:** 以 `threadId` 为任务主键，在 Rust 后端新增统一合并层；实时事件、thread 轮询和 rollout 扫描只提供增量，合并层生成完整任务数组和四类统计后写入 `SnapshotStore`。桌面端和网页端只渲染快照的统一统计，历史由后端按本地小时桶维护，前端固定显示当天 24 个槽位。

**Tech Stack:** Rust/Tokio/Axum/Tauri、serde_json、TypeScript、Vite、Vitest、Playwright。

## Global Constraints

- 不自动批准 Codex 请求；只有用户点击批准/拒绝后才响应 app-server。
- 不改变三界面双击循环、配对流程、远程访问范围和现有奶油白/鼠尾草绿/杏色视觉设计。
- 快照中的 `changed_at` 必须单调递增；旧快照不得覆盖新快照。
- 趋势只显示额度百分比；Token 总量仍可保留在详情顶部卡片，但不能出现在趋势卡中。
- 缺失小时显示空槽，不复制上一小时的额度。

---

### Task 1: 扩展快照模型，建立统一四类统计

**Files:**
- Modify: `src-tauri/src/domain.rs`
- Modify: `src/domain.ts`
- Test: `src-tauri/src/domain.rs` tests
- Test: `src/domain.test.ts`

**Interfaces:**
- Produces Rust `TaskCounts`/snapshot field with `none`, `needs_action`, `running`, `completed` counts.
- Produces TypeScript `TaskCounts` and `taskStatusCounts(snapshot)` that prefers the server field and validates it against the task array in tests.

- [ ] **Step 1: Write failing model tests**

Add Rust tests that construct tasks in all four states, mark one completed task acknowledged, and assert the published counts exclude acknowledged tasks. Add TypeScript tests that normalize snake_case `task_counts` and expose the same four keys.

- [ ] **Step 2: Run focused tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml domain::tests` and `npm test -- --run src/domain.test.ts`. Expected: compilation or assertion failure because the new field/helper does not exist.

- [ ] **Step 3: Implement the model and normalization**

Add serializable `TaskCounts` to Rust `Snapshot`, a `TaskCounts::from_tasks` helper that ignores acknowledged tasks, and a serde-default for backward-compatible snapshots. Mirror the field in TypeScript `Snapshot`, normalize both `task_counts` and `taskCounts`, and make `taskStatusCounts` return the canonical field when present while retaining a task-array fallback for old cached data.

- [ ] **Step 4: Run focused tests and verify pass**

Run the two commands from Step 2. Expected: all focused model tests pass.

- [ ] **Step 5: Commit the isolated model change**

Run `git add src-tauri/src/domain.rs src/domain.ts src/domain.test.ts` and commit with `feat: add canonical task counts`.

### Task 2: Add a backend task registry and deterministic merge rules

**Files:**
- Create: `src-tauri/src/task_registry.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/poller.rs`
- Test: `src-tauri/src/task_registry.rs` tests
- Test: `src-tauri/src/poller.rs` tests

**Interfaces:**
- `TaskRegistry::apply_event(event, now)` updates one thread record.
- `TaskRegistry::merge_threads(threads, rollout_tasks, now)` supplements records without deleting live event tasks.
- `TaskRegistry::snapshot_tasks(now)` returns deduplicated `Vec<TaskSummary>` and `TaskCounts`.

- [ ] **Step 1: Write failing registry tests**

Cover: a red event remains red after a partial active thread poll; three running event tasks remain three after a poll containing one thread; duplicate `itemId`/request updates do not add a task; a completion event clears only its own approval; generated context is rejected as a title/activity.

- [ ] **Step 2: Run registry tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml task_registry::tests poller::tests`. Expected: missing module/functions or failing assertions.

- [ ] **Step 3: Implement deterministic registry**

Create a registry keyed by `threadId`, with per-thread current title, activity, status, updated time, approval request IDs, and a set of seen item/request IDs. Implement status precedence `NeedsAction > Running > Completed > None`. Make thread-list and rollout inputs additive; only remove tasks through the existing age windows. Reuse the existing title/context cleaner and ensure rollout never overwrites an established thread title.

- [ ] **Step 4: Route all poller paths through the registry**

Replace direct `merge_polled_tasks` calls in `poll_once`, task ticks, and event handling with registry updates. Before each publish, build one complete snapshot, recompute `active_task_count` and canonical `task_counts`, and publish only after the short coalescing window. Preserve the current monotonic `changed_at` behavior.

- [ ] **Step 5: Run focused Rust tests**

Run `cargo test --manifest-path src-tauri/Cargo.toml task_registry::tests poller::tests`. Expected: all registry and existing poller tests pass.

- [ ] **Step 6: Commit the backend merge layer**

Run `git add src-tauri/src/task_registry.rs src-tauri/src/lib.rs src-tauri/src/poller.rs` and commit with `feat: unify task snapshot merging`.

### Task 3: Correct app-server approval correlation and Hook compatibility

**Files:**
- Modify: `src-tauri/src/codex_protocol.rs`
- Modify: `src-tauri/src/codex_client.rs`
- Modify: `src-tauri/src/lan_server.rs`
- Modify: `src-tauri/src/task_registry.rs`
- Test: `src-tauri/src/codex_protocol.rs` tests
- Test: `src-tauri/src/lan_server.rs` tests

**Interfaces:**
- `NormalizedTaskEvent` carries `thread_id`, `turn_id`, `item_id`, `request_id`, `waiting_reason`, and request kind.
- `PermissionHookInput` accepts optional snake/camel fields and wrapper payloads.
- `respond_to_approval` resolves only the registered request ID and supports `accept`/`decline`.

- [ ] **Step 1: Add failing protocol fixtures**

Add fixtures for `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`, `item/permissions/requestApproval`, `item/tool/requestUserInput`, and `mcpServer/elicitation/request`. Assert the thread ID is used as task ID, request ID remains separate, and reason is retained. Add Hook fixtures with camelCase, snake_case, and wrapped `payload` fields.

- [ ] **Step 2: Run protocol tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml codex_protocol::tests lan_server::tests`. Expected: the current parser misses at least one fixture or rejects an optional Hook field.

- [ ] **Step 3: Implement request parsing and registration**

Parse `params.threadId`, `params.turnId`, `params.itemId`, `params.reason`, and the JSON-RPC request `id` independently. Recognize all approval/request methods case-insensitively. In `CodexClient`, register every server request with a request ID and method, not only strings containing `requestApproval`; keep the full request until resolved.

- [ ] **Step 4: Implement tolerant Hook input parsing**

Deserialize Hook input as `serde_json::Value`, extract optional fields from root or `payload`, accept both naming styles, and create a red task using session ID, turn ID, or a stable generated fallback. Preserve a readable reason even when model/tool fields are absent. Never auto-approve malformed or incomplete requests.

- [ ] **Step 5: Bind UI decision to request lifecycle**

When accepting/declining, look up the registered request ID, send the decision to app-server, and keep the task red until `serverRequest/resolved` or matching `item/completed` arrives. Then update only that thread and clear its approval fields.

- [ ] **Step 6: Run protocol and LAN tests**

Run the command from Step 2 and verify all fixtures pass, including the existing paired read-only and approval route tests.

- [ ] **Step 7: Commit the approval bridge change**

Run `git add src-tauri/src/codex_protocol.rs src-tauri/src/codex_client.rs src-tauri/src/lan_server.rs src-tauri/src/task_registry.rs` and commit with `fix: correlate Codex approval requests`.

### Task 4: Convert history storage to current-day hourly quota buckets

**Files:**
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/snapshot_store.rs`
- Modify: `src-tauri/src/poller.rs`
- Modify: `src/domain.ts`
- Test: `src-tauri/src/snapshot_store.rs` tests
- Test: `src/domain.test.ts`

**Interfaces:**
- Rust `UsagePoint { at, quota_remaining_percent }` stores one value per local hour.
- Snapshot history contains at most 24 points from the current local date.
- TypeScript normalization maps missing hours to `undefined` without carrying forward old values.

- [ ] **Step 1: Write failing hourly-history tests**

Assert two writes in the same hour collapse to the latest quota, a new local day drops the previous day, more than 24 buckets are pruned, and a missing middle hour remains absent. Assert no serialized history point has a token field.

- [ ] **Step 2: Run history tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml snapshot_store::tests` and `npm test -- --run src/domain.test.ts`. Expected: current 48-point/token assertions fail.

- [ ] **Step 3: Implement hourly bucket storage**

Change `UsagePoint` to quota-only, derive a local-date/hour bucket from the successful fetch time, replace the same bucket instead of appending, reset when the local date changes, and retain at most 24 buckets. Do not add a point for stale/error snapshots.

- [ ] **Step 4: Run history tests and verify pass**

Run the commands from Step 2. Expected: all hourly and existing snapshot-store tests pass.

- [ ] **Step 5: Commit hourly history**

Run `git add src-tauri/src/domain.rs src-tauri/src/snapshot_store.rs src-tauri/src/poller.rs src/domain.ts src/domain.test.ts` and commit with `feat: store hourly quota history`.

### Task 5: Make desktop and web render the canonical snapshot

**Files:**
- Modify: `src/components/FloatingIsland.ts`
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src/components/TaskList.ts`
- Modify: `src/styles.css`
- Modify: `web/index.html`
- Test: `src/components/FloatingIsland.test.ts`
- Test: `tests/ui/pairing.spec.ts`
- Test: `tests/ui/pull-refresh.spec.ts`

**Interfaces:**
- Long bar displays only nonzero color-dot counts from `snapshot.taskCounts`; none displays one gray dot and no status text count.
- Details rows use `title`, optional `activity`, then localized status; red rows show reason plus approve/decline buttons; green rows show “可验收”.
- History renderer receives 24 hourly points and emits quota-only markup.

- [ ] **Step 1: Write failing UI tests**

Add fixtures with red 1, yellow 2, green 3 and assert both long-bar and details summary show the same numbers. Add a no-task fixture and assert the gray dot. Add a red task with request ID and assert both buttons; add a completed task and assert “可验收”. Add a 24-slot history fixture and assert no `history-token`, `Token` trend text, or token bars.

- [ ] **Step 2: Run UI tests and verify failure**

Run `npm test -- --run src/components/FloatingIsland.test.ts` and `npx playwright test tests/ui/pairing.spec.ts tests/ui/pull-refresh.spec.ts`. Expected: current priority-only long-bar text and dual-track history fail.

- [ ] **Step 3: Implement desktop rendering**

Use canonical counts for the island and remove the alternate “活跃任务/已完成” summary. Keep the three segment widths and status-dot styling unchanged. Update details history to fixed 24 quota slots and update task row text order without altering the existing double-click cycle or pairing controls.

- [ ] **Step 4: Implement web rendering**

Normalize `task_counts` from the snapshot, render the same color-dot counts as desktop, and render the same title/activity/status hierarchy and approval/acknowledge controls. Keep the web pull-to-refresh behavior and remove only the history Token track and summary.

- [ ] **Step 5: Run UI and browser tests**

Run the commands from Step 2 plus `npm run build`. Expected: all unit/browser tests pass and the production web bundle builds.

- [ ] **Step 6: Commit the shared rendering change**

Run `git add src/components/FloatingIsland.ts src/components/DetailsPanel.ts src/components/TaskList.ts src/styles.css web/index.html src/components/FloatingIsland.test.ts tests/ui/pairing.spec.ts tests/ui/pull-refresh.spec.ts` and commit with `fix: render canonical task state everywhere`.

### Task 6: End-to-end snapshot consistency verification

**Files:**
- Modify: `src-tauri/src/lan_server.rs` tests
- Modify: `tests/ui/pairing.spec.ts`
- Create: `tests/ui/task-sync.spec.ts`

**Interfaces:**
- WebSocket initial snapshot and subsequent snapshots carry the same `changed_at`, `task_counts`, and task IDs used by desktop render tests.

- [ ] **Step 1: Add consistency fixtures**

Create a fixture sequence containing three running threads, one approval request, one completion, and a partial `thread/list` response. Assert every emitted snapshot has counts equal to its task array and that the partial poll never emits a one-task replacement.

- [ ] **Step 2: Run the focused end-to-end tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml lan_server::tests task_registry::tests` and `npx playwright test tests/ui/task-sync.spec.ts`. Expected: the new assertions fail against the pre-fix pipeline.

- [ ] **Step 3: Implement any integration fixes exposed by the fixtures**

Only adjust the registry/publish boundary or field normalization; do not add a second UI-specific count calculation.

- [ ] **Step 4: Run the full verification set**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run build
npx playwright test
```

Expected: all Rust, TypeScript, build, and browser tests pass.

- [ ] **Step 5: Review the final diff and commit**

Run `git diff --check`, inspect `git diff --stat`, and verify no unrelated files or secrets were included. Commit with `test: verify canonical task synchronization`.

