# Floating Ball State Machine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 将 Windows 悬浮窗实现为“圆球常驻 → 三段状态胶囊 → 详情面板”的平滑三层状态机，并让 Codex 快照变化即时驱动 UI。

**Architecture:** 前端维护 `ball | summary | details` 三个展示状态；Tauri 只负责原生窗口尺寸和快照事件。用户点击与拖动由同一窗口完成，6px 位移阈值区分点击和拖动；Rust `SnapshotStore` 通过 Tauri 事件把每次快照发布推送给前端，2 分钟轮询继续作为读取兜底。

**Tech Stack:** Tauri v2、Rust/Tokio、Vite、TypeScript、Vitest、应用内 Browser、Windows 原生窗口实测。

## Global Constraints

- 保留奶油白 `#ebe4d6`、鼠尾草绿 `#bdcfa2`、杏色 `#ddb480` 和外凸/内凹阴影语言。
- 红色=需要回复/确认/授权，黄色=执行中，绿色=已完成可验收，灰色=无活跃任务。
- 不读取或保存 Codex 登录令牌；不增加 Codex 写操作。
- 自动读取保持 120 秒；实时事件只更新展示层，不绕过快照仓库。
- 原生窗口必须无方形底板、无系统矩形阴影、无调试控制台窗口。

---

### Task 1: 前端三层状态机与点击阈值

**Files:**
- Modify: `src/main.ts`
- Modify: `src/components/FloatingIsland.ts`
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src/styles.css`
- Test: `src/components/FloatingIsland.test.ts`

**Interfaces:**
- `renderFloatingBall(root, snapshot, onOpen): void`
- `renderFloatingIsland(root, snapshot, onOpen): void`
- `renderDetailsPanel(root, snapshot, onRefresh, onAcknowledge, onClose, pairingInfo, isRefreshing): void`
- Main state: `type ViewState = 'ball' | 'summary' | 'details'`.

- [ ] Write tests for ball-only markup, summary click callback, details close callback, and task status labels.
- [ ] Implement `ViewState` transitions: ball click → summary, summary click → details, details close → summary.
- [ ] Add pointer displacement guard so movement above 6px never invokes a view transition.
- [ ] Add `ball`, `summary`, and `details` CSS states with 180ms opacity/scale transitions and `prefers-reduced-motion` fallback.
- [ ] Run `npm.cmd test` and verify the focused component tests pass.

### Task 2: 平滑额度环与窗口尺寸动画

**Files:**
- Modify: `src/components/FloatingIsland.ts`
- Modify: `src/main.ts`
- Modify: `src/styles.css`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- `animateQuotaRing(canvas, fromPercent, toPercent): void`
- `animateWindowHeight(targetHeight, expanded): Promise<void>`
- Existing `set_window_expanded(expanded, height)` remains the native sizing boundary.

- [ ] Preserve the previous displayed percentage and tween the canvas arc over 240ms.
- [ ] Resize native height with a short easing loop when entering and leaving details; clamp to 360–560 logical px in details.
- [ ] Animate panel entrance and return without exposing a scroll bar on the outer window.
- [ ] Add a reduced-motion path that applies the final value immediately.

### Task 3: Codex 快照实时事件与手动刷新

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/main.ts`
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src-tauri/src/tray.rs`
- Test: `src/domain.test.ts` or a new focused frontend test.

**Interfaces:**
- Tauri event `snapshot-updated` carries the serialized `Snapshot`.
- Tauri event `refresh-requested` invokes the same frontend `refresh()` flow as the details button.

- [ ] Subscribe to `SnapshotStore` in `setup` and emit `snapshot-updated` after every publish.
- [ ] Listen for `snapshot-updated` in the frontend and render the current view immediately.
- [ ] Track `isRefreshing`, disable duplicate refresh clicks, and show “正在更新” until completion.
- [ ] Keep the 120-second interval as a fallback and ensure fresh/stale/error copy remains explicit.
- [ ] Run `cargo test` and `npm.cmd test`.

### Task 4: 视觉与行为验收

**Files:**
- Create/Update: `design-qa.md`
- Create: `docs/release/ball-summary-details-*.png`

- [ ] Start the local preview and use the in-app Browser to verify ball → summary → details, refresh feedback, console health, and reduced-motion CSS.
- [ ] Build the release NSIS installer; never use the debug installer for user handoff.
- [ ] Launch the release executable on Windows and verify no console window, no rectangular background, real drag, real click transitions, and live snapshot text.
- [ ] Capture ball, summary, and details screenshots and update `design-qa.md` with the state-by-state comparison and final result.
