# Plus 5 小时额度与持久化趋势 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Plus 用户增加 5 小时额度展示，并将额度趋势持久化为最近 7 天的小时采样，同时保持所有任务与其他现有数据逻辑不变。

**Architecture:** 后端继续把现有 `primary` 解析结果作为周额度，在 `RateLimitResponse`/`Snapshot` 中增加可选的 5 小时额度字段；Plus UI 仅在渲染层选择 5 小时字段，非 Plus 保持原显示。`SnapshotStore` 负责按本地小时去重并保留 168 个额度点，现有快照缓存自动持久化；桌面端与网页端共享同一快照字段。

**Tech Stack:** Rust/Tauri 2、Serde、TypeScript、Vitest、现有快照缓存与 CSS。

## Global Constraints

- 不修改任务状态、Token、配对、可重置机会和套餐识别逻辑。
- 趋势只保存额度百分比，不保存或绘制 Token 用量。
- Plus 缺少 5 小时字段时显示 `--`，不得把周额度冒充 5 小时额度。
- 非 Plus 的现有文案、字段和数值保持不变。

---

### Task 1: 扩展额度协议字段并保持旧结构兼容

**Files:**
- Modify: `src-tauri/src/codex_protocol.rs:16-124`
- Modify: `src-tauri/src/domain.rs:100-140`
- Modify: `src/domain.ts:35-145`
- Test: `src-tauri/src/codex_client_tests.rs`
- Test: `src/domain.test.ts`

**Interfaces:**
- `RateLimitResponse` produces `five_hour_remaining_percent: Option<u8>` and `five_hour_resets_at: Option<i64>` in addition to existing fields.
- `Snapshot` carries the same optional fields serialized as `five_hour_remaining_percent` and `five_hour_resets_at`.
- TypeScript `Snapshot`/`normalizeSnapshot` exposes camelCase `fiveHourRemainingPercent` and `fiveHourResetsAt`.

- [ ] **Step 1: Add failing Rust parser tests**

Add a fixture input containing `rateLimits.primary`, `rateLimits.secondary`, `planType: "Plus"`, and `rateLimitResetCredits`; assert primary and five-hour values are both parsed. Add a second test with only primary and assert the new fields are `None`.

- [ ] **Step 2: Run parser tests and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml codex_client_tests`.
Expected: compilation/test failure because the new fields do not exist.

- [ ] **Step 3: Implement minimal parser/domain changes**

Add a helper that reads `secondary`, `secondaryLimit`, `fiveHour`, or `five_hour` from `rateLimits`; parse `remaining_percent`/`remainingPercent`/`usedPercent` and `resets_at`/`resetsAt` using the existing numeric helpers. Do not change primary or plan parsing. Add serde-defaulted optional fields to Rust and matching optional fields plus `diffSnapshot`/`normalizeSnapshot` support to TypeScript.

- [ ] **Step 4: Run focused tests**

Run `cargo test --manifest-path src-tauri/Cargo.toml codex_client_tests` and `npm test -- --run src/domain.test.ts`.
Expected: PASS; old primary/plan tests remain unchanged.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/codex_protocol.rs src-tauri/src/domain.rs src-tauri/src/codex_client_tests.rs src/domain.ts src/domain.test.ts
git commit -m "feat: expose plus five-hour quota fields"
```

### Task 2: Propagate five-hour values through polling without touching task data

**Files:**
- Modify: `src-tauri/src/poller.rs:35-58,440-486`
- Modify: `src-tauri/src/snapshot_store.rs:60-175`
- Modify: `src-tauri/src/snapshot_cache.rs:1-30`
- Test: `src-tauri/src/poller.rs` existing quota tests
- Test: `src-tauri/src/snapshot_cache.rs`

**Interfaces:**
- `poll_once` and metrics refresh copy only `rate.five_hour_remaining_percent` and `rate.five_hour_resets_at` into `Snapshot`.
- Cache load accepts old snapshots with absent optional fields.

- [ ] **Step 1: Add failing propagation test**

Extend the existing parser/poll snapshot fixture assertion so a Plus response with secondary values produces the two new snapshot fields while `tasks`, `task_counts`, and `today_tokens` remain unchanged.

- [ ] **Step 2: Implement propagation**

Populate the new fields in every `Snapshot` constructor fed by `RateLimitResponse`; on stale/error paths preserve previous five-hour values exactly as existing quota fields are preserved. Do not alter task registry calls or status calculations.

- [ ] **Step 3: Verify cache compatibility**

Add a cache test that serializes a snapshot without five-hour fields, loads it, and asserts successful deserialization with both fields `None`; add a second test that round-trips populated fields.

- [ ] **Step 4: Run Rust tests**

