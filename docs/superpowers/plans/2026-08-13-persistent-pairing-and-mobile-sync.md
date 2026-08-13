# 固定配对码与移动端同步 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 固定保存四位局域网配对码，首次换取设备令牌并自动记住，同时修复网页旧快照覆盖新状态和重复重连问题。

**Architecture:** Rust 局域网服务持久化配对状态并提供换取令牌的接口；详情页显示固定四位码；网页使用 localStorage 保存令牌，HTTP/WebSocket 请求共享单调时间戳过滤器。

**Tech Stack:** Tauri 2、Rust、Axum、TypeScript、原生 HTML、Vitest、Playwright。

## Global Constraints

- 只允许同一 Wi‑Fi 访问，不上传 Codex 凭据或快照。
- 不改变圆球 → 三等分胶囊 → 详情页的双击循环逻辑。
- 配对码固定保存，只有明确重置时才更换。

---

### Task 1: 持久化配对状态与换取令牌接口

**Files:**
- Modify: `src-tauri/src/lan_server.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lan_server.rs` tests

- [ ] 添加 `PairingState { code, session_token }`、文件读写、四位码生成和 `POST /api/pair`。
- [ ] 将路由鉴权改为接受会话令牌，同时兼容旧的 `pair` 查询参数。
- [ ] 在 Tauri setup 中从 app data 目录加载或创建配对状态后启动局域网服务。
- [ ] 为正确/错误配对和状态重启持久化增加 Rust 测试。

### Task 2: 详情页显示固定四位码

**Files:**
- Modify: `src-tauri/src/lan_server.rs`
- Modify: `src/components/DetailsPanel.ts`
- Modify: `src/main.ts`
- Test: `src/components/FloatingIsland.test.ts`

- [ ] 扩展配对信息返回值，显示网页入口和四位码，不显示长期会话令牌。
- [ ] 保留“配对设置”入口与现有三态双击逻辑。
- [ ] 为配对卡片添加四位码断言。

### Task 3: 网页首次配对、自动记住与同步去旧

**Files:**
- Modify: `web/index.html`
- Modify: `tests/ui/pairing.spec.ts`

- [ ] 支持四位码 POST 换取令牌，保存到按主机隔离的 localStorage，并清理地址栏配对参数。
- [ ] HTTP/WebSocket 使用令牌，所有快照按 `changed_at` 严格拒绝旧数据。
- [ ] 维护唯一 WebSocket 和唯一重连计时器，断线自动恢复。
- [ ] 增加 Playwright 测试覆盖四位配对、自动记忆、旧快照过滤和断线重连。

### Task 4: 验证

**Files:**
- Modify: `tests/device-checklist.md`

- [ ] 运行 `npm.cmd test`、`npm.cmd run build`、`cargo test --manifest-path src-tauri/Cargo.toml`。
- [ ] 运行 `npm.cmd run test:ui`，确认移动宽度下配对和实时同步流程通过。
- [ ] 更新设备验收清单，注明首次输入四位码、后续自动连接和重置配对行为。
