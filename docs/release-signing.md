# 发布签名与自动更新 / Release signing and auto-update

## 中文

安装包和更新包必须使用与 `src-tauri/tauri.conf.json` 中公钥匹配的 Tauri 签名私钥。私钥只保存在发布者的密码管理器和 GitHub Actions Secrets 中，禁止提交到仓库、截图或发到聊天中。

首次配置（在发布者自己的电脑上执行）：

```powershell
npx tauri signer generate
```

安全保存命令输出的私钥和密码。随后打开 GitHub 仓库的 **Settings → Secrets and variables → Actions → New repository secret**，新增：

- `TAURI_SIGNING_PRIVATE_KEY`：完整私钥内容
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：生成私钥时设置的密码

工作流会在构建前检查这两个 Secrets；缺少任意一个就停止，不会生成不可验证的发行版。上传新版本时推送 `v*` 标签，Tauri 会生成安装包和 `latest.json`，桌面端更新按钮会下载并验证签名后再安装。

## English

The installer and updater artifacts must be signed with the Tauri private key that matches the public key in `src-tauri/tauri.conf.json`. Keep the private key only in a password manager and GitHub Actions Secrets. Never commit it, screenshot it, or send it in chat.

On the publisher's machine, generate a key once:

```powershell
npx tauri signer generate
```

Store the generated private key and password securely. In the GitHub repository, open **Settings → Secrets and variables → Actions → New repository secret** and add:

- `TAURI_SIGNING_PRIVATE_KEY`: the complete private key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the password used when generating it

The workflow validates both secrets before building and stops if either is missing. Pushing a `v*` tag creates the installer and `latest.json`; the desktop updater installs only after signature verification.
