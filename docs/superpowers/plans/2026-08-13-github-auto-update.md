# GitHub 签名自动更新 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Windows 悬浮窗安全发布到 GitHub，并让托盘“检查更新”从 GitHub Releases 检查、验签、安装和重启。

**Architecture:** Tauri Updater 读取 `releases/latest/download/latest.json`；发布包和清单由 GitHub Actions 在版本标签发布时生成。更新包必须使用固定私钥签名，客户端只内置公开密钥。

**Tech Stack:** Tauri 2、Rust、TypeScript、GitHub Actions、GitHub Releases、NSIS。

## Global Constraints

- 私钥只保存在本机安全目录或 GitHub Actions Secret，不进入 Git。
- 公开仓库不得包含本机绝对路径、配对码、日志、缓存和构建目录。
- 更新必须验签，不能关闭签名校验。
- 保留“悬浮球 → 长条 → 详情 → 悬浮球”双击循环。

---

### Task 1: 清理公开发布内容

**Files:**
- Modify: `.gitignore`
- Modify: `design-qa.md`
- Modify: `docs/release/windows.md`

- [ ] 删除文档中的本机绝对路径，改为仓库相对路径或环境变量。
- [ ] 确认 `target/`、`dist/`、`tmp/`、日志、签名私钥均被忽略。
- [ ] 执行敏感信息扫描，确认无真实配对码、密码、密钥和用户目录。

### Task 2: 接入 Tauri Updater

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/main.ts`

- [ ] 添加 updater/process 插件与权限。
- [ ] 配置 GitHub Releases 的 `latest.json` HTTPS 端点和客户端公钥。
- [ ] 将托盘更新命令改成检查版本、下载安装、提示并重启。
- [ ] 对无更新、网络失败、验签失败提供明确中文提示。

### Task 3: 配置签名发布流水线

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `docs/release/auto-update.md`

- [ ] 生成固定签名密钥；私钥保存在本机忽略目录，公钥写入配置。
- [ ] 工作流从 GitHub Secrets 读取 `TAURI_SIGNING_PRIVATE_KEY` 和密码。
- [ ] 推送 `v*` 标签时运行测试、构建 NSIS、生成签名和 `latest.json`、发布 Release。
- [ ] 文档记录密钥备份和后续发版步骤。

### Task 4: 验证并公开发布

**Files:**
- Test: all frontend, Rust and UI tests

- [ ] 运行 `npm.cmd test`、`npm.cmd run test:ui`、`cargo test --manifest-path src-tauri/Cargo.toml`。
- [ ] 使用签名环境变量构建 Windows 更新包，确认 `.sig` 和安装包生成。
- [ ] 将清理后的完整项目提交并推送到 `nf2huochao/Codex-Floating-ball`。
- [ ] 创建版本标签和 GitHub Release，确认 `latest.json` 可访问。