Run `cargo test --manifest-path src-tauri/Cargo.toml poller snapshot_cache`.
Expected: PASS with no task-count regressions.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/poller.rs src-tauri/src/snapshot_cache.rs src-tauri/src/snapshot_store.rs
git commit -m "feat: carry plus five-hour quota through snapshots"
```

### Task 3: Retain 7 days of hourly quota history

**Files:**
- Modify: `src-tauri/src/snapshot_store.rs:1-57,65-173`
- Modify: `src-tauri/src/snapshot_cache.rs:1-30`
- Test: `src-tauri/src/snapshot_store.rs:390-425`

**Interfaces:**
- `merge_hourly_history(previous, at, quota)` returns at most 168 points, one point per local hour, sorted ascending, across the rolling 7-day window.

- [ ] **Step 1: Replace the current-day test with a rolling-window test**

Feed 200 hourly points spanning more than seven days; assert length is 168, timestamps are sorted, oldest/newest differ by at most 167 hours, and inserting another value in an existing hour replaces that hour’s value.

- [ ] **Step 2: Run the focused test and verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml snapshot_store::tests`.
Expected: FAIL because current code filters to one local date and caps at 24.

- [ ] **Step 3: Implement rolling retention**

Remove date filtering, bucket by local hour, replace matching buckets, sort, and drain older entries until 168 remain. Keep the existing `DataStatus::Fresh` gate and leave cache save/load behavior unchanged except for carrying the longer history.

- [ ] **Step 4: Run history and cache tests**

Run `cargo test --manifest-path src-tauri/Cargo.toml snapshot_store snapshot_cache`.
Expected: PASS, including cache persistence after reload.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/snapshot_store.rs src-tauri/src/snapshot_cache.rs
git commit -m "feat: retain seven days of hourly quota history"
```

### Task 4: Update desktop and web quota-only presentation

**Files:**
- Modify: `src/components/DetailsPanel.ts:1-150`
- Modify: `src/components/FloatingIsland.ts:80-145`
- Modify: `src/styles.css:65-80`
- Modify: `web/index.html:1-110`
- Test: `src/components/DetailsPanel.test.ts` (create if absent)
- Test: `src/components/FloatingIsland.test.ts:1-120`

**Interfaces:**
- Add pure formatting helpers for Plus quota selection and reset text; task rendering callbacks and task arrays remain untouched.
- Desktop and web render the same labels from the same snapshot fields.

- [ ] **Step 1: Add failing UI tests**

Create a DetailsPanel test with `plan: "Plus"`, primary 83%, five-hour 62%, and both reset timestamps; assert title contains `5小时额度剩余 62%`, reset label is `5小时额度重置时间`, value contains `8/30 23:05` without year/seconds, and auxiliary text contains `本周剩余 83%，重置时间为 8/30`. Add a non-Plus assertion that current `本周剩余` text is unchanged; add a missing-five-hour assertion for `--`.

- [ ] **Step 2: Implement desktop rendering**

In `DetailsPanel.ts`, choose five-hour values only when `snapshot.plan` case-insensitively equals `plus`; leave `planValue`, `creditsValue`, token rendering, task list, and pairing handlers unchanged. Add a compact auxiliary line in the title area and use the existing safe formatting helpers.

- [ ] **Step 3: Implement island/ball rendering**

Use the same Plus selector for the percentage/ring and label in `FloatingIsland.ts`; keep higher-plan output on existing primary fields. Do not touch task status counting or token formatting.

- [ ] **Step 4: Implement web rendering and horizontal trend**

Mirror the same selector/labels in `web/index.html`. Render up to 168 hourly points in a fixed-width track inside an `overflow-x:auto` container; show only quota bars and date/hour titles. Add CSS that prevents layout expansion and preserves existing card styling.

- [ ] **Step 5: Run frontend tests/build**

Run `npm test -- --run src/components/DetailsPanel.test.ts src/components/FloatingIsland.test.ts src/domain.test.ts` and `npm run build`.
Expected: PASS and a successful production build.

- [ ] **Step 6: Commit**

```bash
git add src/components/DetailsPanel.ts src/components/FloatingIsland.ts src/styles.css web/index.html src/components/DetailsPanel.test.ts src/components/FloatingIsland.test.ts
git commit -m "feat: show plus five-hour quota and seven-day trend"
```

### Task 5: Regression verification and scope audit

**Files:**
- Test: existing Rust and TypeScript test suites
- Modify: none unless a test exposes a quota-only regression

- [ ] **Step 1: Run all existing tests**

Run `cargo test --manifest-path src-tauri/Cargo.toml` and `npm test -- --run`.
Expected: PASS; no task, Token, pairing, reset-credit, or plan tests change behavior.

- [ ] **Step 2: Inspect the diff for forbidden changes**

Run `git diff HEAD~4 -- src-tauri/src/task_registry.rs src-tauri/src/lan_server.rs src/components/TaskList.ts`.
Expected: no changes in these task/approval files.

- [ ] **Step 3: Verify snapshot compatibility**

Load a pre-change cached snapshot fixture and a populated Plus snapshot through `normalizeSnapshot`; assert both render without exceptions and the new fields are optional.

- [ ] **Step 4: Commit only if verification adds a focused regression test**

```bash
git status --short
```
Expected: clean working tree after the preceding commits, or only the explicitly added test commit.
