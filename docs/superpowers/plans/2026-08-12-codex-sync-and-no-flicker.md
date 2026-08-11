# Codex 状态同步与无闪烁更新 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 Codex 状态同步改为任务高频检查、额度与 Token 差量轮询、前端稳定 DOM 局部更新，消除数据刷新造成的三个界面闪烁。

**Architecture:** Rust 后端保留一份已验证的完整 `Snapshot`，按 1 秒任务检查、15 秒指标检查和 120 秒全量校验的节奏读取数据；只有快照真正变化时才发布 `snapshot-updated`。前端只在三态切换时挂载界面，数据事件通过 `diffSnapshot` 更新当前界面的文字、颜色和进度环，不重建 DOM 或调整窗口尺寸。

**Tech Stack:** Tauri 2、Rust/Tokio、Codex app-server JSON-RPC、TypeScript、Vite、Vitest、jsdom、Playwright。

## Global Constraints

- 任务变化通常 1～2 秒内反映；额度和 Token 在 10～30 秒内校验；120 秒全量轮询作为最终兜底。
- 数据未变化时不得发布状态事件、重播进入动画或重复调整原生窗口尺寸。
- 任务状态灯优先级固定为：需要回复/授权（红）> 有任务完成（绿）> 正在执行（黄）> 无活跃任务（灰）。
- Codex 数据读取失败时保留最近一次已验证数据并标记过期或错误，不更新时间戳，不伪装成最新状态。
- 不修改 Codex 桌面应用本身，不新增云端服务或移动端同步协议。

---

### Task 1: 后端快照去重与分频同步

**Files:**
- Modify: `src-tauri/src/snapshot_store.rs`
- Modify: `src-tauri/src/poller.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/snapshot_store.rs` tests and `src-tauri/src/codex_client_tests.rs`

**Interfaces:**
- Produces `SnapshotStore::publish_if_changed(snapshot: Snapshot) -> bool`.
- Produces `poller::SyncIntervals { task: Duration, metrics: Duration, full: Duration }` with values 1s, 15s, 120s.
- Keeps `poll_once` as the full refresh entry point used by the manual refresh command.

- [ ] **Step 1: Add a failing store test for duplicate snapshots**

Add a test that subscribes before publishing, calls `publish_if_changed` with an identical snapshot, and asserts it returns `false` and the receiver has no change; then changes `today_tokens` and asserts it returns `true`.

Run: `cargo test snapshot_store::tests::duplicate_snapshot_is_not_published`

Expected: FAIL because `publish_if_changed` does not exist.

- [ ] **Step 2: Implement store-side deduplication**

Implement `publish_if_changed` by normalizing acknowledgements and active count exactly as `publish` does, comparing the normalized candidate with the current state, and sending through the watch channel only when `candidate != current`.

Keep `publish` as a compatibility wrapper that always sends, and have `publish_if_changed` return whether a notification was emitted.

- [ ] **Step 3: Add a failing test for the sync cadence contract**

Add a Rust test asserting:

```rust
assert_eq!(SyncIntervals::default().task, Duration::from_secs(1));
assert_eq!(SyncIntervals::default().metrics, Duration::from_secs(15));
assert_eq!(SyncIntervals::default().full, Duration::from_secs(120));
```

Run: `cargo test poller::tests::sync_intervals_match_product_contract`

Expected: FAIL until `SyncIntervals` and its `Default` implementation exist.

- [ ] **Step 4: Implement the split loop**

Replace the single sleep-based `spawn_poll_loop` body with a Tokio `interval` loop that tracks `last_metrics` and `last_full`. Every tick calls `read_threads`; only when the metrics interval has elapsed does it call `read_rate_limits` and `read_usage`; when the full interval elapses it performs all three reads and replaces the previous snapshot. Build the candidate snapshot from the last successful fields, call `publish_if_changed`, and update the local previous snapshot only after the candidate is accepted.

The existing manual `refresh_now` continues to call `poll_once` and therefore performs a full read immediately.

- [ ] **Step 5: Run the backend tests**

Run: `cargo test`

Expected: all existing tests plus the duplicate-publish and cadence tests pass.

- [ ] **Step 6: Commit the backend unit**

```powershell
git add src-tauri/src/snapshot_store.rs src-tauri/src/poller.rs src-tauri/src/lib.rs src-tauri/src/codex_client_tests.rs
git commit -m "feat: deduplicate and split codex sync polling"
```

### Task 2: Snapshot diff model and stable view mounts

