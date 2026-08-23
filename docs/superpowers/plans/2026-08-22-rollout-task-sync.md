# Rollout 任务同步 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 以 Codex Rollout 增量日志作为任务发现和状态主来源，让桌面与网页所有视图共享一致的任务数量、标题、批准和完成状态。

**Architecture:** 在 Rust 端新增无副作用的 RolloutWatcher，按文件偏移量增量读取近期 `.jsonl` 文件，产出 `TaskSummary`；poller 将 watcher 结果与 app-server 线程/事件合并后只发布 `SnapshotStore` 快照。额度与 Token 仍由 app-server 读取，批准响应仍通过当前 app-server 请求 ID 手动返回。

**Tech Stack:** Rust 2021、Tokio、serde_json、Tauri 2、现有 TypeScript/Vitest/Playwright 测试。

## Global Constraints

- 不修改 Codex 配置，不安装额外软件，不自动批准任何请求。
- Rollout 日志只读；无法读取时降级到 app-server `thread/list`。
- 只让 `SnapshotStore` 发布统一快照，界面不各自计算任务源。
- 保持现有奶油白/鼠尾草绿/杏色 UI 和三次双击界面逻辑。

---

### Task 1: 建立可测试的 Rollout 增量观察器

**Files:**
- Create: `src-tauri/src/rollout_watcher.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/rollout_watcher.rs` unit tests

**Interfaces:**
- `RolloutWatcher::new(root: PathBuf) -> Self`
- `RolloutWatcher::scan(&mut self, now: i64) -> Vec<TaskSummary>`
- `extract_rollout_task(text: &str, fallback_id: &str, modified_at: i64, now: i64) -> Option<TaskSummary>`

- [x] **Step 1: Write parser tests** for session metadata ID, first real user request title, running, approval, completed and token records.
- [x] **Step 2: Implement append-only offset tracking** with bounded tail reads for existing large files and reset offsets after truncation.
- [x] **Step 3: Implement filename/date scanning** for recent session directories only, skipping files older than the completed visibility window.
- [x] **Step 4: Run `cargo test rollout_watcher`** and require all parser tests to pass.

### Task 2: Merge Rollout tasks into the poller

**Files:**
- Modify: `src-tauri/src/poller.rs`
- Modify: `src-tauri/src/domain.rs` only if merge helpers need a shared status function
- Test: `src-tauri/src/poller.rs`

**Interfaces:**
- `merge_rollout_tasks(previous, polled, rollout, now) -> Vec<TaskSummary>` preserves approval and acknowledgement state.

- [x] **Step 1: Add failing tests** for two rollout files being retained when `thread/list` returns one, and for rollout title winning over `<recommended_plugins>`.
- [x] **Step 2: Add one watcher instance to `spawn_poll_loop`** and feed its result on the task tick; do not rescan all file contents.
- [x] **Step 3: Merge by task/thread ID**, with event approval > rollout approval > thread status > stale previous state.
- [x] **Step 4: Recalculate active counts from the merged vector** and publish only through `SnapshotStore`.
- [x] **Step 5: Run `cargo test poller`** and verify all existing tests remain green.

### Task 3: Harden approval event correlation

**Files:**
- Modify: `src-tauri/src/codex_protocol.rs`
- Modify: `src-tauri/src/codex_client.rs`
- Modify: `src-tauri/src/poller.rs`
- Test: `src-tauri/src/codex_client_tests.rs` and `src-tauri/src/poller.rs`

- [x] **Step 1: Add fixtures** for `item/commandExecution/requestApproval` and `item/fileChange/requestApproval` with numeric and string IDs.
- [x] **Step 2: Normalize thread ID from `threadId`, `thread_id`, item and turn fields before applying the red state.
- [x] **Step 3: Preserve request IDs in the unified task and remove them only after an explicit accept/decline response.
- [x] **Step 4: Run the focused Rust approval tests.**

### Task 4: Make all UI consumers use the same counts

**Files:**
- Modify: `src/components/FloatingIsland.ts`
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src/components/TaskList.ts`
- Modify: `web/index.html`
- Test: `src/components/FloatingIsland.test.ts`, `tests/ui/no-flicker.spec.ts`

- [x] **Step 1: Add a shared count assertion** that the island’s colored counts equal details’ task status counts for the same snapshot.
- [x] **Step 2: Ensure only non-acknowledged active states count in the island; completed tasks remain in details for “可验收”.
- [x] **Step 3: Keep approval buttons manual and show waiting reason under the same task row.
- [x] **Step 4: Run Vitest and Playwright UI tests.**

### Task 5: Full verification

**Files:**
- Modify: `CHANGELOG.md` only if release notes are required by the existing workflow.

- [x] **Step 1: Run `cargo test --manifest-path src-tauri/Cargo.toml`.**
- [x] **Step 2: Run `npm test -- --run` and `npm run build`.**
- [x] **Step 3: Run the UI smoke suite and inspect the generated preview.**
- [x] **Step 4: Review `git diff` and leave unrelated user changes untouched.**
