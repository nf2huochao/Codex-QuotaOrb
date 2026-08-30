# 长条、趋势与配对设置界面优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 统一桌面与网页长条文案，隐藏趋势原生横向滑轨，并让重置配对按钮与现有界面风格一致。

**Architecture:** 只改前端渲染文案和 CSS。桌面端由 `FloatingIsland.ts` 与 `styles.css` 负责，网页版由 `web/index.html` 自包含模板负责；趋势数据、滚轮切换、拖拽和配对请求保持原样。

**Tech Stack:** TypeScript、原生 DOM、Vite、Tauri、Vitest。

## Global Constraints

- 不修改额度、Token、任务状态、采样、Snapshot 或配对请求逻辑。
- 趋势区域继续支持 24/48/72/96/120/144/168 小时窗口和横向拖拽。
- 桌面端和网页版使用相同的显示文案。

---

### Task 1: 更新桌面端与网页端长条文案

**Files:**
- Modify: `src/components/FloatingIsland.ts`
- Modify: `web/index.html`
- Test: `src/components/FloatingIsland.test.ts`

- [ ] 将 Plus 长条辅助文案从 `周额度剩余 ${weeklyPercent}%` 改为 `周剩余 ${weeklyPercent}%`。
- [ ] 将 Plus 长条主文案从 `5小时额度可用` 改为 `5小时额度`。
- [ ] 将网页端对应模板字符串同步改成相同文案。
- [ ] 更新现有 UI 断言，保留额度字段和状态数据断言。

### Task 2: 隐藏趋势区域原生横向滑轨

**Files:**
- Modify: `src/styles.css`
- Modify: `web/index.html`
- Test: `src/components/FloatingIsland.test.ts`

- [ ] 为桌面端 `.history-scroll` 增加 `scrollbar-width: none` 和 WebKit 滑轨隐藏规则。
- [ ] 为网页版 `.history-scroll` 增加同等规则。
- [ ] 保留 `overflow-x: auto`、横向拖拽和现有滚轮切换行为，不改变历史点数量或样式。
- [ ] 保持 7 天窗口内容仍可通过横向拖拽访问。

### Task 3: 统一重置配对按钮样式

**Files:**
- Modify: `src/styles.css`
- Modify: `web/index.html`

- [ ] 将 `.pairing-reset-button` 设置为与 `.refresh-button` 相同的圆角、浅绿色背景、文字颜色、内外阴影和交互光标。
- [ ] 保留按钮现有点击监听和重置请求，不修改配对逻辑。
- [ ] 网页端配对相关按钮继续使用现有统一按钮规则。

### Task 4: 构建与渲染验证

**Files:**
- Verify: `src/components/FloatingIsland.test.ts`
- Verify: `package.json` build script
- Verify: `http://127.0.0.1:4173/?design-preview=1`

- [ ] 运行 `npm test -- --run src/components/FloatingIsland.test.ts`，确认文案、趋势窗口和滚轮行为通过。
- [ ] 运行 `npm run build`，确认 TypeScript 与 Vite 构建通过。
- [ ] 在本地预览确认桌面长条文案、7 天趋势无原生滑轨、配对按钮样式。