**Files:**
- Modify: `src/domain.ts`
- Modify: `src/components/FloatingIsland.ts`
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src/main.ts`
- Test: `src/domain.test.ts` and `src/components/FloatingIsland.test.ts`

**Interfaces:**
- Produces `diffSnapshot(previous: Snapshot, next: Snapshot): SnapshotChanges`.
- Produces mount/update pairs: `mountFloatingBall`, `mountFloatingIsland`, and `mountDetailsPanel`, each returning `{ update(snapshot): void; setRefreshing(value): void; destroy(): void }`.
- Existing `renderFloatingBall`, `renderFloatingIsland`, and `renderDetailsPanel` remain as compatibility wrappers for unit tests and call the new mount/update path once.

- [ ] **Step 1: Add failing diff tests**

Add tests for `diffSnapshot`:

```ts
expect(diffSnapshot(before, { ...before, todayTokens: 2 })).toEqual({ todayTokens: 2 })
expect(diffSnapshot(before, before)).toEqual({})
expect(diffSnapshot(before, { ...before, tasks: nextTasks })).toEqual({ tasks: nextTasks, activeTaskCount: before.activeTaskCount })
```

Run: `npm.cmd test -- --run src/domain.test.ts`

Expected: FAIL because `diffSnapshot` is not exported.

- [ ] **Step 2: Implement `SnapshotChanges` and `diffSnapshot`**

Compare only the visible fields (`status`, `quotaRemainingPercent`, `quotaResetsAt`, `plan`, `resetCredits`, `todayTokens`, `usageDate`, `activeTaskCount`, `tasks`, `error`, `fetchedAt`). Return a partial object containing only changed values. Compare task arrays by id, status, acknowledged, title, token count, and updated time.

- [ ] **Step 3: Add failing DOM identity tests**

Mount the ball and island, keep a reference to their button, update with a new snapshot, and assert the same button object is still present. Repeat for the details panel and assert the task list node remains identical.

Run: `npm.cmd test -- --run src/components/FloatingIsland.test.ts`

Expected: FAIL while the components still assign `root.innerHTML` for every update.

- [ ] **Step 4: Implement stable mount/update components**

Create the static DOM once in each mount function, retain references to text, ring canvas, status dot, task list, freshness text, and refresh button, and update only those references. Keep pointer/double-click listeners attached once. `update(snapshot)` must not add or remove the surface animation class and must not call any Tauri resize command.

The details mount keeps the existing pairing-settings open state, task acknowledgement listeners, and refresh button listener. Re-render the task rows only when the task array comparison says tasks changed.

- [ ] **Step 5: Update `main.ts` to separate view changes from data changes**

Store one mounted view instance per view state. `renderView()` is called only when `viewState` changes. The `snapshot-updated` listener assigns the normalized snapshot, computes `diffSnapshot`, and calls the active view's `update` method. `refresh()` updates only the details refresh button while the request is running and never calls `render()` for ordinary data completion. `resizeWindow()` remains guarded by the existing target key and is invoked only from `renderView()`.

- [ ] **Step 6: Run frontend tests and build**

Run:

```powershell
npm.cmd test -- --run
npm.cmd run build
```

Expected: all frontend tests pass and Vite produces `dist/` without TypeScript errors.

- [ ] **Step 7: Commit the stable renderer**

```powershell
git add src/domain.ts src/domain.test.ts src/main.ts src/components/FloatingIsland.ts src/components/FloatingIsland.test.ts src/components/DetailsPanel.ts
git commit -m "feat: update codex views without remounting"
```

### Task 3: Debounced event delivery and freshness diagnostics

**Files:**
- Modify: `src-tauri/src/poller.rs`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src/domain.ts`
- Modify: `src/components/DetailsPanel.ts`
- Test: `src-tauri/src/codex_client_tests.rs` and `src/domain.test.ts`

**Interfaces:**
- Adds `changed_at` and `source` to the snapshot payload while preserving existing snake_case compatibility.
- Adds a 150ms frontend coalescing queue for consecutive `snapshot-updated` events.

- [ ] **Step 1: Add freshness and ordering tests**

Test that an event with an older `changed_at` is ignored, an error snapshot does not advance `fetched_at`, and a changed usage date is surfaced as a visible data-date label.

- [ ] **Step 2: Add monotonic event metadata**

Set `changed_at` whenever a candidate snapshot changes; set `source` to `task-watch`, `metrics-poll`, `full-poll`, or `manual-refresh`. In the frontend, retain the newest `changedAt` and ignore older events.

- [ ] **Step 3: Add frontend coalescing**

Queue incoming snapshots for 150ms, keep the newest event, compute one diff, and apply it once. Cancel the queued timer on view destruction. Do not coalesce manual refresh errors with successful data.

- [ ] **Step 4: Surface the sync source in details**

Keep the existing freshness line and append the source only when useful, for example `最近更新 04:41 · 任务监听` or `最近更新 04:41 · 指标校验`. Never expose raw protocol errors or file paths.

- [ ] **Step 5: Run all tests**

Run: `npm.cmd test -- --run` and `cargo test`

Expected: all tests pass, including ordering and freshness cases.

### Task 4: Browser stability and release-readiness verification

**Files:**
- Modify: `tests/interaction.spec.ts` if the existing browser flow needs selectors for update assertions.
- Create: `tests/no-flicker.spec.ts`
- Modify: `docs/release/` only if the verification notes need the new sync behavior.

- [ ] **Step 1: Add the browser flow test**

Open the local preview, double-click through ball, summary, details, and ball; assert the view changes exactly once per double-click and a single click leaves the view unchanged.

- [ ] **Step 2: Add the stability test**

Expose a design-preview-only `window.__codexTestApplySnapshot(snapshot)` bridge that calls the same coalesced snapshot handler as the Tauri event listener. While staying on each view, call it 20 times with unchanged data and assert the active surface node identity, `data-view`, and bounding box remain stable. Call it once with a changed quota and assert only the quota text/canvas changes.

- [ ] **Step 3: Run browser verification**

Run: `npm.cmd run test:ui`

Expected: the three-state flow and no-flicker assertions pass at the configured local viewport.

- [ ] **Step 4: Perform a non-packaging Windows smoke check**

Run `npm.cmd run tauri dev` from the project root, wait for the native window, and stop the dev process with Ctrl+C after the check. Confirm no console window appears, the app remains open when the hidden Codex child exits, and manual refresh still updates the details panel. Do not run `npm.cmd run tauri build` or build an installer during this task unless the user explicitly requests packaging.

- [ ] **Step 5: Commit verification updates**

```powershell
git add tests/interaction.spec.ts tests/no-flicker.spec.ts docs/release
git commit -m "test: verify codex sync stability and state transitions"
```

## Execution Order

Complete Tasks 1 and 2 first because the backend must stop emitting duplicate snapshots before the frontend can prove stable rendering. Complete Task 3 after both sides have their interfaces, then run Task 4 as the final verification gate. Do not create an installer as part of this plan.
