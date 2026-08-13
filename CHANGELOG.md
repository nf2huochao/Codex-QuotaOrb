# Changelog / 版本记录

All notable changes to Codex QuotaOrb are documented here in Chinese and English.

Codex 额度悬浮窗的主要新增、改进和修复均在此以中英文记录。

## v0.1.6 — 2026-08-13

### Added / 新增

- Added transparent icon assets for the Windows shortcut, tray icon, installer, and mobile icon sets. / 为 Windows 桌面快捷方式、托盘、安装包和移动端图标新增透明背景资源。
- Added persistent same-Wi-Fi pairing support and refreshed mobile sync handling. / 增加同一 Wi-Fi 配对保持能力，并更新移动端同步处理。
- Added manual workflow dispatch for the Windows release pipeline. / 为 Windows 发布流水线增加手动触发入口。

### Improved / 改进

- Updated the application version to 0.1.6 across npm, Cargo, and Tauri metadata. / 统一更新 npm、Cargo 和 Tauri 元数据版本号至 0.1.6。
- Regenerated icon sizes from one transparent source so shortcut and tray rendering use the same visual identity. / 从同一透明源重新生成各尺寸图标，保证桌面快捷方式和托盘视觉一致。

### Scope note / 范围说明

- Removed the experimental Cloudflare deployment templates from the project scope. / 移除实验性的 Cloudflare 部署模板，不再把远程访问作为当前版本功能。

## v0.1.5 — 2026-08-12

### Added / 新增

- Added the updater manifest publication step for Windows releases. / 增加 Windows 发布时的更新清单发布步骤。

### Improved / 改进

- Stabilized release metadata and artifact upload behavior for the existing GitHub Release workflow. / 稳定现有 GitHub Release 流程的版本元数据和附件上传行为。
- Kept signing credentials in GitHub Actions secrets instead of the repository. / 将签名凭据保存在 GitHub Actions Secrets 中，不写入仓库。

## v0.1.4 — 2026-08-10

### Added / 新增

- First signed Windows auto-update build. / 首个支持签名自动更新的 Windows 构建版本。
- Floating orb, three-part capsule, and details page interaction model. / 建立悬浮球、三等分胶囊和详情页的交互模型。
- Weekly quota, task status, daily Token usage, tray controls, same-Wi-Fi pairing, and mobile web view. / 加入本周额度、任务状态、本日 Token、托盘控制、同 Wi-Fi 配对和移动网页查看。

### Improved / 改进

- Added test coverage for state transitions, no-flicker updates, pairing, and packaging smoke checks. / 增加状态切换、无闪烁更新、配对和打包冒烟测试。
- Added cream-white, sage-green, and apricot neumorphic styling. / 采用奶油白、鼠尾草绿和杏色的新拟物视觉风格。

## Compatibility and upgrade notes / 兼容与升级说明

- Windows is the supported desktop platform. / 桌面端支持 Windows。
- Installers and signed updater files are published on [GitHub Releases](https://github.com/nf2huochao/Codex-QuotaOrb/releases). / 安装包和签名更新文件发布在 [GitHub Releases](https://github.com/nf2huochao/Codex-QuotaOrb/releases)。
- The application requires Codex to be running locally for live state synchronization. / 要实时同步状态，需要本机 Codex 正在运行。
