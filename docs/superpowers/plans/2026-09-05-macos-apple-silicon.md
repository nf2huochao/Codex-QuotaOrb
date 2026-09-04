# macOS Apple Silicon 支持实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在保持现有 Windows 版本行为不变的前提下，为 Tauri 应用增加可在 Apple Silicon Mac 上安装和运行的 `.app` / `.dmg` 测试包。

**Architecture:** 前端、额度/趋势/预测、局域网网页和 Rust 业务逻辑继续共用；仅将 Codex 可执行文件发现、开机自启、菜单栏托盘和打包更新配置按目标系统分支。macOS 首阶段发布未签名测试包，使用现有 Tauri updater 签名并把 `darwin-aarch64` 写入发布清单。

**Tech Stack:** Tauri 2、Rust cfg(target_os)、tauri-plugin-autostart、GitHub Actions macOS runner、DMG/App bundle。

## Global Constraints

- Apple Silicon 首发目标为 `aarch64-apple-darwin`。
- 不改变现有 Windows NSIS、Windows 注册表自启和 Windows Codex 路径逻辑。
- 继续使用应用标识 `com.codex.quota-floating-window`。
- Mac 测试包暂不承诺 Apple Developer ID 签名和公证；安装时可能出现系统安全提示。
- 更新清单平台键使用 `darwin-aarch64`，Windows 继续使用 `windows-x86_64`。

---

### Task 1: 平台化 Codex 可执行文件发现

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] 将 Windows 专用 Codex 路径放入 `#[cfg(windows)]`，为 macOS 增加 Apple Silicon、Intel、Homebrew 和用户本地 npm 安装路径，并保留 `CODEX_BINARY` 优先级。
- [ ] 将 `winreg` 改为 Windows target-specific 依赖，确保 macOS 不编译 Windows 注册表库。
- [ ] 运行 `cargo check --target aarch64-apple-darwin`（在 macOS runner 上）和现有 Windows 测试。

### Task 2: macOS Tauri bundle 配置

**Files:**
- Create: `src-tauri/tauri.macos.conf.json`
- Modify: `src-tauri/icons/icon.icns`（仅在构建验证发现图标不合格时替换）

- [ ] 覆盖 bundle targets 为 `app`、`dmg`，使用已有 `icon.icns`。
- [ ] 保留无边框、透明、置顶悬浮窗配置；启用 Tauri macOS 透明窗口所需的私有 API 配置，并明确该包面向站外 DMG 分发。
- [ ] 让 Tauri 自动合并该平台配置，不改 Windows 主配置。

### Task 3: macOS GitHub Actions 构建与发布

**Files:**
- Create: `.github/workflows/release-macos.yml`

- [ ] 在 `macos-latest` runner 上安装 Node/Rust，运行现有测试和前端构建。
- [ ] 使用 Tauri action 构建 `aarch64-apple-darwin` 的 `.app`、`.dmg`、updater archive 和 `.sig`。
- [ ] 将构建产物上传到对应 GitHub Release；更新 `latest.json`，合并 `darwin-aarch64` 条目，不覆盖现有 Windows 条目。
- [ ] 支持手动指定已有 tag，便于给当前版本补充 Mac 测试包。

### Task 4: 发布验证与文档

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/release-signing.md`（如已有说明不足）

- [ ] 在 README 中说明 Apple Silicon 测试包下载位置、首次打开可能需要允许，以及当前未公证限制。
- [ ] 验证 GitHub Release 中同时存在 DMG、updater archive、`.sig` 和包含两个平台的 `latest.json`。
- [ ] 在真实 Mac 不可用的情况下，明确把菜单栏、权限、Retina 和更新行为列为待实机验收项。
