# Changelog / 版本记录

All notable changes to Codex QuotaOrb are documented here in Chinese and English.

Codex 额度悬浮窗的主要新增、改进和修复均在此以中英文记录。

## v1.0.1 — 2026-09-04

### Fixed / 修复

- Published a versioned Windows installer and updater manifest so existing v1.0.0 installations can update normally. / 发布带新版本号的 Windows 安装包和更新清单，使现有 v1.0.0 安装可以正常更新。
- Corrected the updater manifest download URL field for future releases. / 修正后续发行版更新清单中的安装包下载地址字段。

### Release notes / 发行说明

- Windows x64 installer and signed updater files are published on [GitHub Releases](https://github.com/nf2huochao/Codex-QuotaOrb/releases/tag/v1.0.1). / Windows x64 安装包和签名更新文件发布在 [GitHub Releases](https://github.com/nf2huochao/Codex-QuotaOrb/releases/tag/v1.0.1)。

## v1.0.0 — 2026-09-03

### Added / 新增

- Published the first 1.0.0 release with signed Windows updater artifacts (`latest.json` and `.sig`). / 发布首个 1.0.0 版本，并提供签名 Windows 更新文件（`latest.json` 和 `.sig`）。
- Added bilingual interface switching, coordinated themes, settings controls, and low-frequency change hints. / 增加中英文界面切换、协调主题、设置项和低频变化提醒。

### Improved / 改进

- Recent trend now follows the reset cycle, keeps a complete seven-day timeline, and distinguishes sampled, carried-forward, and future values with restrained colors. / 近期趋势按重置周期计算，保留完整 7 天时间轴，并用克制的颜色区分采样、顺延和未来数据。
- Added update-data feedback, pairing copy/retest actions, and a compact reset forecast panel for desktop and LAN web views. / 增加更新数据反馈、配对码复制与连接重测，并在桌面端和局域网网页端提供紧凑的重置预测专栏。

### Scope / 范围

- Task status, Token, plan, weekly quota, pairing data model, and reset-credit logic remain unchanged. / 任务状态、Token、套餐、周额度、配对数据模型和可用重置机会逻辑保持不变。

### Release notes / 发行说明

- Windows x64 installer and signed updater files are published on [GitHub Releases](https://github.com/nf2huochao/Codex-QuotaOrb/releases/tag/v1.0.0). / Windows x64 安装包和签名更新文件发布在 [GitHub Releases](https://github.com/nf2huochao/Codex-QuotaOrb/releases/tag/v1.0.0)。

## v0.1.9 — 2026-09-01

### Added / 新增

- Added a compact “Codex 重置预测” panel to the desktop details page and LAN mobile web view. It shows the next-24-hour reset probability, elapsed time since the last reset, resets in the last 30 days, average wait, and the latest reset time. / 在桌面详情页和局域网手机网页新增紧凑的“Codex 重置预测”专栏，显示未来 24 小时重置概率、距上次重置时间、近 30 天重置次数、平均等待时间和最近重置时间。
- Added a LunarWerx source label and “查看证据” link for the public forecast data. / 增加 LunarWerx 数据来源标识和“查看证据”链接。

### Improved / 改进

- Real sampled weekly-quota bars in the recent trend use the accent orange color; inferred or carried-forward values remain muted green. / 近期趋势中真实采样的周额度柱改用橙色，推算或沿用的数据保持低饱和绿色。
- Forecast data refreshes independently and falls back to a clear unavailable state when the public source cannot be reached. / 预测数据独立刷新，公开来源不可访问时显示明确的不可用状态。

### Scope / 范围

- This release does not change task status, Token, plan, weekly-quota, pairing, or reset-credit logic. / 本版本不改动任务状态、Token、套餐、周额度、配对或可用重置机会逻辑。

### Release notes / 发行说明

- Windows installer: [`Codex._0.1.9_x64-setup.exe`](https://github.com/nf2huochao/Codex-QuotaOrb/releases/download/v0.1.9/Codex._0.1.9_x64-setup.exe). / Windows 安装包：[`Codex._0.1.9_x64-setup.exe`](https://github.com/nf2huochao/Codex-QuotaOrb/releases/download/v0.1.9/Codex._0.1.9_x64-setup.exe)。
- Build and test verification: `npm run build` passed; 39 tests passed. / 构建与测试验证：`npm run build` 通过，39 项测试全部通过。

## v0.1.8 — 2026-08-31

### Added / 新增

- Added separate Plus five-hour quota and weekly quota labels, reset times, and remaining percentages. / 为 Plus 套餐增加 5 小时额度与周额度的独立显示、重置时间和剩余百分比。
- Added a fixed seven-day by twenty-four-hour quota view with 168 hourly slots. / 增加固定的最近 7 天 × 24 小时额度视图，共 168 个小时位置。

### Improved / 改进

- Replaced the scrollable trend strip with one compact row of slim bars whose heights show the sampled weekly quota. / 将可滚动趋势条改为一行细柱，柱高直接表示已采样的周额度变化。
- Kept all 168 slots visible on desktop and LAN mobile web views; unsampled hours remain empty instead of carrying old values. / 桌面端和局域网手机网页端始终显示完整 168 个位置，未采样小时保持空白，不沿用旧值。
- Removed the native scrollbar and wheel-based window switching from the quota trend area. / 去除额度趋势区域的原生滑轨和滚轮切换窗口。
- Restored dated Token labels and preserved the shared task snapshot across the floating island, details page, and LAN web view. / 恢复带日期的 Token 标签，并保持悬浮球、详情页和局域网网页使用同一任务快照。

### Release notes / 发行说明

- Windows installer: `Codex 额度悬浮窗_0.1.8_x64-setup.exe`. / Windows 安装包：`Codex 额度悬浮窗_0.1.8_x64-setup.exe`。
- This release is a Windows x64 test build. Signed updater artifacts require the repository signing secrets described in `docs/release-signing.md`. / 本版本为 Windows x64 测试版；签名更新文件需要按 `docs/release-signing.md` 配置仓库签名 Secrets。

## v0.1.7 — 2026-08-24

### Added / 新增

- Added a bilingual README with real desktop and LAN mobile interface previews. / 重写中英文 README，加入真实桌面界面和手机局域网界面预览。
- Documented the Windows desktop workflow, same-Wi‑Fi web observation, privacy boundaries, platform support, and current release status. / 补充 Windows 工作方式、同一 Wi‑Fi 网页观察、隐私边界、平台支持和当前更新状态。

### Improved / 改进

- Unified the desktop capsule, details page, and LAN web view around the same task snapshot and status counts. / 统一桌面长条、详情页和局域网网页的任务快照与状态计数。
- Improved local session task discovery and filtering so internal lifecycle markers do not become fake task names. / 改进本机会话任务发现与过滤，避免内部生命周期标记变成虚假任务名称。
- Removed stale local task rows when the authoritative session source no longer contains them. / 当权威会话源不再包含任务时移除残留任务行。
- Stabilized the local watcher startup path and kept quota/Token reading separate from task-state observation. / 稳定本地监听器启动路径，并将额度/Token 读取与任务状态监听分开。

### Release notes / 发行说明

- Windows installer: `Codex 额度悬浮窗_0.1.7_x64-setup.exe`. / Windows 安装包：`Codex 额度悬浮窗_0.1.7_x64-setup.exe`。
- This release is a stable test build. Signed updater artifacts require the repository signing secrets described in `docs/release-signing.md`. / 本版本为稳定测试版；签名更新文件需要按 `docs/release-signing.md` 配置仓库签名 Secrets。

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
