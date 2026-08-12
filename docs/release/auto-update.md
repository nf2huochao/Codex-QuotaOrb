# GitHub 自动更新

托盘“检查更新”从 GitHub Releases 的 `latest.json` 检查版本；发现新版本后下载、验证 Tauri 签名、被动安装并重启。

签名私钥不得进入仓库。当前本机密钥应备份到安全的离线位置；丢失后，已安装客户端将无法接受后续签名更新。

GitHub 仓库需要配置：

- Actions Secret `TAURI_SIGNING_PRIVATE_KEY`：签名私钥完整内容。
- Actions Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：本机签名密钥的密码。密码保存在本机签名密钥目录的密码文件中；不要把密码写入仓库或公开 issue。

后续发布时同步提升 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 的版本，然后推送 `vX.Y.Z` 标签。发布流水线将生成 Windows 安装包、签名和 `latest.json`。
