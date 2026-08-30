# 趋势窗口交互 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 默认显示当天额度采样，并用趋势区域滚轮在当天到最近 7 天之间切换可见范围。

**Architecture:** 桌面端和网页端各自维护仅用于视图的可见小时数，初始为 24，滚轮每次增减 24 并限制在 24–168；渲染层从已有快照历史排序后截取尾部窗口。后端历史保存逻辑不变。

**Tech Stack:** TypeScript、原生 DOM/CSS、Vitest。

## Global Constraints

- 不修改任务状态、Token、配对、可重置机会和套餐识别。
- 不新增后端接口，不改变已有 168 个小时点的保存规则。
- 趋势无数据时只显示占位，不伪造额度值。

---

### Task 1: 桌面趋势窗口

**Files:**
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src/components/FloatingIsland.test.ts`

**Interfaces:**
- `renderHistory(root, snapshot, visibleHours)` 渲染排序后的历史尾部窗口。
- 趋势卡片监听 `wheel`，将可见小时数限制在 24 到 168。

- [ ] **Step 1: Write the failing test**

在现有详情面板测试中加入 168 点历史，断言首次显示 24 根柱、滚轮展开后显示 48 根，继续展开最多 168 根。

- [ ] **Step 2: Implement the view state**

在 `mountDetailsPanel` 内增加 `visibleHistoryHours = 24`，将其传给 `renderHistory`；在 `.history-card` 上监听 `wheel`，按 `Math.min(168, current + 24)` 或 `Math.max(24, current - 24)` 更新并重绘。

- [ ] **Step 3: Run focused tests**

运行 `npm test -- --run src/components/FloatingIsland.test.ts`，确认趋势窗口和既有任务测试通过。

---

### Task 2: 网页趋势窗口

**Files:**
- Modify: `web/index.html`

**Interfaces:**
- 网页端 `renderHistory` 使用 `visibleHistoryHours`，初始为 24，滚轮范围为 24–168。

- [ ] **Step 1: Add the wheel state and listener**

在网页脚本中增加 `let visibleHistoryHours=24`，趋势渲染按排序后 `slice(-visibleHistoryHours)`；为 `.history-card` 注册 `wheel` 监听，变化后调用 `renderHistory(latestSnapshot?.history||[])` 并阻止页面滚动。

- [ ] **Step 2: Update labels**

当天窗口显示“当天采样”，展开后显示“最近 2 天”至“最近 7 天”；摘要继续显示首尾额度。

- [ ] **Step 3: Run build**

运行 `npm run build`，确认网页脚本和桌面端类型检查通过。

---

### Task 3: Regression verification

**Files:**
- Modify: none

- [ ] **Step 1: Run all tests**

运行 `npm test` 和 `cargo test --manifest-path src-tauri/Cargo.toml`。

- [ ] **Step 2: Audit scope**

确认本次差异只涉及趋势渲染、滚轮交互和测试，不包含任务、Token、配对、重置机会或套餐识别代码。
