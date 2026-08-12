# Codex 额度悬浮窗

Windows 桌面悬浮窗，用于查看 Codex 本周剩余额度、任务状态和本日 Token 消耗。

## 功能

- 双击循环切换：悬浮球 → 三等分胶囊 → 详情页。
- 奶油白、鼠尾草绿、杏色的轻拟物视觉风格。
- 连接本机 Codex app-server，状态变化时增量更新，过期或失败时明确提示。
- 托盘菜单支持刷新、配对设置、开机自启、检查更新和退出。
- Windows 安装包通过 GitHub Releases 发布，并使用 Tauri 签名校验自动更新。

## 获取安装包

请前往 [Releases](https://github.com/nf2huochao/Codex-Floating-ball/releases) 下载最新版 Windows 安装包。

## 本地开发

需要 Node.js、Rust 和 Windows C++ 构建工具。安装依赖后运行：

```powershell
npm.cmd install
npm.cmd run tauri dev
```

构建 Windows 安装包：

```powershell
npm.cmd run tauri build
```

签名更新的配置说明见 [docs/release/auto-update.md](docs/release/auto-update.md)。签名私钥只应保存在本机安全位置或 GitHub Actions Secret 中，不要提交到仓库。

## 隐私

项目不内置 Codex 账号凭据。局域网配对地址和令牌在运行时生成，仅用于同一 Wi‑Fi 下的设备配对。
