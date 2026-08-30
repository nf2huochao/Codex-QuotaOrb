# 周额度历史来源与缺采样显示 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 按额度窗口时长识别 5 小时与周额度，并让趋势只保存、显示周额度；缺采样小时在显示层沿用最近真实周额度。

**Architecture:** Rust 协议解析层从窗口时长提取明确的 weekly/five-hour 字段，快照历史继续只接受 weekly 字段。桌面端与网页端渲染器在已有历史点之间做前向填充，但不修改持久化历史。

**Tech Stack:** Rust、Serde JSON、TypeScript、原生 DOM、Vitest。

## Global Constraints

- 不修改任务状态、Token、配对、可重置机会或套餐识别。
- 不把 5 小时额度写入周额度历史。
- 缺采样补值只发生在显示层，不新增伪造历史点。
- 保留对旧版无 `windowDurationMins` 响应的兼容回退。

---

### Task 1: 按窗口时长解析额度

**Files:**
- Modify: `src-tauri/src/codex_protocol.rs`
- Modify: `src-tauri/src/codex_client_tests.rs`

**Interfaces:**
- `RateLimitResponse.remaining_percent` 始终表示周额度。
- `RateLimitResponse.five_hour_remaining_percent` 始终表示 5 小时额度。

- [ ] **Step 1: Add regression fixtures**

增加窗口顺序相反的 JSON fixture：`primary.windowDurationMins=300`、`secondary.windowDurationMins=10080`，断言 weekly 使用 secondary、five-hour 使用 primary；再增加只有 `primary.windowDurationMins=10080` 的 Plus fixture，断言 weekly 使用 primary。

- [ ] **Step 2: Implement duration-based selection**

为窗口读取 `windowDurationMins/window_duration_mins/windowMinutes`，优先把 10080 分钟窗口解析为 weekly，把 300 分钟窗口解析为 five-hour；命名窗口字段优先级高于位置字段；无时长时保留当前 primary/secondary 兼容回退。

- [ ] **Step 3: Run protocol tests**

运行 `cargo test --manifest-path src-tauri/Cargo.toml codex_client_tests`，确认旧格式和新格式都通过。

### Task 2: 周额度历史缺采样显示

**Files:**
- Modify: `src/components/DetailsPanel.ts`
- Modify: `web/index.html`
- Modify: `src/components/FloatingIsland.test.ts`

**Interfaces:**
- 历史数组仍为真实 `quotaRemainingPercent` 点，不改变 `UsagePoint` 结构。
- 渲染器对选中窗口生成显示副本：缺值小时继承前一个真实周额度，并给补值柱增加提示标题。

- [ ] **Step 1: Add renderer tests**

构造带首尾采样、缺少中间小时的周额度历史，断言缺口柱高度和数值沿用前一个采样；构造没有任何历史点的快照，断言仍显示空占位。

- [ ] **Step 2: Implement forward-fill rendering**

桌面端和网页端在筛选窗口后按时间顺序遍历，维护最近一个非空周额度；已有点使用真实值，缺口仅生成渲染用值并在 `title` 标注“沿用上次采样”。趋势摘要首尾值只取渲染结果中的周额度。

- [ ] **Step 3: Run focused frontend tests and build**

运行 `npm test -- --run src/components/FloatingIsland.test.ts` 与 `npm run build`。

### Task 3: Scope verification

**Files:**
- Modify: none

- [ ] **Step 1: Run full verification**

运行 `npm test`、`cargo test --manifest-path src-tauri/Cargo.toml` 和 `git diff --check`。

- [ ] **Step 2: Audit diff**

确认差异仅涉及额度窗口解析、周额度历史渲染和对应测试，未触及任务、Token、配对、可重置机会或套餐识别。
